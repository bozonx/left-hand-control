//! Return popup text to the captured native window without using the clipboard.

#[cfg(target_os = "macos")]
use enigo::{Enigo, Keyboard, Settings};

use std::sync::atomic::{AtomicU64, Ordering};

static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct ReturnInput {
    target: Option<Target>,
}

impl ReturnInput {
    pub fn capture(&mut self) {
        GENERATION.fetch_add(1, Ordering::SeqCst);
        let target = capture_target();
        if target
            .as_ref()
            .is_some_and(|target| target.process_id as u32 == std::process::id())
        {
            return;
        }
        self.target = target;
    }

    pub fn discard(&mut self) {
        GENERATION.fetch_add(1, Ordering::SeqCst);
        self.target = None;
    }

    pub fn selected(&mut self, text: String) {
        let Some(target) = self.target.take() else {
            return;
        };
        let selection_context = capture_selection_context();
        let generation = GENERATION.load(Ordering::SeqCst);
        let result = std::thread::Builder::new()
            .name("lhc-native-text".into())
            .spawn(move || {
                let valid = || GENERATION.load(Ordering::SeqCst) == generation;
                std::thread::sleep(std::time::Duration::from_millis(300));
                if !valid() {
                    return;
                }
                if let Err(error) = restore_target(&target, selection_context) {
                    log::error!("native focus restore failed: {error}");
                    return;
                }
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while valid() && target_is_foreground(&target) && !modifiers_released() {
                    if std::time::Instant::now() >= deadline {
                        log::warn!("native text input cancelled: modifiers remain held");
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                let check = || valid() && target_is_foreground(&target) && modifiers_released();
                if !check() {
                    return;
                }
                if let Err(error) = type_text(&text, check) {
                    log::error!("native return input unavailable: {error}");
                }
            });
        if let Err(error) = result {
            log::error!("start native text input: {error}");
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct Target {
    window: isize,
    process_id: u32,
}

#[cfg(target_os = "windows")]
type SelectionContext = Option<isize>;

#[cfg(target_os = "windows")]
fn capture_target() -> Option<Target> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    (process_id != 0).then_some(Target {
        window: window.0 as isize,
        process_id,
    })
}

#[cfg(target_os = "windows")]
fn capture_selection_context() -> SelectionContext {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    (process_id == std::process::id()).then_some(window.0 as isize)
}

#[cfg(target_os = "windows")]
fn restore_target(
    target: &Target,
    selection_context: SelectionContext,
) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow},
    };

    if !target_is_valid(target) {
        return Err("target window is no longer valid".into());
    }
    let foreground = unsafe { GetForegroundWindow() }.0 as isize;
    if foreground != target.window && Some(foreground) != selection_context {
        return Err("foreground changed after selection".into());
    }
    if foreground != target.window {
        let restored = unsafe { SetForegroundWindow(HWND(target.window as *mut _)) }.as_bool();
        if !restored {
            return Err("SetForegroundWindow rejected the target".into());
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while std::time::Instant::now() < deadline {
        if target_is_foreground(target) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err("target did not become foreground".into())
}

#[cfg(target_os = "windows")]
fn target_is_valid(target: &Target) -> bool {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    };

    let window = HWND(target.window as *mut _);
    if !unsafe { IsWindow(Some(window)) }.as_bool() {
        return false;
    }
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    process_id == target.process_id
}

#[cfg(target_os = "windows")]
fn target_is_foreground(target: &Target) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    target_is_valid(target) && unsafe { GetForegroundWindow() }.0 as isize == target.window
}

#[cfg(target_os = "windows")]
fn modifiers_released() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    [0x10, 0x11, 0x12, 0x5b, 0x5c]
        .iter()
        .all(|key| unsafe { GetAsyncKeyState(*key) } >= 0)
}

#[cfg(target_os = "windows")]
fn type_text(text: &str, valid: impl Fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
        VIRTUAL_KEY, VK_RETURN, VK_TAB,
    };

    if text.contains('\0') {
        return Err("NUL cannot be typed into native text fields".into());
    }
    let text = text.replace("\r\n", "\n");
    let keyboard_input = |key, scan, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    for ch in text.chars() {
        if !valid() {
            return Err("native text input cancelled: focus or modifiers changed".into());
        }
        let mut units = [0; 2];
        let inputs: Vec<INPUT> = match ch {
            '\n' | '\r' | '\t' => {
                let key = if ch == '\t' { VK_TAB } else { VK_RETURN };
                vec![
                    keyboard_input(key, 0, Default::default()),
                    keyboard_input(key, 0, KEYEVENTF_KEYUP),
                ]
            }
            _ => ch
                .encode_utf16(&mut units)
                .iter()
                .flat_map(|unit| {
                    [
                        keyboard_input(VIRTUAL_KEY(0), *unit, KEYEVENTF_UNICODE),
                        keyboard_input(VIRTUAL_KEY(0), *unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                    ]
                })
                .collect(),
        };
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 {
            // Release any accepted key-down before reporting partial injection.
            let releases: Vec<INPUT> = inputs.into_iter().skip(1).step_by(2).collect();
            unsafe {
                SendInput(&releases, std::mem::size_of::<INPUT>() as i32);
            }
            return Err(
                format!("SendInput accepted {sent} events; native text input stopped").into(),
            );
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
type Target = lhc_core::platform::NativeTextTarget;

#[cfg(target_os = "macos")]
type SelectionContext = u32;

#[cfg(target_os = "macos")]
fn capture_target() -> Option<Target> {
    lhc_core::platform::capture_text_target()
}

#[cfg(target_os = "macos")]
fn capture_selection_context() -> SelectionContext {
    std::process::id()
}

#[cfg(target_os = "macos")]
fn restore_target(
    target: &Target,
    selection_context: SelectionContext,
) -> Result<(), Box<dyn std::error::Error>> {
    lhc_core::platform::restore_text_target(*target, selection_context).map_err(Into::into)
}

#[cfg(target_os = "macos")]
fn target_is_foreground(target: &Target) -> bool {
    capture_target() == Some(*target)
}

#[cfg(target_os = "macos")]
fn modifiers_released() -> bool {
    lhc_core::platform::text_modifiers_released()
}

#[cfg(target_os = "macos")]
fn type_text(text: &str, valid: impl Fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
    if text.contains('\0') {
        return Err("NUL cannot be typed into native text fields".into());
    }
    let mut enigo = Enigo::new(&Settings::default())?;
    for ch in text.replace("\r\n", "\n").chars() {
        if !valid() {
            return Err("native text input cancelled: focus or modifiers changed".into());
        }
        enigo.text(&ch.to_string())?;
    }
    Ok(())
}
