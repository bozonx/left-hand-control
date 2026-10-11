//! Key codes the configuration accepts, grouped as the action picker
//! shows them. Codes follow `KeyboardEvent.code` where it names the key.

/// Key groups in picker order; the shell maps each index to a
/// `PickerCategory`.
pub const CATEGORIES: &[&[&str]] = &[
    // Modifiers.
    &[
        "ShiftLeft",
        "ShiftRight",
        "ControlLeft",
        "ControlRight",
        "AltLeft",
        "AltRight",
        "MetaLeft",
        "MetaRight",
        "CapsLock",
    ],
    // Letters, digits and symbols of the main block.
    &[
        "KeyA",
        "KeyB",
        "KeyC",
        "KeyD",
        "KeyE",
        "KeyF",
        "KeyG",
        "KeyH",
        "KeyI",
        "KeyJ",
        "KeyK",
        "KeyL",
        "KeyM",
        "KeyN",
        "KeyO",
        "KeyP",
        "KeyQ",
        "KeyR",
        "KeyS",
        "KeyT",
        "KeyU",
        "KeyV",
        "KeyW",
        "KeyX",
        "KeyY",
        "KeyZ",
        "Digit0",
        "Digit1",
        "Digit2",
        "Digit3",
        "Digit4",
        "Digit5",
        "Digit6",
        "Digit7",
        "Digit8",
        "Digit9",
        "Backquote",
        "Minus",
        "Equal",
        "BracketLeft",
        "BracketRight",
        "Backslash",
        "Semicolon",
        "Quote",
        "Comma",
        "Period",
        "Slash",
        "IntlBackslash",
    ],
    // Control keys.
    &[
        "Escape",
        "Tab",
        "Enter",
        "Backspace",
        "Space",
        "Insert",
        "Delete",
        "ContextMenu",
        "PrintScreen",
        "ScrollLock",
        "Pause",
    ],
    // Navigation and numpad.
    &[
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "NumLock",
        "Numpad0",
        "Numpad1",
        "Numpad2",
        "Numpad3",
        "Numpad4",
        "Numpad5",
        "Numpad6",
        "Numpad7",
        "Numpad8",
        "Numpad9",
        "NumpadAdd",
        "NumpadSubtract",
        "NumpadMultiply",
        "NumpadDivide",
        "NumpadDecimal",
        "NumpadComma",
        "NumpadEqual",
        "NumpadEnter",
    ],
    // Function keys.
    &[
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14",
        "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
    ],
    // Media, browser and launch keys.
    &[
        "VolumeUp",
        "VolumeDown",
        "VolumeMute",
        "MicMute",
        "MediaPlayPause",
        "MediaStop",
        "MediaNext",
        "MediaPrev",
        "MediaRecord",
        "MediaRewind",
        "MediaFastForward",
        "MediaSelect",
        "BrowserBack",
        "BrowserForward",
        "BrowserRefresh",
        "BrowserStop",
        "BrowserHome",
        "BrowserSearch",
        "BrowserFavorites",
        "LaunchBrowser",
        "LaunchMail",
        "Calculator",
        "LaunchApp1",
    ],
    // System and laptop keys.
    &[
        "Power",
        "Sleep",
        "WakeUp",
        "Eject",
        "BrightnessDown",
        "BrightnessUp",
        "KeyboardBrightnessToggle",
        "KeyboardBrightnessDown",
        "KeyboardBrightnessUp",
        "DisplayToggle",
        "DisplayToggleIntExt",
        "TouchpadToggle",
        "Bluetooth",
        "WLAN",
        "AirplaneMode",
        "Fn",
        "FnLock",
    ],
    // Mouse buttons.
    &[
        "MouseLeft",
        "MouseRight",
        "MouseMiddle",
        "MouseSide",
        "MouseExtra",
        "MouseForward",
        "MouseBack",
        "MouseTask",
    ],
    // Legacy editing, programmable and international keys.
    &[
        "Help",
        "Undo",
        "Redo",
        "Again",
        "Find",
        "Open",
        "Props",
        "Select",
        "Copy",
        "Cut",
        "Paste",
        "Prog1",
        "Prog2",
        "Prog3",
        "Prog4",
        "IntlYen",
        "IntlRo",
        "KanaMode",
        "Convert",
        "NonConvert",
        "Lang1",
        "Lang2",
        "Lang3",
        "Lang4",
        "Lang5",
    ],
];

pub fn valid_key(value: &str) -> bool {
    CATEGORIES.iter().any(|keys| keys.contains(&value))
}

pub fn valid_trigger(value: &str) -> bool {
    trigger_keys(value).is_some()
}

pub fn trigger_keys(value: &str) -> Option<Vec<&str>> {
    let mut keys = Vec::new();
    for part in value.split('+').map(str::trim) {
        let key = match part {
            "Ctrl" | "Control" => "ControlLeft",
            "Shift" => "ShiftLeft",
            "Alt" => "AltLeft",
            "Meta" | "Super" | "Win" => "MetaLeft",
            key => key,
        };
        if !valid_key(key)
            || matches!(key, "MouseLeft" | "MouseRight" | "MouseMiddle")
            || keys.contains(&key)
        {
            return None;
        }
        keys.push(key);
    }
    Some(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn required_keys_and_rule_triggers() {
        assert!(valid_key("MouseLeft"));
        assert!(valid_trigger("F13"));
        assert!(valid_trigger("Ctrl+KeyA"));
        assert!(valid_trigger("ShiftLeft+ControlRight"));
        assert!(!valid_trigger("Ctrl+ControlLeft"));
        assert!(!valid_trigger("KeyA+"));
        for value in ["", "NotAKey", "MouseLeft", "MouseRight", "MouseMiddle"] {
            assert!(!valid_trigger(value));
        }
    }

    #[test]
    fn complete_catalog_has_unique_key_codes() {
        assert_eq!(
            CATEGORIES.iter().map(|c| c.len()).collect::<Vec<_>>(),
            [15, 13, 38, 29, 24, 23, 17, 8, 25]
        );
        let keys: Vec<_> = CATEGORIES
            .iter()
            .flat_map(|keys| keys.iter())
            .copied()
            .collect();
        let unique: std::collections::HashSet<_> = keys.iter().copied().collect();
        assert_eq!(keys.len(), unique.len());
    }
}
