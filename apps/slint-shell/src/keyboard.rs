//! Physical keyboard grids shown by the editor and their key labels.
//!
//! Key codes follow `KeyboardEvent.code` (`utils/keys.ts`); every grid lists
//! codes only, labels come from [`label`].

/// Base keyboard page: 5 rows × 16 caps.
pub const BASE_COLUMNS: usize = 16;
pub const BASE: [&str; 80] = [
    "Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
    "PrintScreen", "ScrollLock", "Pause", //
    "Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
    "Digit9", "Digit0", "Minus", "Equal", "Backspace", "Insert", "Home", //
    "Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP",
    "BracketLeft", "BracketRight", "Backslash", "Delete", "End", //
    "CapsLock", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL",
    "Semicolon", "Quote", "Enter", "PageUp", "PageDown", "ArrowUp", //
    "ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period",
    "Slash", "AltLeft", "Space", "ArrowLeft", "ArrowDown", "ArrowRight",
];

/// Layer grids, split by hand; the last row of each hand is the thumb row.
pub const LEFT_COLUMNS: usize = 6;
pub const LEFT_HAND: [&[&str]; 6] = [
    &["Escape", "F1", "F2", "F3", "F4", "F5"],
    &["Backquote", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5"],
    &["Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT"],
    &["CapsLock", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG"],
    &["ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB"],
    &["ControlLeft", "MetaLeft", "AltLeft", "Space"],
];
pub const RIGHT_COLUMNS: usize = 8;
pub const RIGHT_HAND: [&[&str]; 6] = [
    &["F6", "F7", "F8", "F9", "F10", "F11", "F12", "PrintScreen"],
    &["Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus", "Equal", "Backspace"],
    &["KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft", "BracketRight", "Backslash"],
    &["KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote", "Enter"],
    &["KeyN", "KeyM", "Comma", "Period", "Slash", "ShiftRight"],
    &["AltRight", "MetaRight", "ContextMenu", "ControlRight"],
];

/// Every layer key in index order: left hand rows, then right hand rows.
pub fn layer_keys() -> impl Iterator<Item = &'static str> {
    LEFT_HAND
        .iter()
        .chain(RIGHT_HAND.iter())
        .flat_map(|row| row.iter().copied())
}

pub fn layer_key(index: i32) -> Option<&'static str> {
    usize::try_from(index)
        .ok()
        .and_then(|index| layer_keys().nth(index))
}

pub fn base_key(index: i32) -> Option<&'static str> {
    usize::try_from(index)
        .ok()
        .and_then(|index| BASE.get(index).copied())
}

/// Short US-layout label of a key code.
pub fn label(code: &str) -> &str {
    if let Some(letter) = code.strip_prefix("Key") {
        return letter;
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return digit;
    }
    match code {
        "Escape" => "Esc",
        "PrintScreen" => "PrtSc",
        "ScrollLock" => "ScrL",
        "Backquote" => "`",
        "Minus" => "-",
        "Equal" => "=",
        "Backspace" => "Bksp",
        "Insert" => "Ins",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Delete" => "Del",
        "CapsLock" => "Caps",
        "Semicolon" => ";",
        "Quote" => "'",
        "PageUp" => "PgUp",
        "PageDown" => "PgDn",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "ShiftLeft" | "ShiftRight" => "Shift",
        "ControlLeft" | "ControlRight" => "Ctrl",
        "AltLeft" | "AltRight" => "Alt",
        "MetaLeft" | "MetaRight" => "Meta",
        "ContextMenu" => "Menu",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        other => other,
    }
}

/// System (evdev) code of a key, shown by the "System codes" view.
pub fn system_code(code: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        lhc_core::mapper::keys::code_to_key(code)
            .map(|key| key.code().to_string())
            .unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = code;
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lhc_core::profile::key_catalog::CATEGORIES;

    #[test]
    fn grids_use_catalog_keys_once() {
        let known = |code: &str| CATEGORIES.iter().any(|keys| keys.contains(&code));
        for code in BASE.iter().copied().chain(layer_keys()) {
            assert!(known(code), "{code} is not in the key catalog");
        }
        let mut layer: Vec<_> = layer_keys().collect();
        let count = layer.len();
        layer.sort_unstable();
        layer.dedup();
        assert_eq!(layer.len(), count);
        assert_eq!(BASE.len() % BASE_COLUMNS, 0);
    }

    #[test]
    fn labels_and_lookups() {
        assert_eq!(label("KeyQ"), "Q");
        assert_eq!(label("Digit7"), "7");
        assert_eq!(label("ControlRight"), "Ctrl");
        assert_eq!(layer_key(0), Some("Escape"));
        assert_eq!(layer_key(-1), None);
        assert_eq!(base_key(33), Some("KeyQ"));
        assert_eq!(base_key(80), None);
        #[cfg(target_os = "linux")]
        assert_eq!(system_code("KeyA"), "30");
    }
}
