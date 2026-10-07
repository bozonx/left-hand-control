use super::system::SysCommand;
use crate::events::{self, CoreEvent};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

static GENERATION: AtomicU64 = AtomicU64::new(0);
const OUTPUT_LIMIT: usize = 4096;

struct Job {
    command: SysCommand,
    generation: u64,
}

pub(super) fn cancel() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

fn report(command: &SysCommand, result: Result<(), String>) {
    events::emit(CoreEvent::CommandFinished {
        script: command
            .args
            .last()
            .cloned()
            .unwrap_or_else(|| command.program.clone()),
        result,
    });
}

pub(super) fn enqueue(command: SysCommand) {
    static TX: OnceLock<Result<mpsc::SyncSender<Job>, String>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<Job>(32);
        std::thread::Builder::new()
            .name("lhc-commands".into())
            .spawn(move || {
                for job in rx {
                    if job.generation == GENERATION.load(Ordering::SeqCst) {
                        report(
                            &job.command,
                            run(&job.command, job.generation, job.command.timeout),
                        );
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(tx)
    });
    match tx {
        Ok(tx) => {
            if let Err(error) = tx.try_send(Job {
                command,
                generation: GENERATION.load(Ordering::SeqCst),
            }) {
                let reason = match &error {
                    mpsc::TrySendError::Full(_) => "Command queue is full",
                    mpsc::TrySendError::Disconnected(_) => "Command worker is unavailable",
                };
                let (mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job)) = error;
                report(&job.command, Err(reason.into()));
            }
        }
        Err(error) => report(&command, Err(error.clone())),
    }
}

fn working_directory(value: Option<&str>) -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("Home directory is unavailable")?;
    let value = value.unwrap_or("").trim();
    let directory = if value.is_empty() || value == "~" {
        home
    } else if let Some(relative) = value.strip_prefix("~/") {
        home.join(relative)
    } else {
        let path = std::path::PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            home.join(path)
        }
    };
    if !directory.is_dir() {
        return Err(format!(
            "Working directory does not exist: {}",
            directory.display()
        ));
    }
    Ok(directory)
}

fn run(command: &SysCommand, generation: u64, timeout: Duration) -> Result<(), String> {
    let mut invocation = Command::new(&command.program);
    invocation
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0);
    invocation.current_dir(working_directory(command.working_directory.as_deref())?);
    let mut child = invocation
        .spawn()
        .map_err(|error| format!("Cannot start command: {error}"))?;
    let pid = child.id() as libc::pid_t;
    let mut stderr = child.stderr.take().ok_or("Command stderr is unavailable")?;
    let fd = stderr.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = child.wait();
        return Err("Cannot read command output".into());
    }
    let start = Instant::now();
    let mut output = Vec::new();
    let mut buffer = [0u8; 1024];
    let result = loop {
        for _ in 0..16 {
            match stderr.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => output.extend_from_slice(
                    &buffer[..count.min(OUTPUT_LIMIT.saturating_sub(output.len()))],
                ),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        if generation != GENERATION.load(Ordering::SeqCst) {
            break Err(
                "Command cancelled because the mapper stopped or configuration changed".into(),
            );
        }
        if start.elapsed() >= timeout {
            break Err("Command exceeded its time limit".into());
        }
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let waited = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if waited < 0 {
            break Err(format!(
                "Cannot wait for command: {}",
                std::io::Error::last_os_error()
            ));
        }
        if unsafe { info.si_pid() } != 0 {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let status = child
                .wait()
                .map_err(|error| format!("Cannot wait for command: {error}"))?;
            for _ in 0..16 {
                match stderr.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => output.extend_from_slice(
                        &buffer[..count.min(OUTPUT_LIMIT.saturating_sub(output.len()))],
                    ),
                }
            }
            break if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Command exited with {status}: {}",
                    String::from_utf8_lossy(&output).trim()
                ))
            };
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if child.try_wait().is_ok_and(|status| status.is_none()) {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = child.wait();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_in_selected_directory_and_rejects_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder with spaces");
        std::fs::create_dir(&folder).unwrap();
        let mut command = SysCommand {
            program: "sh".into(),
            args: vec!["-lc".into(), "printf hello > marker".into()],
            working_directory: Some(folder.to_string_lossy().into()),
            timeout: Duration::from_secs(2),
        };
        run(&command, GENERATION.load(Ordering::SeqCst), command.timeout).unwrap();
        assert_eq!(
            std::fs::read_to_string(folder.join("marker")).unwrap(),
            "hello"
        );
        assert_eq!(working_directory(None).unwrap(), dirs::home_dir().unwrap());
        assert_eq!(
            working_directory(Some("~")).unwrap(),
            dirs::home_dir().unwrap()
        );
        assert_eq!(
            working_directory(Some("~/.")).unwrap(),
            dirs::home_dir().unwrap().join(".")
        );
        assert_eq!(
            working_directory(Some(".")).unwrap(),
            dirs::home_dir().unwrap().join(".")
        );
        command.working_directory = Some(dir.path().join("missing").to_string_lossy().into());
        assert!(
            run(&command, GENERATION.load(Ordering::SeqCst), command.timeout)
                .unwrap_err()
                .contains("Working directory does not exist")
        );
    }

    #[test]
    fn stops_background_children_and_bounds_error_output() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("late");
        let command = SysCommand {
            program: "sh".into(),
            working_directory: None,
            timeout: Duration::from_secs(30),
            args: vec![
                "-c".into(),
                format!("(sleep 0.2; touch '{}') &", marker.display()),
            ],
        };
        run(
            &command,
            GENERATION.load(Ordering::SeqCst),
            Duration::from_secs(2),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(250));
        assert!(!marker.exists());
        let command = SysCommand {
            program: "sh".into(),
            working_directory: None,
            timeout: Duration::from_secs(30),
            args: vec!["-c".into(), "head -c 20000 /dev/zero >&2; exit 1".into()],
        };
        let error = run(
            &command,
            GENERATION.load(Ordering::SeqCst),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(error.len() < OUTPUT_LIMIT + 100);
    }

    #[test]
    fn captures_failure_and_limits_runtime() {
        let command = SysCommand {
            program: "sh".into(),
            working_directory: None,
            timeout: Duration::from_secs(30),
            args: vec!["-c".into(), "printf 'failure details' >&2; exit 7".into()],
        };
        let error = run(
            &command,
            GENERATION.load(Ordering::SeqCst),
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(error.contains("failure details"));
        assert!(error.contains('7'));
        let command = SysCommand {
            program: "sh".into(),
            working_directory: None,
            timeout: Duration::from_secs(30),
            args: vec!["-c".into(), "sleep 10".into()],
        };
        let start = Instant::now();
        assert!(
            run(
                &command,
                GENERATION.load(Ordering::SeqCst),
                Duration::from_millis(30)
            )
            .unwrap_err()
            .contains("time limit")
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
