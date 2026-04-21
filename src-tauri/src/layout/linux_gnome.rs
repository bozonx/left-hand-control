//! GNOME keyboard-layout backend — intentional SKELETON while the shipped
//! product supports Linux/KDE only.
//!
//! Planned implementation:
//!   * Query current / configured sources from GSettings:
//!     gsettings get org.gnome.desktop.input-sources current
//!     gsettings get org.gnome.desktop.input-sources sources
//!     The `sources` value is `a(ss)`: list of (type, id) where type is
//!     "xkb" | "ibus" and id is e.g. "us", "ru", or "xkb:ru::rus".
//!   * Watch for changes via:
//!     gsettings monitor org.gnome.desktop.input-sources current
//!     (blocking subprocess, one line per change).
//!   * Optionally fall back to DBus `org.gnome.Shell` when available.

#![cfg(target_os = "linux")]

use tauri::AppHandle;

use super::LayoutInfo;

pub fn current() -> Result<Option<LayoutInfo>, String> {
    eprintln!("[layout/gnome] not implemented yet");
    Ok(None)
}

pub fn start_watcher(_app: AppHandle) {
    eprintln!("[layout/gnome] watcher not implemented yet");
}
