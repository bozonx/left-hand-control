//! Windows keyboard-layout backend — intentional STUB while the shipped
//! product supports Linux/KDE only.
//!
//! Planned implementation (when enabled):
//!   * `GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), …))`
//!     to obtain the current HKL.
//!   * `GetKeyboardLayoutNameW` for the 8-char KLID, then resolve against
//!     `HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Keyboard Layouts`
//!     to get the human-readable name.
//!   * Watch: poll GetKeyboardLayout() on foreground-window changes, or
//!     install a WM_INPUTLANGCHANGE hook via SetWinEventHook.
//!
//! Depends on the `windows` crate (`windows-rs`). Not wired up yet.

#![cfg(target_os = "windows")]

use tauri::AppHandle;

use super::LayoutInfo;

pub fn current() -> Result<Option<LayoutInfo>, String> {
    Ok(None)
}

pub fn start_watcher(_app: AppHandle) {}
