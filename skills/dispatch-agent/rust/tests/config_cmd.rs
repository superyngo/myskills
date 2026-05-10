use std::process::Command;
use tempfile::TempDir;

#[test]
fn config_path_no_config() {
    let dir = TempDir::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("config")
        .arg("path")
        .env("HOME", dir.path())
        .current_dir(dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run config path");

    assert!(
        !out.status.success(),
        "expected non-zero exit with no config"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no config file found") || stderr.contains("error"),
        "stderr: {stderr}"
    );
    // Spec: stderr lists default search locations
    assert!(
        stderr.contains(".config") || stderr.contains("dispatch-agent"),
        "stderr should list search locations, got: {stderr}"
    );
}

#[test]
fn config_show_no_config() {
    let dir = TempDir::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("config")
        .arg("show")
        .env("HOME", dir.path())
        .current_dir(dir.path())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .output()
        .expect("failed to run config show");

    assert!(
        !out.status.success(),
        "expected non-zero exit with no config"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no config file found") || stderr.contains("error"),
        "stderr: {stderr}"
    );
}
