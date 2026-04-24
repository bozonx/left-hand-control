use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::platform::linux::SessionType;

pub static KDOTOOL_WARN_ONCE: AtomicBool = AtomicBool::new(false);

pub fn is_gamemoded_active() -> bool {
    Command::new("gamemoded")
        .arg("-s")
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains("is active")
        })
        .unwrap_or(false)
}

pub fn is_fullscreen_active() -> bool {
    let session = crate::platform::linux::detect();
    match session.session_type {
        SessionType::X11 => is_x11_fullscreen_active(),
        SessionType::Wayland => is_wayland_fullscreen_active(),
        SessionType::Tty | SessionType::Unknown => false,
    }
}

fn is_wayland_fullscreen_active() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    let session = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_default().to_lowercase();

    if desktop.contains("kde") || session.contains("kde") {
        return is_kde_wayland_fullscreen_active();
    } else if desktop.contains("hyprland") || session.contains("hyprland") {
        return is_hyprland_fullscreen_active();
    } else if desktop.contains("gnome") || session.contains("gnome") {
        return is_gnome_wayland_fullscreen_active();
    }

    false
}

fn is_kde_wayland_fullscreen_active() -> bool {
    let output = Command::new("kdotool")
        .arg("getactivewindow")
        .output();

    let Ok(output) = output else {
        if !KDOTOOL_WARN_ONCE.swap(true, Ordering::SeqCst) {
            eprintln!("[gamemode] Для определения полноэкранного режима в KDE Wayland требуется 'kdotool'. Пожалуйста, установите его (например: paru -S kdotool).");
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

    let geom_output = Command::new("kdotool")
        .args(["getwindowgeometry", &window_id])
        .output()
        .ok();

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
    let output = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output();

    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("\"fullscreen\": true") || stdout.contains("\"fullscreen\": 1") || stdout.contains("\"fullscreen\": 2")
}

fn is_gnome_wayland_fullscreen_active() -> bool {
    // GNOME DBus APIs for getting active window geometry or fullscreen status
    // are locked down without an extension. We return false for now.
    false
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
    let output = Command::new("kscreen-doctor")
        .arg("-o")
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Geometry:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let dims: Vec<&str> = parts[2].split('x').collect();
                if dims.len() == 2 {
                    let w: i32 = dims[0].parse().unwrap_or(0);
                    let h: i32 = dims[1].parse().unwrap_or(0);
                    if w > 0 && h > 0 {
                        return Some((w, h));
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

    let Some((window_x, window_y, window_width, window_height)) = active_window_geometry(&window_id)
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
    let output = Command::new("xdotool")
        .arg("getactivewindow")
        .output()
        .ok()?;
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
    let output = Command::new("xprop")
        .args(["-id", window_id, "_NET_WM_STATE"])
        .output();

    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    String::from_utf8_lossy(&output.stdout).contains("_NET_WM_STATE_FULLSCREEN")
}

fn active_window_geometry(window_id: &str) -> Option<(i32, i32, i32, i32)> {
    let output = Command::new("xdotool")
        .args(["getwindowgeometry", "--shell", window_id])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_window_geometry(&String::from_utf8_lossy(&output.stdout))
}

fn display_geometry() -> Option<(i32, i32)> {
    let output = Command::new("xdotool")
        .arg("getdisplaygeometry")
        .output()
        .ok()?;
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
    let mut parts = stdout.split_whitespace();
    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::{parse_display_geometry, parse_window_geometry};

    #[test]
    fn parses_window_geometry() {
        let stdout = "WINDOW=123\nX=0\nY=0\nWIDTH=1920\nHEIGHT=1080\nSCREEN=0\n";
        assert_eq!(parse_window_geometry(stdout), Some((0, 0, 1920, 1080)));
    }

    #[test]
    fn parses_display_geometry() {
        assert_eq!(parse_display_geometry("1920 1080\n"), Some((1920, 1080)));
    }
}
