// Mapping between configuration key codes (following KeyboardEvent.code)
// and Linux evdev `Key` values.

#![cfg(target_os = "linux")]

use evdev::Key;

// Codes newer than the `evdev` crate's constants (linux/input-event-codes.h).
const KEY_NOTIFICATION_CENTER: Key = Key::new(0x1bc);
const KEY_EMOJI_PICKER: Key = Key::new(0x249);
const KEY_DICTATE: Key = Key::new(0x24a);
const KEY_CAMERA_ACCESS_TOGGLE: Key = Key::new(0x24d);
const KEY_ACCESSIBILITY: Key = Key::new(0x24e);
const KEY_DO_NOT_DISTURB: Key = Key::new(0x24f);

pub fn code_to_key(code: &str) -> Option<Key> {
    Some(match code {
        // Letters
        "KeyA" => Key::KEY_A,
        "KeyB" => Key::KEY_B,
        "KeyC" => Key::KEY_C,
        "KeyD" => Key::KEY_D,
        "KeyE" => Key::KEY_E,
        "KeyF" => Key::KEY_F,
        "KeyG" => Key::KEY_G,
        "KeyH" => Key::KEY_H,
        "KeyI" => Key::KEY_I,
        "KeyJ" => Key::KEY_J,
        "KeyK" => Key::KEY_K,
        "KeyL" => Key::KEY_L,
        "KeyM" => Key::KEY_M,
        "KeyN" => Key::KEY_N,
        "KeyO" => Key::KEY_O,
        "KeyP" => Key::KEY_P,
        "KeyQ" => Key::KEY_Q,
        "KeyR" => Key::KEY_R,
        "KeyS" => Key::KEY_S,
        "KeyT" => Key::KEY_T,
        "KeyU" => Key::KEY_U,
        "KeyV" => Key::KEY_V,
        "KeyW" => Key::KEY_W,
        "KeyX" => Key::KEY_X,
        "KeyY" => Key::KEY_Y,
        "KeyZ" => Key::KEY_Z,

        // Digits
        "Digit0" => Key::KEY_0,
        "Digit1" => Key::KEY_1,
        "Digit2" => Key::KEY_2,
        "Digit3" => Key::KEY_3,
        "Digit4" => Key::KEY_4,
        "Digit5" => Key::KEY_5,
        "Digit6" => Key::KEY_6,
        "Digit7" => Key::KEY_7,
        "Digit8" => Key::KEY_8,
        "Digit9" => Key::KEY_9,

        // F-row
        "F1" => Key::KEY_F1,
        "F2" => Key::KEY_F2,
        "F3" => Key::KEY_F3,
        "F4" => Key::KEY_F4,
        "F5" => Key::KEY_F5,
        "F6" => Key::KEY_F6,
        "F7" => Key::KEY_F7,
        "F8" => Key::KEY_F8,
        "F9" => Key::KEY_F9,
        "F10" => Key::KEY_F10,
        "F11" => Key::KEY_F11,
        "F12" => Key::KEY_F12,
        "F13" => Key::KEY_F13,
        "F14" => Key::KEY_F14,
        "F15" => Key::KEY_F15,
        "F16" => Key::KEY_F16,
        "F17" => Key::KEY_F17,
        "F18" => Key::KEY_F18,
        "F19" => Key::KEY_F19,
        "F20" => Key::KEY_F20,
        "F21" => Key::KEY_F21,
        "F22" => Key::KEY_F22,
        "F23" => Key::KEY_F23,
        "F24" => Key::KEY_F24,

        // Navigation / editing
        "Escape" => Key::KEY_ESC,
        "Tab" => Key::KEY_TAB,
        "CapsLock" => Key::KEY_CAPSLOCK,
        "Enter" => Key::KEY_ENTER,
        "Backspace" => Key::KEY_BACKSPACE,
        "Space" => Key::KEY_SPACE,
        "Delete" => Key::KEY_DELETE,
        "Insert" => Key::KEY_INSERT,
        "Home" => Key::KEY_HOME,
        "End" => Key::KEY_END,
        "PageUp" => Key::KEY_PAGEUP,
        "PageDown" => Key::KEY_PAGEDOWN,
        "ArrowLeft" => Key::KEY_LEFT,
        "ArrowRight" => Key::KEY_RIGHT,
        "ArrowUp" => Key::KEY_UP,
        "ArrowDown" => Key::KEY_DOWN,
        "PrintScreen" => Key::KEY_SYSRQ,
        "ScrollLock" => Key::KEY_SCROLLLOCK,
        "Pause" => Key::KEY_PAUSE,

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
        "ShiftLeft" => Key::KEY_LEFTSHIFT,
        "ShiftRight" => Key::KEY_RIGHTSHIFT,
        "ControlLeft" => Key::KEY_LEFTCTRL,
        "ControlRight" => Key::KEY_RIGHTCTRL,
        "AltLeft" => Key::KEY_LEFTALT,
        "AltRight" => Key::KEY_RIGHTALT,
        "MetaLeft" => Key::KEY_LEFTMETA,
        "MetaRight" => Key::KEY_RIGHTMETA,
        "ContextMenu" => Key::KEY_COMPOSE,

        // Numpad
        "NumLock" => Key::KEY_NUMLOCK,
        "Numpad0" => Key::KEY_KP0,
        "Numpad1" => Key::KEY_KP1,
        "Numpad2" => Key::KEY_KP2,
        "Numpad3" => Key::KEY_KP3,
        "Numpad4" => Key::KEY_KP4,
        "Numpad5" => Key::KEY_KP5,
        "Numpad6" => Key::KEY_KP6,
        "Numpad7" => Key::KEY_KP7,
        "Numpad8" => Key::KEY_KP8,
        "Numpad9" => Key::KEY_KP9,
        "NumpadAdd" => Key::KEY_KPPLUS,
        "NumpadSubtract" => Key::KEY_KPMINUS,
        "NumpadMultiply" => Key::KEY_KPASTERISK,
        "NumpadDivide" => Key::KEY_KPSLASH,
        "NumpadDecimal" => Key::KEY_KPDOT,
        "NumpadEnter" => Key::KEY_KPENTER,
        "NumpadEqual" => Key::KEY_KPEQUAL,
        "NumpadComma" => Key::KEY_KPCOMMA,
        "NumpadParenLeft" => Key::KEY_KPLEFTPAREN,
        "NumpadParenRight" => Key::KEY_KPRIGHTPAREN,

        // Mouse buttons
        "MouseLeft" => Key::BTN_LEFT,
        "MouseRight" => Key::BTN_RIGHT,
        "MouseMiddle" => Key::BTN_MIDDLE,
        "MouseSide" => Key::BTN_SIDE,
        "MouseExtra" => Key::BTN_EXTRA,
        "MouseForward" => Key::BTN_FORWARD,
        "MouseBack" => Key::BTN_BACK,
        "MouseTask" => Key::BTN_TASK,

        // Media, browser and launch
        "AudioVolumeUp" => Key::KEY_VOLUMEUP,
        "AudioVolumeDown" => Key::KEY_VOLUMEDOWN,
        "AudioVolumeMute" => Key::KEY_MUTE,
        "MicrophoneMuteToggle" => Key::KEY_MICMUTE,
        "MediaPlayPause" => Key::KEY_PLAYPAUSE,
        "MediaPlay" => Key::KEY_PLAY,
        "MediaPause" => Key::KEY_PAUSECD,
        "MediaStop" => Key::KEY_STOPCD,
        "MediaTrackNext" => Key::KEY_NEXTSONG,
        "MediaTrackPrevious" => Key::KEY_PREVIOUSSONG,
        "MediaRewind" => Key::KEY_REWIND,
        "MediaFastForward" => Key::KEY_FASTFORWARD,
        "MediaRecord" => Key::KEY_RECORD,
        "MediaSelect" => Key::KEY_CONFIG,
        "LaunchMediaPlayer" => Key::KEY_MEDIA,
        "Eject" => Key::KEY_EJECTCD,
        "BrowserBack" => Key::KEY_BACK,
        "BrowserForward" => Key::KEY_FORWARD,
        "BrowserRefresh" => Key::KEY_REFRESH,
        "BrowserStop" => Key::KEY_STOP,
        "BrowserHome" => Key::KEY_HOMEPAGE,
        "BrowserSearch" => Key::KEY_SEARCH,
        "BrowserFavorites" => Key::KEY_BOOKMARKS,
        "LaunchApp1" => Key::KEY_FILE,
        "LaunchApp2" => Key::KEY_CALC,
        "LaunchMail" => Key::KEY_MAIL,
        "LaunchInternetBrowser" => Key::KEY_WWW,
        "LaunchControlPanel" => Key::KEY_CONTROLPANEL,
        "LaunchScreenSaver" => Key::KEY_SCREENSAVER,
        "LaunchAssistant" => Key::KEY_ASSISTANT,
        "LaunchCamera" => Key::KEY_CAMERA,
        "LaunchDocuments" => Key::KEY_DOCUMENTS,
        "LaunchCalendar" => Key::KEY_CALENDAR,
        "ZoomIn" => Key::KEY_ZOOMIN,
        "ZoomOut" => Key::KEY_ZOOMOUT,
        "ZoomToggle" => Key::KEY_FULL_SCREEN,

        // System and laptop
        "Power" => Key::KEY_POWER,
        "Sleep" => Key::KEY_SLEEP,
        "WakeUp" => Key::KEY_WAKEUP,
        "Suspend" => Key::KEY_SUSPEND,
        "LockScreen" => Key::KEY_COFFEE,
        "LogOff" => Key::KEY_LOGOFF,
        "BrightnessUp" => Key::KEY_BRIGHTNESSUP,
        "BrightnessDown" => Key::KEY_BRIGHTNESSDOWN,
        "BrightnessToggle" => Key::KEY_DISPLAYTOGGLE,
        "BrightnessCycle" => Key::KEY_BRIGHTNESS_CYCLE,
        "BrightnessAuto" => Key::KEY_BRIGHTNESS_AUTO,
        "BrightnessMinimum" => Key::KEY_BRIGHTNESS_MIN,
        "BrightnessMaximum" => Key::KEY_BRIGHTNESS_MAX,
        "DisplayOff" => Key::KEY_DISPLAY_OFF,
        "DisplayToggleIntExt" => Key::KEY_SWITCHVIDEOMODE,
        "RotateDisplay" => Key::KEY_ROTATE_DISPLAY,
        "KeyboardBacklightUp" => Key::KEY_KBDILLUMUP,
        "KeyboardBacklightDown" => Key::KEY_KBDILLUMDOWN,
        "KeyboardBacklightToggle" => Key::KEY_KBDILLUMTOGGLE,
        "TouchpadToggle" => Key::KEY_TOUCHPAD_TOGGLE,
        "TouchpadOn" => Key::KEY_TOUCHPAD_ON,
        "TouchpadOff" => Key::KEY_TOUCHPAD_OFF,
        "CameraAccessToggle" => KEY_CAMERA_ACCESS_TOGGLE,
        "PrivacyScreenToggle" => Key::KEY_PRIVACY_SCREEN_TOGGLE,
        "WLAN" => Key::KEY_WLAN,
        "Bluetooth" => Key::KEY_BLUETOOTH,
        "AirplaneMode" => Key::KEY_RFKILL,
        "WWAN" => Key::KEY_WWAN,
        "UWB" => Key::KEY_UWB,
        "Battery" => Key::KEY_BATTERY,
        "Fn" => Key::KEY_FN,
        // Laptops report their Fn-lock toggle (Fn+Esc) with this code.
        "FnLock" => Key::KEY_FN_ESC,
        "KeyboardLayoutSelect" => Key::KEY_KBD_LAYOUT_NEXT,
        "EmojiPicker" => KEY_EMOJI_PICKER,
        "Dictate" => KEY_DICTATE,
        "SpeechInputToggle" => Key::KEY_VOICECOMMAND,
        "Accessibility" => KEY_ACCESSIBILITY,
        "DoNotDisturb" => KEY_DO_NOT_DISTURB,
        "NotificationCenter" => KEY_NOTIFICATION_CENTER,
        "SelectiveScreenshot" => Key::KEY_SELECTIVE_SCREENSHOT,
        "SelectTask" => Key::KEY_APPSELECT,
        "ShowAllWindows" => Key::KEY_SCALE,
        "AllApplications" => Key::KEY_DASHBOARD,
        "CycleWindows" => Key::KEY_CYCLEWINDOWS,

        // Legacy editing, office, programmable, international and CD keys
        "Help" => Key::KEY_HELP,
        "Undo" => Key::KEY_UNDO,
        "Redo" => Key::KEY_REDO,
        "Again" => Key::KEY_AGAIN,
        "Find" => Key::KEY_FIND,
        "Open" => Key::KEY_OPEN,
        // Sun's Props key sends USB "Keyboard Menu", which arrives as
        // KEY_PROPS; the kernel does not map the USB "Props" usage.
        "Props" => Key::KEY_PROPS,
        // USB HID "Keyboard Select" arrives as KEY_FRONT.
        "Select" => Key::KEY_FRONT,
        "Copy" => Key::KEY_COPY,
        "Cut" => Key::KEY_CUT,
        "Paste" => Key::KEY_PASTE,
        "Menu" => Key::KEY_MENU,
        "Prog1" => Key::KEY_PROG1,
        "Prog2" => Key::KEY_PROG2,
        "Prog3" => Key::KEY_PROG3,
        "Prog4" => Key::KEY_PROG4,
        "IntlBackslash" => Key::KEY_102ND,
        "IntlYen" => Key::KEY_YEN,
        "IntlRo" => Key::KEY_RO,
        "KanaMode" => Key::KEY_KATAKANAHIRAGANA,
        "Convert" => Key::KEY_HENKAN,
        "NonConvert" => Key::KEY_MUHENKAN,
        "Lang1" => Key::KEY_HANGEUL,
        "Lang2" => Key::KEY_HANJA,
        "Lang3" => Key::KEY_KATAKANA,
        "Lang4" => Key::KEY_HIRAGANA,
        "Lang5" => Key::KEY_ZENKAKUHANKAKU,
        "New" => Key::KEY_NEW,
        "Close" => Key::KEY_CLOSE,
        "Save" => Key::KEY_SAVE,
        "Print" => Key::KEY_PRINT,
        "Cancel" => Key::KEY_CANCEL,
        "Exit" => Key::KEY_EXIT,
        "Edit" => Key::KEY_EDIT,
        "Move" => Key::KEY_MOVE,
        "MailSend" => Key::KEY_SEND,
        "MailReply" => Key::KEY_REPLY,
        "MailForward" => Key::KEY_FORWARDMAIL,
        "SendFile" => Key::KEY_SENDFILE,
        "DeleteFile" => Key::KEY_DELETEFILE,
        "Transfer" => Key::KEY_XFER,
        "Macro" => Key::KEY_MACRO,
        "ScrollUp" => Key::KEY_SCROLLUP,
        "ScrollDown" => Key::KEY_SCROLLDOWN,
        "LaunchComputer" => Key::KEY_COMPUTER,
        "LaunchWordProcessor" => Key::KEY_WORDPROCESSOR,
        "LaunchSpreadsheet" => Key::KEY_SPREADSHEET,
        "LaunchContacts" => Key::KEY_ADDRESSBOOK,
        "LaunchAudioBrowser" => Key::KEY_AUDIO,
        "LaunchChat" => Key::KEY_CHAT,
        "LaunchPlayer" => Key::KEY_PLAYER,
        "LaunchPhone" => Key::KEY_PHONE,
        "LaunchFinance" => Key::KEY_FINANCE,
        "LaunchSports" => Key::KEY_SPORT,
        "LaunchShop" => Key::KEY_SHOP,
        "LaunchKeyboardLayout" => Key::KEY_KEYBOARD,
        "SpellCheck" => Key::KEY_SPELLCHECK,
        "PlayCD" => Key::KEY_PLAYCD,
        "CloseCD" => Key::KEY_CLOSECD,
        "EjectCloseCD" => Key::KEY_EJECTCLOSECD,
        "BassBoost" => Key::KEY_BASSBOOST,
        "VideoNext" => Key::KEY_VIDEO_NEXT,
        "VideoPrev" => Key::KEY_VIDEO_PREV,
        "CameraFocus" => Key::KEY_CAMERA_FOCUS,
        "CameraZoomIn" => Key::KEY_CAMERA_ZOOMIN,
        "CameraZoomOut" => Key::KEY_CAMERA_ZOOMOUT,
        "CameraUp" => Key::KEY_CAMERA_UP,
        "CameraDown" => Key::KEY_CAMERA_DOWN,
        "CameraLeft" => Key::KEY_CAMERA_LEFT,
        "CameraRight" => Key::KEY_CAMERA_RIGHT,

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
        assert_eq!(code_to_key("KeyA"), Some(Key::KEY_A));
        assert_eq!(code_to_key("KeyZ"), Some(Key::KEY_Z));
    }

    #[test]
    fn digits_map_correctly() {
        assert_eq!(code_to_key("Digit0"), Some(Key::KEY_0));
        assert_eq!(code_to_key("Digit9"), Some(Key::KEY_9));
    }

    #[test]
    fn f_keys_map_correctly() {
        assert_eq!(code_to_key("F1"), Some(Key::KEY_F1));
        assert_eq!(code_to_key("F24"), Some(Key::KEY_F24));
    }

    #[test]
    fn navigation_keys_map_correctly() {
        assert_eq!(code_to_key("Escape"), Some(Key::KEY_ESC));
        assert_eq!(code_to_key("ArrowLeft"), Some(Key::KEY_LEFT));
        assert_eq!(code_to_key("ArrowRight"), Some(Key::KEY_RIGHT));
        assert_eq!(code_to_key("ArrowUp"), Some(Key::KEY_UP));
        assert_eq!(code_to_key("ArrowDown"), Some(Key::KEY_DOWN));
        assert_eq!(code_to_key("Space"), Some(Key::KEY_SPACE));
        assert_eq!(code_to_key("Enter"), Some(Key::KEY_ENTER));
    }

    #[test]
    fn punctuation_keys_map_correctly() {
        assert_eq!(code_to_key("Backquote"), Some(Key::KEY_GRAVE));
        assert_eq!(code_to_key("Minus"), Some(Key::KEY_MINUS));
        assert_eq!(code_to_key("Equal"), Some(Key::KEY_EQUAL));
        assert_eq!(code_to_key("Semicolon"), Some(Key::KEY_SEMICOLON));
        assert_eq!(code_to_key("Slash"), Some(Key::KEY_SLASH));
    }

    #[test]
    fn modifier_keys_map_correctly() {
        assert_eq!(code_to_key("ShiftLeft"), Some(Key::KEY_LEFTSHIFT));
        assert_eq!(code_to_key("ShiftRight"), Some(Key::KEY_RIGHTSHIFT));
        assert_eq!(code_to_key("ControlLeft"), Some(Key::KEY_LEFTCTRL));
        assert_eq!(code_to_key("ControlRight"), Some(Key::KEY_RIGHTCTRL));
        assert_eq!(code_to_key("MetaLeft"), Some(Key::KEY_LEFTMETA));
        assert_eq!(code_to_key("ContextMenu"), Some(Key::KEY_COMPOSE));
    }

    #[test]
    fn media_keys_map_correctly() {
        assert_eq!(code_to_key("AudioVolumeUp"), Some(Key::KEY_VOLUMEUP));
        assert_eq!(code_to_key("AudioVolumeDown"), Some(Key::KEY_VOLUMEDOWN));
        assert_eq!(code_to_key("MediaPlayPause"), Some(Key::KEY_PLAYPAUSE));
        assert_eq!(code_to_key("BrowserBack"), Some(Key::KEY_BACK));
    }

    #[test]
    fn keys_follow_the_linux_hid_mapping() {
        // Codes the kernel reports for the USB HID usages the W3C names.
        assert_eq!(code_to_key("PrintScreen"), Some(Key::KEY_SYSRQ));
        assert_eq!(code_to_key("LaunchApp1"), Some(Key::KEY_FILE));
        assert_eq!(code_to_key("LaunchApp2"), Some(Key::KEY_CALC));
        assert_eq!(code_to_key("MediaSelect"), Some(Key::KEY_CONFIG));
        assert_eq!(code_to_key("Select"), Some(Key::KEY_FRONT));
        assert_eq!(code_to_key("EmojiPicker").map(|k| k.code()), Some(0x249));
    }

    #[test]
    fn numpad_keys_map_correctly() {
        assert_eq!(code_to_key("NumLock"), Some(Key::KEY_NUMLOCK));
        assert_eq!(code_to_key("Numpad0"), Some(Key::KEY_KP0));
        assert_eq!(code_to_key("NumpadEnter"), Some(Key::KEY_KPENTER));
        assert_eq!(code_to_key("NumpadParenLeft"), Some(Key::KEY_KPLEFTPAREN));
        assert_eq!(code_to_key("NumpadParenRight"), Some(Key::KEY_KPRIGHTPAREN));
    }

    #[test]
    fn mouse_buttons_map_correctly() {
        assert_eq!(code_to_key("MouseLeft"), Some(Key::BTN_LEFT));
        assert_eq!(code_to_key("MouseRight"), Some(Key::BTN_RIGHT));
        assert_eq!(code_to_key("MouseMiddle"), Some(Key::BTN_MIDDLE));
        assert_eq!(code_to_key("MouseForward"), Some(Key::BTN_FORWARD));
        assert_eq!(code_to_key("MouseBack"), Some(Key::BTN_BACK));
        assert_eq!(code_to_key("MouseTask"), Some(Key::BTN_TASK));
    }

    #[test]
    fn international_keys_map_correctly() {
        assert_eq!(code_to_key("IntlBackslash"), Some(Key::KEY_102ND));
        assert_eq!(code_to_key("IntlYen"), Some(Key::KEY_YEN));
        assert_eq!(code_to_key("IntlRo"), Some(Key::KEY_RO));
        assert_eq!(code_to_key("KanaMode"), Some(Key::KEY_KATAKANAHIRAGANA));
        assert_eq!(code_to_key("Lang5"), Some(Key::KEY_ZENKAKUHANKAKU));
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
        assert_eq!(evdev_to_code(Key::KEY_UNKNOWN.code()), None);
    }

    #[test]
    fn unknown_code_returns_none() {
        assert_eq!(code_to_key("UnknownKey"), None);
        assert_eq!(code_to_key(""), None);
    }
}
