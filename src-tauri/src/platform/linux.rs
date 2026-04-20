//! Linux session detection: desktop environment, display server, IPC sockets.
//!
//! Runs entirely from the current process environment plus a couple of
//! cheap probes. No DBus round-trips here — each backend does its own
//! introspection beyond this layer.
//!
//! Used by:
//!   * `layout` — to pick the right layout-detection implementation.
//!   * `mapper::system` — to pick the right "system function" resolver.
//!   * `platform::info` — for UI diagnostics.

#![cfg(target_os = "linux")]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Wayland,
    X11,
    Tty,
    Unknown,
}

impl SessionType {
    pub fn label(&self) -> &'static str {
        match self {
            SessionType::Wayland => "wayland",
            SessionType::X11 => "x11",
            SessionType::Tty => "tty",
            SessionType::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desktop {
    Kde,
    Gnome,
    Sway,
    Hyprland,
    Xfce,
    Cinnamon,
    Mate,
    Lxqt,
    Unity,
    Pantheon,
    Unknown,
}

impl Desktop {
    pub fn label(&self) -> &'static str {
        match self {
            Desktop::Kde => "KDE Plasma",
            Desktop::Gnome => "GNOME",
            Desktop::Sway => "Sway",
            Desktop::Hyprland => "Hyprland",
            Desktop::Xfce => "Xfce",
            Desktop::Cinnamon => "Cinnamon",
            Desktop::Mate => "MATE",
            Desktop::Lxqt => "LXQt",
            Desktop::Unity => "Unity",
            Desktop::Pantheon => "Pantheon",
            Desktop::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub desktop: Desktop,
    pub session_type: SessionType,
    /// Raw `$XDG_CURRENT_DESKTOP` (may contain multiple `:`-separated
    /// tokens, e.g. `ubuntu:GNOME`).
    pub xdg_current_desktop: String,
    pub desktop_session: String,
    pub wayland_display: Option<String>,
    pub x11_display: Option<String>,
    pub sway_sock: Option<String>,
}

pub fn detect() -> Session {
    let xdg = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("DESKTOP_SESSION").unwrap_or_default();
    let sess_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok().filter(|s| !s.is_empty());
    let x11 = std::env::var("DISPLAY").ok().filter(|s| !s.is_empty());
    let sway_sock = std::env::var("SWAYSOCK").ok().filter(|s| !s.is_empty());

    let desktop = classify_desktop(&xdg, &session, sway_sock.is_some());

    let session_type = match sess_type.to_ascii_lowercase().as_str() {
        "wayland" => SessionType::Wayland,
        "x11" => SessionType::X11,
        "tty" => SessionType::Tty,
        _ => {
            if wayland.is_some() {
                SessionType::Wayland
            } else if x11.is_some() {
                SessionType::X11
            } else {
                SessionType::Unknown
            }
        }
    };

    Session {
        desktop,
        session_type,
        xdg_current_desktop: xdg,
        desktop_session: session,
        wayland_display: wayland,
        x11_display: x11,
        sway_sock,
    }
}

fn classify_desktop(xdg: &str, session: &str, has_swaysock: bool) -> Desktop {
    let haystacks = [xdg, session];
    let contains = |needle: &str| -> bool {
        haystacks.iter().any(|h| {
            h.split(':')
                .any(|tok| tok.trim().eq_ignore_ascii_case(needle))
        })
    };
    if contains("KDE") {
        return Desktop::Kde;
    }
    if contains("GNOME") || contains("GNOME-Classic") || contains("GNOME-Flashback") {
        return Desktop::Gnome;
    }
    if contains("sway") || has_swaysock {
        return Desktop::Sway;
    }
    if contains("Hyprland") {
        return Desktop::Hyprland;
    }
    if contains("XFCE") {
        return Desktop::Xfce;
    }
    if contains("X-Cinnamon") || contains("Cinnamon") {
        return Desktop::Cinnamon;
    }
    if contains("MATE") {
        return Desktop::Mate;
    }
    if contains("LXQt") {
        return Desktop::Lxqt;
    }
    if contains("Unity") {
        return Desktop::Unity;
    }
    if contains("Pantheon") {
        return Desktop::Pantheon;
    }
    // Fallback: KDE sometimes only sets KDE_FULL_SESSION (old scripts).
    if std::env::var("KDE_FULL_SESSION").is_ok() {
        return Desktop::Kde;
    }
    if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        return Desktop::Gnome;
    }
    Desktop::Unknown
}
