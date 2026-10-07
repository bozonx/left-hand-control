use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::process::Command;

use crate::exec::run_cmd_with_timeout;
use crate::mapper_config::GameModeSettings;
use crate::platform::linux::{Desktop, SessionType, command_available};

use super::{Detection, DetectorAvailability, GameModeDetectors};

const COMMAND_TIMEOUT_MS: u64 = 1000;

pub(super) fn detect(settings: &GameModeSettings) -> Detection {
    let session = crate::platform::linux::detect();
    let daemon = settings.use_gamemoded.then(gamemoded_active).flatten();
    let fullscreen = settings.use_fullscreen.then(fullscreen_active).flatten();
    let need_running = settings
        .process_matchers
        .iter()
        .any(|matcher| !matcher.only_active_window && !matcher.name.trim().is_empty());
    let running = need_running.then(running_process_names).flatten();
    let detectors = GameModeDetectors {
        gamemoded: availability(
            true,
            if settings.use_gamemoded {
                daemon.is_some()
            } else {
                command_available("gamemoded")
            },
        ),
        fullscreen: availability(
            fullscreen_supported(&session.desktop, session.session_type),
            if settings.use_fullscreen {
                fullscreen.is_some()
            } else {
                fullscreen_tools_available(&session.desktop, session.session_type)
            },
        ),
        processes: availability(
            true,
            if need_running {
                running.is_some()
            } else {
                std::fs::read_dir("/proc").is_ok()
            },
        ),
        active_window: crate::active_window::availability(),
    };
    super::evaluate(
        settings,
        detectors,
        running.as_deref().unwrap_or_default(),
        crate::active_window::cached_active_window().as_ref(),
        daemon.unwrap_or(false),
        fullscreen.unwrap_or(false),
    )
}

pub(crate) fn availability(supported: bool, available: bool) -> DetectorAvailability {
    if !supported {
        DetectorAvailability::Unsupported
    } else if available {
        DetectorAvailability::Available
    } else {
        DetectorAvailability::Unavailable
    }
}

fn gamemoded_active() -> Option<bool> {
    let output = run_cmd_with_timeout(
        Command::new("gamemoded").arg("-s").env("LC_ALL", "C"),
        COMMAND_TIMEOUT_MS,
    )?;
    if !output.status.success() {
        return None;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "gamemode is active" | "GameMode is active" => Some(true),
        "gamemode is inactive" | "GameMode is inactive" => Some(false),
        _ => None,
    }
}

fn running_process_names() -> Option<Vec<String>> {
    let mut names = HashSet::new();
    let uid = unsafe { libc::geteuid() };
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        if !entry.metadata().is_ok_and(|metadata| metadata.uid() == uid) {
            continue;
        }
        let path = entry.path();
        let comm = std::fs::read_to_string(path.join("comm")).unwrap_or_default();
        let comm = comm.trim().to_string();
        if !comm.is_empty() {
            names.insert(comm);
        }
        if let Ok(exe) = std::fs::read_link(path.join("exe"))
            && let Some(name) = exe.file_name().and_then(|name| name.to_str())
        {
            names.insert(name.into());
        }
        if let Ok(cmdline) = std::fs::read(path.join("cmdline"))
            && let Some(exe) = cmdline
                .split(|byte| *byte == 0)
                .next()
                .filter(|exe| !exe.is_empty())
            && let Some(name) = std::path::Path::new(&*String::from_utf8_lossy(exe))
                .file_name()
                .and_then(|name| name.to_str())
        {
            names.insert(name.into());
        }
    }
    Some(names.into_iter().collect())
}

fn fullscreen_supported(desktop: &Desktop, session: SessionType) -> bool {
    session == SessionType::X11
        || (session == SessionType::Wayland
            && matches!(desktop, Desktop::Kde | Desktop::Hyprland | Desktop::Sway))
}

fn fullscreen_tools_available(desktop: &Desktop, session: SessionType) -> bool {
    match (desktop, session) {
        (_, SessionType::X11) => command_available("xprop"),
        (Desktop::Kde, SessionType::Wayland) => command_available("kdotool"),
        (Desktop::Hyprland, SessionType::Wayland) => command_available("hyprctl"),
        (Desktop::Sway, SessionType::Wayland) => command_available("swaymsg"),
        _ => false,
    }
}

fn fullscreen_active() -> Option<bool> {
    let session = crate::platform::linux::detect();
    match (session.desktop, session.session_type) {
        (_, SessionType::X11) => x11_fullscreen_active(),
        (Desktop::Kde, SessionType::Wayland) => {
            let output = run_cmd_with_timeout(
                Command::new("kdotool").args(["kwinscript", "--inline", "var w=workspace.activeWindow;output_result(w && w.fullScreen ? 'true' : 'false');"]),
                COMMAND_TIMEOUT_MS,
            )?;
            if !output.status.success() {
                return None;
            }
            String::from_utf8_lossy(&output.stdout).trim().parse().ok()
        }
        (Desktop::Hyprland, SessionType::Wayland) => {
            let output = run_cmd_with_timeout(
                Command::new("hyprctl").args(["activewindow", "-j"]),
                COMMAND_TIMEOUT_MS,
            )?;
            output
                .status
                .success()
                .then(|| parse_hyprland_fullscreen(&String::from_utf8_lossy(&output.stdout)))
                .flatten()
        }
        (Desktop::Sway, SessionType::Wayland) => {
            let output = run_cmd_with_timeout(
                Command::new("swaymsg").args(["-t", "get_tree", "-r"]),
                COMMAND_TIMEOUT_MS,
            )?;
            if !output.status.success() {
                return None;
            }
            let tree: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
            Some(find_focused_fullscreen(&tree, false))
        }
        _ => None,
    }
}

fn x11_fullscreen_active() -> Option<bool> {
    let output = run_cmd_with_timeout(
        Command::new("xprop").args(["-root", "_NET_ACTIVE_WINDOW"]),
        COMMAND_TIMEOUT_MS,
    )?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout.split_whitespace().last()?;
    if id == "0x0" {
        return Some(false);
    }
    if !id.starts_with("0x") {
        return None;
    }
    let output = run_cmd_with_timeout(
        Command::new("xprop").args(["-id", id, "_NET_WM_STATE"]),
        COMMAND_TIMEOUT_MS,
    )?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).contains("_NET_WM_STATE_FULLSCREEN"))
}

fn parse_hyprland_fullscreen(stdout: &str) -> Option<bool> {
    let value: serde_json::Value = serde_json::from_str(stdout).ok()?;
    if value.as_object().is_some_and(|value| value.is_empty()) {
        return Some(false);
    }
    match value.get("fullscreen") {
        Some(serde_json::Value::Bool(value)) => Some(*value),
        Some(serde_json::Value::Number(value)) => value.as_u64().map(|value| value & 2 != 0),
        _ => None,
    }
}

fn find_focused_fullscreen(node: &serde_json::Value, ancestor_fullscreen: bool) -> bool {
    let fullscreen = ancestor_fullscreen
        || node
            .get("fullscreen_mode")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|mode| mode > 0)
        || node
            .get("fullscreen")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    if node.get("focused").and_then(serde_json::Value::as_bool) == Some(true) {
        return fullscreen;
    }
    ["nodes", "floating_nodes"].iter().any(|key| {
        node.get(key)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|nodes| {
                nodes
                    .iter()
                    .any(|node| find_focused_fullscreen(node, fullscreen))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_hyprland_inactive_from_failed_detection() {
        assert_eq!(
            parse_hyprland_fullscreen(r#"{"fullscreen":true}"#),
            Some(true)
        );
        assert_eq!(parse_hyprland_fullscreen(r#"{"fullscreen":2}"#), Some(true));
        assert_eq!(
            parse_hyprland_fullscreen(r#"{"fullscreen":0}"#),
            Some(false)
        );
        assert_eq!(
            parse_hyprland_fullscreen(r#"{"fullscreen":1}"#),
            Some(false)
        );
        assert_eq!(parse_hyprland_fullscreen(r#"{"fullscreen":3}"#), Some(true));
        assert_eq!(
            parse_hyprland_fullscreen(r#"{"fullscreen":0,"fullscreenClient":2}"#),
            Some(false)
        );
        assert_eq!(parse_hyprland_fullscreen("{}"), Some(false));
        assert_eq!(parse_hyprland_fullscreen("not json"), None);
    }

    #[test]
    fn sway_fullscreen_can_belong_to_a_focused_childs_parent() {
        let tree =
            serde_json::json!({"nodes": [{"fullscreen_mode": 1, "nodes": [{"focused": true}]}]});
        assert!(find_focused_fullscreen(&tree, false));
        let tree = serde_json::json!({"nodes": [{"fullscreen_mode": 1, "focused": false}, {"focused": true}]});
        assert!(!find_focused_fullscreen(&tree, false));
        let tree = serde_json::json!({"floating_nodes": [{"fullscreen_mode": 1, "focused": true}]});
        assert!(find_focused_fullscreen(&tree, false));
    }

    #[test]
    fn gnome_wayland_is_explicitly_unsupported() {
        assert!(!fullscreen_supported(&Desktop::Gnome, SessionType::Wayland));
        assert!(fullscreen_supported(&Desktop::Gnome, SessionType::X11));
    }
}
