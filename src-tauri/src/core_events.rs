//! Forwards `lhc_core` events to the webview under the event names the
//! frontend already listens to.

use lhc_core::CoreEvent;
use tauri::{AppHandle, Emitter};

pub fn forward(app: AppHandle) {
    lhc_core::events::bus().subscribe(move |event| {
        let result = match event {
            CoreEvent::LayoutChanged(info) => app.emit("layout-changed", info),
            CoreEvent::GameModeChanged(status) => app.emit("game-mode-changed", status),
            CoreEvent::ActiveWindowChanged(window) => {
                app.emit("active-window-changed", window.clone().unwrap_or_default())
            }
            CoreEvent::MapperStopped(error) => app.emit("mapper-stopped", error),
            CoreEvent::AppAction(name) => app.emit(name, ()),
        };
        if let Err(e) = result {
            log::debug!("[core-events] emit error: {e}");
        }
    });
}
