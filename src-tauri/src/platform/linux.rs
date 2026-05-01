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
    let wayland = std::env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|s| !s.is_empty());
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
    classify_desktop_impl(
        xdg,
        session,
        has_swaysock,
        std::env::var("KDE_FULL_SESSION").is_ok(),
        std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok(),
    )
}

fn classify_desktop_impl(
    xdg: &str,
    session: &str,
    has_swaysock: bool,
    kde_full_session: bool,
    gnome_session_id: bool,
) -> Desktop {
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
    if kde_full_session {
        return Desktop::Kde;
    }
    if gnome_session_id {
        return Desktop::Gnome;
    }
    Desktop::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_kde_from_xdg() {
        assert_eq!(
            classify_desktop_impl("KDE", "", false, false, false),
            Desktop::Kde
        );
    }

    #[test]
    fn classify_kde_from_session() {
        assert_eq!(
            classify_desktop_impl("", "KDE", false, false, false),
            Desktop::Kde
        );
    }

    #[test]
    fn classify_kde_from_colon_separated_xdg() {
        assert_eq!(
            classify_desktop_impl("ubuntu:KDE", "", false, false, false),
            Desktop::Kde
        );
    }

    #[test]
    fn classify_gnome_from_xdg() {
        assert_eq!(
            classify_desktop_impl("GNOME", "", false, false, false),
            Desktop::Gnome
        );
    }

    #[test]
    fn classify_gnome_classic() {
        assert_eq!(
            classify_desktop_impl("GNOME-Classic", "", false, false, false),
            Desktop::Gnome
        );
    }

    #[test]
    fn classify_sway_from_xdg() {
        assert_eq!(
            classify_desktop_impl("sway", "", false, false, false),
            Desktop::Sway
        );
    }

    #[test]
    fn classify_sway_from_sock() {
        assert_eq!(
            classify_desktop_impl("", "", true, false, false),
            Desktop::Sway
        );
    }

    #[test]
    fn classify_hyprland() {
        assert_eq!(
            classify_desktop_impl("Hyprland", "", false, false, false),
            Desktop::Hyprland
        );
    }

    #[test]
    fn classify_xfce() {
        assert_eq!(
            classify_desktop_impl("XFCE", "", false, false, false),
            Desktop::Xfce
        );
    }

    #[test]
    fn classify_cinnamon() {
        assert_eq!(
            classify_desktop_impl("X-Cinnamon", "", false, false, false),
            Desktop::Cinnamon
        );
    }

    #[test]
    fn classify_mate() {
        assert_eq!(
            classify_desktop_impl("MATE", "", false, false, false),
            Desktop::Mate
        );
    }

    #[test]
    fn classify_lxqt() {
        assert_eq!(
            classify_desktop_impl("LXQt", "", false, false, false),
            Desktop::Lxqt
        );
    }

    #[test]
    fn classify_unity() {
        assert_eq!(
            classify_desktop_impl("Unity", "", false, false, false),
            Desktop::Unity
        );
    }

    #[test]
    fn classify_pantheon() {
        assert_eq!(
            classify_desktop_impl("Pantheon", "", false, false, false),
            Desktop::Pantheon
        );
    }

    #[test]
    fn classify_unknown_when_no_match() {
        assert_eq!(
            classify_desktop_impl("", "", false, false, false),
            Desktop::Unknown
        );
    }

    #[test]
    fn classify_kde_fallback_env() {
        assert_eq!(
            classify_desktop_impl("", "", false, true, false),
            Desktop::Kde
        );
    }

    #[test]
    fn classify_gnome_fallback_env() {
        assert_eq!(
            classify_desktop_impl("", "", false, false, true),
            Desktop::Gnome
        );
    }

    #[test]
    fn classify_haystack_beats_env_fallback() {
        assert_eq!(
            classify_desktop_impl("Sway", "", false, true, true),
            Desktop::Sway
        );
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(
            classify_desktop_impl("kde", "", false, false, false),
            Desktop::Kde
        );
        assert_eq!(
            classify_desktop_impl("GnOmE", "", false, false, false),
            Desktop::Gnome
        );
    }
}
