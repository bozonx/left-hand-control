//! Native adapters behind one interface per concern: popup backend,
//! window activation, global hotkey, tray and returning input to the
//! previously focused application.
//!
//! Linux keeps the evdev/ksni/Wayland implementations; Windows and macOS use
//! the portable `global-hotkey` / `tray-icon` / native input adapters.

mod icon;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(target_os = "linux"))]
mod portable;
#[cfg(not(target_os = "linux"))]
pub use portable::*;

/// Popup backend requested with `LHC_POPUPS`; unset means winit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupMode {
    Winit,
    /// Spell when the compositor supports layer-shell, winit otherwise.
    Auto,
    Spell,
}

impl PopupMode {
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("LHC_POPUPS").as_deref() {
            Err(_) | Ok("winit") => Ok(Self::Winit),
            Ok("auto") => Ok(Self::Auto),
            Ok("spell") => Ok(Self::Spell),
            Ok(_) => Err("LHC_POPUPS must be winit, auto or spell".into()),
        }
    }
}
