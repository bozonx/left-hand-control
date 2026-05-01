use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewWindow,
};

pub fn save_window_geometry(app: &tauri::AppHandle) {
    crate::window_state::save(app);
}

pub fn hide_main_window(window: &WebviewWindow) {
    save_window_geometry(window.app_handle());
    let _ = window.set_skip_taskbar(true);
    let _ = window.hide();
}

pub fn show_main_window(window: &WebviewWindow) {
    let _ = window.set_skip_taskbar(false);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

#[tauri::command]
pub fn show_main_window_command(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        show_main_window(&window);
    }
}

#[tauri::command]
pub fn hide_main_window_command(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        hide_main_window(&window);
    }
}

#[tauri::command]
pub fn toggle_main_window_maximized_command(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_maximized().unwrap_or(false) {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    }
}

pub fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
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
                save_window_geometry(app);
                let _ = crate::mapper::stop();
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
                    show_main_window(&w);
                }
            }
        })
        .build(app)?;
    Ok(())
}
