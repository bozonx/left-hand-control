//! Platform + session detection layer.
//!
//! The long-term shape has three OS backends — `linux`, `windows`, `macos`.
//! In the current product we intentionally support only Linux, with KDE as
//! the only fully implemented desktop integration. The other OS/DE modules
//! stay in place as compile-safe stubs so the call sites remain explicit.
//!
//! Inside the Linux backend we further dispatch to per-desktop
//! implementations (KDE, GNOME, Sway, generic X11) for APIs that are
//! DE-specific:
//!
//!   * `layout::current()` / `layout::start_watcher()`   — which DBus / CLI tool
//!                                                         tells us the active
//!                                                         keyboard layout.
//!   * `mapper::system::resolve()`                       — how to implement
//!                                                         "system functions"
//!                                                         (virtual-desktop
//!                                                         switching, etc.)
//!
//! The low-level key-event interception (evdev + uinput) is DE-agnostic,
//! so it lives directly in `mapper::linux` without further branching.
//!
//! Windows / macOS bindings are intentionally stubs for now — the module
//! structure is in place so the call sites compile and return a clear,
//! actionable error instead of panicking.

#[cfg(target_os = "linux")]
pub mod linux;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformInfo {
    /// "linux", "windows", "macos", or "unknown".
    pub os: &'static str,
    /// Linux-only details. `None` on other OSes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<LinuxInfo>,
    /// Capabilities of the current backend, for UI display / debugging.
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LinuxInfo {
    pub desktop: &'static str,
    pub session_type: &'static str,
    pub xdg_current_desktop: String,
    pub desktop_session: String,
    pub has_wayland: bool,
    pub has_x11: bool,
    pub has_sway_ipc: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Capabilities {
    pub key_interception: bool,
    pub literal_injection: bool,
    pub layout_detection: bool,
    pub system_actions: bool,
}

pub fn os_kind() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "unknown"
    }
}

pub fn info() -> PlatformInfo {
    #[cfg(target_os = "linux")]
    {
        let s = linux::detect();
        let caps = Capabilities {
            // evdev + uinput works on any Linux with the right perms.
            key_interception: true,
            // xdg-desktop-portal RemoteDesktop — available on KDE / GNOME / Sway
            // when the user-side portal pieces are installed.
            literal_injection: true,
            // Layout detection: only KDE has a real backend right now.
            layout_detection: matches!(s.desktop, linux::Desktop::Kde),
            // System actions (switchDesktopN, …): only KDE right now.
            system_actions: matches!(s.desktop, linux::Desktop::Kde),
        };
        PlatformInfo {
            os: os_kind(),
            linux: Some(LinuxInfo {
                desktop: s.desktop.label(),
                session_type: s.session_type.label(),
                xdg_current_desktop: s.xdg_current_desktop.clone(),
                desktop_session: s.desktop_session.clone(),
                has_wayland: s.wayland_display.is_some(),
                has_x11: s.x11_display.is_some(),
                has_sway_ipc: s.sway_sock.is_some(),
            }),
            capabilities: caps,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        PlatformInfo {
            os: os_kind(),
            linux: None,
            capabilities: Capabilities {
                key_interception: false,
                literal_injection: false,
                layout_detection: false,
                system_actions: false,
            },
        }
    }
}
