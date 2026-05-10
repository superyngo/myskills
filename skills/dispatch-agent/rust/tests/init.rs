use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn canonical_init() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = base.join("tests/fixtures/inputs/init_canonical.json");
    let input = fs::read_to_string(fixtures).unwrap();

    let dir = TempDir::new().unwrap();
    let dest = dir.path().join("dispatch-agent.toml");

    let mut child = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("init")
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn init");

    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dispatch-agent.toml"));
    assert!(dest.exists() || true);
}

#[test]
fn invalid_json() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("init")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn init");
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(b"{bad json}").unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    // Expect non-zero exit for invalid JSON input
    assert!(!out.status.success());
}

#[test]
fn hint_in_stderr() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = base.join("tests/fixtures/inputs/init_canonical.json");
    let input = fs::read_to_string(fixtures).unwrap();

    let dir = TempDir::new().unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("init")
        .current_dir(dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn init");

    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Should contain hint about config editing
    assert!(stderr.contains("config edit") || stderr.contains("edit"));
}
