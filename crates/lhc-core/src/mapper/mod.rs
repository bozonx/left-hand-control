//! Keyboard mapper: configuration, the platform-independent engine, the
//! shared output loop and the Linux (evdev + uinput) and Windows (hooks +
//! `SendInput`) backends. Text for `text:` actions goes through
//! [`crate::text_input`].

/// Applies the "has a mapper backend" cfg to each item.
macro_rules! with_backend {
    ($($item:item)*) => {
        $( #[cfg(any(target_os = "linux", target_os = "windows"))] $item )*
    };
}

// Available on every platform: configuration model and editor helpers.
pub mod builtin_macros;
pub mod commands;
pub mod config;
pub mod devices;
pub mod key_code;
pub mod layer_state;
pub mod runtime;

with_backend! {
    pub mod action;
    pub mod engine;
    pub mod keys;
    mod output;
    pub mod system;
    pub mod validation;
}

#[cfg(any(target_os = "windows", test))]
mod input_guard;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(any(target_os = "windows", test))]
mod scancode;
#[cfg(target_os = "windows")]
pub mod windows;

pub use devices::{InputDevice, KeyboardDevice};

with_backend! {
    pub(crate) fn notify_mapper_stopped(error: &str) {
        layer_state::publish(layer_state::HeldLayerState::default());
        crate::events::emit(crate::events::CoreEvent::MapperStopped(error.to_string()));
    }

    pub(crate) fn emit_app_event(name: &str) {
        crate::events::emit(crate::events::CoreEvent::AppAction(name.to_string()));
    }

    pub(crate) fn notify_active_layer(layer: Option<String>) {
        crate::events::emit(crate::events::CoreEvent::ActiveLayerChanged(layer));
    }

    pub(crate) fn notify_layer_key_pressed() {
        crate::events::emit(crate::events::CoreEvent::LayerKeyPressed);
    }

    /// Layout-switching actions refresh the cached layout immediately so the
    /// next key is evaluated against the new layout.
    pub(crate) fn refresh_layout() {
        let _ = crate::layout::refresh_cache();
    }
}
