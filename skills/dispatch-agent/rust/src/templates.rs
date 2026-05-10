use anyhow::{anyhow, Context};
use indexmap::IndexMap;
use std::{env, fs};

use crate::types::Template;

#[allow(dead_code)]
pub fn load_templates() -> anyhow::Result<IndexMap<String, Template>> {
    let path = resolve_templates_path()?;
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("reading templates file {}", path.display()))?;
    let content = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let mut map: IndexMap<String, Template> = toml::from_str(content)
        .with_context(|| format!("parsing templates file {}", path.display()))?;
    for tmpl in map.values_mut() {
        if tmpl.version_flag.is_none() {
            tmpl.version_flag = Some("--version".to_string());
        }
    }
    Ok(map)
}

#[allow(dead_code)]
fn resolve_templates_path() -> anyhow::Result<std::path::PathBuf> {
    if let Ok(p) = env::var("DISPATCH_AGENT_TEMPLATES") {
        return Ok(std::path::PathBuf::from(p));
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let p = exe_dir.join("../data/cli-templates.toml");
            let p = p.canonicalize().unwrap_or(p);
            if p.exists() {
                return Ok(p);
            }
            let p = exe_dir.join("data/cli-templates.toml");
            if p.exists() {
                return Ok(p);
            }
        }
    }
    Err(anyhow!(
        "cli-templates.toml not found; set DISPATCH_AGENT_TEMPLATES to its path"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn load_templates_from_env() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = write_toml(dir.path(), "[test-cli]\nprompt_flag = \"-p\"\n");
        let _guard = EnvGuard::set("DISPATCH_AGENT_TEMPLATES", path.to_str().unwrap());
        let map = load_templates().unwrap();
        assert!(map.contains_key("test-cli"));
        assert_eq!(map["test-cli"].prompt_flag.as_deref(), Some("-p"));
    }

    #[test]
    fn version_flag_default_applied() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = write_toml(dir.path(), "[cli]\nprompt_flag = \"-p\"\n");
        let _guard = EnvGuard::set("DISPATCH_AGENT_TEMPLATES", path.to_str().unwrap());
        let map = load_templates().unwrap();
        assert_eq!(map["cli"].version_flag.as_deref(), Some("--version"));
    }

    #[test]
    fn version_flag_explicit_not_overwritten() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = write_toml(
            dir.path(),
            "[cli]\nprompt_flag = \"-p\"\nversion_flag = \"\"\n",
        );
        let _guard = EnvGuard::set("DISPATCH_AGENT_TEMPLATES", path.to_str().unwrap());
        let map = load_templates().unwrap();
        assert_eq!(map["cli"].version_flag.as_deref(), Some(""));
    }

    #[test]
    fn bom_stripped() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let content = "\u{FEFF}[cli]\nprompt_flag = \"-p\"\n";
        let path = write_toml(dir.path(), content);
        let _guard = EnvGuard::set("DISPATCH_AGENT_TEMPLATES", path.to_str().unwrap());
        let map = load_templates().unwrap();
        assert!(map.contains_key("cli"));
    }

    #[test]
    fn insertion_order_preserved() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let toml =
            "[b]\nprompt_flag = \"-p\"\n[a]\nprompt_flag = \"-p\"\n[c]\nprompt_flag = \"-p\"\n";
        let path = write_toml(dir.path(), toml);
        let _guard = EnvGuard::set("DISPATCH_AGENT_TEMPLATES", path.to_str().unwrap());
        let map = load_templates().unwrap();
        let keys: Vec<&String> = map.keys().collect();
        assert_eq!(keys, vec!["b", "a", "c"]);
    }

    #[test]
    fn missing_file_error() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let _guard = EnvGuard::set(
            "DISPATCH_AGENT_TEMPLATES",
            "/nonexistent/path/cli-templates.toml",
        );
        let result = load_templates();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("reading templates file"),
            "expected 'reading templates file' in error, got: {msg}"
        );
    }
}
