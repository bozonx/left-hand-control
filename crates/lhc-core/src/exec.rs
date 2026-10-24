//! Bounded subprocess execution shared by the poll-based watchers
//! (gamemode, active_window).

#![cfg(target_os = "linux")]

use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Poll interval while waiting for a child process.
const WAIT_POLL: Duration = Duration::from_millis(5);

/// Run a command and wait at most `timeout_ms` for it to finish.
///
/// Pipe readers run on helper threads so inherited pipes cannot keep the
/// caller blocked after its deadline. Only the caller reaps the child, so
/// cancellation cannot accidentally signal a recycled process id.
pub fn run_cmd_with_timeout(cmd: &mut Command, timeout_ms: u64) -> Option<Output> {
    run_cmd_with_cancel(cmd, timeout_ms, || false)
}

/// Like [`run_cmd_with_timeout`], but also kills the child when its work is cancelled.
pub(crate) fn run_cmd_with_cancel(
    cmd: &mut Command,
    timeout_ms: u64,
    mut cancelled: impl FnMut() -> bool,
) -> Option<Output> {
    use std::io::Read;
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let (tx, rx) = mpsc::channel();
    let stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    for (index, mut pipe) in [Box::new(stdout) as Box<dyn Read + Send>, Box::new(stderr)]
        .into_iter()
        .enumerate()
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
            let _ = tx.send((index, result));
        });
    }
    drop(tx);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut status = None;
    let mut output = [None, None];
    loop {
        while let Ok((index, result)) = rx.try_recv() {
            output[index] = result.ok();
        }
        if status.is_none() {
            status = child.try_wait().ok().flatten();
        }
        if let Some(status) = status
            && let [Some(stdout), Some(stderr)] = &mut output
        {
            return Some(Output {
                status,
                stdout: std::mem::take(stdout),
                stderr: std::mem::take(stderr),
            });
        }
        if cancelled() || Instant::now() >= deadline {
            if status.is_none() {
                let _ = child.kill();
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            return None;
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(WAIT_POLL),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_output_and_times_out_inherited_pipes() {
        let output = run_cmd_with_timeout(Command::new("printf").arg("hello"), 1000).unwrap();
        assert_eq!(output.stdout, b"hello");
        assert!(output.status.success());
        let start = Instant::now();
        assert!(
            run_cmd_with_timeout(Command::new("sh").args(["-c", "sleep 1 & exit 0"]), 50).is_none()
        );
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn cancelled_tool_is_killed_before_its_deadline() {
        let start = Instant::now();
        let output = run_cmd_with_cancel(Command::new("sleep").arg("10"), 10_000, || {
            start.elapsed() > Duration::from_millis(30)
        });
        assert!(output.is_none());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
