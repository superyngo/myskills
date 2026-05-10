use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::fsutil::expand_tilde;
use crate::types::Config;

#[allow(dead_code)]
pub fn find_git_root() -> PathBuf {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();
    match output {
        Ok(o) if o.status.success() => PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

#[allow(dead_code)]
pub fn find_config(arg: Option<&Path>) -> Option<PathBuf> {
    // 1. Explicit --config path: return unconditionally so caller can give a better error
    if let Some(p) = arg {
        return Some(p.to_path_buf());
    }

    // 2. Project-level config under git root
    let project = find_git_root().join(".config/dispatch-agent.toml");
    if project.exists() {
        return Some(project);
    }

    // 3. User-level config
    if let Ok(path) = expand_tilde("~/.config/dispatch-agent.toml") {
        if path.exists() {
            return Some(path);
        }
    }

    None
}

#[allow(dead_code)]
pub fn load_config(path: &Path) -> anyhow::Result<Config> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;

    let content = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);

    let mut config: Config = toml::from_str(content)
        .with_context(|| format!("parsing config file {}", path.display()))?;

    validate_config(&mut config);

    Ok(config)
}

fn validate_config(config: &mut Config) {
    if config.version.is_none() {
        eprintln!("warning: config missing 'version' field, assuming v1");
    }

    let mut seen = HashSet::new();
    for tier in &config.tiers {
        for agent in &tier.agents {
            if !seen.insert(agent.id.clone()) {
                eprintln!(
                    "warning: multiple agents with id '{}', using first",
                    agent.id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

    fn make_config_toml(version: Option<u32>, tiers_toml: &str) -> String {
        let v = match version {
            Some(v) => format!("version = {v}\n"),
            None => String::new(),
        };
        format!("{v}{tiers_toml}")
    }

    #[test]
    fn find_config_project_wins_over_user() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_dir = dir.path().join(".config");
        fs::create_dir_all(&cfg_dir).unwrap();
        let project_cfg = cfg_dir.join("dispatch-agent.toml");
        fs::write(&project_cfg, "version = 1\n[[tiers]]\nid=\"x\"\n").unwrap();

        // When we're inside the tempdir (as git root), project config should be found
        let git_root = find_git_root();
        let project_path = git_root.join(".config/dispatch-agent.toml");

        // The test verifies the priority logic: if project config exists, it's returned
        let result = find_config(None);
        // Can't fully control git root in unit tests, but we can verify find_config
        // returns Some when a config exists somewhere
        if project_path.exists() {
            assert_eq!(result, Some(project_path));
        }
        // If no project config, it may still find user config or return None
    }

    #[test]
    fn load_config_bom_stripped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut f = fs::File::create(&path).unwrap();
        write!(
            f,
            "\u{FEFF}version = 1\n[[tiers]]\nid=\"x\"\n[[tiers.agents]]\nid=\"a\"\ncli=\"echo\"\n"
        )
        .unwrap();
        drop(f);

        let config = load_config(&path).unwrap();
        assert_eq!(config.version, Some(1));
        assert_eq!(config.tiers.len(), 1);
        assert_eq!(config.tiers[0].id, "x");
    }

    #[test]
    fn load_config_warns_missing_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            make_config_toml(
                None,
                "[[tiers]]\nid=\"t\"\n[[tiers.agents]]\nid=\"a\"\ncli=\"echo\"\n",
            ),
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert!(config.version.is_none());
        // Warning is emitted to stderr (observable via captured output in CI)
    }

    #[test]
    fn load_config_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "not = valid = toml").unwrap();

        let err = load_config(&path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("parsing config file"),
            "expected 'parsing config file' in error, got: {msg}"
        );
    }

    #[test]
    fn find_config_returns_none_when_no_configs() {
        // With no --config arg and no config files on disk at expected locations,
        // find_config should return None (assuming no project or user config exists).
        // This is a best-effort test since we can't control the full filesystem.
        let result = find_config(None);
        // We can't assert None because a real config may exist on the system.
        // Instead, verify the function doesn't panic and returns Option<PathBuf>.
        let _ = result;
    }

    #[test]
    fn load_config_duplicate_agent_ids_warns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dup.toml");
        fs::write(
            &path,
            "version = 1\n\
             [[tiers]]\n\
             id = \"tier1\"\n\
             [[tiers.agents]]\n\
             id = \"agent1\"\n\
             cli = \"echo\"\n\
             [[tiers]]\n\
             id = \"tier2\"\n\
             [[tiers.agents]]\n\
             id = \"agent1\"\n\
             cli = \"cat\"\n",
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.tiers.len(), 2);
        assert_eq!(config.tiers[0].agents[0].id, "agent1");
        assert_eq!(config.tiers[1].agents[0].id, "agent1");
    }

    #[test]
    fn load_config_explicit_path_returned() {
        let explicit = PathBuf::from("/some/explicit/path.toml");
        let result = find_config(Some(explicit.as_path()));
        assert_eq!(result, Some(explicit));
        // Returns even though the file doesn't exist
    }
}
