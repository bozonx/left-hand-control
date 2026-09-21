use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use tauri::{AppHandle, Manager};

fn socket_path() -> Option<PathBuf> {
    std::env::var_os("LHC_BENCH_SOCKET").map(PathBuf::from)
}

fn hide_all(app: &AppHandle) -> Result<(), String> {
    for label in ["main", "quick-menu", "emoji-menu"] {
        if let Some(window) = app.get_webview_window(label) {
            window.hide().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn dispatch(app: &AppHandle, command: &str) -> Result<(), String> {
    match command {
        "hide" => hide_all(app),
        "settings" => {
            hide_all(app)?;
            let window = app
                .get_webview_window("main")
                .ok_or("main window missing")?;
            window.show().map_err(|error| error.to_string())?;
            window.set_focus().map_err(|error| error.to_string())
        }
        "emoji" => {
            hide_all(app)?;
            super::show_emoji_menu_window(app, 1)
        }
        "quick" => {
            hide_all(app)?;
            super::show_quick_menu_window(app, 1)
        }
        "quit" => {
            app.exit(0);
            Ok(())
        }
        _ => Err(format!("unknown command: {command}")),
    }
}

fn serve(stream: UnixStream, app: &AppHandle) {
    let mut writer = match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    };
    for line in BufReader::new(stream).lines() {
        let Ok(command) = line else { break };
        let (sender, receiver) = mpsc::sync_channel(1);
        let handle = app.clone();
        let scheduled = app.run_on_main_thread(move || {
            let _ = sender.send(dispatch(&handle, command.trim()));
        });
        let result = scheduled
            .map_err(|error| error.to_string())
            .and_then(|_| receiver.recv().map_err(|error| error.to_string()))
            .and_then(|result| result);
        let response = result
            .map(|_| "ok".to_string())
            .unwrap_or_else(|error| format!("error {error}"));
        let _ = writeln!(writer, "{response}");
    }
}

pub fn start(app: AppHandle) {
    let Some(path) = socket_path() else { return };
    let _ = std::fs::remove_file(&path);
    std::thread::spawn(move || {
        let Ok(listener) = UnixListener::bind(&path) else {
            return;
        };
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        for stream in listener.incoming().flatten() {
            serve(stream, &app);
        }
        let _ = std::fs::remove_file(path);
    });
}
