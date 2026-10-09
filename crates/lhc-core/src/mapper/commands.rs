//! Fire-and-forget shell commands: each one starts in its own process
//! group and is never waited on by the mapper. Only a failure to start
//! is reported; exit status and output are not collected.

use super::system::SysCommand;
use crate::events::{self, CoreEvent};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

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

/// Starts `command` and returns at once; a reaper thread collects the
/// exit status so finished commands do not linger as zombies.
pub(super) fn spawn(command: SysCommand) {
    let result = start(&command).map(|mut child| {
        let _ = std::thread::Builder::new()
            .name("lhc-command".into())
            .spawn(move || {
                let _ = child.wait();
            });
    });
    report(&command, result);
}

fn start(command: &SysCommand) -> Result<std::process::Child, String> {
    Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .current_dir(working_directory(command.working_directory.as_deref())?)
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
    use std::os::unix::fs::PermissionsExt;
    let directory = working_directory(directory).ok();
    if directory.is_none() {
        return Some(DraftIssue::MissingDirectory);
    }
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

/// `path` quoted for `sh`, so spaces and quotes survive.
pub fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn shell(script: String, directory: Option<String>) -> SysCommand {
        SysCommand {
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
