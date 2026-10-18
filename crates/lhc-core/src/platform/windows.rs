//! Windows: the foreground window, focus-change events, open windows and
//! running processes.
//!
//! Focus events come from an out-of-context WinEvent hook: nothing is
//! injected into other processes, and window titles are read only when a
//! condition needs them.

use std::sync::Mutex;
use std::sync::mpsc;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EVENT_OBJECT_NAMECHANGE, EVENT_SYSTEM_FOREGROUND, EnumChildWindows,
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetClassNameW, GetClientRect, GetForegroundWindow,
    GetMessageW, GetShellWindow, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, MSG, OBJID_WINDOW, PostThreadMessageW,
    TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, Owned, PWSTR};

use crate::active_window::OpenWindow;
use crate::runtime_state::ActiveWindow;

/// Host process of UWP app frames; the app runs in a child window.
const UWP_FRAME_HOST: &str = "ApplicationFrameHost.exe";
/// Window classes of the shell that cover a monitor without being an
/// application: the desktop (`WorkerW` hosts it once a wallpaper is
/// drawn), the taskbars and the task switchers.
const SHELL_WINDOW_CLASSES: [&str; 7] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "MultitaskingViewFrame",
    "XamlExplorerHostIslandWindow",
    "ForegroundStaging",
];

pub(crate) fn active_window(titles: bool) -> Option<ActiveWindow> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    let process_name = process_name(app_pid(window));
    let app_id = process_name.clone().unwrap_or_default();
    let title = if titles {
        window_title(window)
    } else {
        String::new()
    };
    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow {
        title,
        app_id,
        process_name,
        fullscreen: fullscreen_for_window(window),
    })
}

/// The foreground window and its process id, when there is one.
pub(crate) fn foreground_window() -> Option<(isize, u32)> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    Some((window.0 as isize, window_pid(window)))
}

fn window_title(window: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(window) }.max(0) as usize;
    let mut title = vec![0_u16; (length + 1).min(32768)];
    let length = unsafe { GetWindowTextW(window, &mut title) }.max(0) as usize;
    String::from_utf16_lossy(&title[..length])
}

fn window_pid(window: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    pid
}

/// Process of the app showing `window`: for a UWP frame the process of
/// the child window that belongs to another process.
fn app_pid(window: HWND) -> u32 {
    let pid = window_pid(window);
    if !process_name(pid).is_some_and(|name| name.eq_ignore_ascii_case(UWP_FRAME_HOST)) {
        return pid;
    }
    let mut found = (pid, 0_u32);
    unsafe {
        let _ = EnumChildWindows(
            Some(window),
            Some(child_of_other_process),
            LPARAM(&mut found as *mut (u32, u32) as isize),
        );
    }
    if found.1 != 0 { found.1 } else { pid }
}

unsafe extern "system" fn child_of_other_process(child: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` points at the `(u32, u32)` owned by `app_pid`, which
    // outlives the synchronous enumeration.
    let found = unsafe { &mut *(data.0 as *mut (u32, u32)) };
    let pid = window_pid(child);
    if pid != found.0 {
        found.1 = pid;
        return false.into();
    }
    true.into()
}

/// Thread running the WinEvent hook, by thread id.
static FOCUS_EVENTS: Mutex<Option<(u32, JoinHandle<()>)>> = Mutex::new(None);

/// Report foreground changes, and title changes of the foreground window
/// while titles are needed, through `active_window::wake`.
pub(crate) fn start_focus_events() -> bool {
    let (sender, receiver) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("active-window-events".into())
        .spawn(move || {
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            let hooks = [EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_NAMECHANGE].map(|event| unsafe {
                SetWinEventHook(event, event, None, Some(on_focus_event), 0, 0, flags)
            });
            let hooked = !hooks[0].is_invalid();
            let _ = sender.send(hooked.then(|| unsafe { GetCurrentThreadId() }));
            if hooked {
                let mut message = MSG::default();
                while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
                    unsafe {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
            for hook in hooks {
                if !hook.is_invalid() {
                    let _ = unsafe { UnhookWinEvent(hook) };
                }
            }
        });
    let Ok(thread) = thread else {
        return false;
    };
    match receiver.recv().ok().flatten() {
        Some(thread_id) => {
            if let Ok(mut slot) = FOCUS_EVENTS.lock() {
                *slot = Some((thread_id, thread));
            }
            true
        }
        None => {
            let _ = thread.join();
            log::info!("[active-window] WinEvent hook unavailable, polling instead");
            false
        }
    }
}

pub(crate) fn stop_focus_events() {
    let events = FOCUS_EVENTS.lock().ok().and_then(|mut slot| slot.take());
    if let Some((thread_id, thread)) = events {
        let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        let _ = thread.join();
    }
}

unsafe extern "system" fn on_focus_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let relevant = event == EVENT_SYSTEM_FOREGROUND
        || (object == OBJID_WINDOW.0
            && crate::active_window::titles_needed()
            && window == unsafe { GetForegroundWindow() });
    if relevant {
        crate::active_window::wake();
    }
}

/// Visible top-level application windows: no owner, not tool windows.
pub(crate) fn open_windows() -> Vec<OpenWindow> {
    let mut windows: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_window),
            LPARAM(&mut windows as *mut Vec<HWND> as isize),
        );
    }
    windows
        .into_iter()
        .filter_map(|window| {
            let title = window_title(window);
            if title.is_empty() {
                return None;
            }
            let process_name = process_name(app_pid(window));
            Some(OpenWindow {
                app_id: process_name.clone().unwrap_or_default(),
                process_name,
                title,
            })
        })
        .collect()
}

unsafe extern "system" fn collect_window(window: HWND, data: LPARAM) -> BOOL {
    // SAFETY: `data` points at the vector owned by `open_windows`, which
    // outlives the synchronous enumeration.
    let windows = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
    let style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
    let tool = (style & WS_EX_TOOLWINDOW.0) != 0;
    let owned = unsafe { GetWindow(window, GW_OWNER) }.is_ok_and(|owner| !owner.is_invalid());
    if unsafe { IsWindowVisible(window) }.as_bool() && !tool && !owned {
        windows.push(window);
    }
    true.into()
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

/// Executable names of the processes in this logon session; services and
/// other users' sessions are left out, as on Linux.
pub(crate) fn running_process_names() -> Option<Vec<String>> {
    let session = session_of(unsafe { GetCurrentProcessId() });
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
        if !name.is_empty() && (session.is_none() || session_of(entry.th32ProcessID) == session) {
            names.push(name);
        }
        if unsafe { Process32NextW(*snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    Some(names)
}

fn session_of(pid: u32) -> Option<u32> {
    let mut session = 0;
    unsafe { ProcessIdToSessionId(pid, &mut session) }.ok()?;
    Some(session)
}

/// Whether the foreground window is fullscreen: exclusive Direct3D
/// fullscreen as reported by the shell, or a window (often borderless)
/// whose client area covers its monitor. The desktop, the taskbar and
/// the task switchers never count. Presentation mode (set by a slide
/// show or by hand) says nothing about the foreground window, so the
/// geometry decides then.
fn fullscreen_for_window(window: HWND) -> Option<bool> {
    if window.is_invalid()
        || window == unsafe { GetShellWindow() }
        || unsafe { IsIconic(window) }.as_bool()
        || is_shell_window(window)
    {
        return Some(false);
    }
    if unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
    {
        return Some(true);
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

pub(crate) fn is_shell_window(window: HWND) -> bool {
    let mut class = [0_u16; 64];
    let length = unsafe { GetClassNameW(window, &mut class) }.max(0) as usize;
    let class = String::from_utf16_lossy(&class[..length]);
    SHELL_WINDOW_CLASSES.contains(&class.as_str())
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
