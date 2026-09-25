// Detection of the currently focused window across supported Linux DEs.
//
// Provides a poll-based watcher (similar to `gamemode`) that caches the
// latest result, emits `active-window-changed` events, and exposes a
// Tauri command for one-shot reads. The cached value is also consumed
// directly by the mapper engine to evaluate per-rule app conditions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "linux")]
mod linux;

pub use lhc_core::runtime_state::ActiveWindow;

static WATCHER_STOP: AtomicBool = AtomicBool::new(false);
pub fn cached_active_window() -> Option<ActiveWindow> {
    lhc_core::runtime_state::active_window()
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
                    lhc_core::runtime_state::set_active_window(current.clone());
                    let payload = current.clone().unwrap_or_default();
                    if let Err(e) = app.emit("active-window-changed", payload) {
                        log::debug!("[active-window] emit error: {e}");
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

pub fn detect_active_window_now() -> Option<ActiveWindow> {
    let current = detect_active_window();
    lhc_core::runtime_state::set_active_window(current.clone());
    current
}

#[tauri::command]
pub fn get_active_window() -> Option<ActiveWindow> {
    cached_active_window()
}
