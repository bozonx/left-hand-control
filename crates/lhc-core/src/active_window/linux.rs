// Linux-side active window detection.
//
// Dispatches by detected desktop / session type:
//   * KDE Wayland     -> kdotool
//   * Hyprland        -> hyprctl activewindow -j
//   * X11 (any DE)    -> xdotool + xprop
//   * everything else -> None (condition will not match)

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::exec::run_cmd_with_timeout;
use crate::platform::linux::{Desktop, SessionType, command_available};

use super::ActiveWindow;

static KDOTOOL_WARN_ONCE: AtomicBool = AtomicBool::new(false);
const COMMAND_TIMEOUT_MS: u64 = 1000;
static LAST_QUERY: AtomicU8 = AtomicU8::new(0);

pub(super) fn availability() -> crate::gamemode::DetectorAvailability {
    use crate::gamemode::DetectorAvailability;
    let session = crate::platform::linux::detect();
    let available = match (session.desktop, session.session_type) {
        (_, SessionType::X11) => command_available("xdotool") && command_available("xprop"),
        (Desktop::Kde, SessionType::Wayland) => command_available("kdotool"),
        (Desktop::Hyprland, SessionType::Wayland) => command_available("hyprctl"),
        (Desktop::Sway, SessionType::Wayland) => command_available("swaymsg"),
        _ => return DetectorAvailability::Unsupported,
    };
    if available && LAST_QUERY.load(Ordering::Relaxed) != 2 {
        DetectorAvailability::Available
    } else {
        DetectorAvailability::Unavailable
    }
}

pub fn detect() -> Option<ActiveWindow> {
    let session = crate::platform::linux::detect();

    let result = match (session.desktop.clone(), session.session_type) {
        (Desktop::Hyprland, SessionType::Wayland) => detect_hyprland(),
        (Desktop::Kde, SessionType::Wayland) => detect_kde_wayland(),
        (Desktop::Sway, SessionType::Wayland) => detect_sway(),
        (_, SessionType::X11) => detect_x11(),
        _ => Err(()),
    };
    LAST_QUERY.store(if result.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
    result.ok().flatten()
}

fn detect_hyprland() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("hyprctl").args(["activewindow", "-j"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<serde_json::Value>(&stdout).map_err(|_| ())?;
    Ok(parse_hyprctl_json(&stdout))
}

fn detect_kde_wayland() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("kdotool").args([
            "kwinscript", "--inline",
            "var w=workspace.activeWindow;output_result(w ? JSON.stringify({title:w.caption,appId:String(w.resourceClass),pid:w.pid}) : '{}');",
        ]),
        COMMAND_TIMEOUT_MS,
    );
    let Some(output) = output else {
        if !KDOTOOL_WARN_ONCE.swap(true, Ordering::SeqCst) {
            log::debug!("[active-window] KDE Wayland active-window detection requires 'kdotool'");
        }
        return Err(());
    };
    if !output.status.success() {
        return Err(());
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|_| ())?;
    Ok(window_from_json(&value, "title", "appId"))
}

fn detect_sway() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("swaymsg").args(["-t", "get_tree", "-r"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|_| ())?;
    Ok(find_focused_window(&tree))
}

fn find_focused_window(node: &serde_json::Value) -> Option<ActiveWindow> {
    if node.get("focused").and_then(serde_json::Value::as_bool) == Some(true) {
        let title = node
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let app_id = node
            .get("app_id")
            .and_then(serde_json::Value::as_str)
            .or_else(|| node.get("window_properties")?.get("class")?.as_str())
            .unwrap_or_default()
            .to_string();
        if !title.is_empty() || !app_id.is_empty() {
            return Some(ActiveWindow {
                title,
                app_id,
                process_name: process_name(node.get("pid").and_then(serde_json::Value::as_u64)),
            });
        }
    }
    ["nodes", "floating_nodes"].iter().find_map(|key| {
        node.get(key)?
            .as_array()?
            .iter()
            .find_map(find_focused_window)
    })
}

fn process_name(pid: Option<u64>) -> Option<String> {
    let pid = pid.filter(|pid| *pid > 0)?;
    if let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe")) {
        return exe.file_name()?.to_str().map(str::to_owned);
    }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn window_from_json(
    value: &serde_json::Value,
    title_key: &str,
    app_key: &str,
) -> Option<ActiveWindow> {
    let title = value
        .get(title_key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let app_id = value
        .get(app_key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow {
        title,
        app_id,
        process_name: process_name(value.get("pid").and_then(serde_json::Value::as_u64)),
    })
}

fn detect_x11() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("xprop").args(["-root", "_NET_ACTIVE_WINDOW"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let window_id = stdout.split_whitespace().last().ok_or(())?;
    if window_id == "0x0" {
        return Ok(None);
    }
    if !window_id.starts_with("0x") {
        return Err(());
    }
    let title_output = run_cmd_with_timeout(
        Command::new("xdotool").args(["getwindowname", window_id]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    let class_output = run_cmd_with_timeout(
        Command::new("xprop").args(["-id", window_id, "WM_CLASS", "_NET_WM_PID"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !title_output.status.success() || !class_output.status.success() {
        return Err(());
    }
    let title = String::from_utf8_lossy(&title_output.stdout)
        .trim()
        .to_string();
    let stdout = String::from_utf8_lossy(&class_output.stdout);
    let app_id = parse_wm_class(&stdout);
    let pid = stdout
        .lines()
        .find(|line| line.starts_with("_NET_WM_PID"))
        .and_then(|line| line.split_once('='))
        .and_then(|(_, value)| value.trim().parse().ok());
    let process_name = process_name(pid);
    if title.is_empty() && app_id.is_empty() && process_name.is_none() {
        return Ok(None);
    }
    Ok(Some(ActiveWindow {
        title,
        app_id,
        process_name,
    }))
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
    let value: serde_json::Value = serde_json::from_str(stdout).ok()?;
    window_from_json(&value, "title", "class")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sway_finds_native_and_xwayland_focused_windows() {
        let native = serde_json::json!({"nodes": [{"nodes": [{"focused": true, "name": "Game", "app_id": "game"}]}]});
        assert_eq!(find_focused_window(&native).unwrap().app_id, "game");
        let xwayland = serde_json::json!({"floating_nodes": [{"focused": true, "name": "Game", "app_id": null, "window_properties": {"class": "Game.exe"}}]});
        assert_eq!(find_focused_window(&xwayland).unwrap().app_id, "Game.exe");
        assert!(find_focused_window(&serde_json::json!({"focused": true, "name": null})).is_none());
    }

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
