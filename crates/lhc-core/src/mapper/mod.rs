#[cfg(target_os = "linux")]
pub mod action;
pub mod builtin_macros;
#[cfg(target_os = "linux")]
pub mod commands;
#[cfg(target_os = "linux")]
pub mod engine;
#[cfg(target_os = "linux")]
pub mod keys;
pub mod layer_state;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod portal;
pub mod runtime;
#[cfg(target_os = "linux")]
pub mod system;
#[cfg(target_os = "linux")]
pub mod validation;

pub use devices::{InputDevice, KeyboardDevice};

#[cfg(target_os = "linux")]
pub(crate) fn notify_mapper_stopped(error: &str) {
    layer_state::publish(layer_state::HeldLayerState::default());
    crate::events::emit(crate::events::CoreEvent::MapperStopped(error.to_string()));
}

#[cfg(target_os = "linux")]
pub(crate) fn emit_app_event(name: &str) {
    crate::events::emit(crate::events::CoreEvent::AppAction(name.to_string()));
}

#[cfg(target_os = "linux")]
pub(crate) fn notify_active_layer(layer: Option<String>) {
    crate::events::emit(crate::events::CoreEvent::ActiveLayerChanged(layer));
}

#[cfg(target_os = "linux")]
pub(crate) fn notify_layer_key_pressed() {
    crate::events::emit(crate::events::CoreEvent::LayerKeyPressed);
}

/// Layout-switching actions refresh the cached layout immediately so the
/// next key is evaluated against the new layout.
#[cfg(target_os = "linux")]
pub(crate) fn refresh_layout() {
    let _ = crate::layout::refresh_cache();
}

pub mod config;
pub mod devices;
