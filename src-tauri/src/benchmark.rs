use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Manager};

struct Probe {
    window: String,
    started: Instant,
    stages: [Option<f64>; 3],
}

static PROBE: OnceLock<Mutex<Option<Probe>>> = OnceLock::new();

fn probe() -> &'static Mutex<Option<Probe>> {
    PROBE.get_or_init(|| Mutex::new(None))
}

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
        "emoji" | "probe-emoji" => {
            hide_all(app)?;
            super::show_emoji_menu_window(app, 1)
        }
        "quick" | "probe-quick" => {
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

pub fn begin(window: &str) {
    if socket_path().is_none() {
        return;
    }
    *probe().lock().unwrap() = Some(Probe {
        window: window.to_string(),
        started: Instant::now(),
        stages: [None; 3],
    });
}

#[tauri::command]
pub fn popup_stage(window: String, stage: u8) {
    if !(3..=5).contains(&stage) {
        return;
    }
    let mut guard = probe().lock().unwrap();
    let Some(current) = guard.as_mut() else {
        return;
    };
    if current.window == window && current.stages[(stage - 3) as usize].is_none() {
        current.stages[(stage - 3) as usize] =
            Some(current.started.elapsed().as_secs_f64() * 1000.0);
    }
}

#[tauri::command]
pub fn benchmark_active() -> bool {
    socket_path().is_some()
}

fn take_probe(window: &str) -> Option<[Option<f64>; 3]> {
    let mut guard = probe().lock().unwrap();
    if guard
        .as_ref()
        .is_some_and(|current| current.window == window)
    {
        guard.take().map(|current| current.stages)
    } else {
        None
    }
}

fn probe_complete(window: &str) -> bool {
    probe()
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|current| current.window == window && current.stages[2].is_some())
}

fn serve(stream: UnixStream, app: &AppHandle) {
    let mut writer = match stream.try_clone() {
        Ok(stream) => stream,
        Err(_) => return,
    };
    for line in BufReader::new(stream).lines() {
        let Ok(command) = line else { break };
        let command = command.trim().to_string();
        let probe_window = command.strip_prefix("probe-").map(str::to_string);
        if let Some(window) = &probe_window {
            begin(window);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let handle = app.clone();
        let dispatch_command = command.clone();
        let scheduled = app.run_on_main_thread(move || {
            let _ = sender.send(dispatch(&handle, &dispatch_command));
        });
        let result = scheduled
            .map_err(|error| error.to_string())
            .and_then(|_| receiver.recv().map_err(|error| error.to_string()))
            .and_then(|result| result);
        let response = result
            .map(|_| {
                if let Some(window) = probe_window {
                    let deadline = Instant::now() + std::time::Duration::from_secs(2);
                    while !probe_complete(&window) && Instant::now() < deadline {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                    let stages = take_probe(&window).unwrap_or([None; 3]);
                    format!(
                        "ok t3={} t4={} t5={}",
                        stages[0].map_or_else(|| "na".into(), |v| format!("{v:.3}")),
                        stages[1].map_or_else(|| "na".into(), |v| format!("{v:.3}")),
                        stages[2].map_or_else(|| "na".into(), |v| format!("{v:.3}"))
                    )
                } else {
                    "ok".to_string()
                }
            })
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
