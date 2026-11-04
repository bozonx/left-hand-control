//! Keys of the layer keyboard grid. Every other key of a layer keymap is an
//! extra key with its own row and saved display order.

/// Left hand rows; the last row is the thumb row.
pub const LEFT_COLUMNS: usize = 6;
pub const LEFT_HAND: [&[&str]; 5] = [
    &[
        "Backquote",
        "Digit1",
        "Digit2",
        "Digit3",
        "Digit4",
        "Digit5",
    ],
    &["Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT"],
    &["CapsLock", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG"],
    &["ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB"],
    &["ControlLeft", "MetaLeft", "AltLeft", "Space"],
];
/// Right hand rows; the last row is the thumb row.
pub const RIGHT_COLUMNS: usize = 6;
pub const RIGHT_HAND: [&[&str]; 5] = [
    &["Digit6", "Digit7", "Digit8", "Digit9", "Digit0", "Minus"],
    &["KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft"],
    &["KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote"],
    &["KeyN", "KeyM", "Comma", "Period", "Slash", "ShiftRight"],
    &["AltRight", "MetaRight", "ContextMenu", "ControlRight"],
];

/// Every grid key in index order: left hand rows, then right hand rows.
pub fn keys() -> impl Iterator<Item = &'static str> {
    LEFT_HAND
        .iter()
        .chain(RIGHT_HAND.iter())
        .flat_map(|row| row.iter().copied())
}

/// Whether `key` is edited on the grid rather than as an extra key.
pub fn contains(key: &str) -> bool {
    keys().any(|code| code == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_keys_are_unique_catalog_keys() {
        let all: Vec<_> = keys().collect();
        let unique: std::collections::BTreeSet<_> = all.iter().collect();
        assert_eq!(all.len(), unique.len());
        assert!(
            all.iter()
                .all(|key| super::super::key_catalog::valid_key(key))
        );
        assert!(contains("KeyQ") && !contains("F13"));
    }
}
