use indexmap::IndexMap;
use std::process::Command;

use crate::types::{DetectInfo, Template};

pub fn run_detect(templates: &IndexMap<String, Template>) -> IndexMap<String, DetectInfo> {
    let mut result = IndexMap::new();
    for (name, template) in templates {
        let binary_name = template.detect_binary.as_deref().unwrap_or(name.as_str());
        let info = match which::which(binary_name) {
            Err(_) => DetectInfo {
                path: None,
                version: None,
                callable: false,
                verified: template.verified,
            },
            Ok(path) => {
                let version = probe_version(&path, template);
                DetectInfo {
                    path: Some(path.to_string_lossy().into_owned()),
                    version,
                    callable: true,
                    verified: template.verified,
                }
            }
        };
        result.insert(name.clone(), info);
    }
    result
}

fn probe_version(path: &std::path::Path, template: &Template) -> Option<String> {
    let flag = template.version_flag.as_deref()?;
    if flag.is_empty() {
        return None;
    }
    let output = Command::new(path).arg(flag).output().ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    let stdout_trimmed = stdout.trim();
    let text = if stdout_trimmed.is_empty() {
        let stderr = String::from_utf8(output.stderr).ok()?;
        stderr.trim().to_string()
    } else {
        stdout_trimmed.to_string()
    };
    let first_line = text.lines().next()?.trim().to_string();
    if first_line.is_empty() {
        None
    } else {
        Some(first_line)
    }
}

#[allow(dead_code)]
pub fn cmd_detect() -> anyhow::Result<()> {
    let templates = crate::templates::load_templates()?;
    let detect = run_detect(&templates);
    println!("{}", serde_json::to_string_pretty(&detect)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::io::Write;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: String,
        old: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let old = env::var(key).ok();
            env::set_var(key, value);
            Self {
                key: key.to_string(),
                old,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => env::set_var(&self.key, v),
                None => env::remove_var(&self.key),
            }
        }
    }

    fn write_toml(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
        let path = dir.join("cli-templates.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn load_from_toml(toml: &str) -> IndexMap<String, crate::types::Template> {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = write_toml(dir.path(), toml);
        let _guard = EnvGuard::set("DISPATCH_AGENT_TEMPLATES", path.to_str().unwrap());
        crate::templates::load_templates().unwrap()
    }

    #[test]
    fn detect_finds_sh() {
        let templates = load_from_toml("[sh]\ndetect_binary = \"sh\"\nprompt_positional = true\n");
        let result = run_detect(&templates);
        let info = &result["sh"];
        assert!(info.callable);
        assert!(info.path.is_some());
    }

    #[test]
    fn detect_missing_binary() {
        let templates = load_from_toml(
            "[fake]\ndetect_binary = \"dispatch-agent-fake-nonexistent-xyz\"\nprompt_positional = true\n",
        );
        let result = run_detect(&templates);
        let info = &result["fake"];
        assert!(!info.callable);
        assert!(info.path.is_none());
        assert!(info.version.is_none());
    }

    #[test]
    fn detect_empty_version_flag() {
        let templates = load_from_toml(
            "[sh]\ndetect_binary = \"sh\"\nprompt_positional = true\nversion_flag = \"\"\n",
        );
        let result = run_detect(&templates);
        let info = &result["sh"];
        assert!(info.callable);
        assert!(info.version.is_none());
    }

    #[test]
    fn detect_version_flag_runs() {
        // sh --version may error, but callable should still be true
        let templates = load_from_toml("[sh]\ndetect_binary = \"sh\"\nprompt_positional = true\n");
        let result = run_detect(&templates);
        let info = &result["sh"];
        assert!(info.callable);
        // version may be Some or None depending on sh behavior — just assert callable
    }

    #[test]
    fn detect_verified_flag_propagated() {
        let templates = load_from_toml(
            "[sh]\ndetect_binary = \"sh\"\nprompt_positional = true\nverified = false\n",
        );
        let result = run_detect(&templates);
        let info = &result["sh"];
        assert!(!info.verified);
    }

    #[test]
    fn detect_insertion_order() {
        let templates = load_from_toml(
            "[sh]\ndetect_binary = \"sh\"\nprompt_positional = true\n\n[fake]\ndetect_binary = \"dispatch-agent-fake-nonexistent-xyz\"\nprompt_positional = true\n",
        );
        let result = run_detect(&templates);
        let keys: Vec<&String> = result.keys().collect();
        assert_eq!(keys[0], "sh");
        assert_eq!(keys[1], "fake");
    }
}
