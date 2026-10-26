//! Linux platform adapters: evdev hotkeys, ksni tray, Wayland activation and
//! input return.

pub mod backend;
pub mod focus;
pub mod hotkey;
#[cfg(feature = "probes")]
pub mod return_input;
pub mod tray;
