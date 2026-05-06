use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::mapper::config::{GameModeProcessMatchMode, GameModeProcessMatcher};
use crate::platform::linux::{Desktop, Session, SessionType};

pub static KDOTOOL_WARN_ONCE: AtomicBool = AtomicBool::new(false);

/// Run a command with a timeout. The timeout thread sends SIGKILL to the child
/// process if it has not yet exited. The `done` flag prevents a kill after the
/// process has already been reaped and its PID potentially recycled.
fn run_cmd_with_timeout(cmd: &mut Command, timeout_ms: u64) -> Option<std::process::Output> {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let pid = child.id() as libc::pid_t;
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(timeout_ms));
        // Only kill if wait_with_output hasn't returned yet. While
        // wait_with_output is blocked the process is still alive (or a
        // zombie whose PID cannot be recycled), so the kill is safe.
        if !done_clone.load(Ordering::SeqCst) {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    });

    let output = child.wait_with_output().ok();
    done.store(true, Ordering::SeqCst);
    output
}

pub fn is_gamemoded_active() -> bool {
    run_cmd_with_timeout(Command::new("gamemoded").arg("-s"), 3000)
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains("is active")
        })
        .unwrap_or(false)
}

pub fn active_process_match(matchers: &[GameModeProcessMatcher]) -> Option<String> {
    let need_running = matchers.iter().any(|m| !m.only_active_window);
    let running_names = if need_running {
        Some(running_process_names())
    } else {
        None
    };

    for matcher in matchers {
        let name = matcher.name.trim();
        if name.is_empty() {
            continue;
        }
        if matcher.only_active_window {
            if active_window_matches(matcher) {
                return Some(name.to_string());
            }
        } else if let Some(ref names) = running_names {
            let needle = name.to_lowercase();
            if needle.is_empty() {
                continue;
            }
            let matched = match matcher.match_mode {
                GameModeProcessMatchMode::Exact => names.contains(&needle),
                GameModeProcessMatchMode::Substring => names.iter().any(|n| n.contains(&needle)),
            };
            if matched {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn active_window_matches(matcher: &GameModeProcessMatcher) -> bool {
    let Some(window) = crate::active_window::cached_active_window() else {
        return false;
    };
    [window.app_id.as_str(), window.title.as_str()]
        .iter()
        .any(|candidate| process_name_matches(matcher, candidate))
}

fn running_process_names() -> HashSet<String> {
    let mut names = HashSet::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return names;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if !file_name.as_bytes().iter().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let path = entry.path();
        let comm = std::fs::read_to_string(path.join("comm")).unwrap_or_default();
        let comm = comm.trim().to_lowercase();
        if !comm.is_empty() {
            names.insert(comm);
        }
        // Only add the basename of the executable (first cmdline entry), not
        // flags or paths from other arguments — those cause false positives.
        if let Ok(cmdline) = std::fs::read(path.join("cmdline")) {
            if let Some(exe) = cmdline
                .split(|b| *b == 0)
                .next()
                .filter(|p| !p.is_empty())
                .and_then(|p| {
                    std::path::Path::new(&*String::from_utf8_lossy(p))
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.trim().to_lowercase())
                })
                .filter(|s| !s.is_empty())
            {
                names.insert(exe);
            }
        }
    }
    names
}

fn process_name_matches(matcher: &GameModeProcessMatcher, candidate: &str) -> bool {
    let needle = matcher.name.trim().to_lowercase();
    let haystack = candidate.trim().to_lowercase();
    if needle.is_empty() || haystack.is_empty() {
        return false;
    }
    match matcher.match_mode {
        GameModeProcessMatchMode::Exact => haystack == needle,
        GameModeProcessMatchMode::Substring => haystack.contains(&needle),
    }
}

pub fn is_fullscreen_active() -> bool {
    let session = crate::platform::linux::detect();
    match session.session_type {
        SessionType::X11 => is_x11_fullscreen_active(),
        SessionType::Wayland => is_wayland_fullscreen_active(&session),
        SessionType::Tty | SessionType::Unknown => false,
    }
}

fn is_wayland_fullscreen_active(session: &Session) -> bool {
    match session.desktop {
        Desktop::Kde => is_kde_wayland_fullscreen_active(),
        Desktop::Hyprland => is_hyprland_fullscreen_active(),
        Desktop::Gnome => is_gnome_wayland_fullscreen_active(),
        Desktop::Sway => is_sway_fullscreen_active(),
        _ => false,
    }
}

fn is_kde_wayland_fullscreen_active() -> bool {
    let output = run_cmd_with_timeout(Command::new("kdotool").arg("getactivewindow"), 3000);

    let Some(output) = output else {
        if !KDOTOOL_WARN_ONCE.swap(true, Ordering::SeqCst) {
            eprintln!("[gamemode] KDE Wayland fullscreen detection requires 'kdotool'");
        }
        return false;
    };

    if !output.status.success() {
        return false;
    }

    let window_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if window_id.is_empty() {
        return false;
    }

    let geom_output = run_cmd_with_timeout(
        Command::new("kdotool").args(["getwindowgeometry", &window_id]),
        3000,
    );

    let Some(geom_output) = geom_output else {
        return false;
    };

    let Some((window_x, window_y, window_width, window_height)) =
        parse_kdotool_geometry(&String::from_utf8_lossy(&geom_output.stdout))
    else {
        return false;
    };

    let Some((display_width, display_height)) = kde_display_geometry() else {
        return false;
    };

    window_x == 0
        && window_y == 0
        && window_width >= display_width
        && window_height >= display_height
}

fn is_hyprland_fullscreen_active() -> bool {
    let output = run_cmd_with_timeout(Command::new("hyprctl").args(["activewindow", "-j"]), 3000);

    let Some(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    parse_hyprland_fullscreen(&String::from_utf8_lossy(&output.stdout))
}

fn is_gnome_wayland_fullscreen_active() -> bool {
    // GNOME DBus APIs for getting active window geometry or fullscreen status
    // are locked down without an extension. We return false for now.
    false
}

fn is_sway_fullscreen_active() -> bool {
    let output = run_cmd_with_timeout(Command::new("swaymsg").args(["-t", "get_tree", "-r"]), 3000);

    let Some(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    parse_sway_fullscreen(&String::from_utf8_lossy(&output.stdout))
}

fn parse_kdotool_geometry(stdout: &str) -> Option<(i32, i32, i32, i32)> {
    let mut x = 0;
    let mut y = 0;
    let mut w = 0;
    let mut h = 0;
    let mut has_pos = false;
    let mut has_geom = false;

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Position:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let coords: Vec<&str> = parts[1].split(',').collect();
                if coords.len() == 2 {
                    x = coords[0].parse().unwrap_or(0);
                    y = coords[1].parse().unwrap_or(0);
                    has_pos = true;
                }
            }
        } else if line.starts_with("Geometry:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let dims: Vec<&str> = parts[1].split('x').collect();
                if dims.len() == 2 {
                    w = dims[0].parse().unwrap_or(0);
                    h = dims[1].parse().unwrap_or(0);
                    has_geom = true;
                }
            }
        }
    }

    if has_pos && has_geom && w > 0 && h > 0 {
        Some((x, y, w, h))
    } else {
        None
    }
}

fn kde_display_geometry() -> Option<(i32, i32)> {
    let output = run_cmd_with_timeout(Command::new("kscreen-doctor").arg("-o"), 3000)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some((_, rhs)) = line.split_once("Geometry:") {
            for geometry in rhs.split_whitespace() {
                if geometry.contains('x') {
                    if let Some(parsed) = parse_display_geometry(geometry) {
                        return Some(parsed);
                    }
                }
            }
        }
    }
    None
}

fn is_x11_fullscreen_active() -> bool {
    let Some(window_id) = active_window_id() else {
        return false;
    };

    if active_window_has_fullscreen_state(&window_id) {
        return true;
    }

    let Some((window_x, window_y, window_width, window_height)) =
        active_window_geometry(&window_id)
    else {
        return false;
    };
    let Some((display_width, display_height)) = display_geometry() else {
        return false;
    };

    window_x == 0
        && window_y == 0
        && window_width >= display_width
        && window_height >= display_height
}

fn active_window_id() -> Option<String> {
    let output = run_cmd_with_timeout(Command::new("xdotool").arg("getactivewindow"), 3000)?;
    if !output.status.success() {
        return None;
    }
    let window_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if window_id.is_empty() {
        return None;
    }
    Some(window_id)
}

fn active_window_has_fullscreen_state(window_id: &str) -> bool {
    let output = run_cmd_with_timeout(
        Command::new("xprop").args(["-id", window_id, "_NET_WM_STATE"]),
        3000,
    );

    let Some(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    String::from_utf8_lossy(&output.stdout).contains("_NET_WM_STATE_FULLSCREEN")
}

fn active_window_geometry(window_id: &str) -> Option<(i32, i32, i32, i32)> {
    let output = run_cmd_with_timeout(
        Command::new("xdotool").args(["getwindowgeometry", "--shell", window_id]),
        3000,
    )?;
    if !output.status.success() {
        return None;
    }
    parse_window_geometry(&String::from_utf8_lossy(&output.stdout))
}

fn display_geometry() -> Option<(i32, i32)> {
    let output = run_cmd_with_timeout(Command::new("xdotool").arg("getdisplaygeometry"), 3000)?;
    if !output.status.success() {
        return None;
    }
    parse_display_geometry(&String::from_utf8_lossy(&output.stdout))
}

fn parse_window_geometry(stdout: &str) -> Option<(i32, i32, i32, i32)> {
    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;

    for line in stdout.lines() {
        let (key, value) = line.split_once('=')?;
        match key {
            "X" => x = value.parse().ok(),
            "Y" => y = value.parse().ok(),
            "WIDTH" => width = value.parse().ok(),
            "HEIGHT" => height = value.parse().ok(),
            _ => {}
        }
    }

    Some((x?, y?, width?, height?))
}

fn parse_display_geometry(stdout: &str) -> Option<(i32, i32)> {
    let mut tokens = stdout.split_whitespace();
    let first = tokens.next()?;
    if first.contains('x') {
        let size = first.split('+').next()?;
        let mut parts = size.split('x');
        let width = parts.next()?.parse().ok()?;
        let height = parts.next()?.parse().ok()?;
        return Some((width, height));
    }
    let width = first.parse().ok()?;
    let height = tokens.next()?.parse().ok()?;
    Some((width, height))
}

fn parse_hyprland_fullscreen(stdout: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout) else {
        return false;
    };
    match value.get("fullscreen") {
        Some(serde_json::Value::Bool(v)) => *v,
        Some(serde_json::Value::Number(v)) => v.as_i64().is_some_and(|n| n > 0),
        _ => false,
    }
}

fn parse_sway_fullscreen(stdout: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout) else {
        return false;
    };
    find_focused_fullscreen(&value)
}

fn find_focused_fullscreen(value: &serde_json::Value) -> bool {
    if let Some(node) = value.as_object() {
        if node.get("focused").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(mode) = node.get("fullscreen_mode").and_then(|v| v.as_i64()) {
                return mode > 0;
            }
            if let Some(full) = node.get("fullscreen").and_then(|v| v.as_bool()) {
                return full;
            }
            return false;
        }
        for key in ["nodes", "floating_nodes"] {
            if let Some(children) = node.get(key).and_then(|v| v.as_array()) {
                for child in children {
                    if find_focused_fullscreen(child) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        find_focused_fullscreen, parse_display_geometry, parse_hyprland_fullscreen,
        parse_window_geometry,
    };

    #[test]
    fn parses_window_geometry() {
        let stdout = "WINDOW=123\nX=0\nY=0\nWIDTH=1920\nHEIGHT=1080\nSCREEN=0\n";
        assert_eq!(parse_window_geometry(stdout), Some((0, 0, 1920, 1080)));
    }

    #[test]
    fn parses_display_geometry() {
        assert_eq!(parse_display_geometry("1920 1080\n"), Some((1920, 1080)));
    }

    #[test]
    fn parses_display_geometry_from_kscreen_style_token() {
        assert_eq!(parse_display_geometry("1920x1080+0+0"), Some((1920, 1080)));
    }

    #[test]
    fn parses_hyprland_fullscreen_boolean() {
        assert!(parse_hyprland_fullscreen(r#"{"fullscreen":true}"#));
        assert!(!parse_hyprland_fullscreen(r#"{"fullscreen":false}"#));
    }

    #[test]
    fn parses_hyprland_fullscreen_number() {
        assert!(parse_hyprland_fullscreen(r#"{"fullscreen":2}"#));
        assert!(!parse_hyprland_fullscreen(r#"{"fullscreen":0}"#));
    }

    #[test]
    fn parses_sway_fullscreen_boolean() {
        let tree: serde_json::Value =
            serde_json::from_str(r#"{"focused":true,"fullscreen_mode":1}"#).unwrap();
        assert!(find_focused_fullscreen(&tree));
    }

    #[test]
    fn parses_sway_fullscreen_nested() {
        let tree: serde_json::Value = serde_json::from_str(
            r#"{"nodes":[{"focused":false,"nodes":[{"focused":true,"fullscreen":true}]}]}"#,
        )
        .unwrap();
        assert!(find_focused_fullscreen(&tree));
    }

    #[test]
    fn ignores_sway_unfocused_fullscreen() {
        let tree: serde_json::Value =
            serde_json::from_str(r#"{"nodes":[{"focused":false,"fullscreen_mode":1}]}"#).unwrap();
        assert!(!find_focused_fullscreen(&tree));
    }
}
