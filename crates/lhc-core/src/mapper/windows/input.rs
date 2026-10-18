//! `SendInput` helpers: key transitions, system shortcuts and Unicode text.
//! Every event carries [`INJECTED_MARK`]; the hook ignores injected input,
//! so the mapper never processes its own output.

use super::super::key_code::KeyCode;
use super::super::scancode::{NativeKey, to_native};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, SendInput,
    VIRTUAL_KEY, VK_RETURN, VK_TAB,
};

/// `dwExtraInfo` of events the mapper injects ("LHC!").
pub(super) const INJECTED_MARK: usize = 0x4c48_4321;

/// Unassigned virtual key sent before releasing Win or Alt on their own, so
/// the release does not open the Start menu or a menu bar.
const MASK_KEY: u16 = 0xe8;

/// Modifiers with their virtual keys, as `GetAsyncKeyState` reports them.
const MODIFIERS: [(KeyCode, i32); 8] = [
    (KeyCode::KEY_LEFTSHIFT, 0xa0),
    (KeyCode::KEY_RIGHTSHIFT, 0xa1),
    (KeyCode::KEY_LEFTCTRL, 0xa2),
    (KeyCode::KEY_RIGHTCTRL, 0xa3),
    (KeyCode::KEY_LEFTALT, 0xa4),
    (KeyCode::KEY_RIGHTALT, 0xa5),
    (KeyCode::KEY_LEFTMETA, 0x5b),
    (KeyCode::KEY_RIGHTMETA, 0x5c),
];

fn keyboard(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECTED_MARK,
            },
        },
    }
}

/// One key transition, or `None` for keys without a Windows equivalent.
pub(super) fn key(key: KeyCode, down: bool) -> Option<INPUT> {
    let up = if down {
        KEYBD_EVENT_FLAGS(0)
    } else {
        KEYEVENTF_KEYUP
    };
    let extended = |extended: bool| {
        if extended {
            KEYEVENTF_EXTENDEDKEY
        } else {
            KEYBD_EVENT_FLAGS(0)
        }
    };
    Some(match to_native(key)? {
        NativeKey::Scan { code, extended: e } => {
            keyboard(0, code, KEYEVENTF_SCANCODE | extended(e) | up)
        }
        NativeKey::Virtual {
            vk,
            scan,
            extended: e,
        } => keyboard(vk, scan, extended(e) | up),
    })
}

/// Inject `inputs`; partial injection is an error (UIPI or a secure
/// desktop can block input).
pub(super) fn send(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(format!(
            "SendInput accepted {sent} of {} events: {}",
            inputs.len(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn is_down(vk: i32) -> bool {
    // The high bit is the current state; physical keys are swallowed by
    // the hook, so this is the state the mapper injected.
    (unsafe { GetAsyncKeyState(vk) }) < 0
}

/// Press `mods` + `key` as a system shortcut. Modifiers held at that moment
/// are released first and pressed again afterwards, so a held Ctrl does not
/// turn Win+D into another shortcut.
pub(super) fn shortcut(mods: &[KeyCode], main: KeyCode) -> Result<(), String> {
    let held: Vec<KeyCode> = MODIFIERS
        .iter()
        .filter(|(_, vk)| is_down(*vk))
        .map(|(key, _)| *key)
        .collect();
    let mut inputs = Vec::new();
    if held.iter().any(|key| {
        matches!(
            *key,
            KeyCode::KEY_LEFTMETA
                | KeyCode::KEY_RIGHTMETA
                | KeyCode::KEY_LEFTALT
                | KeyCode::KEY_RIGHTALT
        )
    }) {
        inputs.push(keyboard(MASK_KEY, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(keyboard(MASK_KEY, 0, KEYEVENTF_KEYUP));
    }
    inputs.extend(held.iter().filter_map(|k| key(*k, false)));
    inputs.extend(mods.iter().filter_map(|k| key(*k, true)));
    inputs.extend(key(main, true));
    inputs.extend(key(main, false));
    inputs.extend(mods.iter().rev().filter_map(|k| key(*k, false)));
    inputs.extend(held.iter().filter_map(|k| key(*k, true)));
    send(&inputs)
}

/// Type `text` as Unicode characters, independent of the keyboard layout.
/// `valid` is checked before each character so a focus change stops input.
pub fn type_unicode(text: &str, valid: impl Fn() -> bool) -> Result<(), String> {
    if text.contains('\0') {
        return Err("NUL cannot be typed into native text fields".into());
    }
    let text = text.replace("\r\n", "\n");
    for ch in text.chars() {
        if !valid() {
            return Err("text input cancelled: the focused window changed".into());
        }
        let mut units = [0; 2];
        let inputs: Vec<INPUT> = match ch {
            '\n' | '\r' | '\t' => {
                let vk = if ch == '\t' { VK_TAB } else { VK_RETURN };
                vec![
                    keyboard(vk.0, 0, KEYBD_EVENT_FLAGS(0)),
                    keyboard(vk.0, 0, KEYEVENTF_KEYUP),
                ]
            }
            _ => ch
                .encode_utf16(&mut units)
                .iter()
                .flat_map(|unit| {
                    [
                        keyboard(0, *unit, KEYEVENTF_UNICODE),
                        keyboard(0, *unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                    ]
                })
                .collect(),
        };
        if let Err(error) = send(&inputs) {
            // Release any accepted key-down before reporting partial injection.
            let releases: Vec<INPUT> = inputs.into_iter().skip(1).step_by(2).collect();
            let _ = send(&releases);
            return Err(error);
        }
    }
    Ok(())
}
