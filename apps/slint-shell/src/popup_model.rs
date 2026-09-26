//! Popup data and navigation shared by the winit popups and the Spell worker.

use crate::command::Popup;
use slint::{Model, SharedString};

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

#[derive(Default)]
pub struct ConfiguredMenus {
    pub layout: lhc_core::profile::model::LayoutPreset,
}
impl ConfiguredMenus {
    pub fn load() -> Result<Self, String> {
        let paths = lhc_core::storage::StoragePaths::resolve()?;
        let document =
            lhc_core::config_document::ConfigDocument::load(paths).map_err(|e| e.to_string())?;
        Ok(Self {
            layout: document
                .active_layout(&lhc_core::profile::auto_switch::AutoSwitchContext::current())
                .map_err(|error| error.to_string())?,
        })
    }
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
    pub fn apply_emoji(&self, ui: &crate::ui::EmojiPopup) {
        use lhc_core::profile::model::LEFT_HAND_HOTKEYS;
        ui.set_configured(true);
        ui.set_page_names(slint::ModelRc::new(slint::VecModel::from(
            self.layout
                .emoji_pages
                .iter()
                .map(|p| p.name.clone().into())
                .collect::<Vec<SharedString>>(),
        )));
        ui.set_emojis(slint::ModelRc::new(slint::VecModel::from(
            self.layout
                .emoji_pages
                .iter()
                .flat_map(|p| {
                    LEFT_HAND_HOTKEYS
                        .iter()
                        .map(|k| p.cells.get(*k).cloned().unwrap_or_default().into())
                })
                .collect::<Vec<SharedString>>(),
        )));
        ui.set_page(
            ui.get_page()
                .max(0)
                .min(self.layout.emoji_pages.len().saturating_sub(1) as i32),
        );
        ui.set_selected(0);
    }
    pub fn apply_quick(&self, ui: &crate::ui::QuickPopup) {
        let count = self
            .layout
            .quick_actions
            .len()
            .div_ceil(15)
            .max(self.layout.quick_action_pages.len());
        ui.set_page_names(slint::ModelRc::new(slint::VecModel::from(
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
        ui.set_page(ui.get_page().max(0).min(count.saturating_sub(1) as i32));
    }
    pub fn quick(&self, query: &str) -> Vec<(String, String)> {
        self.quick_page(query, None)
    }
    pub fn quick_page(&self, query: &str, page: Option<usize>) -> Vec<(String, String)> {
        let query = query.to_lowercase();
        self.layout
            .quick_actions
            .iter()
            .enumerate()
            .filter(|(i, a)| {
                !a.action.trim().is_empty()
                    && (page.is_none_or(|p| i / 15 == p) || !query.is_empty())
            })
            .map(|(i, a)| {
                let page = self
                    .layout
                    .quick_action_pages
                    .get(i / 15)
                    .map(|p| p.name.as_str())
                    .unwrap_or("");
                let key =
                    lhc_core::profile::model::LEFT_HAND_HOTKEYS[i % 15].trim_start_matches("Key");
                let name = if a.name.is_empty() {
                    &a.action
                } else {
                    &a.name
                };
                (format!("{page} · {key} · {name}"), a.action.clone())
            })
            .filter(|(name, action)| {
                name.to_lowercase().contains(&query) || action.to_lowercase().contains(&query)
            })
            .collect()
    }
}

pub fn configured_emoji_index(ui: &crate::ui::EmojiPopup, index: i32) -> Option<usize> {
    if ui.get_configured() {
        (0..15)
            .contains(&index)
            .then(|| ui.get_page() as usize * 15 + index as usize)
    } else {
        emoji_index(ui.get_page(), index)
    }
}

#[cfg(test)]
mod configured_tests {
    use super::*;
    use lhc_core::profile::{menus::empty_quick_action, model::*};
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

pub fn select_page(
    popup: Popup,
    page: u8,
    emoji: &crate::ui::EmojiPopup,
    quick: &crate::ui::QuickPopup,
) {
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
