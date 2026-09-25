#[cfg(target_os = "linux")]
pub mod action;
#[cfg(target_os = "linux")]
pub mod engine;
#[cfg(target_os = "linux")]
pub mod keys;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod portal;
pub mod runtime;
#[cfg(target_os = "linux")]
pub mod system;
pub mod system_macros;
#[cfg(target_os = "linux")]
pub mod validation;

pub use crate::mapper_types::{InputDevice, KeyboardDevice};

#[cfg(target_os = "linux")]
pub(crate) fn notify_mapper_stopped(error: &str) {
    crate::events::emit(crate::events::CoreEvent::MapperStopped(error.to_string()));
}

#[cfg(target_os = "linux")]
pub(crate) fn emit_app_event(name: &str) {
    crate::events::emit(crate::events::CoreEvent::AppAction(name.to_string()));
}

/// Layout-switching actions refresh the cached layout immediately so the
/// next key is evaluated against the new layout.
#[cfg(target_os = "linux")]
pub(crate) fn refresh_layout() {
    let _ = crate::layout::refresh_cache();
}

pub mod config {
    pub use crate::mapper_config::*;
}
