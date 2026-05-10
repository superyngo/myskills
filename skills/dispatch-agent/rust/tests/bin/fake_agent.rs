fn main() {
    let mode = std::env::var("FAKE_AGENT_MODE").unwrap_or_default();
    match mode.as_str() {
        "exit-0" => std::process::exit(0),
        "exit-N" => {
            let n: i32 = std::env::var("FAKE_AGENT_EXIT_CODE")
                .unwrap_or("1".into())
                .trim()
                .parse()
                .unwrap_or(1);
            std::process::exit(n);
        }
        "sleep" => {
            if let Ok(f) = std::env::var("READY_FILE") {
                std::fs::write(f, "ready").ok();
            }
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        "print-env" => {
            for (k, v) in std::env::vars().filter(|(k, _)| k.starts_with("TEST_")) {
                println!("{k}={v}");
            }
        }
        _ => std::process::exit(0),
    }
}
