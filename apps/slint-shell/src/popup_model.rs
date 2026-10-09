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

/// What a key press in a popup asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    Dismiss,
    /// Run the cell at this index of the current page.
    ChooseCell(i32),
    PageChanged,
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shortcut {
    ChooseCell(i32),
    ChangePage(i32),
    /// Digits 1…9 and 0 open pages 1…10.
    OpenPage(i32),
}

/// Page opened by a digit: `1` → 0, …, `9` → 8, `0` → 9.
fn digit_page(digit: &str) -> Option<i32> {
    match digit.parse::<i32>().ok()? {
        0 => Some(9),
        digit @ 1..=9 => Some(digit - 1),
        _ => None,
    }
}

/// Tab cycles pages, digits open one; the left-hand hotkeys pick a cell.
pub(crate) fn shortcut(code: &str, shift: bool) -> Option<Shortcut> {
    if code == "Tab" {
        return Some(Shortcut::ChangePage(if shift { -1 } else { 1 }));
    }
    if let Some(page) = code.strip_prefix("Digit").and_then(digit_page) {
        return Some(Shortcut::OpenPage(page));
    }
    LEFT_HAND_HOTKEYS
        .iter()
        .position(|key| *key == code)
        .map(|index| Shortcut::ChooseCell(index as i32))
}

#[cfg(all(feature = "spell", target_os = "linux"))]
pub(crate) fn evdev_shortcut(code: u32, shift: bool) -> Option<Shortcut> {
    const DIGITS: [&str; 10] = [
        "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9",
        "Digit0",
    ];
    let name = match code {
        2..=11 => DIGITS[code as usize - 2],
        15 => "Tab",
        20 => "KeyT",
        16 => "KeyQ",
        17 => "KeyW",
        18 => "KeyE",
        19 => "KeyR",
        30 => "KeyA",
        31 => "KeyS",
        32 => "KeyD",
        33 => "KeyF",
        34 => "KeyG",
        44 => "KeyZ",
        45 => "KeyX",
        46 => "KeyC",
        47 => "KeyV",
        48 => "KeyB",
        _ => return None,
    };
    shortcut(name, shift)
}

pub(crate) fn apply_shortcut(
    popup: Popup,
    shortcut: Shortcut,
    emoji: &EmojiPopup,
    quick: &QuickPopup,
) -> KeyOutcome {
    match shortcut {
        Shortcut::ChooseCell(index) => KeyOutcome::ChooseCell(index),
        Shortcut::ChangePage(delta) => {
            match popup {
                Popup::Emoji => emoji.set_page(advance(
                    emoji.get_page(),
                    delta,
                    emoji.get_page_names().row_count(),
                )),
                Popup::Quick => quick.set_page(advance(
                    quick.get_page(),
                    delta,
                    quick.get_page_names().row_count(),
                )),
            }
            KeyOutcome::PageChanged
        }
        Shortcut::OpenPage(page) => {
            let (count, set): (usize, &dyn Fn(i32)) = match popup {
                Popup::Emoji => (emoji.get_page_names().row_count(), &|p| emoji.set_page(p)),
                Popup::Quick => (quick.get_page_names().row_count(), &|p| quick.set_page(p)),
            };
            if (page as usize) < count {
                set(page);
                KeyOutcome::PageChanged
            } else {
                KeyOutcome::Ignored
            }
        }
    }
}

pub fn advance(selected: i32, delta: i32, count: usize) -> i32 {
    if count == 0 {
        0
    } else {
        (selected + delta).rem_euclid(count as i32)
    }
}

/// Index of the left-hand hotkey typed as `key` (`q` → 0, `b` → 14).
fn hotkey_index(key: &str) -> Option<usize> {
    LEFT_HAND_HOTKEYS.iter().position(|code| {
        code.strip_prefix("Key")
            .is_some_and(|letter| key.eq_ignore_ascii_case(letter))
    })
}

/// Fallback for typed text when the physical shortcut was not seen:
/// Escape closes, Tab and digits switch pages, hotkey letters pick a cell.
fn popup_key(page: i32, pages: usize, key: &str) -> (Option<i32>, KeyOutcome) {
    use slint::platform::Key;
    if key == SharedString::from(Key::Escape).as_str() {
        return (None, KeyOutcome::Dismiss);
    }
    if key == SharedString::from(Key::Tab).as_str() {
        return (Some(advance(page, 1, pages)), KeyOutcome::PageChanged);
    }
    if key == SharedString::from(Key::Backtab).as_str() {
        return (Some(advance(page, -1, pages)), KeyOutcome::PageChanged);
    }
    if let Some(page) = digit_page(key) {
        return if (page as usize) < pages {
            (Some(page), KeyOutcome::PageChanged)
        } else {
            (None, KeyOutcome::Ignored)
        };
    }
    match hotkey_index(key) {
        Some(index) => (None, KeyOutcome::ChooseCell(index as i32)),
        None => (None, KeyOutcome::Ignored),
    }
}

pub fn emoji_key(ui: &EmojiPopup, key: &str) -> KeyOutcome {
    let (page, outcome) = popup_key(ui.get_page(), ui.get_page_names().row_count(), key);
    if let Some(page) = page {
        ui.set_page(page);
    }
    outcome
}

pub fn quick_key(ui: &QuickPopup, key: &str) -> KeyOutcome {
    let (page, outcome) = popup_key(ui.get_page(), ui.get_page_names().row_count(), key);
    if let Some(page) = page {
        ui.set_page(page);
    }
    outcome
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
        let page = self
            .layout
            .emoji_pages
            .get(usize::try_from(ui.get_page()).ok()?)?;
        let key = LEFT_HAND_HOTKEYS.get(usize::try_from(index).ok()?)?;
        page.cells
            .get(*key)
            .filter(|value| !value.is_empty())
            .cloned()
    }

    pub fn quick_cell(&self, page: i32, index: i32) -> Option<String> {
        let page = usize::try_from(page).ok()?;
        let index = usize::try_from(index)
            .ok()
            .filter(|index| *index < PAGE_CELLS)?;
        self.layout
            .quick_actions
            .get(page.checked_mul(PAGE_CELLS)?.checked_add(index)?)
            .filter(|item| !item.action.trim().is_empty())
            .map(|item| item.action.clone())
    }

    /// Labels of the 15 cells of quick action `page`; empty cells have
    /// no action.
    pub fn quick_labels(&self, page: i32) -> Vec<String> {
        (0..PAGE_CELLS as i32)
            .map(|index| {
                let Some(action) = self.quick_cell(page, index) else {
                    return String::new();
                };
                let item = &self.layout.quick_actions[page as usize * PAGE_CELLS + index as usize];
                if item.name.trim().is_empty() {
                    action
                } else {
                    item.name.clone()
                }
            })
            .collect()
    }

    /// Show the cells of the popup's current page.
    pub fn fill_quick(&self, ui: &QuickPopup) {
        ui.set_items(ModelRc::new(VecModel::from(
            self.quick_labels(ui.get_page())
                .into_iter()
                .map(SharedString::from)
                .collect::<Vec<_>>(),
        )));
    }
}

/// Open `page` (1-based) of `popup`.
pub fn select_page(popup: Popup, page: u8, emoji: &EmojiPopup, quick: &QuickPopup) {
    let index = i32::from(page.saturating_sub(1));
    match popup {
        Popup::Emoji => {
            emoji.set_page(index.min((emoji.get_page_names().row_count() as i32 - 1).max(0)));
        }
        Popup::Quick => {
            quick.set_page(index.min((quick.get_page_names().row_count() as i32 - 1).max(0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lhc_core::profile::{menus::empty_quick_action, model::*};

    #[test]
    fn physical_shortcuts_pick_cells_and_cycle_pages() {
        for (index, code) in LEFT_HAND_HOTKEYS.iter().enumerate() {
            assert_eq!(
                shortcut(code, false),
                Some(Shortcut::ChooseCell(index as i32))
            );
        }
        assert_eq!(shortcut("Tab", false), Some(Shortcut::ChangePage(1)));
        assert_eq!(shortcut("Tab", true), Some(Shortcut::ChangePage(-1)));
        assert_eq!(shortcut("Digit1", false), Some(Shortcut::OpenPage(0)));
        assert_eq!(shortcut("Digit0", false), Some(Shortcut::OpenPage(9)));
        assert_eq!(shortcut("Enter", false), None);
        assert_eq!(shortcut("Slash", false), None);
    }

    #[cfg(all(feature = "spell", target_os = "linux"))]
    #[test]
    fn spell_evdev_codes_match_the_winit_shortcuts() {
        for (index, code) in [16, 17, 18, 19, 20, 30, 31, 32, 33, 34, 44, 45, 46, 47, 48]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                evdev_shortcut(code, false),
                Some(Shortcut::ChooseCell(index as i32))
            );
        }
        assert_eq!(evdev_shortcut(15, true), Some(Shortcut::ChangePage(-1)));
        assert_eq!(evdev_shortcut(2, false), Some(Shortcut::OpenPage(0)));
        assert_eq!(evdev_shortcut(11, false), Some(Shortcut::OpenPage(9)));
    }

    #[test]
    fn typed_keys_wrap_pages_and_ignore_enter() {
        assert_eq!(advance(0, -1, 3), 2);
        assert_eq!(advance(2, 1, 3), 0);
        assert_eq!(advance(3, 1, 0), 0);
        let tab = SharedString::from(slint::platform::Key::Tab);
        assert_eq!(popup_key(1, 2, &tab), (Some(0), KeyOutcome::PageChanged));
        let enter = SharedString::from(slint::platform::Key::Return);
        assert_eq!(popup_key(0, 2, &enter), (None, KeyOutcome::Ignored));
        assert_eq!(popup_key(0, 2, "\n"), (None, KeyOutcome::Ignored));
        assert_eq!(popup_key(0, 2, "2"), (Some(1), KeyOutcome::PageChanged));
        assert_eq!(popup_key(0, 2, "3"), (None, KeyOutcome::Ignored));
        assert_eq!(popup_key(0, 10, "0"), (Some(9), KeyOutcome::PageChanged));
        assert_eq!(popup_key(0, 2, "B"), (None, KeyOutcome::ChooseCell(14)));
        assert_eq!(hotkey_index("q"), Some(0));
        assert_eq!(hotkey_index("p"), None);
    }

    #[test]
    fn quick_cells_follow_hotkey_positions() {
        let mut layout = LayoutPreset::initial();
        layout.quick_action_pages.push(QuickActionPage {
            id: "second".into(),
            name: "Вторая".into(),
        });
        layout.quick_actions.resize_with(30, empty_quick_action);
        layout.quick_actions[2].name = "Имя".into();
        layout.quick_actions[2].action = "text:first".into();
        layout.quick_actions[19].action = "text:second".into();
        let menus = ConfiguredMenus { layout };
        let first = menus.quick_labels(0);
        assert_eq!(first.len(), PAGE_CELLS);
        assert_eq!(first[2], "Имя");
        assert!(first[0].is_empty());
        assert_eq!(menus.quick_labels(1)[4], "text:second");
        assert_eq!(menus.quick_cell(0, 2).as_deref(), Some("text:first"));
        assert_eq!(menus.quick_cell(1, 4).as_deref(), Some("text:second"));
        assert_eq!(menus.quick_cell(0, 0), None);
        assert_eq!(menus.quick_cell(0, 15), None);
        assert_eq!(menus.quick_cell(-1, 0), None);
    }
}
