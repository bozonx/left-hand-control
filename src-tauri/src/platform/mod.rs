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
//!     tells us the active keyboard layout.
//!   * `mapper::system::resolve()`                       — how to implement
//!     "system functions" (virtual-desktop switching, etc.)
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
use std::path::Path;
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
        let layout_detection = probe_linux_layout_detection(&s);
        let system_actions = probe_linux_system_actions(&s);
        build_linux_platform_info(
            s,
            key_interception,
            literal_injection,
            layout_detection,
            system_actions,
        )
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
    probe_linux_key_interception_paths(Path::new("/dev/input"), Path::new("/dev/uinput"))
}

#[cfg(target_os = "linux")]
fn build_linux_platform_info(
    s: linux::Session,
    key_interception: CapabilityStatus,
    literal_injection: CapabilityStatus,
    layout_detection: CapabilityStatus,
    system_actions: CapabilityStatus,
) -> PlatformInfo {
    let caps = Capabilities {
        key_interception,
        literal_injection,
        layout_detection,
        system_actions,
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

#[cfg(target_os = "linux")]
fn unsupported_capability(detail: impl Into<String>) -> CapabilityStatus {
    CapabilityStatus {
        supported: false,
        available: false,
        detail: Some(detail.into()),
    }
}

#[cfg(target_os = "linux")]
fn unavailable_capability(detail: impl Into<String>) -> CapabilityStatus {
    CapabilityStatus {
        supported: true,
        available: false,
        detail: Some(detail.into()),
    }
}

#[cfg(target_os = "linux")]
fn available_capability() -> CapabilityStatus {
    CapabilityStatus {
        supported: true,
        available: true,
        detail: None,
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_layout_detection(s: &linux::Session) -> CapabilityStatus {
    if !matches!(s.desktop, linux::Desktop::Kde) {
        return unsupported_capability(format!(
            "layout detection is not implemented for desktop '{}'",
            s.desktop.label()
        ));
    }

    match probe_kde_keyboard_layout_service() {
        Ok(()) => available_capability(),
        Err(e) => unavailable_capability(e),
    }
}

#[cfg(target_os = "linux")]
fn probe_kde_keyboard_layout_service() -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let proxy = Proxy::new(
        &conn,
        "org.kde.keyboard",
        "/Layouts",
        "org.kde.KeyboardLayouts",
    )
    .map_err(|e| format!("create KDE keyboard proxy: {e}"))?;
    proxy
        .call_method("getLayoutsList", &())
        .map(|_| ())
        .map_err(|e| format!("KDE keyboard layout service unavailable: {e}"))
}

#[cfg(target_os = "linux")]
fn probe_linux_system_actions(s: &linux::Session) -> CapabilityStatus {
    if !matches!(s.desktop, linux::Desktop::Kde) {
        return unsupported_capability(format!(
            "system actions are not implemented for desktop '{}'",
            s.desktop.label()
        ));
    }

    let required = ["org.kde.KWin", "org.kde.kglobalaccel"];
    match probe_dbus_names_have_owner(&required) {
        Ok(()) => available_capability(),
        Err(e) => unavailable_capability(e),
    }
}

#[cfg(target_os = "linux")]
fn probe_dbus_names_have_owner(names: &[&str]) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let dbus = Proxy::new(
        &conn,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .map_err(|e| format!("create DBus proxy: {e}"))?;

    let mut missing = Vec::new();
    for name in names {
        let msg = dbus
            .call_method("NameHasOwner", &(*name,))
            .map_err(|e| format!("NameHasOwner({name}) failed: {e}"))?;
        let has_owner: bool = msg
            .body()
            .deserialize()
            .map_err(|e| format!("decode NameHasOwner({name}) response: {e}"))?;
        if !has_owner {
            missing.push(*name);
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("missing DBus service(s): {}", missing.join(", ")))
    }
}

#[cfg(target_os = "linux")]
fn probe_linux_key_interception_paths(input_dir: &Path, uinput_path: &Path) -> CapabilityStatus {
    let event_result = fs::read_dir(input_dir)
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
        .open(uinput_path)
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
    probe_linux_literal_injection_with(
        || Connection::session().map_err(|e| e.to_string()),
        |conn| {
            Proxy::new(
                conn,
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.RemoteDesktop",
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        },
    )
}

#[cfg(target_os = "linux")]
fn probe_linux_literal_injection_with<Connect, BuildProxy, Conn>(
    connect_session: Connect,
    build_proxy: BuildProxy,
) -> CapabilityStatus
where
    Connect: FnOnce() -> Result<Conn, String>,
    BuildProxy: FnOnce(&Conn) -> Result<(), String>,
{
    let detail_suffix = "successful text injection still requires a RemoteDesktop portal handshake and user approval";
    match connect_session() {
        Ok(conn) => match build_proxy(&conn) {
            Ok(()) => CapabilityStatus {
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{
        build_linux_platform_info, probe_linux_key_interception_paths,
        probe_linux_literal_injection_with,
    };
    use crate::platform::linux::{Desktop, Session, SessionType};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lhc-platform-{prefix}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn key_interception_probe_reports_missing_devices() {
        let temp = TempDir::new("probe-empty");
        let status = probe_linux_key_interception_paths(&temp.path, &temp.path.join("uinput"));
        assert!(status.supported);
        assert!(!status.available);
        assert!(status
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("no /dev/input/event* devices found"));
    }

    #[test]
    fn literal_injection_probe_can_be_tested_without_dbus() {
        let status = probe_linux_literal_injection_with(
            || Ok::<_, String>("session"),
            |_| Err::<(), _>("missing portal".into()),
        );
        assert!(status.supported);
        assert!(!status.available);
        assert!(status
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("portal backend unavailable"));
    }

    #[test]
    fn linux_platform_info_builder_preserves_session_details() {
        let info = build_linux_platform_info(
            Session {
                desktop: Desktop::Kde,
                session_type: SessionType::Wayland,
                xdg_current_desktop: "KDE".into(),
                desktop_session: "plasma".into(),
                wayland_display: Some("wayland-0".into()),
                x11_display: None,
                sway_sock: None,
            },
            super::CapabilityStatus {
                supported: true,
                available: true,
                detail: None,
            },
            super::CapabilityStatus {
                supported: true,
                available: false,
                detail: Some("portal".into()),
            },
            super::CapabilityStatus {
                supported: true,
                available: true,
                detail: None,
            },
            super::CapabilityStatus {
                supported: true,
                available: true,
                detail: None,
            },
        );

        let linux = info.linux.expect("linux info");
        assert_eq!(linux.desktop, "KDE Plasma");
        assert_eq!(linux.session_type, "wayland");
        assert!(linux.has_wayland);
        assert!(!linux.has_x11);
        assert!(info.capabilities.layout_detection.available);
        assert!(!info.capabilities.literal_injection.available);
    }
}
