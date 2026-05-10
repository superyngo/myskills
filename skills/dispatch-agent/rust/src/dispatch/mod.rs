#![allow(dead_code)]

pub mod command;
pub mod display;
pub mod process;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use indexmap::IndexMap;

use crate::config::{find_config, load_config};
use crate::detect::run_detect;
use crate::env::{build_env, get_source_files};
use crate::rr_state::{load_rr_state, store_rr_state};
use crate::templates::load_templates;
use crate::types::{Agent, Config, Template, Tier};

use process::ChildState;

use crate::cli::DispatchArgs;

fn rr_state_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("dispatch-agent")
        .join("rr-state.json")
}

fn current_depth() -> i64 {
    match std::env::var("DISPATCH_AGENT_DEPTH") {
        Ok(val) => match val.trim().parse::<i64>() {
            Ok(d) => d,
            Err(_) => {
                eprintln!(
                    "error: invalid DISPATCH_AGENT_DEPTH value '{}': expected integer",
                    val
                );
                std::process::exit(1);
            }
        },
        Err(_) => 0,
    }
}

fn read_prompt(args: &DispatchArgs) -> anyhow::Result<String> {
    if let Some(ref file) = args.file {
        return Ok(std::fs::read_to_string(file)?);
    }
    if let Some(ref prompt) = args.prompt {
        return Ok(prompt.clone());
    }
    Ok("<prompt>".to_string())
}

pub fn cmd_dispatch(args: &DispatchArgs, config_path: Option<&Path>) -> anyhow::Result<()> {
    // Recursion guard
    let depth = current_depth();
    if depth >= 5 {
        eprintln!("error: recursion depth limit (5) reached");
        std::process::exit(1);
    }

    // Timeout validation
    if args.timeout == 0 {
        return Err(anyhow!(
            "error: --timeout 0 is invalid; use -1 for no timeout"
        ));
    }

    let templates = load_templates()?;

    // --list mode
    if args.list {
        match find_config(config_path) {
            Some(path) => {
                let config = load_config(&path)?;
                print!("{}", display::format_list(&config));
            }
            None => {
                let detect = run_detect(&templates);
                print!("{}", display::format_list_detect(&detect));
            }
        }
        return Ok(());
    }

    // --show-config mode
    if args.show_config {
        match find_config(config_path) {
            Some(path) => {
                let config = load_config(&path)?;
                print!("{}", display::format_show_config(&config, &path));
            }
            None => {
                eprintln!("error: no config file found. Run 'dispatch-agent init' to create one.");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Load config for remaining modes
    let config = match find_config(config_path) {
        Some(path) => load_config(&path)?,
        None => {
            if args.dry_run {
                // --dry-run is explicitly exempted from the no-config exit-1 rule
                return Ok(());
            }
            eprintln!("error: no config file found. Run 'dispatch-agent init' to create one.");
            std::process::exit(1);
        }
    };

    let prompt = read_prompt(args)?;

    // --dry-run mode
    if args.dry_run {
        return dry_run(&config, &templates, &prompt, &args.tier, &args.agent);
    }

    // Setup signal watcher (once at start of dispatch)
    let state: Arc<Mutex<ChildState>> = Arc::new(Mutex::new(ChildState::default()));
    let shutdown = Arc::new(AtomicBool::new(false));
    let _watcher = process::unix::start_signal_watcher(state.clone(), shutdown.clone());

    let result = if let Some(ref agent_id) = args.agent {
        dispatch_single(&config, &templates, &prompt, agent_id, args, depth, &state)
    } else {
        dispatch_tiers(
            &config, &templates, &prompt, &args.tier, args, depth, &state,
        )
    };

    shutdown.store(true, Ordering::Relaxed);

    let interrupted = state.lock().unwrap().interrupted;
    if interrupted {
        std::process::exit(1);
    }

    result
}

fn dry_run(
    config: &Config,
    templates: &IndexMap<String, Template>,
    prompt: &str,
    tier: &Option<String>,
    agent: &Option<String>,
) -> anyhow::Result<()> {
    let agents = collect_agents(config, tier, agent);
    for agent in agents {
        let tmpl_name = agent.template.as_deref().unwrap_or(&agent.cli);
        let template = match templates.get(tmpl_name) {
            Some(t) => t,
            None => {
                eprintln!(
                    "warning: template '{}' for agent '{}' not found in cli-templates.toml",
                    tmpl_name, agent.id
                );
                continue;
            }
        };
        if !template.verified {
            eprintln!(
                "warning: agent '{}' uses unverified template '{}', skipping",
                agent.id, tmpl_name
            );
            continue;
        }
        if let Some(cmd) = template.build_command(agent, prompt) {
            let cmd = command::wrap_with_sources(cmd, &get_source_files(agent));
            println!("{}", cmd.join(" "));
        }
    }
    Ok(())
}

fn dispatch_single(
    config: &Config,
    templates: &IndexMap<String, Template>,
    prompt: &str,
    agent_id: &str,
    args: &DispatchArgs,
    depth: i64,
    state: &Arc<Mutex<ChildState>>,
) -> anyhow::Result<()> {
    let (agent, tier_id) = find_agent(config, agent_id);
    let agent = match agent {
        Some(a) => a,
        None => {
            eprintln!("error: agent '{}' not found in config", agent_id);
            std::process::exit(1);
        }
    };
    let tier_id = tier_id.unwrap_or("?");

    let tmpl_name = agent.template.as_deref().unwrap_or(&agent.cli);
    let template = match templates.get(tmpl_name) {
        Some(t) => t,
        None => {
            eprintln!(
                "error: template '{}' for agent '{}' not found in cli-templates.toml",
                tmpl_name, agent.id
            );
            std::process::exit(1);
        }
    };

    if !template.verified {
        eprintln!(
            "error: agent '{}' uses unverified template '{}'; cannot dispatch",
            agent.id, tmpl_name
        );
        std::process::exit(1);
    }

    let cmd = match template.build_command(agent, prompt) {
        Some(c) => c,
        None => {
            eprintln!(
                "error: agent '{}' has no prompt delivery method configured",
                agent.id
            );
            std::process::exit(1);
        }
    };

    let cmd_vec = command::wrap_with_sources(cmd, &get_source_files(agent));
    let env_map = build_env(agent, depth);
    let timeout_secs = effective_timeout(args.timeout);

    if args.verbose {
        eprintln!("[attempting {}]", agent.id);
    }

    match process::spawn_and_wait(
        &cmd_vec,
        &env_map,
        timeout_secs,
        &agent.id,
        args.verbose,
        state.clone(),
    ) {
        Ok((status, _, _)) => {
            if status.success() {
                if args.verbose {
                    eprintln!("[{}] (tier: {})", agent.id, tier_id);
                }
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Err(_) => std::process::exit(1),
    }
}

fn dispatch_tiers(
    config: &Config,
    templates: &IndexMap<String, Template>,
    prompt: &str,
    tier_filter: &Option<String>,
    args: &DispatchArgs,
    depth: i64,
    state: &Arc<Mutex<ChildState>>,
) -> anyhow::Result<()> {
    let tiers = collect_tiers(config, tier_filter);
    let rr_path = rr_state_path();
    let rr_snapshot = load_rr_state(&rr_path);
    let timeout_secs = effective_timeout(args.timeout);

    for tier in &tiers {
        let start_idx: usize = rr_snapshot
            .get(&tier.id)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);

        let agents = &tier.agents;
        let n = agents.len();

        for i in 0..n {
            let idx = (start_idx + i) % n;
            let agent = &agents[idx];

            let tmpl_name = agent.template.as_deref().unwrap_or(&agent.cli);
            let template = match templates.get(tmpl_name) {
                Some(t) => t,
                None => {
                    eprintln!(
                        "warning: template '{}' for agent '{}' not found in cli-templates.toml",
                        tmpl_name, agent.id
                    );
                    continue;
                }
            };

            if !template.verified {
                eprintln!(
                    "warning: agent '{}' uses unverified template '{}', skipping",
                    agent.id, tmpl_name
                );
                continue;
            }

            let cmd = match template.build_command(agent, prompt) {
                Some(c) => c,
                None => {
                    eprintln!(
                        "warning: agent '{}' has no prompt delivery method, skipping",
                        agent.id
                    );
                    continue;
                }
            };

            let cmd_vec = command::wrap_with_sources(cmd, &get_source_files(agent));
            let env_map = build_env(agent, depth);

            if args.verbose {
                eprintln!("[attempting {}]", agent.id);
            }

            if let Ok((status, _, child_state)) = process::spawn_and_wait(
                &cmd_vec,
                &env_map,
                timeout_secs,
                &agent.id,
                args.verbose,
                state.clone(),
            ) {
                if status.success() {
                    if args.verbose {
                        eprintln!("[{}] (tier: {})", agent.id, tier.id);
                    }
                    let next_idx = (idx + 1) % n;
                    let mut rr = load_rr_state(&rr_path);
                    rr.insert(tier.id.clone(), next_idx.to_string());
                    store_rr_state(&rr_path, &rr).ok();
                    return Ok(());
                }
                if child_state.interrupted {
                    std::process::exit(1);
                }
            }
        }
    }

    std::process::exit(1);
}

fn find_agent<'a>(config: &'a Config, agent_id: &str) -> (Option<&'a Agent>, Option<&'a str>) {
    for tier in &config.tiers {
        for agent in &tier.agents {
            if agent.id == agent_id {
                return (Some(agent), Some(&tier.id));
            }
        }
    }
    (None, None)
}

fn collect_agents<'a>(
    config: &'a Config,
    tier: &Option<String>,
    agent: &Option<String>,
) -> Vec<&'a Agent> {
    if let Some(ref agent_id) = agent {
        if let (Some(a), _) = find_agent(config, agent_id) {
            return vec![a];
        }
        eprintln!("error: agent '{}' not found in config", agent_id);
        std::process::exit(1);
    }

    if let Some(ref tier_id) = tier {
        let mut found = false;
        let mut result = Vec::new();
        for t in &config.tiers {
            if t.id == *tier_id {
                found = true;
            }
            if found {
                for agent in &t.agents {
                    result.push(agent);
                }
            }
        }
        if !found {
            eprintln!("error: tier '{}' not found in config", tier_id);
            std::process::exit(1);
        }
        return result;
    }

    config.tiers.iter().flat_map(|t| t.agents.iter()).collect()
}

fn collect_tiers<'a>(config: &'a Config, tier: &Option<String>) -> Vec<&'a Tier> {
    if let Some(ref tier_id) = tier {
        let mut found = false;
        let mut result = Vec::new();
        for t in &config.tiers {
            if t.id == *tier_id {
                found = true;
            }
            if found {
                result.push(t);
            }
        }
        if !found {
            eprintln!("error: tier '{}' not found in config", tier_id);
            std::process::exit(1);
        }
        result
    } else {
        config.tiers.iter().collect()
    }
}

fn effective_timeout(timeout: i64) -> i64 {
    if timeout < 0 {
        -1
    } else {
        timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rr_state_path_ends_correctly() {
        let path = rr_state_path();
        assert!(path.ends_with("dispatch-agent/rr-state.json"));
    }

    #[test]
    fn effective_timeout_negative() {
        assert_eq!(effective_timeout(-5), -1);
        assert_eq!(effective_timeout(-1), -1);
    }

    #[test]
    fn effective_timeout_positive() {
        assert_eq!(effective_timeout(30), 30);
        assert_eq!(effective_timeout(1), 1);
    }

    #[test]
    fn read_prompt_prefers_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("prompt.txt");
        std::fs::write(&file_path, "file content").unwrap();

        let args = DispatchArgs {
            prompt: Some("cli prompt".into()),
            file: Some(file_path),
            timeout: -1,
            tier: None,
            agent: None,
            dry_run: false,
            list: false,
            show_config: false,
            verbose: false,
        };
        assert_eq!(read_prompt(&args).unwrap(), "file content");
    }

    #[test]
    fn read_prompt_uses_cli_prompt() {
        let args = DispatchArgs {
            prompt: Some("hello".into()),
            file: None,
            timeout: -1,
            tier: None,
            agent: None,
            dry_run: false,
            list: false,
            show_config: false,
            verbose: false,
        };
        assert_eq!(read_prompt(&args).unwrap(), "hello");
    }

    #[test]
    fn read_prompt_default_placeholder() {
        let args = DispatchArgs {
            prompt: None,
            file: None,
            timeout: -1,
            tier: None,
            agent: None,
            dry_run: false,
            list: false,
            show_config: false,
            verbose: false,
        };
        assert_eq!(read_prompt(&args).unwrap(), "<prompt>");
    }

    #[test]
    fn collect_agents_finds_agent_across_tiers() {
        let config = Config {
            version: Some(1),
            tiers: vec![
                Tier {
                    id: "t1".into(),
                    agents: vec![Agent {
                        id: "a1".into(),
                        cli: "sh".into(),
                        model: None,
                        args: vec![],
                        env: vec![],
                        template: None,
                    }],
                },
                Tier {
                    id: "t2".into(),
                    agents: vec![Agent {
                        id: "a2".into(),
                        cli: "sh".into(),
                        model: None,
                        args: vec![],
                        env: vec![],
                        template: None,
                    }],
                },
            ],
        };
        let agent_id = Some("a2".to_string());
        let agents = collect_agents(&config, &None, &agent_id);
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, "a2");
    }

    #[test]
    fn collect_tiers_from_named_tier_includes_subsequent() {
        let config = Config {
            version: Some(1),
            tiers: vec![
                Tier {
                    id: "first".into(),
                    agents: vec![],
                },
                Tier {
                    id: "second".into(),
                    agents: vec![],
                },
                Tier {
                    id: "third".into(),
                    agents: vec![],
                },
            ],
        };
        let tier = Some("second".to_string());
        let tiers = collect_tiers(&config, &tier);
        assert_eq!(tiers.len(), 2);
        assert_eq!(tiers[0].id, "second");
        assert_eq!(tiers[1].id, "third");
    }
}
