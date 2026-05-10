#![allow(dead_code)]

pub mod unix;
pub mod windows;

use anyhow::Context;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct ChildState {
    pub pid: Option<i32>,
    pub killed_by_timeout: bool,
    pub interrupted: bool,
}

pub type SharedState = Arc<Mutex<ChildState>>;

pub fn spawn_and_wait(
    cmd_vec: &[String],
    env_map: &HashMap<String, String>,
    timeout_secs: i64,
    agent_id: &str,
    verbose: bool,
    state: Arc<Mutex<ChildState>>,
) -> anyhow::Result<(std::process::ExitStatus, Vec<u8>, ChildState)> {
    // build command
    let mut cmd = std::process::Command::new(&cmd_vec[0]);
    cmd.args(&cmd_vec[1..]);
    cmd.env_clear().envs(env_map);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(unix)]
    unix::setup_process_group(&mut cmd);

    let mut child = cmd.spawn().context("failed to spawn child process")?;
    let pid = child.id() as i32;
    {
        let mut st = state.lock().unwrap();
        st.pid = Some(pid);
    }

    // stdout reader thread
    let stdout = child.stdout.take().context("stdout not captured")?;
    let stdout_thread = std::thread::spawn(move || {
        let mut reader = stdout;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = io::stdout().write_all(&buf[..n]);
                    let _ = io::stdout().flush();
                }
            }
        }
    });

    // stderr collector thread
    let stderr = child.stderr.take().context("stderr not captured")?;
    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf_clone = Arc::clone(&stderr_buf);
    let stderr_thread = std::thread::spawn(move || {
        let mut reader = stderr;
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        let mut guard = stderr_buf_clone.lock().unwrap();
        *guard = buf;
    });

    // verbose ticker thread
    let ticker_shutdown = Arc::new(AtomicBool::new(false));
    let ticker_shutdown_clone = Arc::clone(&ticker_shutdown);
    let agent_id_owned = agent_id.to_string();
    let ticker_thread = std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            std::thread::park_timeout(Duration::from_secs(10));
            if ticker_shutdown_clone.load(Ordering::Relaxed) {
                break;
            }
            if verbose {
                let elapsed = start.elapsed().as_secs();
                let _ = writeln!(
                    io::stderr(),
                    "[waiting: {} — {}s elapsed]",
                    agent_id_owned,
                    elapsed
                );
            }
        }
    });

    // wait for child
    let exit_status;
    if timeout_secs > 0 {
        use wait_timeout::ChildExt;
        match child.wait_timeout(Duration::from_secs(timeout_secs as u64))? {
            None => {
                // timeout
                {
                    let mut st = state.lock().unwrap();
                    st.killed_by_timeout = true;
                }
                #[cfg(unix)]
                unix::killpg(pid, libc::SIGKILL);
                exit_status = child.wait()?;
            }
            Some(status) => {
                exit_status = status;
            }
        }
    } else {
        exit_status = child.wait()?;
    }

    // join threads
    ticker_shutdown.store(true, Ordering::Relaxed);
    ticker_thread.thread().unpark();
    // child.kill() is a no-op here (already reaped), but harmless
    stdout_thread.join().ok();
    stderr_thread.join().ok();
    ticker_thread.join().ok();

    // cleanup state
    let state_snapshot = {
        let mut st = state.lock().unwrap();
        st.pid = None;
        st.clone()
    };

    let stderr_bytes = {
        let guard = stderr_buf.lock().unwrap();
        guard.clone()
    };

    Ok((exit_status, stderr_bytes, state_snapshot))
}
