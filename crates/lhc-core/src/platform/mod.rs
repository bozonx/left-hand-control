//! Platform + session detection layer.
//!
//! The long-term shape has three OS backends — `linux`, `windows`, `macos`.
//! In the current product we intentionally support only Linux, with KDE as
//! the only fully implemented desktop integration. The other OS/DE modules
//! stay in place as compile-safe stubs so the call sites remain explicit.
//!
//! Inside the Linux backend we further dispatch to per-desktop
//! implementations (KDE, GNOME, Sway, generic X11) for APIs that are
//! DE-specific:
//!
//!   * `layout::current()` / `layout::start_watcher()`   — which DBus / CLI tool
//!     tells us the active keyboard layout.
//!   * `mapper::system::resolve()`                       — how to implement
//!     "system functions" (virtual-desktop switching, etc.)
//!
//! The low-level key-event interception (evdev + uinput) is DE-agnostic,
//! so it lives directly in `mapper::linux` without further branching.
//!
//! Windows / macOS bindings are intentionally stubs for now — the module
//! structure is in place so the call sites compile and return a clear,
//! actionable error instead of panicking.

#[cfg(target_os = "linux")]
pub mod file_chooser;
#[cfg(not(target_os = "linux"))]
pub mod file_chooser {
    /// Native file selection is not implemented outside the Linux portal backend.
    pub fn pick(_title: &str, _directory: bool) -> Result<Option<std::path::PathBuf>, String> {
        Err("File selection is not implemented on this platform".into())
    }
}
#[cfg(target_os = "linux")]
pub mod kde_shortcuts;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod osd;
#[cfg(not(target_os = "linux"))]
pub mod osd {
    pub fn show(_text: &str) -> Result<(), String> {
        Err("on-screen display is not implemented on this platform".into())
    }
}

#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[cfg(target_os = "linux")]
use std::fs::{self, OpenOptions};
#[cfg(target_os = "linux")]
use std::io::ErrorKind;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Why the mapper cannot reach the devices it reads from or emits through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAccessIssue {
    /// No `/dev/input/event*` device exists.
    NoInputDevices,
    /// The selected keyboard device does not exist.
    KeyboardMissing,
    /// Input devices exist but cannot be opened for reading.
    InputDenied,
    /// `/dev/uinput` does not exist: the kernel module is not loaded.
    UinputMissing,
    /// `/dev/uinput` cannot be opened for reading and writing.
    UinputDenied,
}

/// Check the device access the mapper needs before it is started. With a
/// `keyboard` the check opens that device, otherwise any input device.
/// Empty on platforms without key interception.
pub fn input_access_issues(keyboard: Option<&str>) -> Vec<InputAccessIssue> {
    #[cfg(target_os = "linux")]
    {
        input_access_issues_at(
            Path::new("/dev/input"),
            Path::new("/dev/uinput"),
            keyboard.map(Path::new),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = keyboard;
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn input_access_issues_at(
    input_dir: &Path,
    uinput: &Path,
    keyboard: Option<&Path>,
) -> Vec<InputAccessIssue> {
    let mut issues = Vec::new();
    let input = match keyboard {
        Some(path) => OpenOptions::new()
            .read(true)
            .open(path)
            .map(|_| ())
            .map_err(|error| match error.kind() {
                ErrorKind::NotFound => InputAccessIssue::KeyboardMissing,
                _ => InputAccessIssue::InputDenied,
            }),
        None => {
            let events: Vec<_> = fs::read_dir(input_dir)
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("event"))
                })
                .collect();
            if events.is_empty() {
                Err(InputAccessIssue::NoInputDevices)
            } else if events
                .iter()
                .any(|path| OpenOptions::new().read(true).open(path).is_ok())
            {
                Ok(())
            } else {
                Err(InputAccessIssue::InputDenied)
            }
        }
    };
    issues.extend(input.err());
    if let Err(error) = OpenOptions::new().read(true).write(true).open(uinput) {
        issues.push(match error.kind() {
            ErrorKind::NotFound => InputAccessIssue::UinputMissing,
            _ => InputAccessIssue::UinputDenied,
        });
    }
    issues
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{InputAccessIssue, input_access_issues_at};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lhc-platform-{prefix}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn reports_missing_devices() {
        let temp = TempDir::new("probe-empty");
        let issues = input_access_issues_at(&temp.path, &temp.path.join("uinput"), None);
        assert_eq!(
            issues,
            [
                InputAccessIssue::NoInputDevices,
                InputAccessIssue::UinputMissing
            ]
        );
    }

    #[test]
    fn readable_devices_have_no_issues() {
        let temp = TempDir::new("probe-ok");
        fs::write(temp.path.join("event0"), "").expect("event");
        fs::write(temp.path.join("uinput"), "").expect("uinput");
        let uinput = temp.path.join("uinput");
        assert!(input_access_issues_at(&temp.path, &uinput, None).is_empty());
        let keyboard = temp.path.join("event0");
        assert!(input_access_issues_at(&temp.path, &uinput, Some(&keyboard)).is_empty());
    }

    #[test]
    fn reports_a_missing_keyboard() {
        let temp = TempDir::new("probe-keyboard");
        fs::write(temp.path.join("uinput"), "").expect("uinput");
        let issues = input_access_issues_at(
            &temp.path,
            &temp.path.join("uinput"),
            Some(&temp.path.join("event7")),
        );
        assert_eq!(issues, [InputAccessIssue::KeyboardMissing]);
    }
}

#[cfg(target_os = "macos")]
pub use macos::{
    NativeTextTarget, capture_text_target, restore_text_target, text_modifiers_released,
};
