use std::path::PathBuf;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

mod layout;
mod mapper;
mod platform;
mod storage;

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

fn app_storage(app: &tauri::AppHandle) -> Result<storage::StoragePaths, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("resolve app_config_dir: {e}"))?;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolve app_data_dir: {e}"))?;
    Ok(storage::StoragePaths::new(
        config_dir,
        data_dir,
        legacy_config_dir(),
    ))
}

#[tauri::command]
fn get_config_path(app: tauri::AppHandle) -> Result<String, String> {
    let storage = app_storage(&app)?;
    storage.ensure()?;
    Ok(storage.config_path().to_string_lossy().to_string())
}

#[tauri::command]
fn load_config(app: tauri::AppHandle) -> Result<String, String> {
    app_storage(&app)?.load_config()
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, contents: String) -> Result<(), String> {
    app_storage(&app)?.save_config(&contents)
}

// --- User layouts ------------------------------------------------------------

#[tauri::command]
fn get_layouts_dir(app: tauri::AppHandle) -> Result<String, String> {
    let storage = app_storage(&app)?;
    storage.ensure()?;
    Ok(storage.layouts_dir().to_string_lossy().to_string())
}

#[tauri::command]
fn list_user_layouts(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    app_storage(&app)?.list_user_layouts()
}

#[tauri::command]
fn load_user_layout(app: tauri::AppHandle, name: String) -> Result<String, String> {
    app_storage(&app)?.load_user_layout(&name)
}

#[tauri::command]
fn save_user_layout(
    app: tauri::AppHandle,
    name: String,
    contents: String,
) -> Result<String, String> {
    app_storage(&app)?.save_user_layout(&name, &contents)
}

#[tauri::command]
fn delete_user_layout(app: tauri::AppHandle, name: String) -> Result<(), String> {
    app_storage(&app)?.delete_user_layout(&name)
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
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

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
