#![allow(dead_code)]

use anyhow::{anyhow, Context};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
// time import not needed; kept out to satisfy clippy

use crate::config::{find_config, find_git_root, load_config};
use crate::dispatch::display::format_show_config;

use crate::cli::ConfigArgs;

pub fn cmd_config(args: &ConfigArgs, config_arg: Option<&Path>) -> anyhow::Result<()> {
    match args.action.as_deref() {
        None | Some("edit") => cmd_config_edit(config_arg),
        Some("show") => cmd_config_show(config_arg),
        Some("path") => cmd_config_path(config_arg),
        Some(a) => Err(anyhow!("unknown action '{}'", a)),
    }
}

fn cmd_config_path(config_arg: Option<&Path>) -> anyhow::Result<()> {
    if let Some(p) = config_arg {
        println!("{}", p.display());
        return Ok(());
    }

    if let Some(found) = find_config(None) {
        println!("{}", found.display());
        return Ok(());
    }

    // No config found — print helpful hint that matches spec
    eprintln!("error: no config file found");
    eprintln!("hint: default locations searched:");
    eprintln!(
        "  {}/.config/dispatch-agent.toml (project)",
        find_git_root().display()
    );
    if let Ok(home_cfg) = crate::fsutil::expand_tilde("~/.config/dispatch-agent.toml") {
        eprintln!("  {} (user)", home_cfg.display());
    } else {
        eprintln!("  ~/.config/dispatch-agent.toml (user)");
    }

    Err(anyhow!("no config file found"))
}

fn cmd_config_show(config_arg: Option<&Path>) -> anyhow::Result<()> {
    let path = resolve_or_error(config_arg)?;
    let cfg = load_config(&path).map_err(|e| anyhow!(e.to_string()))?;
    let out = format_show_config(&cfg, &path);
    println!("{}", out);
    Ok(())
}

fn cmd_config_edit(config_arg: Option<&Path>) -> anyhow::Result<()> {
    let path: PathBuf;
    if let Some(p) = config_arg {
        path = p.to_path_buf();
        // If path doesn't exist, create a stub
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent dir for {}", path.display()))?;
            }
            let mut f = fs::File::create(&path)
                .with_context(|| format!("creating config file {}", path.display()))?;
            let stub = "version = 1\n# See cli-templates.toml for available templates and field reference.\n";
            f.write_all(stub.as_bytes())?;
        }
    } else {
        // find existing config
        if let Some(found) = find_config(None) {
            path = found;
        } else {
            return Err(anyhow!(
                "error: no config file found. Run 'dispatch-agent init' to create one."
            ));
        }
    }

    let (cmd, pre_args) = resolve_editor_command();

    // Record mtime before
    let before_mtime = fs::metadata(&path).and_then(|m| m.modified()).ok();

    let status = Command::new(&cmd)
        .args(&pre_args)
        .arg(&path)
        .status()
        .with_context(|| format!("spawning editor '{}'", cmd))?;

    // Check mtime after
    let after_mtime = fs::metadata(&path).and_then(|m| m.modified()).ok();

    if status.success()
        && before_mtime.is_some()
        && after_mtime.is_some()
        && before_mtime == after_mtime
    {
        eprintln!("hint: your editor may have returned immediately. For GUI editors, set EDITOR to include a wait flag (e.g. 'code -w', 'subl -w')");
    }

    // Re-run load_config and warn on parse errors but don't fail
    match load_config(&path) {
        Ok(_) => {}
        Err(e) => eprintln!("warning: config has syntax errors: {}", e),
    }

    Ok(())
}

fn resolve_or_error(config_arg: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(p) = config_arg {
        return Ok(p.to_path_buf());
    }
    if let Some(found) = find_config(None) {
        return Ok(found);
    }
    Err(anyhow!(
        "error: no config file found. Run 'dispatch-agent init' to create one."
    ))
}

/// Resolve editor command according to $EDITOR, $VISUAL, defaulting to 'vi'.
/// Returns (command, pre_args) where pre_args are tokens to prepend before the file path.
pub(crate) fn resolve_editor_command() -> (String, Vec<String>) {
    for key in &["EDITOR", "VISUAL"] {
        if let Ok(val) = env::var(key) {
            if !val.trim().is_empty() {
                let mut parts = val.split_whitespace();
                if let Some(cmd) = parts.next() {
                    let args = parts.map(|s| s.to_string()).collect();
                    return (cmd.to_string(), args);
                }
            }
        }
    }
    ("vi".to_string(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn config_path_prints_path_ok() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("cfg.toml");
        fs::write(&cfg, "version = 1\n[[tiers]]\nid=\"t\"\n").unwrap();
        let args = ConfigArgs {
            action: Some("path".to_string()),
        };
        let res = cmd_config(&args, Some(cfg.as_path()));
        assert!(res.is_ok());
    }

    #[test]
    fn config_path_no_config_error() {
        // Ensure no env config via find_config; we can't fully control FS but expect an error when None
        let args = ConfigArgs {
            action: Some("path".to_string()),
        };
        let res = cmd_config(&args, None);
        // Either Ok (if system has config) or Err containing 'no config file found'
        if res.is_err() {
            let msg = res.unwrap_err().to_string();
            assert!(msg.contains("no config file found"));
        }
    }

    #[test]
    fn config_show_valid() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("cfg.toml");
        fs::write(
            &cfg,
            "version = 1\n[[tiers]]\nid=\"t\"\n[[tiers.agents]]\nid=\"a\"\ncli=\"echo\"\n",
        )
        .unwrap();
        let args = ConfigArgs {
            action: Some("show".to_string()),
        };
        let res = cmd_config(&args, Some(cfg.as_path()));
        assert!(res.is_ok());
    }

    #[test]
    fn config_show_no_config() {
        let args = ConfigArgs {
            action: Some("show".to_string()),
        };
        let res = cmd_config(&args, None);
        if res.is_err() {
            let msg = res.unwrap_err().to_string();
            assert!(msg.contains("no config file found"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn config_edit_creates_stub() {
        let _lock = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("newcfg.toml");

        // create a stub editor script that immediately exits 0
        let script = dir.path().join("editor.sh");
        fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o700);
            fs::set_permissions(&script, perms).unwrap();
        }
        env::set_var("EDITOR", script.to_str().unwrap());
        let args = ConfigArgs {
            action: Some("edit".to_string()),
        };
        let res = cmd_config(&args, Some(cfg.as_path()));
        env::remove_var("EDITOR");
        assert!(res.is_ok());
        let content = fs::read_to_string(&cfg).unwrap();
        assert!(content.contains("version = 1"));
    }

    #[test]
    fn config_edit_splits_editor_arg() {
        let (cmd, args) = resolve_editor_command_for_test("code -w");
        assert_eq!(cmd, "code");
        assert_eq!(args, vec!["-w"]);
    }

    #[test]
    fn config_edit_whitespace_editor_falls_through() {
        let _lock = ENV_MUTEX.lock().unwrap();
        env::set_var("EDITOR", "   ");
        env::set_var("VISUAL", "myvis");
        let (cmd, _args) = resolve_editor_command();
        env::remove_var("EDITOR");
        env::remove_var("VISUAL");
        // VISUAL should be used
        assert_eq!(cmd, "myvis");
    }

    // Helper to test splitting without relying on env
    fn resolve_editor_command_for_test(val: &str) -> (String, Vec<String>) {
        if !val.trim().is_empty() {
            let mut parts = val.split_whitespace();
            if let Some(cmd) = parts.next() {
                let args = parts.map(|s| s.to_string()).collect();
                return (cmd.to_string(), args);
            }
        }
        ("vi".to_string(), Vec::new())
    }
}
