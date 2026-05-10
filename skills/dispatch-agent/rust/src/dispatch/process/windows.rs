#![allow(dead_code)]

use anyhow::Result;

pub fn dispatch_unix_only() -> Result<std::process::ExitStatus> {
    anyhow::bail!("dispatch is unix-only in v1")
}
