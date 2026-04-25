use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewWindow};

#[cfg(target_os = "linux")]
use crate::platform::linux::{self, SessionType};

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

pub fn load(app: &tauri::AppHandle) -> Option<WindowState> {
    let path = state_path(app).ok()?;
    let bytes = fs::read(&path).ok()?;
    serde_json::from_slice::<WindowState>(&bytes).ok()
}

pub fn save(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let maximized = window.is_maximized().unwrap_or(false);

    // While the window is maximized, outer_size reflects the screen, not the
    // user's preferred natural size. Keep the previously saved natural size in
    // that case so it survives a maximize → quit cycle.
    let (width, height) = if maximized {
        let prev = load(app).unwrap_or_default();
        (prev.width, prev.height)
    } else {
        match window.outer_size() {
            Ok(s) => {
                let l = s.to_logical::<f64>(scale);
                (l.width, l.height)
            }
            Err(_) => return,
        }
    };

    if width < 1.0 || height < 1.0 {
        return;
    }

    let (x, y) = if position_supported() && !maximized {
        match window.outer_position() {
            Ok(p) => {
                let l = p.to_logical::<f64>(scale);
                (Some(l.x), Some(l.y))
            }
            Err(_) => (None, None),
        }
    } else {
        // Preserve previously saved position across maximize cycles, and on
        // Wayland avoid writing values that can never be restored anyway.
        let prev = load(app).unwrap_or_default();
        (prev.x, prev.y)
    };

    let state = WindowState {
        width,
        height,
        maximized,
        x,
        y,
    };
    let Ok(path) = state_path(app) else { return };
    if let Ok(json) = serde_json::to_string(&state) {
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, json.as_bytes()).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

pub fn restore(window: &WebviewWindow) {
    let app = window.app_handle();
    let Some(state) = load(app) else { return };
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
