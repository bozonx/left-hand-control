//! Popup data and navigation shared by the winit popups and the Spell worker.
//!
//! Quick items and the emoji catalog are pilot fixtures; the product port
//! replaces them with `quickActions` / `emojiPages` from the config.

use crate::command::Popup;
use slint::SharedString;

/// Emoji cells per regular page (6 rows × 8 columns).
pub const EMOJI_PAGE_SIZE: usize = 48;
pub const EMOJI_COLUMNS: i32 = 8;
/// Index of the diagnostic stress page (key `6`).
pub const STRESS_PAGE: i32 = 5;
pub const STRESS_CELLS: usize = 1500;
/// Distinct emoji in the fixture catalog; pages wrap around it.
pub const EMOJI_COUNT: usize = 240;

pub fn emoji_items() -> Vec<SharedString> {
    (0x1f600..=0x1f64f)
        .chain(0x1f300..=0x1f5ff)
        .filter_map(char::from_u32)
        .take(EMOJI_COUNT)
        .map(|character| character.to_string().into())
        .collect()
}

pub fn quick_items() -> Vec<String> {
    (1..=30)
        .map(|index| format!("Действие {index:02} / Action {index:02}"))
        .collect()
}

pub fn filter(items: &[String], query: &str) -> Vec<SharedString> {
    let query = query.to_lowercase();
    items
        .iter()
        .filter(|item| item.to_lowercase().contains(&query))
        .map(|item| item.as_str().into())
        .collect()
}

/// Number of selectable cells on an emoji page.
pub fn emoji_cells(page: i32) -> usize {
    if page == STRESS_PAGE {
        STRESS_CELLS
    } else {
        EMOJI_PAGE_SIZE
    }
}

/// Catalog index of the selected cell, or `None` when out of range.
pub fn emoji_index(page: i32, selected: i32) -> Option<usize> {
    let selected = usize::try_from(selected).ok()?;
    if page < 0 || selected >= emoji_cells(page) {
        return None;
    }
    let index = if page == STRESS_PAGE {
        selected
    } else {
        usize::try_from(page)
            .ok()?
            .checked_mul(EMOJI_PAGE_SIZE)?
            .checked_add(selected)?
    };
    Some(index % EMOJI_COUNT)
}

pub fn advance(selected: i32, delta: i32, count: usize) -> i32 {
    if count == 0 {
        0
    } else {
        (selected + delta).rem_euclid(count as i32)
    }
}

/// Selection delta for an arrow key; the emoji grid moves in two axes.
pub fn key_delta(popup: Popup, key: &str) -> Option<i32> {
    use slint::platform::Key;
    let is = |expected: Key| key == SharedString::from(expected).as_str();
    let row = match popup {
        Popup::Emoji => EMOJI_COLUMNS,
        Popup::Quick => 1,
    };
    if is(Key::DownArrow) {
        Some(row)
    } else if is(Key::UpArrow) {
        Some(-row)
    } else if popup == Popup::Emoji && is(Key::LeftArrow) {
        Some(-1)
    } else if popup == Popup::Emoji && is(Key::RightArrow) {
        Some(1)
    } else {
        None
    }
}

pub fn is_key(key: &str, expected: slint::platform::Key) -> bool {
    key == SharedString::from(expected).as_str()
}

/// Enter arrives as the platform Return key or as a plain newline.
pub fn is_enter(key: &str) -> bool {
    is_key(key, slint::platform::Key::Return) || key == "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_index_covers_pages_and_stress_page() {
        assert_eq!(emoji_index(0, 0), Some(0));
        assert_eq!(emoji_index(1, 3), Some(51));
        assert_eq!(emoji_index(4, 47), Some(239));
        assert_eq!(emoji_index(0, 48), None);
        assert_eq!(emoji_index(STRESS_PAGE, 1499), Some(1499 % EMOJI_COUNT));
        assert_eq!(emoji_index(STRESS_PAGE, 1500), None);
        assert_eq!(emoji_index(-1, 0), None);
        assert_eq!(emoji_index(0, -1), None);
    }

    #[test]
    fn navigation_wraps() {
        assert_eq!(advance(0, -1, 48), 47);
        assert_eq!(advance(47, 8, 48), 7);
        assert_eq!(advance(3, 1, 0), 0);
        let down = SharedString::from(slint::platform::Key::DownArrow);
        assert_eq!(key_delta(Popup::Emoji, &down), Some(8));
        assert_eq!(key_delta(Popup::Quick, &down), Some(1));
        let left = SharedString::from(slint::platform::Key::LeftArrow);
        assert_eq!(key_delta(Popup::Quick, &left), None);
    }

    #[test]
    fn filter_is_case_insensitive_for_cyrillic() {
        let items = quick_items();
        assert_eq!(filter(&items, "ДЕЙСТВИЕ 03").len(), 1);
        assert_eq!(filter(&items, "").len(), 30);
    }
}
