use std::fs;
use std::path::PathBuf;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

mod layout;
mod mapper;
mod platform;

fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("resolve app_config_dir: {e}"))
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("config.json"))
}

fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("resolve app_data_dir: {e}"))
}

fn layouts_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("layouts"))
}

#[cfg(target_os = "linux")]
fn legacy_config_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".config").join("LeftHandControl"))
}

#[cfg(not(target_os = "linux"))]
fn legacy_config_dir() -> Option<PathBuf> {
    None
}

fn migrate_file_if_missing(from: &PathBuf, to: &PathBuf) -> Result<(), String> {
    if to.exists() || !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create_dir_all: {e}"))?;
    }
    fs::copy(from, to).map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))?;
    Ok(())
}

fn migrate_layouts_if_missing(from_dir: &PathBuf, to_dir: &PathBuf) -> Result<(), String> {
    if !from_dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(to_dir).map_err(|e| format!("create_dir_all: {e}"))?;
    for entry in fs::read_dir(from_dir).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let from = entry.path();
        if !from.is_file() {
            continue;
        }
        let to = to_dir.join(entry.file_name());
        migrate_file_if_missing(&from, &to)?;
    }
    Ok(())
}

fn ensure_app_storage(app: &tauri::AppHandle) -> Result<(), String> {
    let config_dir = config_dir(app)?;
    let data_dir = data_dir(app)?;
    fs::create_dir_all(&config_dir).map_err(|e| format!("create_dir_all: {e}"))?;
    fs::create_dir_all(&data_dir).map_err(|e| format!("create_dir_all: {e}"))?;

    if let Some(legacy_dir) = legacy_config_dir() {
        migrate_file_if_missing(
            &legacy_dir.join("config.json"),
            &config_dir.join("config.json"),
        )?;
        migrate_layouts_if_missing(&legacy_dir.join("layouts"), &data_dir.join("layouts"))?;
    }

    Ok(())
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

fn layout_path(app: &tauri::AppHandle, name: &str) -> Result<PathBuf, String> {
    let safe = sanitize_layout_name(name)?;
    Ok(layouts_dir(app)?.join(format!("{safe}.yaml")))
}

#[tauri::command]
fn get_config_path(app: tauri::AppHandle) -> Result<String, String> {
    ensure_app_storage(&app)?;
    Ok(config_path(&app)?.to_string_lossy().to_string())
}

#[tauri::command]
fn load_config(app: tauri::AppHandle) -> Result<String, String> {
    ensure_app_storage(&app)?;
    let path = config_path(&app)?;
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, contents: String) -> Result<(), String> {
    ensure_app_storage(&app)?;
    let dir = config_dir(&app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let path = dir.join("config.json");
    let tmp = dir.join("config.json.tmp");
    fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

// --- User layouts ------------------------------------------------------------

#[tauri::command]
fn get_layouts_dir(app: tauri::AppHandle) -> Result<String, String> {
    ensure_app_storage(&app)?;
    Ok(layouts_dir(&app)?.to_string_lossy().to_string())
}

#[tauri::command]
fn list_user_layouts(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    ensure_app_storage(&app)?;
    let dir = layouts_dir(&app)?;
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
fn load_user_layout(app: tauri::AppHandle, name: String) -> Result<String, String> {
    ensure_app_storage(&app)?;
    let path = layout_path(&app, &name)?;
    if !path.exists() {
        return Err(format!("layout '{name}' not found"));
    }
    fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
}

#[tauri::command]
fn save_user_layout(
    app: tauri::AppHandle,
    name: String,
    contents: String,
) -> Result<String, String> {
    ensure_app_storage(&app)?;
    let dir = layouts_dir(&app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
    let safe = sanitize_layout_name(&name)?;
    let path = dir.join(format!("{safe}.yaml"));
    let tmp = dir.join(format!("{safe}.yaml.tmp"));
    fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(safe)
}

#[tauri::command]
fn delete_user_layout(app: tauri::AppHandle, name: String) -> Result<(), String> {
    ensure_app_storage(&app)?;
    let path = layout_path(&app, &name)?;
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
fn start_mapper(
    app: tauri::AppHandle,
    device_path: String,
    config_json: Option<String>,
) -> Result<(), String> {
    eprintln!("[cmd] start_mapper device={device_path}");
    let raw = match config_json {
        Some(s) if !s.trim().is_empty() => s,
        _ => load_config(app)?,
    };
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

#[tauri::command]
fn get_platform_info() -> platform::PlatformInfo {
    platform::info()
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
    let app = tauri::Builder::default()
        .setup(|app| {
            build_tray(app.handle())?;
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
            get_platform_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_, event| {
        if let RunEvent::Exit = event {
            let _ = mapper::stop();
            layout::stop_watcher();
        }
    });
}
