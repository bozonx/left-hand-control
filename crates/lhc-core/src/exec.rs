//! Bounded subprocess execution shared by the poll-based watchers
//! (gamemode, active_window).

#![cfg(target_os = "linux")]

use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Poll interval while waiting for a child process.
const WAIT_POLL: Duration = Duration::from_millis(5);

/// Run a command and wait at most `timeout_ms` for it to finish.
///
/// Pipes are nonblocking and output is bounded. On cancellation the process
/// group is killed before the child is reaped, so an inherited pipe cannot
/// leak reader threads or cause a recycled process id to be signalled.
pub fn run_cmd_with_timeout(cmd: &mut Command, timeout_ms: u64) -> Option<Output> {
    run_cmd_with_cancel(cmd, timeout_ms, || false)
}

/// Like [`run_cmd_with_timeout`], but also kills the child when its work is cancelled.
pub(crate) fn run_cmd_with_cancel(
    cmd: &mut Command,
    timeout_ms: u64,
    mut cancelled: impl FnMut() -> bool,
) -> Option<Output> {
    let mut child = cmd
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let result = (|| {
        let mut stdout = child.stdout.take()?;
        let mut stderr = child.stderr.take()?;
        for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
            // SAFETY: both descriptors belong to live child pipes.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                return None;
            }
        }
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut output = [Vec::new(), Vec::new()];
        let mut eof = [false; 2];
        loop {
            if cancelled() || Instant::now() >= deadline {
                return None;
            }
            for (index, pipe) in [&mut stdout as &mut dyn Read, &mut stderr]
                .into_iter()
                .enumerate()
            {
                if !eof[index] {
                    eof[index] = read_available(pipe, &mut output[index]).ok()?;
                }
            }
            // WNOWAIT keeps the child's pid reserved until the group has been
            // signalled or successful output collection is complete.
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            let rc = unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if rc != 0 {
                return None;
            }
            let exited = unsafe { info.assume_init().si_pid() != 0 };
            if exited && eof.iter().all(|done| *done) {
                let status = child.wait().ok()?;
                return Some(Output {
                    status,
                    stdout: std::mem::take(&mut output[0]),
                    stderr: std::mem::take(&mut output[1]),
                });
            }
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(WAIT_POLL),
            );
        }
    })();
    if result.is_none() {
        // SAFETY: the unreaped child reserves its pid and process group id.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.kill();
        // A kernel task in uninterruptible sleep must not extend the deadline.
        let _ = std::thread::Builder::new()
            .name("lhc-tool-reaper".into())
            .spawn(move || {
                let _ = child.wait();
            });
    }
    result
}

const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

fn read_available(pipe: &mut dyn Read, output: &mut Vec<u8>) -> std::io::Result<bool> {
    let mut buf = [0; 8192];
    // Bound work per iteration even when a child continuously produces output.
    for _ in 0..8 {
        match pipe.read(&mut buf) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                if output.len() + count > MAX_OUTPUT_BYTES {
                    return Err(std::io::Error::other("subprocess output limit exceeded"));
                }
                output.extend_from_slice(&buf[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(false)
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
    fn excessive_output_is_rejected() {
        let start = Instant::now();
        assert!(
            run_cmd_with_timeout(Command::new("sh").args(["-c", "yes output"]), 2000).is_none()
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn inherited_pipe_is_closed_when_the_process_group_is_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("survived");
        let script = format!("(sleep 0.3; touch '{}') & exit 0", marker.display());
        assert!(run_cmd_with_timeout(Command::new("sh").args(["-c", &script]), 50).is_none());
        std::thread::sleep(Duration::from_millis(400));
        assert!(!marker.exists());
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
