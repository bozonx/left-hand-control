//! Generic X11 keyboard-layout backend — SKELETON.
//!
//! Planned implementation:
//!   * One-shot read: `setxkbmap -query` — text key/value output including
//!     `layout:` and `variant:`. No external crate needed.
//!   * Watch: subscribe to XKB `XkbStateNotify` via xcb (`xcb` crate with
//!     `xkb` feature) or shell out to `xev -root -event owner_grab`.
//!     Polling every 500 ms with `setxkbmap -query` is a cheap fallback.
//!   * This backend also serves as a last-resort fallback for unknown DEs
//!     that at least provide an X11 server.

#![cfg(target_os = "linux")]

use tauri::AppHandle;

use super::LayoutInfo;

pub fn current() -> Result<Option<LayoutInfo>, String> {
    eprintln!("[layout/x11] not implemented yet");
    Ok(None)
}

pub fn start_watcher(_app: AppHandle) {
    eprintln!("[layout/x11] watcher not implemented yet");
}
