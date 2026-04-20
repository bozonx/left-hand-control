// Left Hand Control — key-mapper core.
//
// Linux-only for now. Windows/macOS remain explicit stubs until the project
// grows a real non-Linux backend. Reads events from a grabbed evdev keyboard
// and emits remapped events via a uinput virtual keyboard. Supports:
//   * layer activation on hold (tap-hold)
//   * single-tap action (also tap-hold with 0-layer)
//   * per-layer keymap remap (1:1 key -> keystroke with modifiers)

pub mod config;

#[cfg(target_os = "linux")]
mod action;
#[cfg(target_os = "linux")]
mod engine;
#[cfg(target_os = "linux")]
mod keys;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod portal;
#[cfg(target_os = "linux")]
mod system;
#[cfg(target_os = "linux")]
mod system_macros;

use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct KeyboardDevice {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MapperStatus {
    pub running: bool,
    pub device_path: Option<String>,
    pub last_error: Option<String>,
}

/// Global mapper handle. `None` when stopped.
static STATE: Mutex<MapperState> = Mutex::new(MapperState::new());

struct MapperState {
    #[cfg(target_os = "linux")]
    handle: Option<linux::Handle>,
    status: MapperStatus,
}

impl MapperState {
    const fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            handle: None,
            status: MapperStatus {
                running: false,
                device_path: None,
                last_error: None,
            },
        }
    }
}

#[cfg(target_os = "linux")]
pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    linux::list_keyboards()
}

#[cfg(not(target_os = "linux"))]
pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    Err(unsupported_os_msg("listing keyboards"))
}

#[cfg(not(target_os = "linux"))]
fn unsupported_os_msg(op: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "{op}: Windows backend not implemented yet (planned: LowLevelKeyboardProc + SendInput)"
        )
    }
    #[cfg(target_os = "macos")]
    {
        format!(
            "{op}: macOS backend not implemented yet (planned: CGEventTap + CGEventPost, requires Accessibility permission)"
        )
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        format!("{op}: this operating system is not supported")
    }
}

#[cfg(target_os = "linux")]
pub fn start(device_path: &str, config_json: &str) -> Result<(), String> {
    let cfg: config::AppConfig =
        serde_json::from_str(config_json).map_err(|e| format!("parse config: {e}"))?;
    let mut st = STATE.lock().unwrap();
    if let Some(handle) = st.handle.as_mut() {
        if handle.reap_if_finished() {
            st.handle = None;
            st.status.running = false;
        }
    }
    if st.handle.is_some() {
        return Err("mapper already running".into());
    }
    let handle = linux::spawn(device_path.to_string(), cfg)?;
    st.handle = Some(handle);
    st.status = MapperStatus {
        running: true,
        device_path: Some(device_path.to_string()),
        last_error: None,
    };
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn start(_device_path: &str, _config_json: &str) -> Result<(), String> {
    Err(unsupported_os_msg("starting mapper"))
}

pub fn stop() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut st = STATE.lock().unwrap();
        if let Some(handle) = st.handle.take() {
            drop(st); // release lock before joining
            handle.stop();
            let mut st = STATE.lock().unwrap();
            st.status.running = false;
            return Ok(());
        }
        Err("mapper is not running".into())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(unsupported_os_msg("stopping mapper"))
    }
}

pub fn status() -> MapperStatus {
    let mut st = STATE.lock().unwrap();
    // Refresh live status from linux handle if present.
    #[cfg(target_os = "linux")]
    if let Some(h) = st.handle.as_mut() {
        let err = h.last_error();
        let finished = h.reap_if_finished();
        if let Some(err) = err {
            st.status.last_error = Some(err);
        }
        if finished {
            st.handle = None;
            st.status.running = false;
        }
    }
    st.status.clone()
}
