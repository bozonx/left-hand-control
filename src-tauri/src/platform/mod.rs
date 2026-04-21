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

#[cfg(target_os = "linux")]
use std::fs::{self, OpenOptions};
#[cfg(target_os = "linux")]
use zbus::blocking::{Connection, Proxy};

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
    pub key_interception: CapabilityStatus,
    pub literal_injection: CapabilityStatus,
    pub layout_detection: CapabilityStatus,
    pub system_actions: CapabilityStatus,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CapabilityStatus {
    pub supported: bool,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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
        let key_interception = probe_linux_key_interception();
        let literal_injection = probe_linux_literal_injection();
        let caps = Capabilities {
            key_interception,
            literal_injection,
            layout_detection: CapabilityStatus {
                supported: matches!(s.desktop, linux::Desktop::Kde),
                available: matches!(s.desktop, linux::Desktop::Kde),
                detail: None,
            },
            system_actions: CapabilityStatus {
                supported: matches!(s.desktop, linux::Desktop::Kde),
                available: matches!(s.desktop, linux::Desktop::Kde),
                detail: None,
            },
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
                key_interception: CapabilityStatus {
                    supported: false,
                    available: false,
                    detail: None,
                },
                literal_injection: CapabilityStatus {
                    supported: false,
                    available: false,
                    detail: None,
                },
                layout_detection: CapabilityStatus {
                    supported: false,
                    available: false,
                    detail: None,
                },
                system_actions: CapabilityStatus {
                    supported: false,
                    available: false,
                    detail: None,
                },
            },
        }
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_key_interception() -> CapabilityStatus {
    let event_result = fs::read_dir("/dev/input")
        .map_err(|e| format!("cannot read /dev/input: {e}"))
        .and_then(|dir| {
            let mut event_paths = dir
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.starts_with("event"))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            event_paths.sort();

            if event_paths.is_empty() {
                return Err("no /dev/input/event* devices found".into());
            }

            if event_paths
                .iter()
                .any(|path| OpenOptions::new().read(true).open(path).is_ok())
            {
                Ok(())
            } else {
                Err("cannot open any /dev/input/event* device for reading".into())
            }
        });

    let uinput_result = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .map(|_| ())
        .map_err(|e| format!("cannot open /dev/uinput read-write: {e}"));

    match (event_result, uinput_result) {
        (Ok(()), Ok(())) => CapabilityStatus {
            supported: true,
            available: true,
            detail: None,
        },
        (Err(event_err), Ok(())) => CapabilityStatus {
            supported: true,
            available: false,
            detail: Some(event_err),
        },
        (Ok(()), Err(uinput_err)) => CapabilityStatus {
            supported: true,
            available: false,
            detail: Some(uinput_err),
        },
        (Err(event_err), Err(uinput_err)) => CapabilityStatus {
            supported: true,
            available: false,
            detail: Some(format!("{event_err}; {uinput_err}")),
        },
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_literal_injection() -> CapabilityStatus {
    let detail_suffix = "successful text injection still requires a RemoteDesktop portal handshake and user approval";
    match Connection::session() {
        Ok(conn) => match Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.RemoteDesktop",
        ) {
            Ok(_) => CapabilityStatus {
                supported: true,
                available: true,
                detail: Some(detail_suffix.into()),
            },
            Err(e) => CapabilityStatus {
                supported: true,
                available: false,
                detail: Some(format!("portal backend unavailable: {e}; {detail_suffix}")),
            },
        },
        Err(e) => CapabilityStatus {
            supported: true,
            available: false,
            detail: Some(format!(
                "cannot connect to session bus: {e}; {detail_suffix}"
            )),
        },
    }
}
