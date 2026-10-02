//! Popup data and keyboard handling shared by the winit popups and the
//! Spell worker.

use crate::{
    command::Popup,
    ui::{EmojiPopup, QuickPopup},
};
use lhc_core::profile::model::{LEFT_HAND_HOTKEYS, LayoutPreset};
use slint::{Model, ModelRc, SharedString, VecModel};

/// Cells per menu page: one per left-hand hotkey, 3 rows × 5 columns.
pub const PAGE_CELLS: usize = LEFT_HAND_HOTKEYS.len();
pub const COLUMNS: i32 = 5;

/// What a key press in a popup asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    Dismiss,
    /// Run the item at this index of the current page.
    Choose(i32),
    /// The selection or page changed.
    Moved,
    Ignored,
}

pub fn is_key(key: &str, expected: slint::platform::Key) -> bool {
    key == SharedString::from(expected).as_str()
}

/// Enter arrives as the platform Return key or as a plain newline.
pub fn is_enter(key: &str) -> bool {
    is_key(key, slint::platform::Key::Return) || key == "\n"
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
    let row = match popup {
        Popup::Emoji => COLUMNS,
        Popup::Quick => 1,
    };
    if is_key(key, Key::DownArrow) {
        Some(row)
    } else if is_key(key, Key::UpArrow) {
        Some(-row)
    } else if popup == Popup::Emoji && is_key(key, Key::LeftArrow) {
        Some(-1)
    } else if popup == Popup::Emoji && is_key(key, Key::RightArrow) {
        Some(1)
    } else {
        None
    }
}

/// Index of the left-hand hotkey typed as `key` (`q` → 0, `b` → 14).
fn hotkey_index(key: &str) -> Option<usize> {
    LEFT_HAND_HOTKEYS.iter().position(|code| {
        code.strip_prefix("Key")
            .is_some_and(|letter| key.eq_ignore_ascii_case(letter))
    })
}

/// Digits switch pages, hotkey letters pick a cell, arrows move.
pub fn emoji_key(ui: &EmojiPopup, key: &str) -> KeyOutcome {
    if is_key(key, slint::platform::Key::Escape) {
        return KeyOutcome::Dismiss;
    }
    if is_enter(key) {
        return KeyOutcome::Choose(ui.get_selected());
    }
    if let Ok(page) = key.parse::<i32>()
        && page > 0
        && page as usize <= ui.get_page_names().row_count()
    {
        ui.set_page(page - 1);
        ui.set_selected(0);
        return KeyOutcome::Moved;
    }
    if let Some(index) = hotkey_index(key) {
        return KeyOutcome::Choose(index as i32);
    }
    if let Some(delta) = key_delta(Popup::Emoji, key) {
        ui.set_selected(advance(ui.get_selected(), delta, PAGE_CELLS));
        return KeyOutcome::Moved;
    }
    KeyOutcome::Ignored
}

pub fn quick_key(ui: &QuickPopup, key: &str) -> KeyOutcome {
    if is_key(key, slint::platform::Key::Escape) {
        return KeyOutcome::Dismiss;
    }
    if is_enter(key) {
        return KeyOutcome::Choose(ui.get_selected());
    }
    if let Some(delta) = key_delta(Popup::Quick, key) {
        ui.set_selected(advance(ui.get_selected(), delta, ui.get_items().row_count()));
        return KeyOutcome::Moved;
    }
    KeyOutcome::Ignored
}

/// Emoji and quick action menus of one layout.
#[derive(Clone, Default)]
pub struct ConfiguredMenus {
    pub layout: LayoutPreset,
}

impl ConfiguredMenus {
    /// Menus of layout `id`, read from the configuration on disk.
    pub fn load_for(id: Option<&str>) -> Result<Self, String> {
        let document = lhc_core::config_document::ConfigDocument::load(
            lhc_core::storage::StoragePaths::resolve()?,
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            layout: document
                .layout_for_activation(id)
                .map_err(|error| error.to_string())?,
        })
    }

    pub fn apply_emoji(&self, ui: &EmojiPopup) {
        let pages = &self.layout.emoji_pages;
        ui.set_page_names(ModelRc::new(VecModel::from(
            pages
                .iter()
                .map(|p| p.name.clone().into())
                .collect::<Vec<SharedString>>(),
        )));
        ui.set_emojis(ModelRc::new(VecModel::from(
            pages
                .iter()
                .flat_map(|p| {
                    LEFT_HAND_HOTKEYS
                        .iter()
                        .map(|k| p.cells.get(*k).cloned().unwrap_or_default().into())
                })
                .collect::<Vec<SharedString>>(),
        )));
        ui.set_page(ui.get_page().clamp(0, pages.len().saturating_sub(1) as i32));
        ui.set_selected(0);
    }

    pub fn apply_quick(&self, ui: &QuickPopup) {
        let count = self
            .layout
            .quick_actions
            .len()
            .div_ceil(PAGE_CELLS)
            .max(self.layout.quick_action_pages.len());
        ui.set_page_names(ModelRc::new(VecModel::from(
            (0..count)
                .map(|i| {
                    self.layout
                        .quick_action_pages
                        .get(i)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| (i + 1).to_string())
                        .into()
                })
                .collect::<Vec<SharedString>>(),
        )));
        ui.set_page(ui.get_page().clamp(0, count.saturating_sub(1) as i32));
    }

    /// Emoji in cell `index` of the current page, if it is not empty.
    pub fn emoji(&self, ui: &EmojiPopup, index: i32) -> Option<String> {
        let page = self.layout.emoji_pages.get(usize::try_from(ui.get_page()).ok()?)?;
        let key = LEFT_HAND_HOTKEYS.get(usize::try_from(index).ok()?)?;
        page.cells.get(*key).filter(|value| !value.is_empty()).cloned()
    }

    /// `(label, action)` of the quick actions on `page`; a query searches
    /// every page.
    pub fn quick_page(&self, query: &str, page: Option<usize>) -> Vec<(String, String)> {
        let query = query.to_lowercase();
        self.layout
            .quick_actions
            .iter()
            .enumerate()
            .filter(|(i, a)| {
                !a.action.trim().is_empty()
                    && (page.is_none_or(|p| i / PAGE_CELLS == p) || !query.is_empty())
            })
            .map(|(i, a)| {
                let page = self
                    .layout
                    .quick_action_pages
                    .get(i / PAGE_CELLS)
                    .map(|p| p.name.as_str())
                    .unwrap_or("");
                let key = LEFT_HAND_HOTKEYS[i % PAGE_CELLS].trim_start_matches("Key");
                let name = if a.name.is_empty() { &a.action } else { &a.name };
                (format!("{page} · {key} · {name}"), a.action.clone())
            })
            .filter(|(name, action)| {
                name.to_lowercase().contains(&query) || action.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Show the quick actions matching the popup's query and page; returns
    /// their actions in display order.
    pub fn filter_quick(&self, ui: &QuickPopup) -> Vec<String> {
        let page = usize::try_from(ui.get_page()).ok();
        let (labels, actions): (Vec<SharedString>, Vec<String>) = self
            .quick_page(&ui.get_query(), page)
            .into_iter()
            .map(|(label, action)| (label.into(), action))
            .unzip();
        ui.set_items(ModelRc::new(VecModel::from(labels)));
        ui.set_selected(0);
        actions
    }
}

/// Open `page` (1-based) of `popup`.
pub fn select_page(popup: Popup, page: u8, emoji: &EmojiPopup, quick: &QuickPopup) {
    let index = i32::from(page.saturating_sub(1));
    match popup {
        Popup::Emoji => {
            emoji.set_page(index.min((emoji.get_page_names().row_count() as i32 - 1).max(0)));
            emoji.set_selected(0);
        }
        Popup::Quick => {
            quick.set_page(index.min((quick.get_page_names().row_count() as i32 - 1).max(0)));
            quick.set_query("".into());
            quick.set_selected(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lhc_core::profile::{menus::empty_quick_action, model::*};

    #[test]
    fn navigation_wraps() {
        assert_eq!(advance(0, -1, 15), 14);
        assert_eq!(advance(12, 5, 15), 2);
        assert_eq!(advance(3, 1, 0), 0);
        let down = SharedString::from(slint::platform::Key::DownArrow);
        assert_eq!(key_delta(Popup::Emoji, &down), Some(5));
        assert_eq!(key_delta(Popup::Quick, &down), Some(1));
        let left = SharedString::from(slint::platform::Key::LeftArrow);
        assert_eq!(key_delta(Popup::Quick, &left), None);
        assert_eq!(hotkey_index("q"), Some(0));
        assert_eq!(hotkey_index("B"), Some(14));
        assert_eq!(hotkey_index("p"), None);
    }

    #[test]
    fn filtered_results_keep_their_actions_across_pages_and_empty_slots() {
        let mut layout = LayoutPreset::initial();
        layout.quick_action_pages.push(QuickActionPage {
            id: "second".into(),
            name: "Вторая".into(),
        });
        layout.quick_actions.resize_with(30, empty_quick_action);
        layout.quick_actions[2].name = "Одинаковое имя".into();
        layout.quick_actions[2].action = "text:first".into();
        layout.quick_actions[19].name = "Одинаковое имя".into();
        layout.quick_actions[19].action = "text:second".into();
        let menus = ConfiguredMenus { layout };
        assert_eq!(menus.quick_page("", Some(0))[0].1, "text:first");
        assert_eq!(menus.quick_page("", Some(1))[0].1, "text:second");
        assert_eq!(menus.quick_page("ОДИНАКОВОЕ", Some(0)).len(), 2);
        assert_eq!(menus.quick_page("ВТОРАЯ", Some(0))[0].1, "text:second");
        assert!(menus.quick_page("missing", Some(1)).is_empty());
    }
}
