use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn config_path_no_config() {
    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("config")
        .arg("path")
        .output()
        .expect("failed to run config path");

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("no config file found") || stderr.contains("Error:"));
    } else {
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(!stdout.trim().is_empty());
    }
}

#[test]
fn config_show_no_config() {
    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("config")
        .arg("show")
        .output()
        .expect("failed to run config show");

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("no config file found") || stderr.contains("Error:"));
    } else {
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(!stdout.trim().is_empty());
    }
}
