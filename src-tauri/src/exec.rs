//! Bounded subprocess execution shared by the poll-based watchers
//! (gamemode, active_window).

#![cfg(target_os = "linux")]

use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Run a command and wait at most `timeout_ms` for it to finish.
///
/// The wait happens on a helper thread so the caller is *always* bounded
/// by the timeout — `wait_with_output()` alone can block forever when a
/// grandchild process inherits the stdout pipe and never exits (killing
/// the direct child does not close the pipe then). On timeout the direct
/// child is SIGKILLed; the helper thread has not reaped it yet at that
/// point, so the PID is still ours (alive or zombie) and the kill cannot
/// hit a recycled PID.
pub fn run_cmd_with_timeout(cmd: &mut Command, timeout_ms: u64) -> Option<Output> {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let pid = child.id() as libc::pid_t;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(result) => result.ok(),
        Err(_) => {
            unsafe { libc::kill(pid, libc::SIGKILL) };
            None
        }
    }
}
