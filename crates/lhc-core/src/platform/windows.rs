use windows::Win32::Foundation::{HANDLE, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{
    QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetForegroundWindow, GetShellWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic,
};
use windows::core::{Owned, PWSTR};

use crate::runtime_state::ActiveWindow;

pub(crate) fn active_window() -> Option<ActiveWindow> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    let process_name = process_name(pid);
    let app_id = process_name.clone().unwrap_or_default();
    let length = unsafe { GetWindowTextLengthW(window) }.max(0) as usize;
    let mut title = vec![0_u16; (length + 1).min(32768)];
    let length = unsafe { GetWindowTextW(window, &mut title) }.max(0) as usize;
    let title = String::from_utf16_lossy(&title[..length]);
    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow {
        title,
        app_id,
        process_name,
        fullscreen: fullscreen_active(),
    })
}

fn process_name(pid: u32) -> Option<String> {
    let process: Owned<HANDLE> =
        unsafe { Owned::new(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?) };
    let mut path = vec![0_u16; 32768];
    let mut length = path.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            *process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    let path = String::from_utf16_lossy(&path[..length as usize]);
    std::path::Path::new(&path)
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

pub(crate) fn running_process_names() -> Option<Vec<String>> {
    let snapshot: Owned<HANDLE> =
        unsafe { Owned::new(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?) };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..PROCESSENTRY32W::default()
    };
    unsafe { Process32FirstW(*snapshot, &mut entry) }.ok()?;
    let mut names = Vec::new();
    loop {
        let length = entry
            .szExeFile
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
        if !name.is_empty() {
            names.push(name);
        }
        if unsafe { Process32NextW(*snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    Some(names)
}

/// Whether the foreground window is fullscreen: exclusive Direct3D
/// fullscreen as reported by the shell, or a window (often borderless)
/// whose client area covers its monitor.
pub(crate) fn fullscreen_active() -> Option<bool> {
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) if state == QUNS_RUNNING_D3D_FULL_SCREEN => return Some(true),
        Ok(state) if state == QUNS_PRESENTATION_MODE => return Some(false),
        _ => {}
    }
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid()
        || window == unsafe { GetShellWindow() }
        || unsafe { IsIconic(window) }.as_bool()
    {
        return Some(false);
    }
    let client = client_rect_on_screen(window)?;
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    Some(
        client.left <= info.rcMonitor.left
            && client.top <= info.rcMonitor.top
            && client.right >= info.rcMonitor.right
            && client.bottom >= info.rcMonitor.bottom,
    )
}

fn client_rect_on_screen(window: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(window, &mut rect) }.ok()?;
    let mut origin = POINT::default();
    if !unsafe { ClientToScreen(window, &mut origin) }.as_bool() {
        return None;
    }
    rect.left += origin.x;
    rect.right += origin.x;
    rect.top += origin.y;
    rect.bottom += origin.y;
    Some(rect)
}
