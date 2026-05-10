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

    let mut child = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("init")
        .env("HOME", dir.path())
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
    assert!(
        out.status.success(),
        "init failed: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );

    // File should be created at ~/<HOME>/.config/dispatch-agent.toml
    let config_path = dir.path().join(".config").join("dispatch-agent.toml");
    assert!(
        config_path.exists(),
        "config file not created at {}",
        config_path.display()
    );

    // Should be valid TOML with a version field
    let content = fs::read_to_string(&config_path).unwrap();
    let parsed: toml::Value = toml::from_str(&content).expect("config should be valid TOML");
    assert!(
        parsed.get("version").is_some(),
        "config should have version field, got: {content}"
    );

    // On unix: file mode should be 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&config_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "file mode should be 0600, got {:o}",
            perms.mode() & 0o777
        );
    }
}

#[test]
fn invalid_json() {
    let dir = TempDir::new().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("init")
        .env("HOME", dir.path())
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn init");
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(b"{bad json}").unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    assert!(
        !out.status.success(),
        "expected non-zero exit for invalid JSON"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Spec: stderr contains "invalid JSON"
    assert!(
        stderr.to_lowercase().contains("json") || stderr.contains("parse"),
        "stderr should mention JSON error, got: {stderr}"
    );
}

#[test]
fn hint_in_stderr() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = base.join("tests/fixtures/inputs/init_canonical.json");
    let input = fs::read_to_string(fixtures).unwrap();

    let dir = TempDir::new().unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_dispatch-agent"))
        .arg("init")
        .env("HOME", dir.path())
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
    // Spec: successful init prints stderr hint containing "config edit"
    assert!(
        stderr.contains("config edit"),
        "stderr should contain 'config edit' hint, got: {stderr}"
    );
}
