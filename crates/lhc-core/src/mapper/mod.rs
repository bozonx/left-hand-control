#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod action;
pub mod builtin_macros;
pub mod commands;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod engine;
pub mod key_code;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod keys;
pub mod layer_state;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod output;
#[cfg(target_os = "linux")]
pub mod portal;
pub mod runtime;
#[cfg(any(target_os = "windows", test))]
mod scancode;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod system;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod validation;
#[cfg(target_os = "windows")]
pub mod windows;

pub use devices::{InputDevice, KeyboardDevice};

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn notify_mapper_stopped(error: &str) {
    layer_state::publish(layer_state::HeldLayerState::default());
    crate::events::emit(crate::events::CoreEvent::MapperStopped(error.to_string()));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn emit_app_event(name: &str) {
    crate::events::emit(crate::events::CoreEvent::AppAction(name.to_string()));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn notify_active_layer(layer: Option<String>) {
    crate::events::emit(crate::events::CoreEvent::ActiveLayerChanged(layer));
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn notify_layer_key_pressed() {
    crate::events::emit(crate::events::CoreEvent::LayerKeyPressed);
}

/// Layout-switching actions refresh the cached layout immediately so the
/// next key is evaluated against the new layout.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) fn refresh_layout() {
    let _ = crate::layout::refresh_cache();
}

pub mod config;
pub mod devices;

#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "linux")]
mod portal_clipboard;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod text_job;
