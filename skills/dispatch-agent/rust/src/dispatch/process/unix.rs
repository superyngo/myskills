use super::ChildState;
use std::os::unix::process::CommandExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub fn setup_process_group(cmd: &mut std::process::Command) {
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

pub fn killpg(pgid: i32, sig: libc::c_int) {
    unsafe {
        libc::killpg(pgid, sig);
    }
}

pub fn start_signal_watcher(
    state: Arc<Mutex<ChildState>>,
    shutdown: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use signal_hook::consts::{SIGINT, SIGTERM};
        use signal_hook::iterator::Signals;
        let mut signals = match Signals::new([SIGINT, SIGTERM]) {
            Ok(s) => s,
            Err(_) => return,
        };
        for sig in &mut signals {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            let pid = state.lock().unwrap().pid;
            if let Some(pid) = pid {
                killpg(pid, sig);
            }
            state.lock().unwrap().interrupted = true;
        }
    })
}
