#![allow(dead_code)]

use std::collections::HashMap;

use crate::fsutil::expand_tilde;
use crate::types::{Agent, EnvEntry};

/// Resolve a single non-Source env entry to `(name, value)`.
/// Returns `None` for Source entries or on read/lookup failure.
pub fn resolve_env_var(ev: &EnvEntry) -> Option<(String, String)> {
    match ev {
        EnvEntry::File { name, path } => match std::fs::read_to_string(path) {
            Ok(contents) => Some((name.clone(), contents.trim().to_string())),
            Err(_) => {
                eprintln!("dispatch-agent: failed to read env file: {path}");
                None
            }
        },
        EnvEntry::Env { name, var } => match std::env::var(var) {
            Ok(value) => Some((name.clone(), value)),
            Err(_) => None,
        },
        EnvEntry::Source { .. } => None,
    }
}

/// Return expanded paths for all Source entries in the agent's env list.
pub fn get_source_files(agent: &Agent) -> Vec<String> {
    agent
        .env
        .iter()
        .filter_map(|ev| {
            if let EnvEntry::Source { path } = ev {
                match expand_tilde(path) {
                    Ok(p) => Some(p.to_string_lossy().into_owned()),
                    Err(e) => {
                        eprintln!("dispatch-agent: cannot expand source path '{path}': {e}");
                        None
                    }
                }
            } else {
                None
            }
        })
        .collect()
}

/// Build the environment map for launching an agent sub-process.
///
/// 1. Inherit all current env vars.
/// 2. Overlay each non-Source entry (File / Env).
/// 3. Set `DISPATCH_AGENT_DEPTH` to `current_depth + 1`.
pub fn build_env(agent: &Agent, current_depth: i64) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = std::env::vars().collect();

    for ev in &agent.env {
        if let Some((k, v)) = resolve_env_var(ev) {
            map.insert(k, v);
        }
    }

    map.insert(
        "DISPATCH_AGENT_DEPTH".to_string(),
        (current_depth + 1).to_string(),
    );

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EnvEntry;
    use std::io::Write;

    fn make_agent(env: Vec<EnvEntry>) -> Agent {
        Agent {
            id: "test".into(),
            cli: "claude".into(),
            model: None,
            args: vec![],
            env,
            template: None,
        }
    }

    #[test]
    fn file_read_roundtrip() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "  secret_value  ").unwrap();
        let path = tmp.path().to_string_lossy().into_owned();

        let entry = EnvEntry::File {
            name: "MY_SECRET".into(),
            path,
        };
        let result = resolve_env_var(&entry);
        assert_eq!(result, Some(("MY_SECRET".into(), "secret_value".into())));
    }

    #[test]
    fn env_var_lookup() {
        std::env::set_var("_TEST_DISPATCH_ENV_VAR", "hello123");
        let entry = EnvEntry::Env {
            name: "MAPPED_NAME".into(),
            var: "_TEST_DISPATCH_ENV_VAR".into(),
        };
        let result = resolve_env_var(&entry);
        assert_eq!(result, Some(("MAPPED_NAME".into(), "hello123".into())));
    }

    #[test]
    fn source_returns_none() {
        let entry = EnvEntry::Source {
            path: "/some/path".into(),
        };
        assert_eq!(resolve_env_var(&entry), None);
    }

    #[test]
    fn build_env_inherits_overlays_bumps_depth() {
        // Set a known env var in the parent process
        std::env::set_var("_TEST_PARENT_VAR", "parent_value");

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "file_value").unwrap();
        let path = tmp.path().to_string_lossy().into_owned();

        let agent = make_agent(vec![
            EnvEntry::File {
                name: "_TEST_PARENT_VAR".into(), // overwrite the parent var
                path,
            },
            EnvEntry::Source {
                path: "~/nonexistent.sh".into(), // should be skipped
            },
        ]);

        let depth = 3_i64;
        let env = build_env(&agent, depth);

        // Inherited key exists and is overwritten by File entry
        assert_eq!(
            env.get("_TEST_PARENT_VAR").map(String::as_str),
            Some("file_value")
        );

        // Depth bumped
        assert_eq!(
            env.get("DISPATCH_AGENT_DEPTH").map(String::as_str),
            Some("4")
        );
    }
}
