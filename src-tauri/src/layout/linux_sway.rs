//! Sway / wlroots keyboard-layout backend — intentional SKELETON while the
//! shipped product supports Linux/KDE only.
//!
//! Planned implementation:
//!   * `swaymsg -t get_inputs -r` returns JSON array with per-input
//!     entries including `xkb_active_layout_name` and `xkb_layout_names`.
//!     Pick the first keyboard-like entry.
//!   * Subscribe to layout changes with `swaymsg -t subscribe '["input"]'`
//!     which streams JSON events; filter on `change == "xkb_layout"`.
//!   * Hyprland would use `hyprctl devices -j` + the `activelayout` event
//!     on the Hyprland socket — separate backend.

#![cfg(target_os = "linux")]

use tauri::AppHandle;

use super::LayoutInfo;

pub fn current() -> Result<Option<LayoutInfo>, String> {
    eprintln!("[layout/sway] not implemented yet");
    Ok(None)
}

pub fn start_watcher(_app: AppHandle) {
    eprintln!("[layout/sway] watcher not implemented yet");
}
