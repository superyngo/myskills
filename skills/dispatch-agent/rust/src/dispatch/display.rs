#![allow(dead_code)]

use std::path::Path;

use indexmap::IndexMap;

use crate::types::{Config, DetectInfo, EnvEntry};

pub fn format_list(config: &Config) -> String {
    let mut blocks = Vec::new();
    for tier in &config.tiers {
        let mut block = format!("Tier: {}", tier.id);
        for agent in &tier.agents {
            let mark = if which::which(&agent.cli).is_ok() {
                "✓"
            } else {
                "✗"
            };
            block.push_str(&format!("\n  [{}] {} ({})", mark, agent.id, agent.cli));
        }
        blocks.push(block);
    }
    blocks.join("\n\n")
}

pub fn format_show_config(config: &Config, path: &Path) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "Config: {} {}\n",
        path.display(),
        path_label(path)
    ));
    out.push_str(&format!("version: {}\n", config.version.unwrap_or(1)));
    out.push('\n');

    let mut first_tier = true;
    for tier in &config.tiers {
        if !first_tier {
            out.push('\n');
        }
        first_tier = false;
        out.push_str(&format!("Tier: {}\n", tier.id));
        for agent in &tier.agents {
            out.push_str(&format!("  Agent: {}\n", agent.id));
            out.push_str(&format!("    cli: {}\n", agent.cli));
            out.push_str(&format!(
                "    model: {}\n",
                agent.model.as_deref().unwrap_or("default")
            ));
            out.push_str(&format!("    args: {:?}\n", agent.args));
            if agent.env.is_empty() {
                out.push_str("    env: (none)\n");
            } else {
                out.push_str("    env:\n");
                for entry in &agent.env {
                    match entry {
                        EnvEntry::File { name, path } => {
                            out.push_str(&format!("      file: {} <- {}\n", name, path));
                        }
                        EnvEntry::Env { name, var } => {
                            out.push_str(&format!("      env: {} <- {}\n", name, var));
                        }
                        EnvEntry::Source { path } => {
                            out.push_str(&format!("      source: {}\n", path));
                        }
                    }
                }
            }
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

pub fn format_list_detect(detect: &IndexMap<String, DetectInfo>) -> String {
    let mut out = String::from("Available agent CLIs (no config — showing detected templates):");

    if detect.is_empty() {
        return out;
    }

    let max_name_len = detect.keys().map(|k| k.len()).max().unwrap_or(0);

    for (name, info) in detect {
        let mark = if info.callable { "✓" } else { "✗" };
        let status = if info.callable {
            match &info.version {
                Some(v) => v.clone(),
                None => "(version probe disabled)".to_string(),
            }
        } else {
            "(not found)".to_string()
        };
        out.push_str(&format!(
            "\n  [{}] {:width$}  {}",
            mark,
            name,
            status,
            width = max_name_len
        ));
    }

    out
}

fn path_label(path: &Path) -> &'static str {
    let home = dirs::home_dir().unwrap_or_default();
    let path_str = path.to_string_lossy();
    if path_str.contains(".config/dispatch-agent.toml") && path.starts_with(&home) {
        return "(user)";
    }
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return "(project)";
        }
        dir = d.parent();
    }
    "(user)"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Agent, Tier};

    #[test]
    fn format_list_marks_available() {
        let config = Config {
            version: Some(1),
            tiers: vec![Tier {
                id: "primary".into(),
                agents: vec![
                    Agent {
                        id: "shell".into(),
                        cli: "sh".into(),
                        model: None,
                        args: vec![],
                        env: vec![],
                        template: None,
                    },
                    Agent {
                        id: "missing".into(),
                        cli: "dispatch-agent-fake-nonexistent-cli-xyz".into(),
                        model: None,
                        args: vec![],
                        env: vec![],
                        template: None,
                    },
                ],
            }],
        };
        let out = format_list(&config);
        assert!(out.contains("[✓] shell (sh)"), "sh should be [✓]");
        assert!(
            out.contains("[✗] missing (dispatch-agent-fake-nonexistent-cli-xyz)"),
            "fake cli should be [✗]"
        );
    }

    #[test]
    fn format_show_config_basic() {
        let config = Config {
            version: Some(1),
            tiers: vec![Tier {
                id: "primary".into(),
                agents: vec![Agent {
                    id: "test-agent".into(),
                    cli: "claude".into(),
                    model: None,
                    args: vec![],
                    env: vec![],
                    template: None,
                }],
            }],
        };
        let out = format_show_config(&config, Path::new("/tmp/test.toml"));
        assert!(out.contains("Tier: primary"));
        assert!(out.contains("Agent: test-agent"));
        assert!(out.contains("cli: claude"));
        assert!(out.contains("env: (none)"));
    }

    #[test]
    fn format_show_config_env_entries() {
        let config = Config {
            version: Some(1),
            tiers: vec![Tier {
                id: "t".into(),
                agents: vec![Agent {
                    id: "a".into(),
                    cli: "sh".into(),
                    model: None,
                    args: vec![],
                    env: vec![
                        EnvEntry::File {
                            name: "TOKEN".into(),
                            path: "/tmp/token".into(),
                        },
                        EnvEntry::Env {
                            name: "KEY".into(),
                            var: "MY_ENV_VAR".into(),
                        },
                        EnvEntry::Source {
                            path: "/tmp/source".into(),
                        },
                    ],
                    template: None,
                }],
            }],
        };
        let out = format_show_config(&config, Path::new("/tmp/test.toml"));
        assert!(out.contains("file: TOKEN <- /tmp/token"));
        assert!(out.contains("env: KEY <- MY_ENV_VAR"));
        assert!(out.contains("source: /tmp/source"));
        // Env values must never appear — these are names/paths, not resolved values
        assert!(!out.contains("sk-secret-value-should-not-appear-xyz"));
    }

    #[test]
    fn format_list_detect_callable() {
        let mut detect = IndexMap::new();
        detect.insert(
            "claude".into(),
            DetectInfo {
                path: Some("/usr/bin/claude".into()),
                version: Some("1.2".into()),
                callable: true,
                verified: true,
            },
        );
        let out = format_list_detect(&detect);
        assert!(out.contains("[✓]"));
        assert!(out.contains("1.2"));
    }

    #[test]
    fn format_list_detect_not_found() {
        let mut detect = IndexMap::new();
        detect.insert(
            "gemini".into(),
            DetectInfo {
                path: None,
                version: None,
                callable: false,
                verified: false,
            },
        );
        let out = format_list_detect(&detect);
        assert!(out.contains("[✗]"));
        assert!(out.contains("(not found)"));
    }

    #[test]
    fn format_list_detect_no_version() {
        let mut detect = IndexMap::new();
        detect.insert(
            "codex".into(),
            DetectInfo {
                path: Some("/usr/bin/codex".into()),
                version: None,
                callable: true,
                verified: false,
            },
        );
        let out = format_list_detect(&detect);
        assert!(out.contains("(version probe disabled)"));
    }

    #[test]
    fn format_show_config_no_env() {
        let config = Config {
            version: None,
            tiers: vec![Tier {
                id: "fallback".into(),
                agents: vec![Agent {
                    id: "agent".into(),
                    cli: "gemini".into(),
                    model: Some("pro".into()),
                    args: vec![],
                    env: vec![],
                    template: None,
                }],
            }],
        };
        let out = format_show_config(&config, Path::new("/tmp/noenv.toml"));
        assert!(out.contains("env: (none)"));
        assert!(out.contains("model: pro"));
        assert!(out.contains("version: 1"));
    }
}
