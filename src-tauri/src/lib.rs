use std::fs;
use std::path::PathBuf;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

mod layout;
mod mapper;

fn config_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| format!("HOME not set: {e}"))?;
    Ok(PathBuf::from(home).join(".config").join("LeftHandControl"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("config.json"))
}

fn layouts_dir() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("layouts"))
}

// Keep layout names filesystem-safe: letters, digits, '-' '_' '.' and space.
// Everything else is replaced with '_'. Also strips leading dots to avoid
// hidden files and collapses empty names.
fn sanitize_layout_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("layout name is empty".into());
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' ') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let out = out.trim_start_matches('.').trim().to_string();
    if out.is_empty() {
        return Err("layout name has no valid characters".into());
    }
    Ok(out)
}

fn layout_path(name: &str) -> Result<PathBuf, String> {
    let safe = sanitize_layout_name(name)?;
    Ok(layouts_dir()?.join(format!("{safe}.yaml")))
}

#[tauri::command]
fn get_config_path() -> Result<String, String> {
    Ok(config_path()?.to_string_lossy().to_string())
}

#[tauri::command]
fn load_config() -> Result<String, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
}

#[tauri::command]
fn save_config(contents: String) -> Result<(), String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let path = dir.join("config.json");
    let tmp = dir.join("config.json.tmp");
    fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

// --- User layouts ------------------------------------------------------------

#[tauri::command]
fn get_layouts_dir() -> Result<String, String> {
    Ok(layouts_dir()?.to_string_lossy().to_string())
}

#[tauri::command]
fn list_user_layouts() -> Result<Vec<String>, String> {
    let dir = layouts_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            out.push(stem.to_string());
        }
    }
    out.sort_unstable();
    Ok(out)
}

#[tauri::command]
fn load_user_layout(name: String) -> Result<String, String> {
    let path = layout_path(&name)?;
    if !path.exists() {
        return Err(format!("layout '{name}' not found"));
    }
    fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
}

#[tauri::command]
fn save_user_layout(name: String, contents: String) -> Result<String, String> {
    let dir = layouts_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let safe = sanitize_layout_name(&name)?;
    let path = dir.join(format!("{safe}.yaml"));
    let tmp = dir.join(format!("{safe}.yaml.tmp"));
    fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(safe)
}

#[tauri::command]
fn delete_user_layout(name: String) -> Result<(), String> {
    let path = layout_path(&name)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("remove_file: {e}"))?;
    }
    Ok(())
}

// --- Mapper commands ---------------------------------------------------------

#[tauri::command]
fn list_keyboards() -> Result<Vec<mapper::KeyboardDevice>, String> {
    eprintln!("[cmd] list_keyboards");
    let r = mapper::list_keyboards();
    match &r {
        Ok(v) => eprintln!("[cmd] list_keyboards -> {} devices", v.len()),
        Err(e) => eprintln!("[cmd] list_keyboards ERR: {e}"),
    }
    r
}

#[tauri::command]
fn start_mapper(device_path: String) -> Result<(), String> {
    eprintln!("[cmd] start_mapper device={device_path}");
    let raw = load_config()?;
    if raw.is_empty() {
        let msg = "config.json not found — save settings first".to_string();
        eprintln!("[cmd] start_mapper ERR: {msg}");
        return Err(msg);
    }
    let r = mapper::start(&device_path, &raw);
    if let Err(e) = &r {
        eprintln!("[cmd] start_mapper ERR: {e}");
    }
    r
}

#[tauri::command]
fn stop_mapper() -> Result<(), String> {
    eprintln!("[cmd] stop_mapper");
    mapper::stop()
}

#[tauri::command]
fn mapper_status() -> mapper::MapperStatus {
    mapper::status()
}

#[tauri::command]
fn get_current_layout() -> Result<Option<layout::LayoutInfo>, String> {
    layout::current()
}

// --- Tray --------------------------------------------------------------------

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Показать окно", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Скрыть окно", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("tray icon".into()))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Left Hand Control")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            "hide" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            "quit" => {
                // Best-effort stop of the mapper so we always ungrab the device.
                let _ = mapper::stop();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    if w.is_visible().unwrap_or(false) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                        let _ = w.unminimize();
                        let _ = w.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            build_tray(&app.handle())?;
            layout::start_watcher(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide the window instead of exiting — the mapper stays alive.
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config_path,
            load_config,
            save_config,
            get_layouts_dir,
            list_user_layouts,
            load_user_layout,
            save_user_layout,
            delete_user_layout,
            list_keyboards,
            start_mapper,
            stop_mapper,
            mapper_status,
            get_current_layout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
