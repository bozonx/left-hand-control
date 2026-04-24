use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WebviewWindow, WindowEvent,
};
use tauri_plugin_window_state::{AppHandleExt, WindowExt, StateFlags};
use std::time::Duration;

mod layout;
mod mapper;
mod platform;
mod storage;

fn app_storage(app: &tauri::AppHandle) -> Result<storage::StoragePaths, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("resolve app_config_dir: {e}"))?;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolve app_data_dir: {e}"))?;
    Ok(storage::StoragePaths::new(config_dir, data_dir))
}

#[tauri::command]
fn get_settings_dir(app: tauri::AppHandle) -> Result<String, String> {
    let storage = app_storage(&app)?;
    storage.ensure()?;
    Ok(storage.settings_dir().to_string_lossy().to_string())
}

#[tauri::command]
fn load_config(app: tauri::AppHandle) -> Result<String, String> {
    app_storage(&app)?.load_config()
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, contents: String) -> Result<(), String> {
    app_storage(&app)?.save_config(&contents)
}

#[tauri::command]
fn load_current_layout(app: tauri::AppHandle) -> Result<String, String> {
    app_storage(&app)?.load_current_layout()
}

#[tauri::command]
fn save_current_layout(app: tauri::AppHandle, contents: String) -> Result<(), String> {
    app_storage(&app)?.save_current_layout(&contents)
}

#[tauri::command]
fn load_ui_state(app: tauri::AppHandle) -> Result<String, String> {
    app_storage(&app)?.load_ui_state()
}

#[tauri::command]
fn save_ui_state(app: tauri::AppHandle, contents: String) -> Result<(), String> {
    app_storage(&app)?.save_ui_state(&contents)
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
    overwrite: bool,
) -> Result<String, String> {
    app_storage(&app)?.save_user_layout(&name, &contents, overwrite)
}

#[tauri::command]
fn rename_user_layout(
    app: tauri::AppHandle,
    old_name: String,
    new_name: String,
    contents: String,
    overwrite: bool,
) -> Result<String, String> {
    app_storage(&app)?.rename_user_layout(&old_name, &new_name, &contents, overwrite)
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
    _app: tauri::AppHandle,
    device_path: String,
    config_json: Option<String>,
) -> Result<(), String> {
    eprintln!("[cmd] start_mapper device={device_path}");
    let raw = match config_json {
        Some(s) if !s.trim().is_empty() => s,
        _ => {
            let msg = "configJson is required to start mapper".to_string();
            eprintln!("[cmd] start_mapper ERR: {msg}");
            return Err(msg);
        }
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

#[tauri::command]
fn quit_application(app: tauri::AppHandle) {
    let _ = mapper::stop();
    app.exit(0);
}

// --- Tray --------------------------------------------------------------------

fn window_geometry_state_flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED | StateFlags::FULLSCREEN
}

fn restore_window_geometry(window: &WebviewWindow) {
    let _ = window.restore_state(window_geometry_state_flags());
}

fn schedule_window_geometry_restore(window: WebviewWindow) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        let window_for_restore = window.clone();
        let _ = window.run_on_main_thread(move || {
            restore_window_geometry(&window_for_restore);
        });
    });
}

fn show_main_window(window: &WebviewWindow) {
    restore_window_geometry(window);
    let _ = window.show();
    let _ = window.unminimize();
    restore_window_geometry(window);
    let _ = window.set_focus();
}

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
                    show_main_window(&w);
                }
            }
            "quit" => {
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
                        show_main_window(&w);
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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            build_tray(app.handle())?;
            layout::start_watcher(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                schedule_window_geometry_restore(window);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.app_handle().save_window_state(StateFlags::all());
                // Hide the window instead of exiting — the mapper stays alive.
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings_dir,
            load_config,
            save_config,
            load_current_layout,
            save_current_layout,
            load_ui_state,
            save_ui_state,
            get_layouts_dir,
            list_user_layouts,
            load_user_layout,
            save_user_layout,
            rename_user_layout,
            delete_user_layout,
            list_keyboards,
            start_mapper,
            stop_mapper,
            mapper_status,
            get_current_layout,
            get_platform_info,
            quit_application,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let RunEvent::Exit = event {
            let _ = mapper::stop();
            layout::stop_watcher();
        }
    });
}
