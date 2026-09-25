//! Native adapters behind one interface per concern: popup backend,
//! window activation, global hotkey, tray and returning input to the
//! previously focused application.
//!
//! Linux keeps the evdev/ksni/Wayland implementations; Windows and macOS use
//! the portable `global-hotkey` / `tray-icon` / native input adapters.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(target_os = "linux"))]
mod portable;
#[cfg(not(target_os = "linux"))]
pub use portable::*;
