// Parse user-facing action strings (from config.json) into a concrete
// sequence of key events the mapper should emit.

#![cfg(target_os = "linux")]

use super::keys::{char_to_key, code_to_key};
use super::system::SysAction;
use evdev::Key;

/// A keystroke = optional modifiers + one main key.
#[derive(Clone, Debug)]
pub struct Keystroke {
    pub mods: Vec<Key>,
    pub key: Key,
}

/// A single step of a macro. Macros may mix raw keystrokes with
/// system-function invocations (e.g. `sys:switchDesktop1`) and layout-aware
/// character literals (resolved at execution time against the current XKB
/// layout, so they behave correctly on non-US keyboards).
#[derive(Clone, Debug)]
pub enum MacroStepItem {
    Stroke(Keystroke),
    System(SysAction),
    Literal(String),
}

/// Heuristic: if a raw action string is exactly one Unicode character, we
/// treat it as a literal to be resolved later against the active XKB
/// layout. Multi-character tokens (e.g. "Escape", "Ctrl+C") keep going
/// through `parse_action` / `code_to_key`.
pub fn literal_text(action: &str) -> Option<String> {
    let trimmed = action.trim();
    let mut it = trimmed.chars();
    let first = it.next()?;
    if it.next().is_none() {
        return Some(first.to_string());
    }
    if trimmed.contains('+') {
        return None;
    }
    if code_to_key(trimmed).is_some() {
        return None;
    }
    if trimmed.is_ascii() {
        return None;
    }
    Some(trimmed.to_string())
}

/// Parse an action like:
///   "Escape", "Left", "Ctrl+C", "Shift+Tab", "!", "@", "a"
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

    // Single token.
    if trimmed.chars().count() == 1 {
        let c = trimmed.chars().next().unwrap();
        if let Some((shift, key)) = char_to_key(c) {
            let mods = if shift {
                vec![Key::KEY_LEFTSHIFT]
            } else {
                vec![]
            };
            return Some(Keystroke { mods, key });
        }
    }
    let key = parse_single(trimmed)?;
    Some(Keystroke { mods: vec![], key })
}

fn parse_modifier(token: &str) -> Option<Key> {
    Some(match token {
        "Ctrl" | "Control" | "ControlLeft" => Key::KEY_LEFTCTRL,
        "ControlRight" => Key::KEY_RIGHTCTRL,
        "Shift" | "ShiftLeft" => Key::KEY_LEFTSHIFT,
        "ShiftRight" => Key::KEY_RIGHTSHIFT,
        "Alt" | "AltLeft" => Key::KEY_LEFTALT,
        "AltRight" | "AltGr" => Key::KEY_RIGHTALT,
        "Meta" | "Super" | "Win" | "MetaLeft" => Key::KEY_LEFTMETA,
        "MetaRight" => Key::KEY_RIGHTMETA,
        _ => return None,
    })
}

fn parse_single(token: &str) -> Option<Key> {
    if let Some(k) = code_to_key(token) {
        return Some(k);
    }
    // Fallback: treat single char.
    if token.chars().count() == 1 {
        let ch = token.chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            return code_to_key(&format!("Key{}", ch.to_ascii_uppercase()));
        }
        let (_, k) = char_to_key(ch)?;
        return Some(k);
    }
    None
}
