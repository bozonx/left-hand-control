//! Low-level keyboard (and optional mouse) hooks on a dedicated thread.
//!
//! The keyboard callback only translates the event, queues it for the
//! mapper thread and swallows it: Windows silently removes a hook whose
//! callback exceeds `LowLevelHooksTimeout`, so nothing slow runs here.
//! Injected input passes through untouched, which covers the mapper's own
//! output and other tools. Mouse buttons are observed, never swallowed,
//! like the ungrabbed mouse on Linux.

use super::super::key_code::KeyCode;
use super::super::scancode::from_native;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_EXTENDED, LLKHF_INJECTED, LLKHF_UP, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE,
    PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
};

/// Input for the mapper loop.
pub(super) enum HookEvent {
    Key {
        key: KeyCode,
        down: bool,
        at: Instant,
    },
    /// Auto-repeat of a key that is already down.
    Repeat(KeyCode),
    /// Return to the loop so control messages are handled.
    Wake,
}

/// Scan codes Windows marks with this bit are the Left Ctrl it synthesizes
/// for AltGr layouts; injecting AltGr makes Windows synthesize it again.
const FAKE_ALTGR_CTRL: u32 = 0x200;

static SINK: Mutex<Option<mpsc::Sender<HookEvent>>> = Mutex::new(None);

/// Keys down as the keyboard hook saw them; a second press is a repeat.
static DOWN: Mutex<Vec<KeyCode>> = Mutex::new(Vec::new());

/// Forget every key the hook saw pressed and return them. Used when their
/// releases were lost on the secure desktop, so the next press of such a
/// key is a press again and not a repeat.
pub(super) fn forget_down() -> Vec<KeyCode> {
    std::mem::take(&mut *DOWN.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
pub(super) fn mark_down(key: KeyCode) {
    DOWN.lock().unwrap_or_else(|e| e.into_inner()).push(key);
}

fn deliver(event: HookEvent) -> bool {
    SINK.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|sink| sink.send(event).is_ok())
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if !info.flags.contains(LLKHF_INJECTED) {
            if info.scanCode & FAKE_ALTGR_CTRL != 0 {
                // Native AltGr keeps its Ctrl once the mapper loop is gone.
                if SINK.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
                    return LRESULT(1);
                }
                return unsafe { CallNextHookEx(None, code, wparam, lparam) };
            }
            let down = !info.flags.contains(LLKHF_UP);
            if let Some(key) = from_native(
                info.vkCode,
                info.scanCode,
                info.flags.contains(LLKHF_EXTENDED),
            ) {
                let repeat = {
                    let mut keys = DOWN.lock().unwrap_or_else(|e| e.into_inner());
                    let held = keys.contains(&key);
                    if !down {
                        keys.retain(|k| *k != key);
                    } else if !held {
                        keys.push(key);
                    }
                    down && held
                };
                let event = if repeat {
                    HookEvent::Repeat(key)
                } else {
                    HookEvent::Key {
                        key,
                        down,
                        at: Instant::now(),
                    }
                };
                // Without a live mapper loop the key goes to the OS.
                if deliver(event) {
                    return LRESULT(1);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, lparam points to an MSLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if info.flags & LLMHF_INJECTED == 0 {
            let x_button = || match (info.mouseData >> 16) as u16 {
                XBUTTON1 => Some(KeyCode::BTN_SIDE),
                XBUTTON2 => Some(KeyCode::BTN_EXTRA),
                _ => None,
            };
            let event = match wparam.0 as u32 {
                WM_LBUTTONDOWN => Some((KeyCode::BTN_LEFT, true)),
                WM_LBUTTONUP => Some((KeyCode::BTN_LEFT, false)),
                WM_RBUTTONDOWN => Some((KeyCode::BTN_RIGHT, true)),
                WM_RBUTTONUP => Some((KeyCode::BTN_RIGHT, false)),
                WM_MBUTTONDOWN => Some((KeyCode::BTN_MIDDLE, true)),
                WM_MBUTTONUP => Some((KeyCode::BTN_MIDDLE, false)),
                WM_XBUTTONDOWN => x_button().map(|key| (key, true)),
                WM_XBUTTONUP => x_button().map(|key| (key, false)),
                _ => None,
            };
            if let Some((key, down)) = event {
                deliver(HookEvent::Key {
                    key,
                    down,
                    at: Instant::now(),
                });
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Installed hooks; dropping it removes them and stops the hook thread.
pub(super) struct Hooks {
    thread_id: u32,
    join: Option<thread::JoinHandle<()>>,
}

impl Hooks {
    /// Install the hooks; events go to `sink` until the hooks are dropped.
    pub(super) fn install(sink: mpsc::Sender<HookEvent>, mouse: bool) -> Result<Self, String> {
        *SINK.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink);
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        let join = thread::Builder::new()
            .name("lhc-hook".into())
            .spawn(move || run(ready_tx, mouse))
            .map_err(|error| format!("spawn hook thread: {error}"))?;
        let mut hooks = Self {
            thread_id: 0,
            join: Some(join),
        };
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => {
                hooks.thread_id = thread_id;
                Ok(hooks)
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err("keyboard hook thread exited before installing hooks".into()),
        }
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        *SINK.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if self.thread_id != 0 {
            let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run(ready: mpsc::Sender<Result<u32, String>>, mouse: bool) {
    let mut msg = MSG::default();
    // Create this thread's message queue before anyone posts WM_QUIT to it.
    let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
    forget_down();
    let module = unsafe { GetModuleHandleW(None) }
        .ok()
        .map(|module| module.into());
    let keyboard =
        match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0) } {
            Ok(hook) => hook,
            Err(error) => {
                let _ = ready.send(Err(format!("install keyboard hook: {error}")));
                return;
            }
        };
    let mouse_hook: Option<HHOOK> = if mouse {
        match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0) } {
            Ok(hook) => Some(hook),
            Err(error) => {
                let _ = unsafe { UnhookWindowsHookEx(keyboard) };
                let _ = ready.send(Err(format!("install mouse hook: {error}")));
                return;
            }
        }
    } else {
        None
    };
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    // GetMessage returns 0 for WM_QUIT and -1 on error.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    if let Some(hook) = mouse_hook {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
    let _ = unsafe { UnhookWindowsHookEx(keyboard) };
}

/// Wake the mapper loop without input.
pub(super) fn wake(sink: &mpsc::Sender<HookEvent>) {
    let _ = sink.send(HookEvent::Wake);
}
