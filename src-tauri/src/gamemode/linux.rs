use std::process::Command;

use crate::platform::linux::SessionType;

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
        SessionType::Wayland | SessionType::Tty | SessionType::Unknown => false,
    }
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
