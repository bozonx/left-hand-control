use crate::gamemode::get_gamemode_status;
use tauri::{Emitter, Listener, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};

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
    app_storage(&app)?.save_config(&contents)?;
    gamemode::update_settings_from_config_json(&contents);
    Ok(())
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

fn validate_device_path(path: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Keyboard device path is required".into());
    }
    if !path.starts_with("/dev/input/") {
        return Err(format!(
            "device path must be under /dev/input/, got: {path}"
        ));
    }
    if path.contains("..") {
        return Err("Device path cannot contain '..'".into());
    }
    Ok(())
}

#[tauri::command]
fn list_input_devices() -> Result<Vec<mapper::InputDevice>, String> {
    log::debug!("[cmd] list_input_devices");
    let r = mapper::list_input_devices();
    match &r {
        Ok(v) => log::debug!("[cmd] list_input_devices -> {} devices", v.len()),
        Err(e) => log::debug!("[cmd] list_input_devices ERR: {e}"),
    }
    r
}

#[tauri::command]
fn list_keyboards() -> Result<Vec<mapper::KeyboardDevice>, String> {
    log::debug!("[cmd] list_keyboards");
    let r = mapper::list_keyboards();
    match &r {
        Ok(v) => log::debug!("[cmd] list_keyboards -> {} devices", v.len()),
        Err(e) => log::debug!("[cmd] list_keyboards ERR: {e}"),
    }
    r
}

#[tauri::command]
fn list_mice() -> Result<Vec<mapper::KeyboardDevice>, String> {
    log::debug!("[cmd] list_mice");
    let r = mapper::list_mice();
    match &r {
        Ok(v) => log::debug!("[cmd] list_mice -> {} devices", v.len()),
        Err(e) => log::debug!("[cmd] list_mice ERR: {e}"),
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
    log::debug!("[cmd] start_mapper device={device_path} mouse={mouse_device_path:?}");
    validate_device_path(&device_path)?;
    if let Some(ref mouse) = mouse_device_path {
        if !mouse.trim().is_empty() {
            validate_device_path(mouse)?;
        }
    }
    let raw = match config_json {
        Some(s) if !s.trim().is_empty() => s,
        _ => {
            let msg = "Mapper configuration is required".to_string();
            log::debug!("[cmd] start_mapper ERR: {msg}");
            return Err(msg);
        }
    };
    if raw.is_empty() {
        let msg = "config.json not found — save settings first".to_string();
        log::debug!("[cmd] start_mapper ERR: {msg}");
        return Err(msg);
    }
    let mouse = mouse_device_path.as_deref().filter(|s| !s.is_empty());
    let r = mapper::start(&device_path, mouse, &raw);
    if let Err(e) = &r {
        log::debug!("[cmd] start_mapper ERR: {e}");
    }
    r
}

#[tauri::command]
fn stop_mapper() -> Result<(), String> {
    log::debug!("[cmd] stop_mapper");
    mapper::stop()
}

#[tauri::command]
fn update_mapper_config(config_json: String) -> Result<(), String> {
    log::debug!("[cmd] update_mapper_config");
    gamemode::update_settings_from_config_json(&config_json);
    let r = mapper::update_config(&config_json);
    if let Err(e) = &r {
        log::debug!("[cmd] update_mapper_config ERR: {e}");
    }
    r
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
    log::debug!("[cmd] set_current_layout index={index}");
    let r = layout::set(index);
    if let Err(e) = &r {
        log::debug!("[cmd] set_current_layout ERR: {e}");
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

#[tauri::command]
fn execute_action(action: String) -> Result<(), String> {
    log::debug!("[cmd] execute_action action={action}");
    mapper::execute_action(action)
}

#[tauri::command]
fn hide_quick_menu(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick-menu") {
        window.hide().map_err(|e| format!("hide quick menu: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn hide_emoji_menu(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("emoji-menu") {
        window.hide().map_err(|e| format!("hide emoji menu: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn insert_text(text: String) -> Result<(), String> {
    let action = format!("text:{}", text);
    log::debug!("[cmd] insert_text action={action}");
    mapper::execute_action(action)
}

fn show_quick_menu_window(app: &tauri::AppHandle, page: u8) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window("quick-menu") {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            "quick-menu",
            WebviewUrl::App(format!("quick-menu?page={page}").into()),
        )
        .title("Quick Actions")
        .inner_size(760.0, 520.0)
        .min_inner_size(520.0, 120.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("create quick menu window: {e}"))?
    };
    window
        .center()
        .map_err(|e| format!("center quick menu: {e}"))?;
    window.show().map_err(|e| format!("show quick menu: {e}"))?;
    window
        .set_focus()
        .map_err(|e| format!("focus quick menu: {e}"))?;
    app.emit_to("quick-menu", "open_quick_menu_page", page)
        .map_err(|e| format!("emit quick menu page: {e}"))?;
    Ok(())
}

fn show_emoji_menu_window(app: &tauri::AppHandle, page: u8) -> Result<(), String> {
    let window = if let Some(window) = app.get_webview_window("emoji-menu") {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            "emoji-menu",
            WebviewUrl::App(format!("emoji-menu?page={page}").into()),
        )
        .title("Emoji")
        .inner_size(520.0, 500.0)
        .min_inner_size(360.0, 260.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("create emoji menu window: {e}"))?
    };
    window
        .center()
        .map_err(|e| format!("center emoji menu: {e}"))?;
    window.show().map_err(|e| format!("show emoji menu: {e}"))?;
    window
        .set_focus()
        .map_err(|e| format!("focus emoji menu: {e}"))?;
    app.emit_to("emoji-menu", "open_emoji_menu_page", page)
        .map_err(|e| format!("emit emoji menu page: {e}"))?;
    Ok(())
}

fn listen_menu_pages(
    app: &tauri::App,
    event_prefix: &'static str,
    log_prefix: &'static str,
    show: fn(&tauri::AppHandle, u8) -> Result<(), String>,
) {
    for page in 1..=5 {
        let menu_app = app.handle().clone();
        app.listen(format!("{event_prefix}_{page}"), move |_| {
            let app = menu_app.clone();
            let app_for_thread = app.clone();
            if let Err(e) = app.run_on_main_thread(move || {
                if let Err(e) = show(&app_for_thread, page) {
                    log::debug!("[{log_prefix}] {e}");
                }
            }) {
                log::debug!("[{log_prefix}] schedule show failed: {e}");
            }
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            mapper::set_app_handle(app.handle().clone());
            tray::build_tray(app.handle())?;
            if let Ok(storage) = app_storage(app.handle()) {
                let _ = storage.ensure();
                mapper::set_portal_token_dir(storage.data_dir().clone());
            }
            layout::start_watcher(app.handle().clone());
            gamemode::start_watcher(app.handle().clone());
            active_window::start_watcher(app.handle().clone());
            listen_menu_pages(app, "show_quick_menu", "quick-menu", show_quick_menu_window);
            listen_menu_pages(app, "show_emoji_menu", "emoji-menu", show_emoji_menu_window);
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
                if window.label() == "quick-menu" || window.label() == "emoji-menu" {
                    let _ = window.hide();
                } else if let Some(w) = window.app_handle().get_webview_window("main") {
                    tray::hide_main_window(&w);
                }
            }
            WindowEvent::Focused(false) => {
                if window.label() == "quick-menu" || window.label() == "emoji-menu" {
                    let _ = window.hide();
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
            list_input_devices,
            list_keyboards,
            list_mice,
            start_mapper,
            stop_mapper,
            update_mapper_config,
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
            execute_action,
            insert_text,
            hide_quick_menu,
            hide_emoji_menu,
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
