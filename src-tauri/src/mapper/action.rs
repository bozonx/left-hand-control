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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_text_extracts_after_prefix() {
        assert_eq!(explicit_text("text:hello"), Some("hello".into()));
    }

    #[test]
    fn explicit_text_returns_none_without_prefix() {
        assert_eq!(explicit_text("hello"), None);
    }

    #[test]
    fn explicit_text_trims_input() {
        assert_eq!(explicit_text("  text:hello  "), Some("hello".into()));
    }

    #[test]
    fn parse_single_key() {
        let k = parse_action("Escape").unwrap();
        assert_eq!(k.key, Key::KEY_ESC);
        assert!(k.mods.is_empty());
    }

    #[test]
    fn parse_modifier_plus_key() {
        let k = parse_action("Ctrl+KeyC").unwrap();
        assert_eq!(k.key, Key::KEY_C);
        assert_eq!(k.mods, vec![Key::KEY_LEFTCTRL]);
    }

    #[test]
    fn parse_multiple_modifiers() {
        let k = parse_action("Ctrl+Shift+Tab").unwrap();
        assert_eq!(k.key, Key::KEY_TAB);
        assert_eq!(k.mods, vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTSHIFT]);
    }

    #[test]
    fn parse_unknown_key_returns_none() {
        assert!(parse_action("UnknownKey").is_none());
    }

    #[test]
    fn parse_empty_returns_none() {
        assert!(parse_action("").is_none());
    }

    #[test]
    fn parse_modifier_aliases() {
        assert_eq!(parse_modifier("Ctrl"), Some(Key::KEY_LEFTCTRL));
        assert_eq!(parse_modifier("ControlLeft"), Some(Key::KEY_LEFTCTRL));
        assert_eq!(parse_modifier("ControlRight"), Some(Key::KEY_RIGHTCTRL));
        assert_eq!(parse_modifier("Shift"), Some(Key::KEY_LEFTSHIFT));
        assert_eq!(parse_modifier("ShiftLeft"), Some(Key::KEY_LEFTSHIFT));
        assert_eq!(parse_modifier("ShiftRight"), Some(Key::KEY_RIGHTSHIFT));
        assert_eq!(parse_modifier("Alt"), Some(Key::KEY_LEFTALT));
        assert_eq!(parse_modifier("AltLeft"), Some(Key::KEY_LEFTALT));
        assert_eq!(parse_modifier("AltRight"), Some(Key::KEY_RIGHTALT));
        assert_eq!(parse_modifier("AltGr"), Some(Key::KEY_RIGHTALT));
        assert_eq!(parse_modifier("Meta"), Some(Key::KEY_LEFTMETA));
        assert_eq!(parse_modifier("MetaLeft"), Some(Key::KEY_LEFTMETA));
        assert_eq!(parse_modifier("MetaRight"), Some(Key::KEY_RIGHTMETA));
    }

    #[test]
    fn parse_unknown_modifier_returns_none() {
        assert!(parse_modifier("Cmd").is_none());
    }
}
