// Linux-side active window detection.
//
// Dispatches by detected desktop / session type:
//   * KDE Wayland     -> kdotool
//   * Hyprland        -> hyprctl activewindow -j
//   * X11 (any DE)    -> xdotool + xprop
//   * everything else -> None (condition will not match)

use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::platform::linux::{Desktop, SessionType};

use super::ActiveWindow;

static KDOTOOL_WARN_ONCE: AtomicBool = AtomicBool::new(false);
const COMMAND_TIMEOUT_MS: u64 = 3000;

pub fn detect() -> Option<ActiveWindow> {
    let session = crate::platform::linux::detect();

    match (session.desktop.clone(), session.session_type) {
        (Desktop::Hyprland, _) => detect_hyprland(),
        (Desktop::Kde, SessionType::Wayland) => detect_kde_wayland(),
        (_, SessionType::X11) => detect_x11(),
        _ => None,
    }
}

fn detect_hyprland() -> Option<ActiveWindow> {
    let output = run_cmd_with_timeout(Command::new("hyprctl").args(["activewindow", "-j"]))?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_hyprctl_json(&stdout)
}

fn detect_kde_wayland() -> Option<ActiveWindow> {
    let id_output = run_cmd_with_timeout(Command::new("kdotool").arg("getactivewindow"));
    let Some(id_output) = id_output else {
        if !KDOTOOL_WARN_ONCE.swap(true, Ordering::SeqCst) {
            log::debug!(
                "[active-window] Для определения активного окна в KDE Wayland требуется 'kdotool'. Установите его (например: paru -S kdotool)."
            );
        }
        return None;
    };
    if !id_output.status.success() {
        return None;
    }
    let window_id = String::from_utf8_lossy(&id_output.stdout)
        .trim()
        .to_string();
    if window_id.is_empty() {
        return None;
    }

    let title = run_cmd_with_timeout(Command::new("kdotool").args(["getwindowname", &window_id]))
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let app_id =
        run_cmd_with_timeout(Command::new("kdotool").args(["getwindowclassname", &window_id]))
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();

    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow { title, app_id })
}

fn detect_x11() -> Option<ActiveWindow> {
    let id_output = run_cmd_with_timeout(Command::new("xdotool").arg("getactivewindow"))?;
    if !id_output.status.success() {
        return None;
    }
    let window_id = String::from_utf8_lossy(&id_output.stdout)
        .trim()
        .to_string();
    if window_id.is_empty() {
        return None;
    }

    let title = run_cmd_with_timeout(Command::new("xdotool").args(["getwindowname", &window_id]))
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let class_output =
        run_cmd_with_timeout(Command::new("xprop").args(["-id", &window_id, "WM_CLASS"]));
    let app_id = class_output
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).into_owned())
            } else {
                None
            }
        })
        .map(|s| parse_wm_class(&s))
        .unwrap_or_default();

    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow { title, app_id })
}

fn run_cmd_with_timeout(cmd: &mut Command) -> Option<Output> {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let pid = child.id() as libc::pid_t;
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(COMMAND_TIMEOUT_MS));
        if !done_clone.load(Ordering::SeqCst) {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    });

    let output = child.wait_with_output().ok();
    done.store(true, Ordering::SeqCst);
    output
}

// Parses `xprop WM_CLASS` output of the form:
//   WM_CLASS(STRING) = "instance", "Class"
// Returns the class (second value) when present, otherwise the instance,
// otherwise empty.
pub(crate) fn parse_wm_class(stdout: &str) -> String {
    let line = stdout
        .lines()
        .find(|l| l.contains("WM_CLASS"))
        .unwrap_or("");
    let Some((_, rhs)) = line.split_once('=') else {
        return String::new();
    };
    let mut parts: Vec<String> = rhs
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() >= 2 {
        parts.remove(1)
    } else if !parts.is_empty() {
        parts.remove(0)
    } else {
        String::new()
    }
}

pub(crate) fn parse_hyprctl_json(stdout: &str) -> Option<ActiveWindow> {
    #[derive(serde::Deserialize)]
    struct HyprWindow {
        #[serde(default)]
        title: String,
        #[serde(default, rename = "class")]
        app_id: String,
    }

    let parsed: HyprWindow = serde_json::from_str(stdout).ok()?;
    let title = parsed.title;
    let app_id = parsed.app_id;
    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow { title, app_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wm_class_class_value() {
        let s = "WM_CLASS(STRING) = \"firefox\", \"firefox\"\n";
        assert_eq!(parse_wm_class(s), "firefox");
    }

    #[test]
    fn parses_wm_class_with_distinct_values() {
        let s = "WM_CLASS(STRING) = \"navigator\", \"Firefox\"\n";
        assert_eq!(parse_wm_class(s), "Firefox");
    }

    #[test]
    fn parses_wm_class_single_value() {
        let s = "WM_CLASS(STRING) = \"only\"\n";
        assert_eq!(parse_wm_class(s), "only");
    }

    #[test]
    fn parses_hyprctl_json_basic() {
        let s =
            r#"{"address":"0x1","title":"My Doc - Editor","class":"editor","fullscreen":false}"#;
        let aw = parse_hyprctl_json(s).unwrap();
        assert_eq!(aw.title, "My Doc - Editor");
        assert_eq!(aw.app_id, "editor");
    }

    #[test]
    fn parses_hyprctl_json_missing_fields_returns_none() {
        assert!(parse_hyprctl_json("{}").is_none());
    }

    #[test]
    fn parses_hyprctl_json_escaped_unicode() {
        let s = r#"{"title":"\u0422\u0435\u0441\u0442","class":"kitty"}"#;
        let aw = parse_hyprctl_json(s).unwrap();
        assert_eq!(aw.title, "Тест");
        assert_eq!(aw.app_id, "kitty");
    }
}
