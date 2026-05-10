use dispatch_agent::dispatch::display::{format_list, format_list_detect, format_show_config};
use dispatch_agent::types::{Agent, Config, DetectInfo, EnvEntry, Tier};
use indexmap::IndexMap;
use std::path::Path;

#[test]
fn test_format_list_multi_tier() {
    let config = Config {
        version: Some(1),
        tiers: vec![
            Tier {
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
                        cli: "dispatch-agent-nonexistent-xyz".into(),
                        model: None,
                        args: vec![],
                        env: vec![],
                        template: None,
                    },
                ],
            },
            Tier {
                id: "fallback".into(),
                agents: vec![
                    Agent {
                        id: "shell2".into(),
                        cli: "sh".into(),
                        model: Some("gpt-4".into()),
                        args: vec!["--verbose".into()],
                        env: vec![],
                        template: None,
                    },
                    Agent {
                        id: "missing2".into(),
                        cli: "dispatch-agent-nonexistent-xyz".into(),
                        model: Some("claude".into()),
                        args: vec![],
                        env: vec![],
                        template: None,
                    },
                ],
            },
        ],
    };

    insta::assert_snapshot!("format_list_multi_tier", format_list(&config));
}

#[test]
fn test_format_show_config_all_env_variants() {
    let config = Config {
        version: Some(1),
        tiers: vec![Tier {
            id: "production".into(),
            agents: vec![Agent {
                id: "main-agent".into(),
                cli: "claude".into(),
                model: Some("sonnet-4".into()),
                args: vec!["--timeout".into(), "30".into()],
                env: vec![
                    EnvEntry::File {
                        name: "API_TOKEN".into(),
                        path: "/etc/secrets/token".into(),
                    },
                    EnvEntry::Env {
                        name: "API_KEY".into(),
                        var: "CLAUDE_API_KEY".into(),
                    },
                    EnvEntry::Source {
                        path: "/opt/env/production.env".into(),
                    },
                ],
                template: None,
            }],
        }],
    };

    insta::assert_snapshot!(
        "format_show_config_all_env_variants",
        format_show_config(&config, Path::new("/tmp/dispatch-agent-test.toml"))
    );
}

#[test]
fn test_format_list_detect_mixed() {
    let mut detect = IndexMap::new();

    // Callable with version
    detect.insert(
        "claude".into(),
        DetectInfo {
            path: Some("/usr/local/bin/claude".into()),
            version: Some("2.1.5".into()),
            callable: true,
            verified: true,
        },
    );

    // Callable without version
    detect.insert(
        "gemini".into(),
        DetectInfo {
            path: Some("/usr/bin/gemini".into()),
            version: None,
            callable: true,
            verified: false,
        },
    );

    // Not callable
    detect.insert(
        "codex".into(),
        DetectInfo {
            path: None,
            version: None,
            callable: false,
            verified: false,
        },
    );

    insta::assert_snapshot!("format_list_detect_mixed", format_list_detect(&detect));
}
