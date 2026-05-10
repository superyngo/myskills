use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};

#[allow(dead_code)]
pub fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent directory: {}", path.display()))?;
    fs::create_dir_all(parent)?;

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tmp".to_string());
    let pid = std::process::id();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.subsec_nanos();
    let temp_name = format!(".{stem}.{pid}.{nanos}.tmp");
    let temp_path = parent.join(temp_name);

    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temp_path)
            .map_err(|e| cleanup_and_return(&temp_path, e))?
    };

    #[cfg(not(unix))]
    let mut file = {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|e| cleanup_and_return(&temp_path, e))?
    };

    if let Err(e) = file.write_all(content) {
        let _ = fs::remove_file(&temp_path);
        return Err(e.into());
    }
    if let Err(e) = file.flush() {
        let _ = fs::remove_file(&temp_path);
        return Err(e.into());
    }
    drop(file);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&temp_path);
            return Err(e.into());
        }
    }

    fs::rename(&temp_path, path).map_err(|e| cleanup_and_return(&temp_path, e))?;
    Ok(())
}

fn cleanup_and_return(temp_path: &Path, err: std::io::Error) -> std::io::Error {
    let _ = fs::remove_file(temp_path);
    err
}

#[allow(dead_code)]
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if path == "~" || path.starts_with("~/") {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow!("cannot determine home directory ($HOME not set)"))?;
        let rest = path.trim_start_matches("~/").trim_start_matches("~");
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~").unwrap(), home);
    }

    #[test]
    fn expand_tilde_subpath() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~/foo/bar").unwrap(), home.join("foo/bar"));
    }

    #[test]
    fn expand_tilde_absolute() {
        assert_eq!(
            expand_tilde("/abs/path").unwrap(),
            PathBuf::from("/abs/path")
        );
    }

    #[test]
    fn expand_tilde_relative() {
        assert_eq!(expand_tilde("rel/path").unwrap(), PathBuf::from("rel/path"));
    }

    #[test]
    fn expand_tilde_empty() {
        assert_eq!(expand_tilde("").unwrap(), PathBuf::from(""));
    }

    #[test]
    fn write_atomic_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        let content = b"hello world";
        write_atomic(&path, content).unwrap();
        let read_back = fs::read(&path).unwrap();
        assert_eq!(read_back, content);
    }

    #[test]
    fn write_atomic_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c/file.txt");
        write_atomic(&path, b"deep").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"deep");
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mode_test.txt");
        write_atomic(&path, b"permissions").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
