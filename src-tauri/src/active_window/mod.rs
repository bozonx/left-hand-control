// Detection of the currently focused window across supported Linux DEs.
//
// Provides a poll-based watcher (similar to `gamemode`) that caches the
// latest result, emits `active-window-changed` events, and exposes a
// Tauri command for one-shot reads. The cached value is also consumed
// directly by the mapper engine to evaluate per-rule app conditions.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "linux")]
mod linux;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindow {
    pub title: String,
    pub app_id: String,
}

static WATCHER_STOP: AtomicBool = AtomicBool::new(false);
static CACHED: Mutex<Option<ActiveWindow>> = Mutex::new(None);

pub fn cached_active_window() -> Option<ActiveWindow> {
    CACHED.lock().ok().and_then(|guard| guard.clone())
}

#[cfg(test)]
pub fn set_cached_for_test(value: Option<ActiveWindow>) {
    if let Ok(mut guard) = CACHED.lock() {
        *guard = value;
    }
}

pub fn stop_watcher() {
    WATCHER_STOP.store(true, Ordering::SeqCst);
}

fn watcher_stop_requested() -> bool {
    WATCHER_STOP.load(Ordering::SeqCst)
}

pub fn start_watcher(app: AppHandle) {
    WATCHER_STOP.store(false, Ordering::SeqCst);

    let _ = thread::Builder::new()
        .name("active-window-watcher".into())
        .spawn(move || {
            let mut last: Option<ActiveWindow> = None;

            while !watcher_stop_requested() {
                let current = detect_active_window();

                if current != last {
                    if let Ok(mut guard) = CACHED.lock() {
                        *guard = current.clone();
                    }
                    let payload = current.clone().unwrap_or_default();
                    if let Err(e) = app.emit("active-window-changed", payload) {
                        eprintln!("[active-window] emit error: {e}");
                    }
                    last = current;
                }

                thread::sleep(Duration::from_millis(500));
            }
        });
}

fn detect_active_window() -> Option<ActiveWindow> {
    #[cfg(target_os = "linux")]
    {
        return linux::detect();
    }
    #[allow(unreachable_code)]
    {
        None
    }
}

#[tauri::command]
pub fn get_active_window() -> Option<ActiveWindow> {
    cached_active_window()
}
