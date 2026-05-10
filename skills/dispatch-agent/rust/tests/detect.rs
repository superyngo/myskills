use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn detect_json() {
    let dir = TempDir::new().unwrap();
    let templates = dir.path().join("templates.toml");
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/inputs/fake-detect-templates.toml");
    fs::copy(&fixtures, &templates).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("detect")
        .env("DISPATCH_AGENT_TEMPLATES", templates)
        .env("HOME", dir.path())
        .output()
        .expect("failed to run detect");

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        parsed.get("fake-agent").is_some(),
        "JSON output should contain 'fake-agent' key, got: {stdout}"
    );
}
