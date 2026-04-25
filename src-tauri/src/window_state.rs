use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewWindow};

#[cfg(target_os = "linux")]
use crate::platform::linux::{self, SessionType};

static LAST_STATE: OnceLock<Mutex<Option<WindowState>>> = OnceLock::new();

#[derive(Default, Serialize, Deserialize, Clone, Copy)]
pub struct WindowState {
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub maximized: bool,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

fn position_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        matches!(linux::detect().session_type, SessionType::X11)
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

fn state_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app_config_dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    Ok(dir.join("window-state.json"))
}

fn last_state() -> &'static Mutex<Option<WindowState>> {
    LAST_STATE.get_or_init(|| Mutex::new(None))
}

pub fn load(app: &tauri::AppHandle) -> Option<WindowState> {
    let path = state_path(app).ok()?;
    let bytes = fs::read(&path).ok()?;
    serde_json::from_slice::<WindowState>(&bytes).ok()
}

fn write(app: &tauri::AppHandle, state: WindowState) {
    let Ok(path) = state_path(app) else { return };
    if let Ok(json) = serde_json::to_string(&state) {
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, json.as_bytes()).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

pub fn remember(window: &WebviewWindow) {
    let app = window.app_handle();
    let scale = window.scale_factor().unwrap_or(1.0);
    let maximized = window.is_maximized().unwrap_or(false);
    let prev = last_state()
        .lock()
        .ok()
        .and_then(|state| *state)
        .or_else(|| load(app))
        .unwrap_or_default();

    let (width, height) = if maximized {
        (prev.width, prev.height)
    } else {
        let s = window.inner_size().unwrap_or_default();
        let l = s.to_logical::<f64>(scale);
        (l.width, l.height)
    };

    if width < 1.0 || height < 1.0 {
        return;
    };

    let (x, y) = if position_supported() && !maximized {
        match window.outer_position() {
            Ok(p) => {
                let l = p.to_logical::<f64>(scale);
                (Some(l.x), Some(l.y))
            }
            Err(_) => (None, None),
        }
    } else {
        (prev.x, prev.y)
    };

    let state = WindowState {
        width,
        height,
        maximized,
        x,
        y,
    };

    if let Ok(mut last) = last_state().lock() {
        *last = Some(state);
    }
}

pub fn save(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        remember(&window);
    }
    if let Ok(last) = last_state().lock() {
        if let Some(state) = *last {
            write(app, state);
            return;
        }
    }
    if let Some(state) = load(app) {
        write(app, state);
    }
}

pub fn restore(window: &WebviewWindow) {
    let app = window.app_handle();
    let Some(state) = load(app) else { return };
    if let Ok(mut last) = last_state().lock() {
        *last = Some(state);
    }
    if state.width >= 200.0 && state.height >= 200.0 {
        let _ = window.set_size(LogicalSize::new(state.width, state.height));
    }
    if position_supported() {
        if let (Some(x), Some(y)) = (state.x, state.y) {
            let _ = window.set_position(LogicalPosition::new(x, y));
        }
    }
    if state.maximized {
        let _ = window.maximize();
    }
}
