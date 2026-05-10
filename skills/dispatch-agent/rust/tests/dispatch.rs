use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn write_fake_agent_templates(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("templates.toml");
    let fake_agent = env!("CARGO_BIN_EXE_fake_agent");
    let content = format!(
        r#"[fake-agent]
detect_binary = "{fake_agent}"
prompt_flag = "-p"
verified = true
version_flag = "--version"
"#
    );
    fs::write(&path, content).unwrap();
    path
}

fn write_fake_agent_config(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("config.toml");
    let content = format!(
        r#"version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "fake-default"
  cli = "fake-agent"
  model = "default"
  args = []
"#
    );
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn dry_run_happy_path() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("--dry-run")
        .arg("-p")
        .arg("hello")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should contain the fake-agent template name or the built command
    assert!(stdout.contains("fake-agent") || stdout.contains("-p hello"));
}

#[test]
fn dry_run_no_prompt() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("--dry-run")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should contain <prompt> literal when no prompt is provided
    assert!(stdout.contains("<prompt>"));
}

#[test]
fn list_no_config() {
    let dir = TempDir::new().unwrap();
    let templates_path = write_fake_agent_templates(&dir);

    // No --config, HOME set to empty dir so no user config exists
    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("dispatch")
        .arg("--list")
        .env("DISPATCH_AGENT_TEMPLATES", &templates_path)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .current_dir(dir.path())
        .output()
        .expect("failed to run dispatch-agent");

    // Should exit 0 (falls back to detect)
    assert!(out.status.success());
}

#[test]
fn list_with_config() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("--list")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Should show agents from config
    assert!(stdout.contains("fake-default") || stdout.contains("primary"));
}

#[test]
fn show_config_no_config() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("dispatch")
        .arg("--show-config")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .current_dir(dir.path())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no config file found"));
}

#[test]
fn agent_not_found() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("--agent")
        .arg("BAD")
        .arg("-p")
        .arg("test")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not found") || stderr.contains("BAD"));
}

#[test]
fn tier_not_found() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("--tier")
        .arg("BAD")
        .arg("-p")
        .arg("test")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not found") || stderr.contains("BAD"));
}

#[test]
fn recursion_guard() {
    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("dispatch")
        .arg("-p")
        .arg("test")
        .env("DISPATCH_AGENT_DEPTH", "5")
        .env("HOME", "/tmp")
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run dispatch-agent");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("recursion depth limit") || stderr.contains("depth"));
}

#[test]
fn fake_agent_exit_0() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("-p")
        .arg("test prompt")
        .env("DISPATCH_AGENT_TEMPLATES", &templates)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("FAKE_AGENT_MODE", "exit-0")
        .output()
        .expect("failed to run dispatch-agent");

    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn fake_agent_exit_nonzero() {
    let dir = TempDir::new().unwrap();
    let templates = write_fake_agent_templates(&dir);
    let config = write_fake_agent_config(&dir);

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("--config")
        .arg(config.to_str().unwrap())
        .arg("dispatch")
        .arg("-p")
        .arg("test prompt")
        .env("DISPATCH_AGENT_TEMPLATES", &templates)
        .env("HOME", dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("FAKE_AGENT_MODE", "exit-N")
        .env("FAKE_AGENT_EXIT_CODE", "42")
        .output()
        .expect("failed to run dispatch-agent");

    // Dispatcher should exit 1 after exhausting all tiers, not 42
    assert_eq!(out.status.code(), Some(1));
}
