//! Shell commands: direct actions run independently; macro steps report
//! completion and can be cancelled without blocking keyboard input. Linux runs the
//! `posix_script` with `sh`, Windows the `powershell_script` with PowerShell
//! and no console window. Draft validation and quoting are also available
//! to portable shells.

#[cfg(any(target_os = "linux", target_os = "windows"))]
use super::system::ProcessInvocation;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::events::{self, CoreEvent};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::process::{Command, Stdio};

/// The process that runs `command` on this OS, or `None` when it has no
/// script for it.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn shell_command(
    command: &super::config::Command,
    working_directory: Option<String>,
) -> Option<ProcessInvocation> {
    #[cfg(target_os = "linux")]
    let (program, flags, script) = ("sh", &["-lc"][..], command.posix_script.trim());
    #[cfg(target_os = "windows")]
    let (program, flags, script) = (
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
        ][..],
        command.powershell_script.trim(),
    );
    if script.is_empty() {
        return None;
    }
    let mut args: Vec<String> = flags.iter().map(|flag| flag.to_string()).collect();
    args.push(script.to_owned());
    Some(ProcessInvocation {
        program: program.into(),
        args,
        working_directory,
    })
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn report(command: &ProcessInvocation, result: Result<(), String>) {
    events::emit(CoreEvent::CommandLaunchResult {
        script: command
            .args
            .last()
            .cloned()
            .unwrap_or_else(|| command.program.clone()),
        result,
    });
}

/// Completion token for one command step. Cancellation also revokes a job
/// that is still waiting for popup focus and has not been submitted yet.
#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Clone, Debug, Default)]
pub struct MacroCommand(std::sync::Arc<CommandState>);

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Debug, Default)]
struct CommandState {
    result: std::sync::Mutex<Option<Result<(), String>>>,
    cancelled: std::sync::atomic::AtomicBool,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl MacroCommand {
    pub(crate) fn cancel(&self) {
        self.0
            .cancelled
            .store(true, std::sync::atomic::Ordering::Release);
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.0.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }
    pub(crate) fn result(&self) -> Option<Result<(), String>> {
        self.0
            .result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn complete(&self, result: Result<(), String>) {
        *self.0.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
    }
    pub(crate) fn spawn(&self, command: ProcessInvocation) {
        let job = self.clone();
        let failed = self.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("lhc-macro-command".into())
            .spawn(move || {
                if job.cancelled() {
                    return;
                }
                let result = (|| {
                    let mut child = start_with_stderr(&command, Stdio::piped())?;
                    let mut stderr = match CommandStderr::new(child.stderr.take()) {
                        Ok(stderr) => stderr,
                        Err(error) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(error);
                        }
                    };
                    loop {
                        if job.cancelled() {
                            #[cfg(target_os = "linux")]
                            // Each command owns its process group, including shell children.
                            unsafe {
                                libc::kill(-(child.id() as i32), libc::SIGKILL);
                            }
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err("Macro command cancelled".into());
                        }
                        stderr.drain();
                        match child.try_wait() {
                            Ok(Some(status)) if status.success() => return Ok(()),
                            Ok(Some(status)) => {
                                stderr.drain();
                                return Err(format!(
                                    "Macro command {} exited with {status}{}",
                                    command.program,
                                    stderr.message()
                                ));
                            }
                            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
                            Err(error) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                return Err(format!("Cannot wait for macro command: {error}"));
                            }
                        }
                    }
                })();
                job.complete(result);
            })
        {
            failed.complete(Err(format!("Cannot start macro command worker: {error}")));
        }
    }
}

/// Starts `command` and returns at once; a reaper thread collects the
/// exit status so finished commands do not linger as zombies.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn spawn(command: ProcessInvocation) {
    let result = start(&command).map(|mut child| {
        let _ = std::thread::Builder::new()
            .name("lhc-command".into())
            .spawn(move || {
                let _ = child.wait();
            });
    });
    report(&command, result);
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn start(command: &ProcessInvocation) -> Result<std::process::Child, String> {
    start_with_stderr(command, Stdio::null())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn start_with_stderr(
    command: &ProcessInvocation,
    stderr: Stdio,
) -> Result<std::process::Child, String> {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .current_dir(working_directory(command.working_directory.as_deref())?);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        process.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    process
        .spawn()
        .map_err(|error| format!("Cannot start command: {error}"))
}

/// Drain stderr without blocking cancellation or retaining unbounded output.
#[cfg(any(target_os = "linux", target_os = "windows"))]
struct CommandStderr {
    pipe: Option<std::process::ChildStderr>,
    bytes: Vec<u8>,
    truncated: bool,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl CommandStderr {
    const LIMIT: usize = 16 * 1024;

    fn new(pipe: Option<std::process::ChildStderr>) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        if let Some(pipe) = &pipe {
            use std::os::fd::AsRawFd;
            // SAFETY: the pipe is live and fcntl does not take ownership.
            let ready = unsafe {
                let flags = libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL);
                flags >= 0
                    && libc::fcntl(pipe.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) >= 0
            };
            if !ready {
                return Err(format!(
                    "Cannot capture macro command stderr: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(Self {
            pipe,
            bytes: Vec::new(),
            truncated: false,
        })
    }

    fn drain(&mut self) {
        use std::io::Read;
        let Some(pipe) = self.pipe.as_mut() else {
            return;
        };
        let mut buffer = [0u8; 4096];
        // Bound each poll as well as storage when a command writes continuously.
        for _ in 0..16 {
            #[cfg(target_os = "windows")]
            let count = match available_stderr(pipe) {
                Ok(0) => break,
                Ok(count) => count.min(buffer.len()),
                Err(_) => {
                    self.pipe = None;
                    break;
                }
            };
            #[cfg(target_os = "linux")]
            let count = buffer.len();
            match pipe.read(&mut buffer[..count]) {
                Ok(0) => {
                    self.pipe = None;
                    break;
                }
                Ok(count) => {
                    let keep = count.min(Self::LIMIT - self.bytes.len());
                    self.bytes.extend_from_slice(&buffer[..keep]);
                    self.truncated |= keep < count;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.pipe = None;
                    break;
                }
            }
        }
    }

    fn message(&self) -> String {
        let text = String::from_utf8_lossy(&self.bytes);
        let text = text.trim();
        if text.is_empty() {
            String::new()
        } else {
            format!(
                ": {text}{}",
                if self.truncated {
                    " [stderr truncated]"
                } else {
                    ""
                }
            )
        }
    }
}

#[cfg(target_os = "windows")]
fn available_stderr(pipe: &std::process::ChildStderr) -> std::io::Result<usize> {
    use std::os::windows::io::AsRawHandle;
    // PeekNamedPipe only reads the byte count; no blocking ReadFile is issued
    // while a shell (or a background child inheriting stderr) holds the pipe open.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn PeekNamedPipe(
            pipe: *mut std::ffi::c_void,
            buffer: *mut std::ffi::c_void,
            size: u32,
            read: *mut u32,
            available: *mut u32,
            left: *mut u32,
        ) -> i32;
    }
    let mut available = 0;
    // SAFETY: pipe is a live handle; only the valid available pointer is written.
    let ok = unsafe {
        PeekNamedPipe(
            pipe.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(available as usize)
    }
}

/// Directory a command runs in: home when empty, `~/` and relative paths
/// resolve against home.
pub fn working_directory(value: Option<&str>) -> Result<std::path::PathBuf, String> {
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

/// Problem the editor can point out before a command ever runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftIssue {
    MissingDirectory,
    /// The command is just the path of a file without the execute bit.
    NotExecutable,
}

pub fn draft_issue(script: &str, directory: Option<&str>) -> Option<DraftIssue> {
    let directory = working_directory(directory).ok();
    if directory.is_none() {
        return Some(DraftIssue::MissingDirectory);
    }
    #[cfg(not(unix))]
    {
        let _ = script;
        None
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = script.trim();
        let path = script
            .strip_prefix('\'')
            .and_then(|rest| rest.strip_suffix('\''))
            .or_else(|| {
                script
                    .strip_prefix('"')
                    .and_then(|rest| rest.strip_suffix('"'))
            })
            .unwrap_or(script);
        if path.is_empty() || (path == script && path.contains(char::is_whitespace)) {
            return None;
        }
        let path = match path.strip_prefix("~/") {
            Some(rest) => dirs::home_dir()?.join(rest),
            None => directory?.join(path),
        };
        let mode = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.is_file())?
            .permissions()
            .mode();
        (mode & 0o111 == 0).then_some(DraftIssue::NotExecutable)
    }
}

/// `path` quoted for `sh`, so spaces and quotes survive.
pub fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

/// A command line that runs the program at `path` in this OS's command
/// shell: a quoted path for `sh`, the call operator for PowerShell.
pub fn invoke_path(path: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("& '{}'", path.replace('\'', "''"))
    } else {
        shell_quote(path)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn shell(script: String, directory: Option<String>) -> ProcessInvocation {
        ProcessInvocation {
            program: "sh".into(),
            args: vec!["-c".into(), script],
            working_directory: directory,
        }
    }

    fn wait_for(path: &std::path::Path) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if path.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn runs_in_selected_directory_and_rejects_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("folder with spaces");
        std::fs::create_dir(&folder).unwrap();
        let mut command = shell(
            "printf hello > marker".into(),
            Some(folder.to_string_lossy().into()),
        );
        start(&command).unwrap().wait().unwrap();
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
            working_directory(Some(".")).unwrap(),
            dirs::home_dir().unwrap().join(".")
        );
        command.working_directory = Some(dir.path().join("missing").to_string_lossy().into());
        assert!(
            start(&command)
                .unwrap_err()
                .contains("Working directory does not exist")
        );
    }

    #[test]
    fn flags_scripts_without_execute_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("my script.sh");
        std::fs::write(&script, "#!/bin/sh\n").unwrap();
        let quoted = shell_quote(&script.to_string_lossy());
        assert_eq!(draft_issue(&quoted, None), Some(DraftIssue::NotExecutable));
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(draft_issue(&quoted, None), None);
        assert_eq!(draft_issue("ls -la", None), None);
        assert_eq!(
            draft_issue("ls", Some("/definitely/missing")),
            Some(DraftIssue::MissingDirectory)
        );
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn returns_before_long_commands_finish() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("late");
        let begun = Instant::now();
        spawn(shell(
            format!("sleep 0.3; touch '{}'", marker.display()),
            None,
        ));
        assert!(begun.elapsed() < Duration::from_millis(200));
        assert!(!marker.exists());
        assert!(wait_for(&marker));
    }
    fn wait_job(job: &MacroCommand) -> Result<(), String> {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = job.result() {
                return result;
            }
            assert!(Instant::now() < until, "command completion was lost");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn macro_command_reports_exit_status_and_launch_failure() {
        let dir = tempfile::tempdir().unwrap();
        for (script, expected) in [("exit 0", true), ("exit 7", false)] {
            let job = MacroCommand::default();
            job.spawn(shell(
                script.into(),
                Some(dir.path().to_string_lossy().into()),
            ));
            assert_eq!(wait_job(&job).is_ok(), expected);
        }
        let job = MacroCommand::default();
        job.spawn(shell(
            "true".into(),
            Some(dir.path().join("missing").to_string_lossy().into()),
        ));
        assert!(wait_job(&job).is_err());
    }

    #[test]
    fn macro_command_captures_bounded_stderr_without_blocking_on_background_children() {
        let job = MacroCommand::default();
        job.spawn(shell("printf 'permission denied' >&2; exit 7".into(), None));
        let error = wait_job(&job).unwrap_err();
        assert!(error.contains("sh"));
        assert!(error.contains("7"));
        assert!(error.contains("permission denied"));

        let job = MacroCommand::default();
        job.spawn(shell("head -c 200000 /dev/zero >&2; exit 9".into(), None));
        let error = wait_job(&job).unwrap_err();
        assert!(error.contains("9"));
        assert!(error.contains("stderr truncated"));
        assert!(error.len() < CommandStderr::LIMIT + 200);

        let job = MacroCommand::default();
        let started = Instant::now();
        job.spawn(shell(
            "sleep 1 >&2 & printf 'failed' >&2; exit 3".into(),
            None,
        ));
        assert!(wait_job(&job).unwrap_err().contains("failed"));
        assert!(started.elapsed() < Duration::from_millis(700));
    }

    #[test]
    fn cancelling_macro_command_terminates_its_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let started = dir.path().join("started");
        let late = dir.path().join("late");
        let job = MacroCommand::default();
        job.spawn(shell(
            "printf started > started; (sleep 0.3; printf late > late) & wait".into(),
            Some(dir.path().to_string_lossy().into()),
        ));
        assert!(wait_for(&started));
        job.cancel();
        assert!(wait_job(&job).is_err());
        std::thread::sleep(Duration::from_millis(400));
        assert!(!late.exists(), "cancelled command left its child running");
    }

    #[test]
    fn cancelled_macro_command_cannot_start_a_process() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("marker");
        let job = MacroCommand::default();
        job.cancel();
        job.spawn(shell(
            "printf bad > marker".into(),
            Some(dir.path().to_string_lossy().into()),
        ));
        std::thread::sleep(Duration::from_millis(30));
        assert!(!marker.exists());
    }
}

#[cfg(test)]
mod portable_tests {
    use super::*;

    #[test]
    fn validates_working_directories_without_a_mapper_backend() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().to_str().unwrap();
        assert_eq!(working_directory(Some(existing)).unwrap(), dir.path());
        assert_eq!(draft_issue("echo hello", Some(existing)), None);
        let missing = dir.path().join("missing");
        assert_eq!(
            draft_issue("echo hello", missing.to_str()),
            Some(DraftIssue::MissingDirectory)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn each_os_runs_its_own_script() {
        let command = |linux: &str, windows: &str| super::super::config::Command {
            id: "c".into(),
            posix_script: linux.into(),
            powershell_script: windows.into(),
            working_directory: None,
        };
        let both = shell_command(&command(" ls ", " dir "), Some("~".into())).unwrap();
        assert_eq!(both.working_directory.as_deref(), Some("~"));
        if cfg!(target_os = "windows") {
            assert_eq!(both.program, "powershell.exe");
            assert_eq!(both.args.last().map(String::as_str), Some("dir"));
            assert!(shell_command(&command("ls", ""), None).is_none());
        } else {
            assert_eq!(both.args, ["-lc", "ls"]);
            assert!(shell_command(&command("", "dir"), None).is_none());
        }
    }

    #[test]
    fn paths_are_invoked_by_the_os_shell() {
        if cfg!(target_os = "windows") {
            assert_eq!(invoke_path(r"C:\it's\run.ps1"), r"& 'C:\it''s\run.ps1'");
        } else {
            assert_eq!(invoke_path("/it's"), "'/it'\\''s'");
        }
    }
}
