//! `sys:` actions on Windows. Window actions post documented window
//! messages to the foreground window, layouts use `WM_INPUTLANGCHANGEREQUEST`,
//! and shell actions and vertical maximization press their fixed system
//! shortcuts. Windows has no
//! public API to switch to or move a window to virtual desktop N, so those
//! ids do not resolve. Every call returns without waiting for the target.

use super::super::key_code::KeyCode;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetForegroundWindow, GetWindowLongW, GetWindowRect, HWND_BROADCAST, HWND_TOPMOST,
    IsZoomed, PostMessageW, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_MONITORPOWER, SC_RESTORE,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, WM_SYSCOMMAND,
    WS_EX_TOPMOST,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsAction {
    /// One-based position in the user's input language list.
    SwitchLayout(u32),
    WindowClose,
    WindowMinimize,
    WindowMaximize,
    WindowMaximizeVertical,
    WindowKeepAbove,
    ShowDesktop,
    Overview,
    AppLauncher,
    ScreenOff,
}

pub fn resolve(name: &str) -> Option<WindowsAction> {
    if let Some(n) = name
        .strip_prefix("switchLayout")
        .and_then(|rest| rest.parse::<u32>().ok())
        .filter(|n| (1..=crate::profile::actions::SYSTEM_NUMBERED_COUNT).contains(n))
    {
        return Some(WindowsAction::SwitchLayout(n));
    }
    Some(match name {
        "windowClose" => WindowsAction::WindowClose,
        "windowMinimize" => WindowsAction::WindowMinimize,
        "windowMaximize" => WindowsAction::WindowMaximize,
        "windowMaximizeVertical" => WindowsAction::WindowMaximizeVertical,
        "windowKeepAbove" => WindowsAction::WindowKeepAbove,
        "showDesktop" => WindowsAction::ShowDesktop,
        "overview" => WindowsAction::Overview,
        "appLauncher" => WindowsAction::AppLauncher,
        "screenOff" => WindowsAction::ScreenOff,
        _ => return None,
    })
}

/// The foreground window when it belongs to an application, not to the
/// desktop or taskbar: closing the desktop would open the shutdown dialog.
fn app_window() -> Result<HWND, String> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() || crate::platform::windows::is_shell_window(window) {
        return Err("No application window is focused".into());
    }
    Ok(window)
}

fn system_command(command: u32) -> Result<(), String> {
    let window = app_window()?;
    unsafe {
        PostMessageW(
            Some(window),
            WM_SYSCOMMAND,
            WPARAM(command as usize),
            LPARAM(0),
        )
    }
    .map_err(|error| format!("Window command failed: {error}"))
}

fn toggle_keep_above() -> Result<(), String> {
    let window = app_window()?;
    let topmost = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0 != 0;
    // HWND_NOTOPMOST is (HWND)-2.
    let after = if topmost {
        HWND(-2isize as _)
    } else {
        HWND_TOPMOST
    };
    unsafe {
        SetWindowPos(
            window,
            Some(after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        )
    }
    .map_err(|error| format!("Keep above failed: {error}"))
}

/// Slack for the invisible resize borders that `GetWindowRect` includes
/// (about 8 px at 100 % scale, more at higher scales).
const EDGE_SLACK: i32 = 24;

/// Whether the window spans its monitor's work area from top to bottom.
fn spans_work_area_vertically(window: HWND) -> Result<bool, String> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(window, &mut rect) }
        .map_err(|error| format!("Window geometry failed: {error}"))?;
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Err("Monitor geometry failed".into());
    }
    let work = info.rcWork;
    Ok(
        (rect.top - work.top).abs() <= EDGE_SLACK
            && (rect.bottom - work.bottom).abs() <= EDGE_SLACK,
    )
}

/// Win+Shift+Up stretches the window to the work area height; Win+Shift+Down
/// gives it back the height Windows remembered, which makes the action a
/// toggle like on KDE.
fn toggle_maximize_vertical() -> Result<(), String> {
    let window = app_window()?;
    let key = if spans_work_area_vertically(window)? {
        KeyCode::KEY_DOWN
    } else {
        KeyCode::KEY_UP
    };
    super::input::shortcut(&[KeyCode::KEY_LEFTMETA, KeyCode::KEY_LEFTSHIFT], key)
}

pub(super) fn run(action: WindowsAction) -> Result<(), String> {
    match action {
        WindowsAction::SwitchLayout(n) => crate::layout::set(n - 1),
        WindowsAction::WindowClose => system_command(SC_CLOSE),
        WindowsAction::WindowMinimize => system_command(SC_MINIMIZE),
        WindowsAction::WindowMaximize => {
            let window = app_window()?;
            if unsafe { IsZoomed(window) }.as_bool() {
                system_command(SC_RESTORE)
            } else {
                system_command(SC_MAXIMIZE)
            }
        }
        WindowsAction::WindowMaximizeVertical => toggle_maximize_vertical(),
        WindowsAction::WindowKeepAbove => toggle_keep_above(),
        WindowsAction::ShowDesktop => {
            super::input::shortcut(&[KeyCode::KEY_LEFTMETA], KeyCode::KEY_D)
        }
        WindowsAction::Overview => {
            super::input::shortcut(&[KeyCode::KEY_LEFTMETA], KeyCode::KEY_TAB)
        }
        WindowsAction::AppLauncher => {
            super::input::shortcut(&[KeyCode::KEY_LEFTCTRL], KeyCode::KEY_ESC)
        }
        // 2 = the display is being shut off.
        WindowsAction::ScreenOff => unsafe {
            PostMessageW(
                Some(HWND_BROADCAST),
                WM_SYSCOMMAND,
                WPARAM(SC_MONITORPOWER as usize),
                LPARAM(2),
            )
        }
        .map_err(|error| format!("Screen off failed: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_every_plain_action_but_not_virtual_desktops() {
        for id in crate::profile::actions::PLAIN_SYSTEM_ACTIONS {
            assert!(resolve(id).is_some(), "{id}");
        }
        assert_eq!(
            resolve("switchLayout3"),
            Some(WindowsAction::SwitchLayout(3))
        );
        for id in [
            "switchLayout0",
            "switchLayout11",
            "switchDesktop1",
            "windowToDesktop2",
        ] {
            assert!(resolve(id).is_none(), "{id}");
        }
    }
}
