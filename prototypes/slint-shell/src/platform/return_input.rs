#[cfg(target_os = "macos")]
use enigo::{Enigo, Keyboard, Settings};

#[derive(Default)]
pub struct ReturnInput {
    target: Option<Target>,
}

impl ReturnInput {
    pub fn capture(&mut self) {
        self.target = capture_target();
    }

    pub fn selected(&mut self, text: String) {
        let Some(target) = self.target.take() else {
            log::error!("native return input has no captured target");
            return;
        };
        slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
            if let Err(error) = restore_target(target) {
                log::error!("native focus restore failed: {error}");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Err(error) = type_text(&text) {
                log::error!("native return input unavailable: {error}");
            }
        });
    }
}

#[cfg(target_os = "windows")]
type Target = isize;

#[cfg(target_os = "windows")]
fn capture_target() -> Option<Target> {
    let window = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    (!window.0.is_null()).then_some(window.0 as isize)
}

#[cfg(target_os = "windows")]
fn restore_target(target: Target) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::SetForegroundWindow};
    let restored = unsafe { SetForegroundWindow(HWND(target as *mut _)) }.as_bool();
    restored
        .then_some(())
        .ok_or_else(|| "SetForegroundWindow rejected the target".into())
}

#[cfg(target_os = "windows")]
fn type_text(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let previous = clipboard_text();
    set_clipboard_text(text)?;
    paste()?;
    std::thread::sleep(std::time::Duration::from_millis(150));
    if let Some(previous) = previous {
        set_clipboard_text(&previous)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn paste() -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_V,
    };

    let keyboard_input = |key, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [
        keyboard_input(VK_CONTROL, Default::default()),
        keyboard_input(VK_V, Default::default()),
        keyboard_input(VK_V, KEYEVENTF_KEYUP),
        keyboard_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(format!("SendInput accepted {sent} of {} events", inputs.len()).into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn clipboard_text() -> Option<String> {
    use windows::Win32::{
        Foundation::HGLOBAL,
        System::{
            DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
            Ole::CF_UNICODETEXT,
        },
    };

    unsafe {
        OpenClipboard(None).ok()?;
        let result = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            let memory = HGLOBAL(handle.0);
            let pointer = GlobalLock(memory).cast::<u16>();
            if pointer.is_null() {
                return None;
            }
            let capacity = GlobalSize(memory) / std::mem::size_of::<u16>();
            let slice = std::slice::from_raw_parts(pointer, capacity);
            let length = slice.iter().position(|unit| *unit == 0).unwrap_or(capacity);
            let text = String::from_utf16(slice.get(..length)?).ok();
            let _ = GlobalUnlock(memory);
            text
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(target_os = "windows")]
fn set_clipboard_text(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::{
        Foundation::HANDLE,
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GHND, GlobalAlloc, GlobalLock, GlobalUnlock},
            Ole::CF_UNICODETEXT,
        },
    };

    let encoded = text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        OpenClipboard(None)?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            EmptyClipboard()?;
            let memory = GlobalAlloc(GHND, encoded.len() * std::mem::size_of::<u16>())?;
            let pointer = GlobalLock(memory).cast::<u16>();
            if pointer.is_null() {
                return Err("GlobalLock rejected clipboard allocation".into());
            }
            std::ptr::copy_nonoverlapping(encoded.as_ptr(), pointer, encoded.len());
            let _ = GlobalUnlock(memory);
            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(memory.0)))?;
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(target_os = "macos")]
type Target = i32;

#[cfg(target_os = "macos")]
fn capture_target() -> Option<Target> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get unix id of first application process whose frontmost is true",
        ])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .flatten()
}

#[cfg(target_os = "macos")]
fn restore_target(target: Target) -> Result<(), Box<dyn std::error::Error>> {
    let script = format!(
        "tell application \"System Events\" to set frontmost of first application process whose unix id is {target} to true"
    );
    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("osascript exited with {status}").into())
}

#[cfg(target_os = "macos")]
fn type_text(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.text(text)?;
    Ok(())
}
