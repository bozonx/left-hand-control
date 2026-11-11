//! macOS keyboard-layout backend — intentional STUB while the shipped
//! product supports Linux/KDE only.
//!
//! Planned implementation (when enabled):
//!   * `TISCopyCurrentKeyboardInputSource()` from Carbon/TextInputServices
//!     (available via `core-foundation` / `core-graphics` crates or raw
//!     FFI to the Carbon.framework). Read `kTISPropertyInputSourceID`
//!     and `kTISPropertyLocalizedName`.
//!   * Watch: `NSDistributedNotificationCenter` observing
//!     `com.apple.Carbon.TISNotifySelectedKeyboardInputSourceChanged`.
//!
//! Not wired up yet.

#![cfg(target_os = "macos")]

use super::KeyboardLayoutInfo;

pub fn current() -> Result<Option<KeyboardLayoutInfo>, String> {
    Ok(None)
}

pub fn start_watcher() {}
