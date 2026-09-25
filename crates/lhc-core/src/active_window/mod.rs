// Detection of the currently focused window across supported Linux DEs.
//
// Provides a poll-based watcher (similar to `gamemode`) that caches the
// latest result in `runtime_state` and emits
// `CoreEvent::ActiveWindowChanged`. The cached value is also consumed
// directly by the mapper engine to evaluate per-rule app conditions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
mod linux;

pub use crate::runtime_state::ActiveWindow;

static WATCHER_STOP: AtomicBool = AtomicBool::new(false);

pub fn cached_active_window() -> Option<ActiveWindow> {
    crate::runtime_state::active_window()
}

pub fn stop_watcher() {
    WATCHER_STOP.store(true, Ordering::SeqCst);
}

fn watcher_stop_requested() -> bool {
    WATCHER_STOP.load(Ordering::SeqCst)
}

pub fn start_watcher() {
    WATCHER_STOP.store(false, Ordering::SeqCst);

    let _ = thread::Builder::new()
        .name("active-window-watcher".into())
        .spawn(move || {
            let mut last: Option<ActiveWindow> = None;

            while !watcher_stop_requested() {
                let current = detect_active_window();

                if current != last {
                    crate::runtime_state::set_active_window(current.clone());
                    crate::events::emit(crate::events::CoreEvent::ActiveWindowChanged(
                        current.clone(),
                    ));
                    last = current;
                }

                thread::sleep(Duration::from_millis(500));
            }
        });
}

fn detect_active_window() -> Option<ActiveWindow> {
    #[cfg(target_os = "linux")]
    {
        linux::detect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Detect synchronously and refresh the cache (used before showing popups).
pub fn detect_active_window_now() -> Option<ActiveWindow> {
    let current = detect_active_window();
    crate::runtime_state::set_active_window(current.clone());
    current
}
