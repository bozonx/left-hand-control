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
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize)]
pub struct KeyboardDevice {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MapperStatus {
    pub running: bool,
    pub device_path: Option<String>,
    pub mouse_device_path: Option<String>,
    pub last_error: Option<String>,
}

/// Global mapper handle. `None` when stopped.
static STATE: Mutex<MapperRuntime<OsBackend>> = Mutex::new(MapperRuntime::new(OsBackend::new()));

trait BackendHandle: Send {
    fn stop(self: Box<Self>);
    fn last_error(&self) -> Option<String>;
    fn reap_if_finished(&mut self) -> bool;
}

trait MapperBackend: Send + Sync + 'static {
    fn list_keyboards(&self) -> Result<Vec<KeyboardDevice>, String>;
    fn list_mice(&self) -> Result<Vec<KeyboardDevice>, String>;
    fn spawn(
        &self,
        device_path: String,
        mouse_path: Option<String>,
        cfg: config::AppConfig,
    ) -> Result<Box<dyn BackendHandle>, String>;
}

struct MapperRuntime<B> {
    backend: B,
    handle: Option<Box<dyn BackendHandle>>,
    status: MapperStatus,
}

impl<B> MapperRuntime<B> {
    const fn new(backend: B) -> Self {
        Self {
            backend,
            handle: None,
            status: MapperStatus {
                running: false,
                device_path: None,
                mouse_device_path: None,
                last_error: None,
            },
        }
    }
}

impl<B: MapperBackend> MapperRuntime<B> {
    fn list_keyboards(&self) -> Result<Vec<KeyboardDevice>, String> {
        self.backend.list_keyboards()
    }

    fn list_mice(&self) -> Result<Vec<KeyboardDevice>, String> {
        self.backend.list_mice()
    }

    fn start(
        &mut self,
        device_path: &str,
        mouse_path: Option<&str>,
        cfg: config::AppConfig,
    ) -> Result<(), String> {
        if let Some(handle) = self.handle.as_mut() {
            if handle.reap_if_finished() {
                self.handle = None;
                self.status.running = false;
            }
        }
        if self.handle.is_some() {
            return Err("mapper already running".into());
        }
        let mouse = mouse_path.map(|s| s.to_string());
        let handle = self
            .backend
            .spawn(device_path.to_string(), mouse.clone(), cfg)?;
        self.handle = Some(handle);
        self.status = MapperStatus {
            running: true,
            device_path: Some(device_path.to_string()),
            mouse_device_path: mouse,
            last_error: None,
        };
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(handle) = self.handle.take() {
            handle.stop();
            self.status.running = false;
            return Ok(());
        }
        Err("mapper is not running".into())
    }

    fn status(&mut self) -> MapperStatus {
        if let Some(handle) = self.handle.as_mut() {
            let err = handle.last_error();
            let finished = handle.reap_if_finished();
            if let Some(err) = err {
                self.status.last_error = Some(err);
            }
            if finished {
                self.handle = None;
                self.status.running = false;
            }
        }
        self.status.clone()
    }
}

#[cfg(target_os = "linux")]
impl BackendHandle for linux::Handle {
    fn stop(self: Box<Self>) {
        (*self).stop();
    }

    fn last_error(&self) -> Option<String> {
        self.last_error()
    }

    fn reap_if_finished(&mut self) -> bool {
        self.reap_if_finished()
    }
}

#[cfg(target_os = "linux")]
struct LinuxBackend;

#[cfg(target_os = "linux")]
impl LinuxBackend {
    const fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "linux")]
impl MapperBackend for LinuxBackend {
    fn list_keyboards(&self) -> Result<Vec<KeyboardDevice>, String> {
        linux::list_keyboards()
    }

    fn list_mice(&self) -> Result<Vec<KeyboardDevice>, String> {
        linux::list_mice()
    }

    fn spawn(
        &self,
        device_path: String,
        mouse_path: Option<String>,
        cfg: config::AppConfig,
    ) -> Result<Box<dyn BackendHandle>, String> {
        linux::spawn(device_path, mouse_path, cfg)
            .map(|handle| Box::new(handle) as Box<dyn BackendHandle>)
    }
}

#[cfg(not(target_os = "linux"))]
struct UnsupportedBackend;

#[cfg(not(target_os = "linux"))]
impl UnsupportedBackend {
    const fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "linux"))]
impl MapperBackend for UnsupportedBackend {
    fn list_keyboards(&self) -> Result<Vec<KeyboardDevice>, String> {
        Err(unsupported_os_msg("listing keyboards"))
    }

    fn list_mice(&self) -> Result<Vec<KeyboardDevice>, String> {
        Err(unsupported_os_msg("listing mice"))
    }

    fn spawn(
        &self,
        _device_path: String,
        _mouse_path: Option<String>,
        _cfg: config::AppConfig,
    ) -> Result<Box<dyn BackendHandle>, String> {
        Err(unsupported_os_msg("starting mapper"))
    }
}

#[cfg(target_os = "linux")]
type OsBackend = LinuxBackend;

#[cfg(not(target_os = "linux"))]
type OsBackend = UnsupportedBackend;

fn lock_state() -> MutexGuard<'static, MapperRuntime<OsBackend>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    lock_state().list_keyboards()
}

pub fn list_mice() -> Result<Vec<KeyboardDevice>, String> {
    lock_state().list_mice()
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

pub fn start(device_path: &str, mouse_path: Option<&str>, config_json: &str) -> Result<(), String> {
    let cfg: config::AppConfig =
        serde_json::from_str(config_json).map_err(|e| format!("parse config: {e}"))?;
    lock_state().start(device_path, mouse_path, cfg)
}

pub fn stop() -> Result<(), String> {
    lock_state().stop()
}

pub fn status() -> MapperStatus {
    lock_state().status()
}

/// Tell the portal singleton where to read/write the saved
/// `restore_token`. Should be called once at app startup, before any
/// literal-injection request.
#[cfg(target_os = "linux")]
pub fn set_portal_token_dir(dir: std::path::PathBuf) {
    portal::set_token_dir(dir);
}

#[cfg(not(target_os = "linux"))]
pub fn set_portal_token_dir(_dir: std::path::PathBuf) {}

#[cfg(test)]
mod tests {
    use super::{BackendHandle, KeyboardDevice, MapperBackend, MapperRuntime};
    use crate::mapper::config::{AppConfig, Settings};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    struct FakeHandle {
        stop_called: Arc<Mutex<bool>>,
        finished: bool,
        last_error: Option<String>,
    }

    impl BackendHandle for FakeHandle {
        fn stop(self: Box<Self>) {
            if let Ok(mut slot) = self.stop_called.lock() {
                *slot = true;
            }
        }

        fn last_error(&self) -> Option<String> {
            self.last_error.clone()
        }

        fn reap_if_finished(&mut self) -> bool {
            self.finished
        }
    }

    type HandleQueue = Arc<Mutex<VecDeque<Result<Box<dyn BackendHandle>, String>>>>;

    struct FakeBackend {
        devices: Vec<KeyboardDevice>,
        next_handles: HandleQueue,
    }

    impl FakeBackend {
        fn new(
            devices: Vec<KeyboardDevice>,
            next_handles: Vec<Result<Box<dyn BackendHandle>, String>>,
        ) -> Self {
            Self {
                devices,
                next_handles: Arc::new(Mutex::new(next_handles.into())),
            }
        }
    }

    impl MapperBackend for FakeBackend {
        fn list_keyboards(&self) -> Result<Vec<KeyboardDevice>, String> {
            Ok(self.devices.clone())
        }

        fn list_mice(&self) -> Result<Vec<KeyboardDevice>, String> {
            Ok(Vec::new())
        }

        fn spawn(
            &self,
            _device_path: String,
            _mouse_path: Option<String>,
            _cfg: AppConfig,
        ) -> Result<Box<dyn BackendHandle>, String> {
            self.next_handles
                .lock()
                .expect("lock handles")
                .pop_front()
                .unwrap_or_else(|| Err("no fake handle".into()))
        }
    }

    fn empty_cfg() -> AppConfig {
        AppConfig {
            rules: Vec::new(),
            layer_keymaps: Default::default(),
            macros: Vec::new(),
            commands: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn runtime_updates_status_from_finished_handle() {
        let runtime_backend = FakeBackend::new(
            Vec::new(),
            vec![Ok(Box::new(FakeHandle {
                stop_called: Arc::new(Mutex::new(false)),
                finished: true,
                last_error: Some("boom".into()),
            }))],
        );
        let mut runtime = MapperRuntime::new(runtime_backend);

        runtime
            .start("/dev/input/event1", None, empty_cfg())
            .expect("start");
        let status = runtime.status();

        assert!(!status.running);
        assert_eq!(status.device_path.as_deref(), Some("/dev/input/event1"));
        assert_eq!(status.last_error.as_deref(), Some("boom"));
    }

    #[test]
    fn runtime_stop_calls_handle_stop() {
        let stop_called = Arc::new(Mutex::new(false));
        let runtime_backend = FakeBackend::new(
            Vec::new(),
            vec![Ok(Box::new(FakeHandle {
                stop_called: stop_called.clone(),
                finished: false,
                last_error: None,
            }))],
        );
        let mut runtime = MapperRuntime::new(runtime_backend);

        runtime
            .start("/dev/input/event2", None, empty_cfg())
            .expect("start");
        runtime.stop().expect("stop");

        assert!(*stop_called.lock().expect("lock stop flag"));
        assert!(!runtime.status().running);
    }

    #[test]
    fn runtime_lists_devices_via_backend() {
        let runtime = MapperRuntime::new(FakeBackend::new(
            vec![KeyboardDevice {
                path: "/dev/input/event3".into(),
                name: "Test Keyboard".into(),
            }],
            Vec::new(),
        ));

        let devices = runtime.list_keyboards().expect("list keyboards");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Test Keyboard");
    }
}
