//! Fire-and-forget shell commands: each one starts in its own process
//! group and is never waited on by the mapper. Only a failure to start
//! is reported; exit status and output are not collected. Linux runs the
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
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
