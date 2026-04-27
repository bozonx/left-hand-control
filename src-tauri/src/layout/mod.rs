// Current keyboard layout detection.
//
// Current product support is Linux/KDE. The facade keeps the other backends
// visible as explicit stubs/skeletons so unsupported platforms and desktop
// environments fail predictably instead of silently diverging.
//
// Backends:
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
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
mod linux_gnome;
#[cfg(target_os = "linux")]
mod linux_kde;
#[cfg(target_os = "linux")]
mod linux_sway;
#[cfg(target_os = "linux")]
mod linux_x11;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

static WATCHER_STOP: AtomicBool = AtomicBool::new(false);

pub fn stop_watcher() {
    WATCHER_STOP.store(true, Ordering::SeqCst);
}

fn watcher_stop_requested() -> bool {
    WATCHER_STOP.load(Ordering::SeqCst)
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

pub fn cached_layout_short() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{detect, Desktop};
        match detect().desktop {
            Desktop::Kde => linux_kde::cached_layout_short(),
            _ => None,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

pub fn available_layouts() -> Result<Vec<LayoutInfo>, String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{detect, Desktop};
        match detect().desktop {
            Desktop::Kde => linux_kde::available_layouts(),
            _ => Ok(vec![]),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(vec![])
    }
}

/// Switch the active OS keyboard layout to the given zero-based index.
/// Currently implemented for KDE Plasma; other desktops/OSes return an
/// explicit error so the frontend can disable the control.
pub fn set(index: u32) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{detect, Desktop};
        match detect().desktop {
            Desktop::Kde => linux_kde::set_layout(index),
            d => Err(format!(
                "switching layout is not implemented for desktop '{}'",
                d.label()
            )),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = index;
        Err("switching layout is not implemented on this OS".to_string())
    }
}

/// Start a background watcher that emits `layout-changed` events.
/// Safe to call once at app startup. No-op if no backend is available.
pub fn start_watcher(app: tauri::AppHandle) {
    WATCHER_STOP.store(false, Ordering::SeqCst);
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
