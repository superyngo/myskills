#![allow(dead_code)]

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::Deserialize;

use crate::config::find_git_root;
use crate::fsutil::{expand_tilde, write_atomic};
use crate::types::{Agent, Config, EnvEntry, Tier};

const DEFAULT_MODELS: &[(&str, &str)] = &[
    ("claude", "claude-sonnet-4-5"),
    ("gemini", "gemini-2.0-flash"),
    ("codex", "codex-mini"),
    ("copilot", "gpt-4o"),
    ("opencode", "anthropic/claude-sonnet-4-5"),
    ("gemini-npx", "gemini-2.0-flash"),
];

#[derive(Deserialize)]
struct InitInput {
    save_location: String,
    tier_order: Vec<String>,
    #[serde(default)]
    agents: Vec<InitAgent>,
}

#[derive(Deserialize)]
struct InitAgent {
    id: String,
    cli: String,
    tier: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    env: Vec<EnvEntry>,
}

fn parse_save_location(s: &str) -> anyhow::Result<PathBuf> {
    match s {
        "user" => expand_tilde("~/.config/dispatch-agent.toml"),
        "project" => Ok(find_git_root().join(".config/dispatch-agent.toml")),
        other => bail!("error: invalid save_location '{}'", other),
    }
}

fn validate_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn resolve_model(cli: &str, input_model: &Option<String>) -> Option<String> {
    match input_model {
        Some(m) if m == "default" => DEFAULT_MODELS
            .iter()
            .find(|(c, _)| *c == cli)
            .map(|(_, m)| (*m).to_string()),
        Some(m) => Some(m.clone()),
        None => DEFAULT_MODELS
            .iter()
            .find(|(c, _)| *c == cli)
            .map(|(_, m)| (*m).to_string()),
    }
}

fn validate_env(agent_id: &str, env: &[EnvEntry]) -> anyhow::Result<()> {
    for entry in env {
        match entry {
            EnvEntry::File { name: _, path } => {
                if path.is_empty() {
                    bail!(
                        "error: agent '{}' env entry of type 'file' missing 'path'",
                        agent_id
                    );
                }
            }
            EnvEntry::Source { path } => {
                if path.is_empty() {
                    bail!(
                        "error: agent '{}' env entry of type 'source' missing 'path'",
                        agent_id
                    );
                }
            }
            EnvEntry::Env { .. } => {}
        }
    }
    Ok(())
}

fn validate_and_build(input: InitInput) -> anyhow::Result<Config> {
    if input.tier_order.is_empty() {
        bail!("error: missing field 'tier_order'");
    }

    let mut seen_ids = HashSet::new();
    for agent in &input.agents {
        if !validate_agent_id(&agent.id) {
            bail!("error: invalid agent id '{}'", agent.id);
        }
        if !seen_ids.insert(agent.id.clone()) {
            bail!("error: duplicate agent id '{}'", agent.id);
        }
    }

    let tier_set: HashSet<&str> = input.tier_order.iter().map(|s| s.as_str()).collect();
    for agent in &input.agents {
        if !tier_set.contains(agent.tier.as_str()) {
            bail!(
                "error: agent '{}' references unknown tier '{}'",
                agent.id,
                agent.tier
            );
        }
        validate_env(&agent.id, &agent.env)?;
    }

    let mut tiers: Vec<Tier> = input
        .tier_order
        .iter()
        .map(|id| Tier {
            id: id.clone(),
            agents: Vec::new(),
        })
        .collect();

    for agent in &input.agents {
        let tier_idx = tiers.iter().position(|t| t.id == agent.tier).unwrap();
        tiers[tier_idx].agents.push(Agent {
            id: agent.id.clone(),
            cli: agent.cli.clone(),
            model: resolve_model(&agent.cli, &agent.model),
            args: agent.args.clone(),
            env: agent.env.clone(),
            template: agent.template.clone(),
        });
    }

    Ok(Config {
        version: Some(1),
        tiers,
    })
}

pub fn cmd_init() -> anyhow::Result<()> {
    run_init(std::io::stdin())
}

pub fn run_init(reader: impl Read) -> anyhow::Result<()> {
    run_init_to(reader, None)
}

fn run_init_to(mut reader: impl Read, dest_override: Option<&Path>) -> anyhow::Result<()> {
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .context("reading init JSON from stdin")?;

    let input: InitInput = match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(e) => bail!("error: invalid JSON: {}", e),
    };

    let _ = parse_save_location(&input.save_location)?;
    let dest = match dest_override {
        Some(p) => p.to_path_buf(),
        None => parse_save_location(&input.save_location)?,
    };

    let config = validate_and_build(input)?;

    let toml_str = toml::to_string_pretty(&config)?;
    let _: Config =
        toml::from_str(&toml_str).context("generated TOML failed round-trip validation")?;

    write_atomic(&dest, toml_str.as_bytes())?;

    println!("{}", dest.display());
    eprintln!("hint: run 'dispatch-agent config edit' to fine-tune your configuration");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_input_json(save_location: &str, tier_order: &[&str], agents_json: &str) -> String {
        let tiers = serde_json::to_string(tier_order).unwrap();
        format!(
            r#"{{"save_location":"{save_location}","tier_order":{tiers},"agents":{agents_json}}}"#
        )
    }

    fn run_test(json: &str) -> anyhow::Result<tempfile::TempDir> {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("dispatch-agent.toml");
        run_init_to(json.as_bytes(), Some(&dest))?;
        assert!(dest.exists(), "config file should exist at {:?}", dest);
        Ok(dir)
    }

    #[test]
    fn init_canonical_input() {
        let json = make_input_json(
            "user",
            &["primary", "fallback"],
            &serde_json::json!([
                {
                    "id": "claude-default",
                    "cli": "claude",
                    "tier": "primary",
                    "model": "default",
                    "args": ["--dangerously-skip-permissions"],
                    "env": [
                        {"type": "file", "name": "X", "path": "~/p"},
                        {"type": "env", "name": "Y", "var": "Y"}
                    ]
                },
                {
                    "id": "gemini-backup",
                    "cli": "gemini",
                    "tier": "fallback"
                }
            ])
            .to_string(),
        );

        let dir = run_test(&json).unwrap();
        let dest = dir.path().join("dispatch-agent.toml");
        let content = fs::read_to_string(&dest).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        assert_eq!(config.version, Some(1));
        assert_eq!(config.tiers.len(), 2);
        assert_eq!(config.tiers[0].id, "primary");
        assert_eq!(config.tiers[1].id, "fallback");
        assert_eq!(config.tiers[0].agents.len(), 1);
        assert_eq!(config.tiers[0].agents[0].id, "claude-default");
        assert_eq!(
            config.tiers[0].agents[0].model,
            Some("claude-sonnet-4-5".to_string())
        );
        assert_eq!(
            config.tiers[1].agents[0].model,
            Some("gemini-2.0-flash".to_string())
        );
    }

    #[test]
    fn init_missing_tier_order() {
        let json = r#"{"save_location":"user","agents":[]}"#;
        let result = run_test(json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("tier_order"),
            "expected 'tier_order' in error, got: {err}"
        );
    }

    #[test]
    fn init_invalid_save_location() {
        let json = r#"{"save_location":"nowhere","tier_order":["primary"],"agents":[]}"#;
        let result = run_test(json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("save_location"),
            "expected 'save_location' in error, got: {err}"
        );
    }

    #[test]
    fn init_invalid_agent_id() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{"id":"bad id!","cli":"claude","tier":"primary"}]).to_string(),
        );
        let result = run_test(&json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("bad id!"),
            "expected 'bad id!' in error, got: {err}"
        );
    }

    #[test]
    fn init_duplicate_agent_ids() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([
                {"id":"dup","cli":"claude","tier":"primary"},
                {"id":"dup","cli":"gemini","tier":"primary"}
            ])
            .to_string(),
        );
        let result = run_test(&json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("duplicate agent id 'dup'"),
            "expected duplicate error, got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn init_file_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{"id":"a","cli":"claude","tier":"primary"}]).to_string(),
        );
        let dir = run_test(&json).unwrap();
        let dest = dir.path().join("dispatch-agent.toml");
        let mode = fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn init_stderr_hint() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{"id":"a","cli":"claude","tier":"primary"}]).to_string(),
        );
        let result = run_test(&json);
        assert!(result.is_ok());
    }

    #[test]
    fn init_malformed_json() {
        let result = run_test(r#"{not valid}"#);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid JSON"),
            "expected 'invalid JSON' in error, got: {err}"
        );
    }

    #[test]
    fn init_unknown_tier_reference() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{"id":"a","cli":"claude","tier":"nonexistent"}]).to_string(),
        );
        let result = run_test(&json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("references unknown tier 'nonexistent'"),
            "expected unknown tier error, got: {err}"
        );
    }

    #[test]
    fn init_env_file_missing_path() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{
                "id":"a",
                "cli":"claude",
                "tier":"primary",
                "env":[{"type":"file","name":"X","path":""}]
            }])
            .to_string(),
        );
        let result = run_test(&json);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("env entry of type 'file' missing 'path'"),
            "expected file missing path error, got: {err}"
        );
    }

    #[test]
    fn init_explicit_model_preserved() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{
                "id":"a",
                "cli":"claude",
                "tier":"primary",
                "model":"claude-opus-4-7"
            }])
            .to_string(),
        );
        let dir = run_test(&json).unwrap();
        let dest = dir.path().join("dispatch-agent.toml");
        let content = fs::read_to_string(&dest).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        assert_eq!(
            config.tiers[0].agents[0].model,
            Some("claude-opus-4-7".to_string())
        );
    }

    #[test]
    fn init_unknown_cli_no_model() {
        let json = make_input_json(
            "user",
            &["primary"],
            &serde_json::json!([{
                "id":"a",
                "cli":"unknown-cli",
                "tier":"primary"
            }])
            .to_string(),
        );
        let dir = run_test(&json).unwrap();
        let dest = dir.path().join("dispatch-agent.toml");
        let content = fs::read_to_string(&dest).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        assert_eq!(config.tiers[0].agents[0].model, None);
    }

    #[test]
    fn init_multiple_agents_across_tiers() {
        let json = make_input_json(
            "user",
            &["primary", "fallback"],
            &serde_json::json!([
                {"id":"a1","cli":"claude","tier":"primary"},
                {"id":"a2","cli":"gemini","tier":"primary"},
                {"id":"b1","cli":"copilot","tier":"fallback"}
            ])
            .to_string(),
        );
        let dir = run_test(&json).unwrap();
        let dest = dir.path().join("dispatch-agent.toml");
        let content = fs::read_to_string(&dest).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        assert_eq!(config.tiers[0].agents.len(), 2);
        assert_eq!(config.tiers[1].agents.len(), 1);
        assert_eq!(config.tiers[0].agents[0].id, "a1");
        assert_eq!(config.tiers[0].agents[1].id, "a2");
        assert_eq!(config.tiers[1].agents[0].id, "b1");
    }
}
