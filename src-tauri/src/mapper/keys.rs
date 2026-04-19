// Mapping between frontend key codes (KeyboardEvent.code-ish, see utils/keys.ts)
// and Linux evdev `Key` values.

#![cfg(target_os = "linux")]

use evdev::Key;

pub fn code_to_key(code: &str) -> Option<Key> {
    Some(match code {
        // Letters
        "KeyA" => Key::KEY_A, "KeyB" => Key::KEY_B, "KeyC" => Key::KEY_C,
        "KeyD" => Key::KEY_D, "KeyE" => Key::KEY_E, "KeyF" => Key::KEY_F,
        "KeyG" => Key::KEY_G, "KeyH" => Key::KEY_H, "KeyI" => Key::KEY_I,
        "KeyJ" => Key::KEY_J, "KeyK" => Key::KEY_K, "KeyL" => Key::KEY_L,
        "KeyM" => Key::KEY_M, "KeyN" => Key::KEY_N, "KeyO" => Key::KEY_O,
        "KeyP" => Key::KEY_P, "KeyQ" => Key::KEY_Q, "KeyR" => Key::KEY_R,
        "KeyS" => Key::KEY_S, "KeyT" => Key::KEY_T, "KeyU" => Key::KEY_U,
        "KeyV" => Key::KEY_V, "KeyW" => Key::KEY_W, "KeyX" => Key::KEY_X,
        "KeyY" => Key::KEY_Y, "KeyZ" => Key::KEY_Z,

        // Digits
        "Digit0" => Key::KEY_0, "Digit1" => Key::KEY_1, "Digit2" => Key::KEY_2,
        "Digit3" => Key::KEY_3, "Digit4" => Key::KEY_4, "Digit5" => Key::KEY_5,
        "Digit6" => Key::KEY_6, "Digit7" => Key::KEY_7, "Digit8" => Key::KEY_8,
        "Digit9" => Key::KEY_9,

        // F-row
        "F1" => Key::KEY_F1, "F2" => Key::KEY_F2, "F3" => Key::KEY_F3,
        "F4" => Key::KEY_F4, "F5" => Key::KEY_F5, "F6" => Key::KEY_F6,
        "F7" => Key::KEY_F7, "F8" => Key::KEY_F8, "F9" => Key::KEY_F9,
        "F10" => Key::KEY_F10, "F11" => Key::KEY_F11, "F12" => Key::KEY_F12,

        // Navigation / editing
        "Escape" | "Esc" => Key::KEY_ESC,
        "Tab" => Key::KEY_TAB,
        "CapsLock" => Key::KEY_CAPSLOCK,
        "Enter" | "Return" => Key::KEY_ENTER,
        "Backspace" => Key::KEY_BACKSPACE,
        "Space" => Key::KEY_SPACE,
        "Delete" | "Del" => Key::KEY_DELETE,
        "Insert" | "Ins" => Key::KEY_INSERT,
        "Home" => Key::KEY_HOME,
        "End" => Key::KEY_END,
        "PageUp" | "PgUp" => Key::KEY_PAGEUP,
        "PageDown" | "PgDn" => Key::KEY_PAGEDOWN,
        "ArrowLeft" | "Left" => Key::KEY_LEFT,
        "ArrowRight" | "Right" => Key::KEY_RIGHT,
        "ArrowUp" | "Up" => Key::KEY_UP,
        "ArrowDown" | "Down" => Key::KEY_DOWN,

        // Punctuation
        "Backquote" => Key::KEY_GRAVE,
        "Minus" => Key::KEY_MINUS,
        "Equal" => Key::KEY_EQUAL,
        "BracketLeft" => Key::KEY_LEFTBRACE,
        "BracketRight" => Key::KEY_RIGHTBRACE,
        "Backslash" => Key::KEY_BACKSLASH,
        "Semicolon" => Key::KEY_SEMICOLON,
        "Quote" => Key::KEY_APOSTROPHE,
        "Comma" => Key::KEY_COMMA,
        "Period" => Key::KEY_DOT,
        "Slash" => Key::KEY_SLASH,

        // Modifiers
        "ShiftLeft" | "Shift" => Key::KEY_LEFTSHIFT,
        "ShiftRight" => Key::KEY_RIGHTSHIFT,
        "ControlLeft" | "Control" | "Ctrl" => Key::KEY_LEFTCTRL,
        "ControlRight" => Key::KEY_RIGHTCTRL,
        "AltLeft" | "Alt" => Key::KEY_LEFTALT,
        "AltRight" => Key::KEY_RIGHTALT,
        "MetaLeft" | "Meta" | "Super" | "Win" => Key::KEY_LEFTMETA,
        "MetaRight" => Key::KEY_RIGHTMETA,
        "ContextMenu" | "Menu" => Key::KEY_COMPOSE,

        // Media / browser
        "BrowserBack" => Key::KEY_BACK,
        "BrowserForward" => Key::KEY_FORWARD,
        "BrowserRefresh" => Key::KEY_REFRESH,
        "BrowserHome" => Key::KEY_HOMEPAGE,
        "VolumeUp" => Key::KEY_VOLUMEUP,
        "VolumeDown" => Key::KEY_VOLUMEDOWN,
        "VolumeMute" | "Mute" => Key::KEY_MUTE,
        "MediaPlayPause" | "PlayPause" => Key::KEY_PLAYPAUSE,
        "MediaNext" | "Next" => Key::KEY_NEXTSONG,
        "MediaPrev" | "Prev" | "Previous" => Key::KEY_PREVIOUSSONG,

        _ => return None,
    })
}

// Single-character shortcuts (e.g. the symbols in default-layers.yaml).
// Returns (needs_shift, Key).
pub fn char_to_key(ch: char) -> Option<(bool, Key)> {
    Some(match ch {
        // Unshifted
        '`' => (false, Key::KEY_GRAVE),
        '-' => (false, Key::KEY_MINUS),
        '=' => (false, Key::KEY_EQUAL),
        '[' => (false, Key::KEY_LEFTBRACE),
        ']' => (false, Key::KEY_RIGHTBRACE),
        '\\' => (false, Key::KEY_BACKSLASH),
        ';' => (false, Key::KEY_SEMICOLON),
        '\'' => (false, Key::KEY_APOSTROPHE),
        ',' => (false, Key::KEY_COMMA),
        '.' => (false, Key::KEY_DOT),
        '/' => (false, Key::KEY_SLASH),
        ' ' => (false, Key::KEY_SPACE),

        // Shifted
        '~' => (true, Key::KEY_GRAVE),
        '!' => (true, Key::KEY_1),
        '@' => (true, Key::KEY_2),
        '#' => (true, Key::KEY_3),
        '$' => (true, Key::KEY_4),
        '%' => (true, Key::KEY_5),
        '^' => (true, Key::KEY_6),
        '&' => (true, Key::KEY_7),
        '*' => (true, Key::KEY_8),
        '(' => (true, Key::KEY_9),
        ')' => (true, Key::KEY_0),
        '_' => (true, Key::KEY_MINUS),
        '+' => (true, Key::KEY_EQUAL),
        '{' => (true, Key::KEY_LEFTBRACE),
        '}' => (true, Key::KEY_RIGHTBRACE),
        '|' => (true, Key::KEY_BACKSLASH),
        ':' => (true, Key::KEY_SEMICOLON),
        '"' => (true, Key::KEY_APOSTROPHE),
        '<' => (true, Key::KEY_COMMA),
        '>' => (true, Key::KEY_DOT),
        '?' => (true, Key::KEY_SLASH),

        c if c.is_ascii_digit() => (false, match c {
            '0' => Key::KEY_0, '1' => Key::KEY_1, '2' => Key::KEY_2,
            '3' => Key::KEY_3, '4' => Key::KEY_4, '5' => Key::KEY_5,
            '6' => Key::KEY_6, '7' => Key::KEY_7, '8' => Key::KEY_8,
            '9' => Key::KEY_9, _ => unreachable!(),
        }),
        c if c.is_ascii_lowercase() => {
            // Map a..z to KEY_A..KEY_Z via numeric code offset.
            let base = Key::KEY_A.code();
            let key = Key::new(base + (c as u16 - b'a' as u16));
            (false, key)
        }
        c if c.is_ascii_uppercase() => {
            let base = Key::KEY_A.code();
            let key = Key::new(base + (c as u16 - b'A' as u16));
            (true, key)
        }

        _ => return None,
    })
}
