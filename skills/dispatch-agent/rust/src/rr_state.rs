#![allow(dead_code)]

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::Result;
use fs2::FileExt;
use indexmap::IndexMap;

fn lock_file_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.lock", path.display()))
}

fn acquire_lock(path: &Path) -> Result<File> {
    let lock_path = lock_file_path(path);
    if let Some(parent) = lock_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = File::create(&lock_path)?;
    file.lock_exclusive()?;
    Ok(file)
}

pub fn load_rr_state(path: &Path) -> IndexMap<String, String> {
    let _lock = match acquire_lock(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("warning: cannot lock rr-state at {}: {e}", path.display());
            return IndexMap::new();
        }
    };

    let data = match fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => return IndexMap::new(),
            std::io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "warning: cannot read rr-state at {}: permission denied",
                    path.display()
                );
                return IndexMap::new();
            }
            _ => {
                eprintln!("warning: cannot read rr-state at {}: {e}", path.display());
                return IndexMap::new();
            }
        },
    };

    match serde_json::from_str(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("warning: cannot parse rr-state at {}: {e}", path.display());
            IndexMap::new()
        }
    }
    // _lock dropped here → unlock
}

pub fn store_rr_state(path: &Path, state: &IndexMap<String, String>) -> Result<()> {
    let _lock = acquire_lock(path)?;
    let json = serde_json::to_string_pretty(state)?;
    crate::fsutil::write_atomic(path, json.as_bytes())?;
    Ok(())
    // _lock dropped here → unlock
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn make_state(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rr-state.json");
        let state = make_state(&[
            ("agent-a.example.com", "2025-01-15T10:00:00Z"),
            ("agent-b.example.com", "2025-01-15T11:30:00Z"),
        ]);
        store_rr_state(&path, &state).unwrap();
        let loaded = load_rr_state(&path);
        assert_eq!(loaded, state);
    }

    #[test]
    fn not_found_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        let loaded = load_rr_state(&path);
        assert!(loaded.is_empty());
    }

    #[test]
    fn concurrent_load_store_no_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("concurrent.json");

        let path_clone = path.clone();
        let store_handle = thread::spawn(move || {
            for i in 0..50 {
                let mut state = IndexMap::new();
                state.insert("counter".to_string(), i.to_string());
                state.insert("agent".to_string(), "test-agent".to_string());
                if store_rr_state(&path_clone, &state).is_err() {
                    return false;
                }
            }
            true
        });

        let path_clone = path.clone();
        let load_handle = thread::spawn(move || {
            for _ in 0..50 {
                let loaded = load_rr_state(&path_clone);
                // Must always be valid JSON (either empty or containing our keys)
                if !loaded.is_empty() {
                    assert!(loaded.contains_key("counter"), "missing 'counter'");
                    assert!(loaded.contains_key("agent"), "missing 'agent'");
                }
            }
        });

        assert!(store_handle.join().unwrap());
        load_handle.join().unwrap();

        // Final load must succeed and return valid data
        let final_state = load_rr_state(&path);
        assert!(serde_json::to_string(&final_state).is_ok());
        assert_eq!(final_state["agent"], "test-agent");
    }
}
