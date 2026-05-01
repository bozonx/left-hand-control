use crate::gamemode::get_gamemode_status;
use tauri::{Manager, RunEvent, WindowEvent};

mod active_window;
mod gamemode;
mod layout;
mod mapper;
mod platform;
mod storage;
mod tray;
mod window_state;

fn app_storage(app: &tauri::AppHandle) -> Result<storage::StoragePaths, String> {
    storage::resolve_storage_paths(app)
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
fn list_mice() -> Result<Vec<mapper::KeyboardDevice>, String> {
    eprintln!("[cmd] list_mice");
    let r = mapper::list_mice();
    match &r {
        Ok(v) => eprintln!("[cmd] list_mice -> {} devices", v.len()),
        Err(e) => eprintln!("[cmd] list_mice ERR: {e}"),
    }
    r
}

#[tauri::command]
fn start_mapper(
    _app: tauri::AppHandle,
    device_path: String,
    mouse_device_path: Option<String>,
    config_json: Option<String>,
) -> Result<(), String> {
    eprintln!("[cmd] start_mapper device={device_path} mouse={mouse_device_path:?}");
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
    let mouse = mouse_device_path.as_deref().filter(|s| !s.is_empty());
    let r = mapper::start(&device_path, mouse, &raw);
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
fn get_system_layouts() -> Result<Vec<layout::LayoutInfo>, String> {
    layout::available_layouts()
}

#[tauri::command]
fn set_current_layout(index: u32) -> Result<(), String> {
    eprintln!("[cmd] set_current_layout index={index}");
    let r = layout::set(index);
    if let Err(e) = &r {
        eprintln!("[cmd] set_current_layout ERR: {e}");
    }
    r
}

#[tauri::command]
fn get_platform_info() -> platform::PlatformInfo {
    platform::info()
}

#[tauri::command]
fn quit_application(app: tauri::AppHandle) {
    window_state::save(&app);
    let _ = mapper::stop();
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            tray::build_tray(app.handle())?;
            if let Ok(storage) = app_storage(app.handle()) {
                let _ = storage.ensure();
                mapper::set_portal_token_dir(storage.data_dir().clone());
            }
            layout::start_watcher(app.handle().clone());
            gamemode::start_watcher(app.handle().clone());
            active_window::start_watcher(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                window_state::restore(&window);
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if let Some(w) = window.app_handle().get_webview_window("main") {
                    tray::hide_main_window(&w);
                }
            }
            WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
                if let Some(w) = window.app_handle().get_webview_window("main") {
                    window_state::remember(&w);
                }
            }
            _ => {}
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
            list_mice,
            start_mapper,
            stop_mapper,
            mapper_status,
            get_current_layout,
            get_system_layouts,
            set_current_layout,
            get_gamemode_status,
            get_platform_info,
            active_window::get_active_window,
            tray::show_main_window_command,
            tray::hide_main_window_command,
            tray::toggle_main_window_maximized_command,
            quit_application,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let RunEvent::Exit = event {
            let _ = mapper::stop();
            gamemode::stop_watcher();
            layout::stop_watcher();
            active_window::stop_watcher();
        }
    });
}
