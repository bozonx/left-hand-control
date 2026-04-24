// Parse user-facing action strings (from config.json) into a concrete
// sequence of key events the mapper should emit.

#![cfg(target_os = "linux")]

use super::keys::code_to_key;
use super::system::{SysAction, SysCommand};
use evdev::Key;

/// A keystroke = optional modifiers + one main key.
#[derive(Clone, Debug)]
pub struct Keystroke {
    pub mods: Vec<Key>,
    pub key: Key,
}

/// A single step of a macro. Macros may mix raw keystrokes with
/// system-function invocations (e.g. `sys:switchDesktop1`) and explicit
/// text literals (`text:TODO: `).
#[derive(Clone, Debug)]
pub enum MacroStepItem {
    Stroke(Keystroke),
    System(SysAction),
    Command(SysCommand),
    Literal(String),
}

pub fn explicit_text(action: &str) -> Option<String> {
    action
        .trim()
        .strip_prefix("text:")
        .map(|text| text.to_string())
}

/// Parse an action like:
///   "Escape", "ArrowLeft", "Ctrl+KeyC", "Shift+Tab", "Digit1"
///
/// Returns None if the action is unknown / not supported yet.
pub fn parse_action(action: &str) -> Option<Keystroke> {
    let trimmed = action.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Handle "Mod+Mod+Key" combos.
    if trimmed.contains('+') && trimmed.len() > 1 {
        let parts: Vec<&str> = trimmed.split('+').map(|p| p.trim()).collect();
        if parts.len() >= 2 {
            let (key_part, mod_parts) = parts.split_last().unwrap();
            let mut mods = Vec::with_capacity(mod_parts.len());
            for m in mod_parts {
                let k = parse_modifier(m)?;
                mods.push(k);
            }
            let key = parse_single(key_part)?;
            return Some(Keystroke { mods, key });
        }
    }

    let key = parse_single(trimmed)?;
    Some(Keystroke { mods: vec![], key })
}

fn parse_modifier(token: &str) -> Option<Key> {
    Some(match token {
        "Ctrl" => Key::KEY_LEFTCTRL,
        "ControlLeft" => Key::KEY_LEFTCTRL,
        "ControlRight" => Key::KEY_RIGHTCTRL,
        "Shift" => Key::KEY_LEFTSHIFT,
        "ShiftLeft" => Key::KEY_LEFTSHIFT,
        "ShiftRight" => Key::KEY_RIGHTSHIFT,
        "Alt" => Key::KEY_LEFTALT,
        "AltLeft" => Key::KEY_LEFTALT,
        "AltRight" => Key::KEY_RIGHTALT,
        "AltGr" => Key::KEY_RIGHTALT,
        "Meta" => Key::KEY_LEFTMETA,
        "MetaLeft" => Key::KEY_LEFTMETA,
        "MetaRight" => Key::KEY_RIGHTMETA,
        _ => return None,
    })
}

fn parse_single(token: &str) -> Option<Key> {
    code_to_key(token)
}
