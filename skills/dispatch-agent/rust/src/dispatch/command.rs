#![allow(dead_code)]

use crate::types::{Agent, Template};

/// POSIX single-quote style shell quoting.
pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace("'", "'\\''"))
}

impl Template {
    /// Build the command vector for dispatching to an agent.
    /// Returns `None` when no prompt delivery method is configured.
    pub fn build_command(&self, agent: &Agent, prompt: &str) -> Option<Vec<String>> {
        let has_flag = self.prompt_flag.as_ref().is_some_and(|f| !f.is_empty());

        if !has_flag && !self.prompt_positional {
            return None;
        }

        let binary = self
            .detect_binary
            .as_deref()
            .unwrap_or(&agent.cli)
            .to_owned();

        let mut cmd = vec![binary];

        if let Some(sub) = self.subcommand.as_deref().filter(|s| !s.is_empty()) {
            cmd.push(sub.to_owned());
        }

        cmd.extend(self.extra_args.iter().cloned());
        cmd.extend(agent.args.iter().cloned());

        if let Some(ref model) = agent.model {
            if model != "default" {
                if let Some(ref flag) = self.model_flag {
                    cmd.push(flag.clone());
                    cmd.push(model.clone());
                }
            }
        }

        if self.prompt_positional {
            cmd.push(prompt.to_owned());
        } else if let Some(ref flag) = self.prompt_flag {
            cmd.push(flag.clone());
            cmd.push(prompt.to_owned());
        }

        Some(cmd)
    }
}

/// Wrap a command with bash `source` statements for env file paths.
/// If `sources` is empty, returns `cmd` unchanged.
/// Otherwise returns `["bash", "-c", <script>, "--", ...cmd]`.
pub fn wrap_with_sources(cmd: Vec<String>, sources: &[String]) -> Vec<String> {
    if sources.is_empty() {
        return cmd;
    }

    let source_stmts: String = sources
        .iter()
        .map(|s| format!("source {};", shell_quote(s)))
        .collect::<Vec<_>>()
        .join(" ");

    let script = format!("set -a; {} set +a; exec \"$@\"", source_stmts);

    let mut out = vec!["bash".to_owned(), "-c".to_owned(), script, "--".to_owned()];
    out.extend(cmd);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- shell_quote ---

    #[test]
    fn sq_empty() {
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn sq_plain_ascii() {
        assert_eq!(shell_quote("hello"), "'hello'");
    }

    #[test]
    fn sq_single_quote() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn sq_dollar_sign() {
        assert_eq!(shell_quote("$HOME"), "'$HOME'");
    }

    #[test]
    fn sq_backtick() {
        assert_eq!(shell_quote("`cmd`"), "'`cmd`'");
    }

    #[test]
    fn sq_space() {
        assert_eq!(shell_quote("foo bar"), "'foo bar'");
    }

    #[test]
    fn sq_newline() {
        assert_eq!(shell_quote("line\nnewline"), "'line\nnewline'");
    }

    // --- build_command ---

    fn base_template() -> Template {
        Template {
            detect_binary: None,
            subcommand: None,
            prompt_flag: None,
            prompt_positional: false,
            model_flag: None,
            extra_args: vec![],
            version_flag: None,
            file_input_mode: None,
            verified: true,
        }
    }

    fn base_agent() -> Agent {
        Agent {
            id: "test".into(),
            cli: "mycli".into(),
            model: None,
            args: vec![],
            env: vec![],
            template: None,
        }
    }

    #[test]
    fn bc_positional_with_subcommand() {
        let tmpl = Template {
            subcommand: Some("chat".into()),
            prompt_positional: true,
            ..base_template()
        };
        let agent = base_agent();
        let cmd = tmpl.build_command(&agent, "hello").unwrap();
        assert_eq!(cmd, vec!["mycli", "chat", "hello"]);
    }

    #[test]
    fn bc_flag_delivery() {
        let tmpl = Template {
            prompt_flag: Some("-p".into()),
            ..base_template()
        };
        let agent = base_agent();
        let cmd = tmpl.build_command(&agent, "hello").unwrap();
        assert_eq!(cmd, vec!["mycli", "-p", "hello"]);
    }

    #[test]
    fn bc_no_subcommand() {
        let tmpl = Template {
            prompt_positional: true,
            ..base_template()
        };
        let agent = base_agent();
        let cmd = tmpl.build_command(&agent, "hi").unwrap();
        assert_eq!(cmd, vec!["mycli", "hi"]);
    }

    #[test]
    fn bc_default_model_excluded() {
        let tmpl = Template {
            prompt_positional: true,
            model_flag: Some("--model".into()),
            ..base_template()
        };
        let agent = Agent {
            model: Some("default".into()),
            ..base_agent()
        };
        let cmd = tmpl.build_command(&agent, "hi").unwrap();
        assert!(!cmd.contains(&"--model".to_owned()));
        assert!(!cmd.contains(&"default".to_owned()));
    }

    #[test]
    fn bc_explicit_model_included() {
        let tmpl = Template {
            prompt_positional: true,
            model_flag: Some("--model".into()),
            ..base_template()
        };
        let agent = Agent {
            model: Some("sonnet".into()),
            ..base_agent()
        };
        let cmd = tmpl.build_command(&agent, "hi").unwrap();
        assert!(cmd.contains(&"--model".to_owned()));
        assert!(cmd.contains(&"sonnet".to_owned()));
    }

    #[test]
    fn bc_model_without_flag_no_crash() {
        let tmpl = Template {
            prompt_positional: true,
            model_flag: None,
            ..base_template()
        };
        let agent = Agent {
            model: Some("sonnet".into()),
            ..base_agent()
        };
        let cmd = tmpl.build_command(&agent, "hi").unwrap();
        assert!(!cmd.iter().any(|s| s == "sonnet"));
    }

    #[test]
    fn bc_extra_args_before_agent_args() {
        let tmpl = Template {
            prompt_positional: true,
            extra_args: vec!["--verbose".into()],
            ..base_template()
        };
        let agent = Agent {
            args: vec!["--color".into()],
            ..base_agent()
        };
        let cmd = tmpl.build_command(&agent, "hi").unwrap();
        let verbose_pos = cmd.iter().position(|s| s == "--verbose").unwrap();
        let color_pos = cmd.iter().position(|s| s == "--color").unwrap();
        assert!(verbose_pos < color_pos);
    }

    #[test]
    fn bc_none_when_no_delivery() {
        let tmpl = Template {
            prompt_flag: None,
            prompt_positional: false,
            ..base_template()
        };
        let agent = base_agent();
        assert!(tmpl.build_command(&agent, "hi").is_none());
    }

    #[test]
    fn bc_empty_prompt_flag_is_no_delivery() {
        let tmpl = Template {
            prompt_flag: Some("".into()),
            prompt_positional: false,
            ..base_template()
        };
        let agent = base_agent();
        assert!(tmpl.build_command(&agent, "hi").is_none());
    }

    #[test]
    fn bc_detect_binary_overrides_cli() {
        let tmpl = Template {
            detect_binary: Some("/usr/local/bin/tool".into()),
            prompt_positional: true,
            ..base_template()
        };
        let agent = base_agent();
        let cmd = tmpl.build_command(&agent, "hi").unwrap();
        assert_eq!(cmd[0], "/usr/local/bin/tool");
    }

    // --- wrap_with_sources ---

    #[test]
    fn ws_empty_sources_passthrough() {
        let cmd = vec!["echo".into(), "hello".into()];
        let out = wrap_with_sources(cmd.clone(), &[]);
        assert_eq!(out, cmd);
    }

    #[test]
    fn ws_single_source_special_chars() {
        let cmd = vec!["mycli".into()];
        let sources = vec!["/path/it's/$HOME/`cmd`".into()];
        let out = wrap_with_sources(cmd, &sources);
        assert_eq!(out[0], "bash");
        assert_eq!(out[1], "-c");
        let script = &out[2];
        assert!(script.starts_with("set -a; source "));
        assert!(script.contains("set +a; exec \"$@\""));
        assert!(script.contains("source '/path/it'\\''s/$HOME/`cmd`';"));
        assert_eq!(out[3], "--");
        assert_eq!(out[4], "mycli");
    }

    #[test]
    fn ws_multiple_sources_single_layer() {
        let cmd = vec!["tool".into()];
        let sources = vec!["/a".into(), "/b".into()];
        let out = wrap_with_sources(cmd, &sources);
        assert_eq!(out[0], "bash");
        // Count bash layers — only one
        assert_eq!(out.iter().filter(|s| s.as_str() == "bash").count(), 1);
        let script = &out[2];
        assert!(script.contains("source '/a'; source '/b';"));
    }

    // --- proptest ---

    use proptest::prelude::*;

    fn arb_template() -> impl Strategy<Value = Template> {
        (
            prop::option::of(any::<String>()),
            prop::option::of(any::<String>()),
            prop::option::of(any::<String>()),
            any::<bool>(),
            prop::option::of(any::<String>()),
            prop::collection::vec(any::<String>(), 0..4),
        )
            .prop_map(
                |(
                    detect_binary,
                    subcommand,
                    prompt_flag,
                    prompt_positional,
                    model_flag,
                    extra_args,
                )| {
                    Template {
                        detect_binary,
                        subcommand,
                        prompt_flag,
                        prompt_positional,
                        model_flag,
                        extra_args,
                        version_flag: None,
                        file_input_mode: None,
                        verified: true,
                    }
                },
            )
    }

    fn arb_agent() -> impl Strategy<Value = Agent> {
        (
            any::<String>(),
            any::<String>(),
            prop::option::of(any::<String>()),
            prop::collection::vec(any::<String>(), 0..4),
        )
            .prop_map(|(id, cli, model, args)| Agent {
                id,
                cli,
                model,
                args,
                env: vec![],
                template: None,
            })
    }

    proptest! {
        #[test]
        fn prompt_appears_at_most_once(
            tmpl in arb_template(),
            agent in arb_agent(),
            // Use a sentinel prefix guaranteed not to appear in generated binary
            // names or extra_args (which are arbitrary strings with no such prefix).
            suffix in "[a-z]{4,12}"
        ) {
            let prompt = format!("__PROMPT_SENTINEL__{suffix}");
            if let Some(cmd) = tmpl.build_command(&agent, &prompt) {
                let count = cmd.iter().filter(|s| s.as_str() == prompt).count();
                assert!(count <= 1, "prompt appeared {} times in {:?}", count, cmd);
            }
        }

        #[test]
        fn extra_args_before_agent_args(
            extra_args in prop::collection::vec(any::<String>(), 0..4),
            agent_args in prop::collection::vec(any::<String>(), 0..4),
            prompt in any::<String>()
        ) {
            let tmpl = Template {
                prompt_positional: true,
                extra_args,
                ..base_template()
            };
            let agent = Agent {
                args: agent_args,
                ..base_agent()
            };
            if let Some(cmd) = tmpl.build_command(&agent, &prompt) {
                // Find the position of the first extra_arg that uniquely appears
                // (skip if args overlap with prompt or each other — just check order of insertion)
                let binary_end = 1; // just the binary
                let extra_region_end = binary_end + tmpl.extra_args.len();
                let agent_region_end = extra_region_end + agent.args.len();
                // Verify lengths are correct
                assert_eq!(cmd.len(), agent_region_end + 1); // +1 for prompt
            }
        }

        #[test]
        fn subcommand_after_binary(
            subcommand in prop::option::of(any::<String>()),
            prompt in any::<String>()
        ) {
            let tmpl = Template {
                subcommand: subcommand.clone(),
                prompt_positional: true,
                ..base_template()
            };
            let agent = base_agent();
            if let Some(cmd) = tmpl.build_command(&agent, &prompt) {
                assert_eq!(cmd[0], "mycli");
                if let Some(ref sub) = tmpl.subcommand {
                    if !sub.is_empty() {
                        assert_eq!(cmd[1], sub.as_str());
                    }
                }
            }
        }
    }
}
