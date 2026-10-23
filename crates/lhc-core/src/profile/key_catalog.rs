//! Key codes the configuration accepts, grouped as the action picker
//! shows them. Names follow the W3C `KeyboardEvent.code` spec; keys it
//! does not cover use Chromium's code name or `DomCode` identifier
//! (`dom_code_data.inc`) and, failing that, a name derived from the Linux
//! `KEY_*` code.

/// Key groups in picker order; the shell maps each index to a
/// `PickerCategory`.
pub const CATEGORIES: &[&[&str]] = &[
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
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
        "F9",
        "F10",
        "F11",
        "F12",
        "F13",
        "F14",
        "F15",
        "F16",
        "F17",
        "F18",
        "F19",
        "F20",
        "F21",
        "F22",
        "F23",
        "F24",
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
        "NumpadParenLeft",
        "NumpadParenRight",
        "NumpadEnter",
    ],
    // Media, browser and launch keys.
    &[
        "AudioVolumeUp",
        "AudioVolumeDown",
        "AudioVolumeMute",
        "MicrophoneMuteToggle",
        "MediaPlayPause",
        "MediaPlay",
        "MediaPause",
        "MediaStop",
        "MediaTrackNext",
        "MediaTrackPrevious",
        "MediaRewind",
        "MediaFastForward",
        "MediaRecord",
        "MediaSelect",
        "LaunchMediaPlayer",
        "Eject",
        "BrowserBack",
        "BrowserForward",
        "BrowserRefresh",
        "BrowserStop",
        "BrowserHome",
        "BrowserSearch",
        "BrowserFavorites",
        "LaunchApp1",
        "LaunchApp2",
        "LaunchMail",
        "LaunchInternetBrowser",
        "LaunchControlPanel",
        "LaunchScreenSaver",
        "LaunchAssistant",
        "LaunchCamera",
        "LaunchDocuments",
        "LaunchCalendar",
        "ZoomIn",
        "ZoomOut",
        "ZoomToggle",
    ],
    // System and laptop keys.
    &[
        "Power",
        "Sleep",
        "WakeUp",
        "Suspend",
        "LockScreen",
        "LogOff",
        "BrightnessUp",
        "BrightnessDown",
        "BrightnessToggle",
        "BrightnessCycle",
        "BrightnessAuto",
        "BrightnessMinimum",
        "BrightnessMaximum",
        "DisplayOff",
        "DisplayToggleIntExt",
        "RotateDisplay",
        "KeyboardBacklightUp",
        "KeyboardBacklightDown",
        "KeyboardBacklightToggle",
        "TouchpadToggle",
        "TouchpadOn",
        "TouchpadOff",
        "CameraAccessToggle",
        "PrivacyScreenToggle",
        "WLAN",
        "Bluetooth",
        "AirplaneMode",
        "WWAN",
        "UWB",
        "Battery",
        "Fn",
        "FnLock",
        "KeyboardLayoutSelect",
        "EmojiPicker",
        "Dictate",
        "SpeechInputToggle",
        "Accessibility",
        "DoNotDisturb",
        "NotificationCenter",
        "SelectiveScreenshot",
        "SelectTask",
        "ShowAllWindows",
        "AllApplications",
        "CycleWindows",
    ],
    // Mouse buttons.
    &[
        "MouseLeft",
        "MouseRight",
        "MouseMiddle",
        "MouseBack",
        "MouseForward",
        "Mouse6",
        "Mouse7",
        "Mouse8",
    ],
    // Legacy editing, office, programmable, international and CD keys.
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
        "Menu",
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
        "New",
        "Close",
        "Save",
        "Print",
        "Cancel",
        "Exit",
        "Edit",
        "Move",
        "MailSend",
        "MailReply",
        "MailForward",
        "SendFile",
        "DeleteFile",
        "Transfer",
        "Macro",
        "ScrollUp",
        "ScrollDown",
        "LaunchComputer",
        "LaunchWordProcessor",
        "LaunchSpreadsheet",
        "LaunchContacts",
        "LaunchAudioBrowser",
        "LaunchChat",
        "LaunchPlayer",
        "LaunchPhone",
        "LaunchFinance",
        "LaunchSports",
        "LaunchShop",
        "LaunchKeyboardLayout",
        "SpellCheck",
        "PlayCD",
        "CloseCD",
        "EjectCloseCD",
        "BassBoost",
        "VideoNext",
        "VideoPrev",
        "CameraFocus",
        "CameraZoomIn",
        "CameraZoomOut",
        "CameraUp",
        "CameraDown",
        "CameraLeft",
        "CameraRight",
    ],
];

pub fn valid_key(value: &str) -> bool {
    CATEGORIES.iter().any(|keys| keys.contains(&value))
}

/// Key code a chord modifier stands for: a side-neutral `Ctrl`, `Shift`,
/// `Alt` or `Meta` means the left key, a sided code means itself.
pub fn modifier_key(token: &str) -> Option<&'static str> {
    Some(match token {
        "Ctrl" | "ControlLeft" => "ControlLeft",
        "ControlRight" => "ControlRight",
        "Shift" | "ShiftLeft" => "ShiftLeft",
        "ShiftRight" => "ShiftRight",
        "Alt" | "AltLeft" => "AltLeft",
        "AltRight" => "AltRight",
        "Meta" | "MetaLeft" => "MetaLeft",
        "MetaRight" => "MetaRight",
        _ => return None,
    })
}

/// Entries of a comma-separated key list (`isolate`) that are
/// not catalog key codes.
pub fn unknown_keys(list: &str) -> impl Iterator<Item = &str> {
    list.split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty() && !valid_key(code))
}

pub fn valid_trigger(value: &str) -> bool {
    trigger_keys(value).is_some()
}

pub fn trigger_keys(value: &str) -> Option<Vec<&str>> {
    let mut keys = Vec::new();
    for part in value.split('+').map(str::trim) {
        let key = modifier_key(part).unwrap_or(part);
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
        for value in [
            "",
            "NotAKey",
            "MouseLeft",
            "MouseRight",
            "MouseMiddle",
            "Control+KeyA",
            "Super+KeyA",
            "Win+KeyA",
            "AltGr+KeyA",
        ] {
            assert!(!valid_trigger(value));
        }
    }

    #[test]
    fn modifiers_and_key_lists() {
        assert_eq!(modifier_key("Ctrl"), Some("ControlLeft"));
        assert_eq!(modifier_key("MetaRight"), Some("MetaRight"));
        for token in [
            "Control", "Super", "Win", "Cmd", "AltGr", "CapsLock", "KeyA",
        ] {
            assert_eq!(modifier_key(token), None);
        }
        assert_eq!(unknown_keys("KeyA, Tab,, NumpadParenLeft").count(), 0);
        assert_eq!(unknown_keys("KeyA, KeyTab").collect::<Vec<_>>(), ["KeyTab"]);
    }

    #[test]
    fn complete_catalog_has_unique_key_codes() {
        assert_eq!(
            CATEGORIES.iter().map(|c| c.len()).collect::<Vec<_>>(),
            [48, 9, 35, 29, 36, 44, 8, 69]
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
