// Current keyboard layout detection.
//
// Cross-platform facade. Backends:
//   * Linux + KDE Plasma     — DBus `org.kde.keyboard`            (implemented)
//   * Linux + GNOME          — GSettings `input-sources`           (skeleton)
//   * Linux + Sway / wlroots — `swaymsg -t get_inputs`             (skeleton)
//   * Linux + generic X11    — `setxkbmap -query`                  (skeleton,
//                                                                   also used
//                                                                   as fallback
//                                                                   for unknown
//                                                                   DEs)
//   * Windows                — GetKeyboardLayoutName               (stub)
//   * macOS                  — TISCopyCurrentKeyboardInputSource   (stub)
//
// Frontend contract:
//   * command `get_current_layout()` -> Option<LayoutInfo>
//   * event   `layout-changed`       -> LayoutInfo

use serde::Serialize;

#[cfg(target_os = "linux")]
mod linux_kde;
#[cfg(target_os = "linux")]
mod linux_gnome;
#[cfg(target_os = "linux")]
mod linux_sway;
#[cfg(target_os = "linux")]
mod linux_x11;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Serialize)]
pub struct LayoutInfo {
    /// Short code, e.g. "us", "ru".
    pub short: String,
    /// Optional display/variant name, e.g. "lat".
    pub display: String,
    /// Long human-readable name, e.g. "English (US)".
    pub long: String,
    /// Zero-based index among the configured layouts.
    pub index: u32,
    /// Which backend produced this info.
    pub backend: &'static str,
}

pub fn current() -> Result<Option<LayoutInfo>, String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{detect, Desktop};
        match detect().desktop {
            Desktop::Kde => linux_kde::current(),
            Desktop::Gnome => linux_gnome::current(),
            Desktop::Sway => linux_sway::current(),
            // Everything else — Hyprland, Xfce, MATE, Unknown, … — falls
            // through to the generic X11 backend. It is still a skeleton
            // but is the correct catch-all once implemented.
            _ => linux_x11::current(),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::current()
    }
    #[cfg(target_os = "macos")]
    {
        macos::current()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Ok(None)
    }
}

/// Start a background watcher that emits `layout-changed` events.
/// Safe to call once at app startup. No-op if no backend is available.
pub fn start_watcher(app: tauri::AppHandle) {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{detect, Desktop};
        let s = detect();
        eprintln!(
            "[layout] linux session: desktop={} session_type={} xdg={:?}",
            s.desktop.label(),
            s.session_type.label(),
            s.xdg_current_desktop,
        );
        match s.desktop {
            Desktop::Kde => linux_kde::start_watcher(app),
            Desktop::Gnome => linux_gnome::start_watcher(app),
            Desktop::Sway => linux_sway::start_watcher(app),
            _ => linux_x11::start_watcher(app),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::start_watcher(app);
    }
    #[cfg(target_os = "macos")]
    {
        macos::start_watcher(app);
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = app;
    }
}
