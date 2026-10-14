// Parse user-facing action strings (from config.json) into a concrete
// sequence of key events the mapper should emit.

#![cfg(target_os = "linux")]

use super::keys::code_to_key;
use super::system::{SysAction, SysCommand};
use evdev::KeyCode;
use std::time::Duration;

/// A keystroke = optional modifiers + one main key.
#[derive(Clone, Debug)]
pub struct Keystroke {
    pub mods: Vec<KeyCode>,
    pub key: KeyCode,
}

/// A single step of a macro. Macros may mix raw keystrokes with
/// system/app invocations (e.g. `sys:switchDesktop1`, `app:showQuickMenu1`) and explicit
/// text literals (`text:TODO: `).
#[derive(Clone, Debug)]
pub enum MacroStepItem {
    Stroke(Keystroke),
    System(SysAction),
    Command(SysCommand),
    Literal(String),
    Pause(Duration),
}

pub fn explicit_text(action: &str) -> Option<String> {
    action
        .trim_start()
        .strip_prefix("text:")
        .map(|text| text.to_string())
}

pub fn explicit_pause(action: &str) -> Option<Duration> {
    let raw = action.trim();
    let ms = raw.strip_prefix("pause:")?.parse::<u64>().ok()?;
    if ms > 10_000 {
        return None;
    }
    Some(Duration::from_millis(ms))
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
        if let Some((key_part, mod_parts)) = parts.split_last()
            && !mod_parts.is_empty()
        {
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

fn parse_modifier(token: &str) -> Option<KeyCode> {
    crate::profile::key_catalog::modifier_key(token).and_then(code_to_key)
}

fn parse_single(token: &str) -> Option<KeyCode> {
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
    fn explicit_text_preserves_payload_whitespace() {
        assert_eq!(explicit_text("  text:hello  "), Some("hello  ".into()));
        for text in [" ", "   ", "  a  b  ", "a\nb\n", "\t\n", "Привет 👋"] {
            assert_eq!(explicit_text(&format!("text:{text}")), Some(text.into()));
        }
    }

    #[test]
    fn parse_single_key() {
        let k = parse_action("Escape").unwrap();
        assert_eq!(k.key, KeyCode::KEY_ESC);
        assert!(k.mods.is_empty());
    }

    #[test]
    fn parse_modifier_plus_key() {
        let k = parse_action("Ctrl+KeyC").unwrap();
        assert_eq!(k.key, KeyCode::KEY_C);
        assert_eq!(k.mods, vec![KeyCode::KEY_LEFTCTRL]);
    }

    #[test]
    fn parse_multiple_modifiers() {
        let k = parse_action("Ctrl+Shift+Tab").unwrap();
        assert_eq!(k.key, KeyCode::KEY_TAB);
        assert_eq!(k.mods, vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT]);
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
        assert_eq!(parse_modifier("Ctrl"), Some(KeyCode::KEY_LEFTCTRL));
        assert_eq!(parse_modifier("ControlLeft"), Some(KeyCode::KEY_LEFTCTRL));
        assert_eq!(parse_modifier("ControlRight"), Some(KeyCode::KEY_RIGHTCTRL));
        assert_eq!(parse_modifier("Shift"), Some(KeyCode::KEY_LEFTSHIFT));
        assert_eq!(parse_modifier("ShiftLeft"), Some(KeyCode::KEY_LEFTSHIFT));
        assert_eq!(parse_modifier("ShiftRight"), Some(KeyCode::KEY_RIGHTSHIFT));
        assert_eq!(parse_modifier("Alt"), Some(KeyCode::KEY_LEFTALT));
        assert_eq!(parse_modifier("AltLeft"), Some(KeyCode::KEY_LEFTALT));
        assert_eq!(parse_modifier("AltRight"), Some(KeyCode::KEY_RIGHTALT));
        assert_eq!(parse_modifier("Meta"), Some(KeyCode::KEY_LEFTMETA));
        assert_eq!(parse_modifier("MetaLeft"), Some(KeyCode::KEY_LEFTMETA));
        assert_eq!(parse_modifier("MetaRight"), Some(KeyCode::KEY_RIGHTMETA));
    }

    #[test]
    fn parse_unknown_modifier_returns_none() {
        for token in ["Cmd", "AltGr", "Control", "Super", "Win"] {
            assert!(parse_modifier(token).is_none());
        }
    }
}
