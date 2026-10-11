// Mapping between configuration key codes (following KeyboardEvent.code)
// and Linux evdev `KeyCode` values.

#![cfg(target_os = "linux")]

use evdev::KeyCode;

// Codes and canonical names newer than the `evdev` crate's constants
// (linux/input-event-codes.h).
const KEY_ALL_APPLICATIONS: KeyCode = KeyCode::new(0x0cc);
const KEY_NOTIFICATION_CENTER: KeyCode = KeyCode::new(0x1bc);
const KEY_EMOJI_PICKER: KeyCode = KeyCode::new(0x249);
const KEY_DICTATE: KeyCode = KeyCode::new(0x24a);
const KEY_CAMERA_ACCESS_TOGGLE: KeyCode = KeyCode::new(0x24d);
const KEY_ACCESSIBILITY: KeyCode = KeyCode::new(0x24e);
const KEY_DO_NOT_DISTURB: KeyCode = KeyCode::new(0x24f);

pub fn code_to_key(code: &str) -> Option<KeyCode> {
    Some(match code {
        // Letters
        "KeyA" => KeyCode::KEY_A,
        "KeyB" => KeyCode::KEY_B,
        "KeyC" => KeyCode::KEY_C,
        "KeyD" => KeyCode::KEY_D,
        "KeyE" => KeyCode::KEY_E,
        "KeyF" => KeyCode::KEY_F,
        "KeyG" => KeyCode::KEY_G,
        "KeyH" => KeyCode::KEY_H,
        "KeyI" => KeyCode::KEY_I,
        "KeyJ" => KeyCode::KEY_J,
        "KeyK" => KeyCode::KEY_K,
        "KeyL" => KeyCode::KEY_L,
        "KeyM" => KeyCode::KEY_M,
        "KeyN" => KeyCode::KEY_N,
        "KeyO" => KeyCode::KEY_O,
        "KeyP" => KeyCode::KEY_P,
        "KeyQ" => KeyCode::KEY_Q,
        "KeyR" => KeyCode::KEY_R,
        "KeyS" => KeyCode::KEY_S,
        "KeyT" => KeyCode::KEY_T,
        "KeyU" => KeyCode::KEY_U,
        "KeyV" => KeyCode::KEY_V,
        "KeyW" => KeyCode::KEY_W,
        "KeyX" => KeyCode::KEY_X,
        "KeyY" => KeyCode::KEY_Y,
        "KeyZ" => KeyCode::KEY_Z,

        // Digits
        "Digit0" => KeyCode::KEY_0,
        "Digit1" => KeyCode::KEY_1,
        "Digit2" => KeyCode::KEY_2,
        "Digit3" => KeyCode::KEY_3,
        "Digit4" => KeyCode::KEY_4,
        "Digit5" => KeyCode::KEY_5,
        "Digit6" => KeyCode::KEY_6,
        "Digit7" => KeyCode::KEY_7,
        "Digit8" => KeyCode::KEY_8,
        "Digit9" => KeyCode::KEY_9,

        // F-row
        "F1" => KeyCode::KEY_F1,
        "F2" => KeyCode::KEY_F2,
        "F3" => KeyCode::KEY_F3,
        "F4" => KeyCode::KEY_F4,
        "F5" => KeyCode::KEY_F5,
        "F6" => KeyCode::KEY_F6,
        "F7" => KeyCode::KEY_F7,
        "F8" => KeyCode::KEY_F8,
        "F9" => KeyCode::KEY_F9,
        "F10" => KeyCode::KEY_F10,
        "F11" => KeyCode::KEY_F11,
        "F12" => KeyCode::KEY_F12,
        "F13" => KeyCode::KEY_F13,
        "F14" => KeyCode::KEY_F14,
        "F15" => KeyCode::KEY_F15,
        "F16" => KeyCode::KEY_F16,
        "F17" => KeyCode::KEY_F17,
        "F18" => KeyCode::KEY_F18,
        "F19" => KeyCode::KEY_F19,
        "F20" => KeyCode::KEY_F20,
        "F21" => KeyCode::KEY_F21,
        "F22" => KeyCode::KEY_F22,
        "F23" => KeyCode::KEY_F23,
        "F24" => KeyCode::KEY_F24,

        // Navigation / editing
        "Escape" => KeyCode::KEY_ESC,
        "Tab" => KeyCode::KEY_TAB,
        "CapsLock" => KeyCode::KEY_CAPSLOCK,
        "Enter" => KeyCode::KEY_ENTER,
        "Backspace" => KeyCode::KEY_BACKSPACE,
        "Space" => KeyCode::KEY_SPACE,
        "Delete" => KeyCode::KEY_DELETE,
        "Insert" => KeyCode::KEY_INSERT,
        "Home" => KeyCode::KEY_HOME,
        "End" => KeyCode::KEY_END,
        "PageUp" => KeyCode::KEY_PAGEUP,
        "PageDown" => KeyCode::KEY_PAGEDOWN,
        "ArrowLeft" => KeyCode::KEY_LEFT,
        "ArrowRight" => KeyCode::KEY_RIGHT,
        "ArrowUp" => KeyCode::KEY_UP,
        "ArrowDown" => KeyCode::KEY_DOWN,
        "PrintScreen" => KeyCode::KEY_SYSRQ,
        "ScrollLock" => KeyCode::KEY_SCROLLLOCK,
        "Pause" => KeyCode::KEY_PAUSE,

        // Punctuation
        "Backquote" => KeyCode::KEY_GRAVE,
        "Minus" => KeyCode::KEY_MINUS,
        "Equal" => KeyCode::KEY_EQUAL,
        "BracketLeft" => KeyCode::KEY_LEFTBRACE,
        "BracketRight" => KeyCode::KEY_RIGHTBRACE,
        "Backslash" => KeyCode::KEY_BACKSLASH,
        "Semicolon" => KeyCode::KEY_SEMICOLON,
        "Quote" => KeyCode::KEY_APOSTROPHE,
        "Comma" => KeyCode::KEY_COMMA,
        "Period" => KeyCode::KEY_DOT,
        "Slash" => KeyCode::KEY_SLASH,

        // Modifiers
        "ShiftLeft" => KeyCode::KEY_LEFTSHIFT,
        "ShiftRight" => KeyCode::KEY_RIGHTSHIFT,
        "ControlLeft" => KeyCode::KEY_LEFTCTRL,
        "ControlRight" => KeyCode::KEY_RIGHTCTRL,
        "AltLeft" => KeyCode::KEY_LEFTALT,
        "AltRight" => KeyCode::KEY_RIGHTALT,
        "MetaLeft" => KeyCode::KEY_LEFTMETA,
        "MetaRight" => KeyCode::KEY_RIGHTMETA,
        "ContextMenu" => KeyCode::KEY_COMPOSE,

        // Numpad
        "NumLock" => KeyCode::KEY_NUMLOCK,
        "Numpad0" => KeyCode::KEY_KP0,
        "Numpad1" => KeyCode::KEY_KP1,
        "Numpad2" => KeyCode::KEY_KP2,
        "Numpad3" => KeyCode::KEY_KP3,
        "Numpad4" => KeyCode::KEY_KP4,
        "Numpad5" => KeyCode::KEY_KP5,
        "Numpad6" => KeyCode::KEY_KP6,
        "Numpad7" => KeyCode::KEY_KP7,
        "Numpad8" => KeyCode::KEY_KP8,
        "Numpad9" => KeyCode::KEY_KP9,
        "NumpadAdd" => KeyCode::KEY_KPPLUS,
        "NumpadSubtract" => KeyCode::KEY_KPMINUS,
        "NumpadMultiply" => KeyCode::KEY_KPASTERISK,
        "NumpadDivide" => KeyCode::KEY_KPSLASH,
        "NumpadDecimal" => KeyCode::KEY_KPDOT,
        "NumpadEnter" => KeyCode::KEY_KPENTER,
        "NumpadEqual" => KeyCode::KEY_KPEQUAL,
        "NumpadComma" => KeyCode::KEY_KPCOMMA,
        "NumpadParenLeft" => KeyCode::KEY_KPLEFTPAREN,
        "NumpadParenRight" => KeyCode::KEY_KPRIGHTPAREN,

        // Mouse buttons
        "MouseLeft" => KeyCode::BTN_LEFT,
        "MouseRight" => KeyCode::BTN_RIGHT,
        "MouseMiddle" => KeyCode::BTN_MIDDLE,
        "MouseSide" => KeyCode::BTN_SIDE,
        "MouseExtra" => KeyCode::BTN_EXTRA,
        "MouseForward" => KeyCode::BTN_FORWARD,
        "MouseBack" => KeyCode::BTN_BACK,
        "MouseTask" => KeyCode::BTN_TASK,

        // Media, browser and launch
        "AudioVolumeUp" => KeyCode::KEY_VOLUMEUP,
        "AudioVolumeDown" => KeyCode::KEY_VOLUMEDOWN,
        "AudioVolumeMute" => KeyCode::KEY_MUTE,
        "MicrophoneMuteToggle" => KeyCode::KEY_MICMUTE,
        "MediaPlayPause" => KeyCode::KEY_PLAYPAUSE,
        "MediaPlay" => KeyCode::KEY_PLAY,
        "MediaPause" => KeyCode::KEY_PAUSECD,
        "MediaStop" => KeyCode::KEY_STOPCD,
        "MediaTrackNext" => KeyCode::KEY_NEXTSONG,
        "MediaTrackPrevious" => KeyCode::KEY_PREVIOUSSONG,
        "MediaRewind" => KeyCode::KEY_REWIND,
        "MediaFastForward" => KeyCode::KEY_FASTFORWARD,
        "MediaRecord" => KeyCode::KEY_RECORD,
        "MediaSelect" => KeyCode::KEY_CONFIG,
        "LaunchMediaPlayer" => KeyCode::KEY_MEDIA,
        "Eject" => KeyCode::KEY_EJECTCD,
        "BrowserBack" => KeyCode::KEY_BACK,
        "BrowserForward" => KeyCode::KEY_FORWARD,
        "BrowserRefresh" => KeyCode::KEY_REFRESH,
        "BrowserStop" => KeyCode::KEY_STOP,
        "BrowserHome" => KeyCode::KEY_HOMEPAGE,
        "BrowserSearch" => KeyCode::KEY_SEARCH,
        "BrowserFavorites" => KeyCode::KEY_BOOKMARKS,
        "LaunchApp1" => KeyCode::KEY_FILE,
        "LaunchApp2" => KeyCode::KEY_CALC,
        "LaunchMail" => KeyCode::KEY_MAIL,
        "LaunchInternetBrowser" => KeyCode::KEY_WWW,
        "LaunchControlPanel" => KeyCode::KEY_CONTROLPANEL,
        "LaunchScreenSaver" => KeyCode::KEY_SCREENSAVER,
        "LaunchAssistant" => KeyCode::KEY_ASSISTANT,
        "LaunchCamera" => KeyCode::KEY_CAMERA,
        "LaunchDocuments" => KeyCode::KEY_DOCUMENTS,
        "LaunchCalendar" => KeyCode::KEY_CALENDAR,
        "ZoomIn" => KeyCode::KEY_ZOOMIN,
        "ZoomOut" => KeyCode::KEY_ZOOMOUT,
        "ZoomToggle" => KeyCode::KEY_FULL_SCREEN,

        // System and laptop
        "Power" => KeyCode::KEY_POWER,
        "Sleep" => KeyCode::KEY_SLEEP,
        "WakeUp" => KeyCode::KEY_WAKEUP,
        "Suspend" => KeyCode::KEY_SUSPEND,
        "LockScreen" => KeyCode::KEY_COFFEE,
        "LogOff" => KeyCode::KEY_LOGOFF,
        "BrightnessUp" => KeyCode::KEY_BRIGHTNESSUP,
        "BrightnessDown" => KeyCode::KEY_BRIGHTNESSDOWN,
        "BrightnessToggle" => KeyCode::KEY_DISPLAYTOGGLE,
        "BrightnessCycle" => KeyCode::KEY_BRIGHTNESS_CYCLE,
        "BrightnessAuto" => KeyCode::KEY_BRIGHTNESS_AUTO,
        "BrightnessMinimum" => KeyCode::KEY_BRIGHTNESS_MIN,
        "BrightnessMaximum" => KeyCode::KEY_BRIGHTNESS_MAX,
        "DisplayOff" => KeyCode::KEY_DISPLAY_OFF,
        "DisplayToggleIntExt" => KeyCode::KEY_SWITCHVIDEOMODE,
        "RotateDisplay" => KeyCode::KEY_ROTATE_DISPLAY,
        "KeyboardBacklightUp" => KeyCode::KEY_KBDILLUMUP,
        "KeyboardBacklightDown" => KeyCode::KEY_KBDILLUMDOWN,
        "KeyboardBacklightToggle" => KeyCode::KEY_KBDILLUMTOGGLE,
        "TouchpadToggle" => KeyCode::KEY_TOUCHPAD_TOGGLE,
        "TouchpadOn" => KeyCode::KEY_TOUCHPAD_ON,
        "TouchpadOff" => KeyCode::KEY_TOUCHPAD_OFF,
        "CameraAccessToggle" => KEY_CAMERA_ACCESS_TOGGLE,
        "PrivacyScreenToggle" => KeyCode::KEY_PRIVACY_SCREEN_TOGGLE,
        "WLAN" => KeyCode::KEY_WLAN,
        "Bluetooth" => KeyCode::KEY_BLUETOOTH,
        "AirplaneMode" => KeyCode::KEY_RFKILL,
        "WWAN" => KeyCode::KEY_WWAN,
        "UWB" => KeyCode::KEY_UWB,
        "Battery" => KeyCode::KEY_BATTERY,
        "Fn" => KeyCode::KEY_FN,
        // Laptops report their Fn-lock toggle (Fn+Esc) with this code.
        "FnLock" => KeyCode::KEY_FN_ESC,
        "KeyboardLayoutSelect" => KeyCode::KEY_KBD_LAYOUT_NEXT,
        "EmojiPicker" => KEY_EMOJI_PICKER,
        "Dictate" => KEY_DICTATE,
        "SpeechInputToggle" => KeyCode::KEY_VOICECOMMAND,
        "Accessibility" => KEY_ACCESSIBILITY,
        "DoNotDisturb" => KEY_DO_NOT_DISTURB,
        "NotificationCenter" => KEY_NOTIFICATION_CENTER,
        "SelectiveScreenshot" => KeyCode::KEY_SELECTIVE_SCREENSHOT,
        "SelectTask" => KeyCode::KEY_APPSELECT,
        "ShowAllWindows" => KeyCode::KEY_SCALE,
        "AllApplications" => KEY_ALL_APPLICATIONS,
        "CycleWindows" => KeyCode::KEY_CYCLEWINDOWS,

        // Legacy editing, office, programmable, international and CD keys
        "Help" => KeyCode::KEY_HELP,
        "Undo" => KeyCode::KEY_UNDO,
        "Redo" => KeyCode::KEY_REDO,
        "Again" => KeyCode::KEY_AGAIN,
        "Find" => KeyCode::KEY_FIND,
        "Open" => KeyCode::KEY_OPEN,
        // Sun's Props key sends USB "Keyboard Menu", which arrives as
        // KEY_PROPS; the kernel does not map the USB "Props" usage.
        "Props" => KeyCode::KEY_PROPS,
        // USB HID "Keyboard Select" arrives as KEY_FRONT.
        "Select" => KeyCode::KEY_FRONT,
        "Copy" => KeyCode::KEY_COPY,
        "Cut" => KeyCode::KEY_CUT,
        "Paste" => KeyCode::KEY_PASTE,
        "Menu" => KeyCode::KEY_MENU,
        "Prog1" => KeyCode::KEY_PROG1,
        "Prog2" => KeyCode::KEY_PROG2,
        "Prog3" => KeyCode::KEY_PROG3,
        "Prog4" => KeyCode::KEY_PROG4,
        "IntlBackslash" => KeyCode::KEY_102ND,
        "IntlYen" => KeyCode::KEY_YEN,
        "IntlRo" => KeyCode::KEY_RO,
        "KanaMode" => KeyCode::KEY_KATAKANAHIRAGANA,
        "Convert" => KeyCode::KEY_HENKAN,
        "NonConvert" => KeyCode::KEY_MUHENKAN,
        "Lang1" => KeyCode::KEY_HANGEUL,
        "Lang2" => KeyCode::KEY_HANJA,
        "Lang3" => KeyCode::KEY_KATAKANA,
        "Lang4" => KeyCode::KEY_HIRAGANA,
        "Lang5" => KeyCode::KEY_ZENKAKUHANKAKU,
        "New" => KeyCode::KEY_NEW,
        "Close" => KeyCode::KEY_CLOSE,
        "Save" => KeyCode::KEY_SAVE,
        "Print" => KeyCode::KEY_PRINT,
        "Cancel" => KeyCode::KEY_CANCEL,
        "Exit" => KeyCode::KEY_EXIT,
        "Edit" => KeyCode::KEY_EDIT,
        "Move" => KeyCode::KEY_MOVE,
        "MailSend" => KeyCode::KEY_SEND,
        "MailReply" => KeyCode::KEY_REPLY,
        "MailForward" => KeyCode::KEY_FORWARDMAIL,
        "SendFile" => KeyCode::KEY_SENDFILE,
        "DeleteFile" => KeyCode::KEY_DELETEFILE,
        "Transfer" => KeyCode::KEY_XFER,
        "Macro" => KeyCode::KEY_MACRO,
        "ScrollUp" => KeyCode::KEY_SCROLLUP,
        "ScrollDown" => KeyCode::KEY_SCROLLDOWN,
        "LaunchComputer" => KeyCode::KEY_COMPUTER,
        "LaunchWordProcessor" => KeyCode::KEY_WORDPROCESSOR,
        "LaunchSpreadsheet" => KeyCode::KEY_SPREADSHEET,
        "LaunchContacts" => KeyCode::KEY_ADDRESSBOOK,
        "LaunchAudioBrowser" => KeyCode::KEY_AUDIO,
        "LaunchChat" => KeyCode::KEY_CHAT,
        "LaunchPlayer" => KeyCode::KEY_PLAYER,
        "LaunchPhone" => KeyCode::KEY_PHONE,
        "LaunchFinance" => KeyCode::KEY_FINANCE,
        "LaunchSports" => KeyCode::KEY_SPORT,
        "LaunchShop" => KeyCode::KEY_SHOP,
        "LaunchKeyboardLayout" => KeyCode::KEY_KEYBOARD,
        "SpellCheck" => KeyCode::KEY_SPELLCHECK,
        "PlayCD" => KeyCode::KEY_PLAYCD,
        "CloseCD" => KeyCode::KEY_CLOSECD,
        "EjectCloseCD" => KeyCode::KEY_EJECTCLOSECD,
        "BassBoost" => KeyCode::KEY_BASSBOOST,
        "VideoNext" => KeyCode::KEY_VIDEO_NEXT,
        "VideoPrev" => KeyCode::KEY_VIDEO_PREV,
        "CameraFocus" => KeyCode::KEY_CAMERA_FOCUS,
        "CameraZoomIn" => KeyCode::KEY_CAMERA_ZOOMIN,
        "CameraZoomOut" => KeyCode::KEY_CAMERA_ZOOMOUT,
        "CameraUp" => KeyCode::KEY_CAMERA_UP,
        "CameraDown" => KeyCode::KEY_CAMERA_DOWN,
        "CameraLeft" => KeyCode::KEY_CAMERA_LEFT,
        "CameraRight" => KeyCode::KEY_CAMERA_RIGHT,

        _ => return None,
    })
}

/// Config key code of an evdev key code, if the catalog has one.
pub fn evdev_to_code(code: u16) -> Option<&'static str> {
    crate::profile::key_catalog::CATEGORIES
        .iter()
        .flat_map(|keys| keys.iter().copied())
        .find(|name| code_to_key(name).is_some_and(|key| key.code() == code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_map_correctly() {
        assert_eq!(code_to_key("KeyA"), Some(KeyCode::KEY_A));
        assert_eq!(code_to_key("KeyZ"), Some(KeyCode::KEY_Z));
    }

    #[test]
    fn digits_map_correctly() {
        assert_eq!(code_to_key("Digit0"), Some(KeyCode::KEY_0));
        assert_eq!(code_to_key("Digit9"), Some(KeyCode::KEY_9));
    }

    #[test]
    fn f_keys_map_correctly() {
        assert_eq!(code_to_key("F1"), Some(KeyCode::KEY_F1));
        assert_eq!(code_to_key("F24"), Some(KeyCode::KEY_F24));
    }

    #[test]
    fn navigation_keys_map_correctly() {
        assert_eq!(code_to_key("Escape"), Some(KeyCode::KEY_ESC));
        assert_eq!(code_to_key("ArrowLeft"), Some(KeyCode::KEY_LEFT));
        assert_eq!(code_to_key("ArrowRight"), Some(KeyCode::KEY_RIGHT));
        assert_eq!(code_to_key("ArrowUp"), Some(KeyCode::KEY_UP));
        assert_eq!(code_to_key("ArrowDown"), Some(KeyCode::KEY_DOWN));
        assert_eq!(code_to_key("Space"), Some(KeyCode::KEY_SPACE));
        assert_eq!(code_to_key("Enter"), Some(KeyCode::KEY_ENTER));
    }

    #[test]
    fn punctuation_keys_map_correctly() {
        assert_eq!(code_to_key("Backquote"), Some(KeyCode::KEY_GRAVE));
        assert_eq!(code_to_key("Minus"), Some(KeyCode::KEY_MINUS));
        assert_eq!(code_to_key("Equal"), Some(KeyCode::KEY_EQUAL));
        assert_eq!(code_to_key("Semicolon"), Some(KeyCode::KEY_SEMICOLON));
        assert_eq!(code_to_key("Slash"), Some(KeyCode::KEY_SLASH));
    }

    #[test]
    fn modifier_keys_map_correctly() {
        assert_eq!(code_to_key("ShiftLeft"), Some(KeyCode::KEY_LEFTSHIFT));
        assert_eq!(code_to_key("ShiftRight"), Some(KeyCode::KEY_RIGHTSHIFT));
        assert_eq!(code_to_key("ControlLeft"), Some(KeyCode::KEY_LEFTCTRL));
        assert_eq!(code_to_key("ControlRight"), Some(KeyCode::KEY_RIGHTCTRL));
        assert_eq!(code_to_key("MetaLeft"), Some(KeyCode::KEY_LEFTMETA));
        assert_eq!(code_to_key("ContextMenu"), Some(KeyCode::KEY_COMPOSE));
    }

    #[test]
    fn media_keys_map_correctly() {
        assert_eq!(code_to_key("AudioVolumeUp"), Some(KeyCode::KEY_VOLUMEUP));
        assert_eq!(
            code_to_key("AudioVolumeDown"),
            Some(KeyCode::KEY_VOLUMEDOWN)
        );
        assert_eq!(code_to_key("MediaPlayPause"), Some(KeyCode::KEY_PLAYPAUSE));
        assert_eq!(code_to_key("BrowserBack"), Some(KeyCode::KEY_BACK));
    }

    #[test]
    fn keys_follow_the_linux_hid_mapping() {
        // Codes the kernel reports for the USB HID usages the W3C names.
        assert_eq!(code_to_key("PrintScreen"), Some(KeyCode::KEY_SYSRQ));
        assert_eq!(code_to_key("LaunchApp1"), Some(KeyCode::KEY_FILE));
        assert_eq!(code_to_key("LaunchApp2"), Some(KeyCode::KEY_CALC));
        assert_eq!(code_to_key("MediaSelect"), Some(KeyCode::KEY_CONFIG));
        assert_eq!(code_to_key("Select"), Some(KeyCode::KEY_FRONT));
        assert_eq!(code_to_key("EmojiPicker").map(|k| k.code()), Some(0x249));
    }

    #[test]
    fn numpad_keys_map_correctly() {
        assert_eq!(code_to_key("NumLock"), Some(KeyCode::KEY_NUMLOCK));
        assert_eq!(code_to_key("Numpad0"), Some(KeyCode::KEY_KP0));
        assert_eq!(code_to_key("NumpadEnter"), Some(KeyCode::KEY_KPENTER));
        assert_eq!(
            code_to_key("NumpadParenLeft"),
            Some(KeyCode::KEY_KPLEFTPAREN)
        );
        assert_eq!(
            code_to_key("NumpadParenRight"),
            Some(KeyCode::KEY_KPRIGHTPAREN)
        );
    }

    #[test]
    fn mouse_buttons_map_correctly() {
        assert_eq!(code_to_key("MouseLeft"), Some(KeyCode::BTN_LEFT));
        assert_eq!(code_to_key("MouseRight"), Some(KeyCode::BTN_RIGHT));
        assert_eq!(code_to_key("MouseMiddle"), Some(KeyCode::BTN_MIDDLE));
        assert_eq!(code_to_key("MouseForward"), Some(KeyCode::BTN_FORWARD));
        assert_eq!(code_to_key("MouseBack"), Some(KeyCode::BTN_BACK));
        assert_eq!(code_to_key("MouseTask"), Some(KeyCode::BTN_TASK));
    }

    #[test]
    fn international_keys_map_correctly() {
        assert_eq!(code_to_key("IntlBackslash"), Some(KeyCode::KEY_102ND));
        assert_eq!(code_to_key("IntlYen"), Some(KeyCode::KEY_YEN));
        assert_eq!(code_to_key("IntlRo"), Some(KeyCode::KEY_RO));
        assert_eq!(code_to_key("KanaMode"), Some(KeyCode::KEY_KATAKANAHIRAGANA));
        assert_eq!(code_to_key("Lang5"), Some(KeyCode::KEY_ZENKAKUHANKAKU));
    }

    #[test]
    fn every_catalog_key_maps_to_a_distinct_evdev_key_and_round_trips() {
        let mut codes = std::collections::HashSet::new();
        for name in crate::profile::key_catalog::CATEGORIES
            .iter()
            .flat_map(|keys| keys.iter())
        {
            let key = code_to_key(name).unwrap_or_else(|| panic!("{name} has no evdev key"));
            assert!(codes.insert(key.code()), "{name} duplicates an evdev key");
            assert_eq!(evdev_to_code(key.code()), Some(*name));
        }
        assert_eq!(evdev_to_code(KeyCode::KEY_UNKNOWN.code()), None);
    }

    #[test]
    fn unknown_code_returns_none() {
        assert_eq!(code_to_key("UnknownKey"), None);
        assert_eq!(code_to_key(""), None);
    }
}
