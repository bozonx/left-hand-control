//! Popup data and keyboard handling shared by the winit popups and the
//! Spell worker.

use crate::{
    command::Popup,
    ui::{EmojiPopup, QuickPopup},
};
use lhc_core::profile::{
    actions::Action,
    model::{LEFT_HAND_HOTKEYS, LayoutPreset, QuickAction},
};
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

/// Text a popup action types, if it is a `text:` action.
#[cfg(any(not(target_os = "linux"), feature = "probes"))]
pub(crate) fn typed_text(action: &str) -> Option<String> {
    match Action::parse(Some(action)) {
        Action::Text(text) => Some(text),
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

/// Same as [`shortcut`] for an evdev key code, as the Spell worker reads it.
#[cfg(all(feature = "spell", target_os = "linux"))]
pub(crate) fn evdev_shortcut(code: u32, shift: bool) -> Option<Shortcut> {
    const DIGITS: [&str; 10] = [
        "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9",
        "Digit0",
    ];
    let name = ["Tab"]
        .iter()
        .chain(&DIGITS)
        .chain(&LEFT_HAND_HOTKEYS)
        .find(|name| {
            lhc_core::mapper::keys::code_to_key(name)
                .is_some_and(|key| u32::from(key.code()) == code)
        })?;
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
        let count = self.layout.quick_action_pages.len();
        ui.set_page_names(ModelRc::new(VecModel::from(
            self.layout
                .quick_action_pages
                .iter()
                .map(|p| p.name.as_str().into())
                .collect::<Vec<SharedString>>(),
        )));
        ui.set_page(ui.get_page().clamp(0, count.saturating_sub(1) as i32));
    }

    /// Apply the menus to both popups. Each popup stays on the page it
    /// showed under `previous` while a page with that id exists, so the
    /// last used page survives edits, reordering and layout switches.
    pub fn apply(
        &self,
        previous: Option<&ConfiguredMenus>,
        emoji: &EmojiPopup,
        quick: &QuickPopup,
    ) {
        let emoji_ids = |menus: &ConfiguredMenus| -> Vec<String> {
            menus
                .layout
                .emoji_pages
                .iter()
                .map(|p| p.id.clone())
                .collect()
        };
        let quick_ids = |menus: &ConfiguredMenus| -> Vec<String> {
            menus
                .layout
                .quick_action_pages
                .iter()
                .map(|p| p.id.clone())
                .collect()
        };
        let emoji_page = previous
            .and_then(|old| moved_page(&emoji_ids(old), emoji.get_page(), &emoji_ids(self)));
        let quick_page = previous
            .and_then(|old| moved_page(&quick_ids(old), quick.get_page(), &quick_ids(self)));
        self.apply_emoji(emoji);
        self.apply_quick(quick);
        if let Some(page) = emoji_page {
            emoji.set_page(page);
        }
        if let Some(page) = quick_page {
            quick.set_page(page);
        }
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

    /// Action of cell `index` on the open page of `popup`; an emoji is
    /// typed as text.
    pub fn action(
        &self,
        popup: Popup,
        emoji: &EmojiPopup,
        quick: &QuickPopup,
        index: i32,
    ) -> Option<String> {
        match popup {
            Popup::Emoji => self
                .emoji(emoji, index)
                .and_then(|emoji| Action::Text(emoji).format()),
            Popup::Quick => self.quick_cell(quick.get_page(), index),
        }
    }

    pub fn quick_cell(&self, page: i32, index: i32) -> Option<String> {
        self.quick_item(page, index).map(|item| item.action.clone())
    }

    fn quick_item(&self, page: i32, index: i32) -> Option<&QuickAction> {
        let page = self
            .layout
            .quick_action_pages
            .get(usize::try_from(page).ok()?)?;
        let key = LEFT_HAND_HOTKEYS.get(usize::try_from(index).ok()?)?;
        page.cells
            .get(*key)
            .filter(|item| !item.action.trim().is_empty())
    }

    /// Labels of the 15 cells of quick action `page`; empty cells have
    /// no action.
    pub fn quick_labels(&self, page: i32) -> Vec<String> {
        (0..PAGE_CELLS as i32)
            .map(|index| {
                let Some(item) = self.quick_item(page, index) else {
                    return String::new();
                };
                if item.name.trim().is_empty() {
                    item.action.clone()
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

/// Index in `current` of the page that was `page` in `previous`.
fn moved_page(previous: &[String], page: i32, current: &[String]) -> Option<i32> {
    let id = previous.get(usize::try_from(page).ok()?)?;
    current
        .iter()
        .position(|item| item == id)
        .and_then(|index| i32::try_from(index).ok())
}

/// Open command for an already visible popup. Repeating the command that
/// opened it (`last` is its page) or a command without a page turns to the
/// next page like Tab; a command for another page opens that page directly.
pub fn repeat_open(
    popup: Popup,
    page: Option<u32>,
    last: Option<u32>,
    emoji: &EmojiPopup,
    quick: &QuickPopup,
) {
    match popup {
        Popup::Emoji => emoji.set_page(repeat_page(
            emoji.get_page(),
            emoji.get_page_names().row_count(),
            page,
            last,
        )),
        Popup::Quick => quick.set_page(repeat_page(
            quick.get_page(),
            quick.get_page_names().row_count(),
            page,
            last,
        )),
    }
}

/// Page index a repeated open command leaves the popup on; see `repeat_open`.
fn repeat_page(current: i32, count: usize, page: Option<u32>, last: Option<u32>) -> i32 {
    match page {
        Some(page) if Some(page) != last && page_index(page) != current => {
            page_index(page).min((count as i32 - 1).max(0))
        }
        _ => advance(current, 1, count),
    }
}

/// Zero-based index of the 1-based `page`; a page past the end is
/// clamped by the caller.
fn page_index(page: u32) -> i32 {
    i32::try_from(page.saturating_sub(1)).unwrap_or(i32::MAX)
}

/// Open `page` (1-based) of `popup`.
pub fn select_page(popup: Popup, page: u32, emoji: &EmojiPopup, quick: &QuickPopup) {
    let index = page_index(page);
    match popup {
        Popup::Emoji => {
            emoji.set_page(index.min((emoji.get_page_names().row_count() as i32 - 1).max(0)));
        }
        Popup::Quick => {
            quick.set_page(index.min((quick.get_page_names().row_count() as i32 - 1).max(0)));
        }
    }
}

/// Run a chosen action at the latest this long after its popup hid, even
/// if the window system did not report the focus leaving the popup.
pub const FOCUS_RETURN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(300);

/// An action chosen in a popup, waiting for the keyboard focus to return
/// to the application it is meant for.
#[derive(Debug)]
pub struct PendingAction {
    pub popup: Popup,
    pub action: String,
    pub chosen: std::time::Instant,
}

/// State of the popups shared by the winit windows and the Spell worker:
/// which popup is open and focused, the page its open command asked for,
/// and the chosen action that waits for the focus to leave.
#[derive(Debug, Default)]
pub struct PopupSession {
    visible: Option<Popup>,
    focused: Option<Popup>,
    /// Page of the command that opened or last turned the visible popup;
    /// tells a repeated open command from one for another page.
    requested_page: Option<u32>,
    pending: Option<PendingAction>,
}

impl PopupSession {
    pub fn visible(&self) -> Option<Popup> {
        self.visible
    }

    pub fn focused(&self) -> Option<Popup> {
        self.focused
    }

    /// Handle a command opening `popup` at `page`. An open popup turns its
    /// page and `true` is returned; a hidden one selects the page before it
    /// is shown.
    pub fn open(
        &mut self,
        popup: Popup,
        page: Option<u32>,
        open: bool,
        emoji: &EmojiPopup,
        quick: &QuickPopup,
    ) -> bool {
        let last = std::mem::replace(&mut self.requested_page, page);
        if open {
            repeat_open(popup, page, last, emoji, quick);
            return true;
        }
        if let Some(page) = page {
            select_page(popup, page, emoji, quick);
        }
        false
    }

    /// `popup` is now shown; returns the popup it replaced.
    pub fn shown(&mut self, popup: Popup) -> Option<Popup> {
        self.focused = None;
        self.visible
            .replace(popup)
            .filter(|previous| *previous != popup)
    }

    /// `popup` was hidden; `false` when it was not the visible one.
    pub fn hidden(&mut self, popup: Popup) -> bool {
        if self.visible != Some(popup) {
            return false;
        }
        self.visible = None;
        self.focused = None;
        true
    }

    /// Keyboard focus of `popup` changed. Returns the pending action to run
    /// now that the focus left its popup, and whether the visible popup
    /// lost the focus it had and should hide.
    pub fn focus(&mut self, popup: Popup, active: bool) -> (Option<PendingAction>, FocusChange) {
        let action = if active {
            None
        } else {
            self.pending.take_if(|pending| pending.popup == popup)
        };
        let change = if self.visible != Some(popup) {
            FocusChange::None
        } else if active {
            self.focused = Some(popup);
            FocusChange::Entered
        } else if self.focused == Some(popup) {
            FocusChange::Left
        } else {
            FocusChange::LeftBeforeEnter
        };
        (action, change)
    }

    /// Run `action` once the focus left `popup`, or after the timeout.
    pub fn choose(&mut self, popup: Popup, action: String) {
        self.pending = Some(PendingAction {
            popup,
            action,
            chosen: std::time::Instant::now(),
        });
    }

    /// The pending action of `popup`, whatever the focus did.
    pub fn take_pending(&mut self, popup: Popup) -> Option<PendingAction> {
        self.pending.take_if(|pending| pending.popup == popup)
    }

    /// The pending action whose focus return timed out.
    pub fn timed_out(&mut self) -> Option<PendingAction> {
        self.pending
            .take_if(|pending| pending.chosen.elapsed() >= FOCUS_RETURN_TIMEOUT)
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

/// What a focus change means for the visible popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusChange {
    /// Not about the visible popup.
    None,
    Entered,
    /// The popup lost the focus it had: hide it.
    Left,
    /// The focus left before the popup ever had it.
    LeftBeforeEnter,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lhc_core::profile::model::*;

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
    fn repeated_open_command_cycles_all_pages() {
        // The same page-bound hotkey keeps turning pages, also past page 2.
        let mut page = 0;
        let mut seen = Vec::new();
        for _ in 0..4 {
            page = repeat_page(page, 3, Some(1), Some(1));
            seen.push(page);
        }
        assert_eq!(seen, [1, 2, 0, 1]);
        // Without a page every repeat turns the page as well.
        assert_eq!(repeat_page(2, 3, None, None), 0);
        // A hotkey for another page opens it directly, clamped to the last.
        assert_eq!(repeat_page(0, 3, Some(3), Some(1)), 2);
        assert_eq!(repeat_page(0, 3, Some(9), None), 2);
        // A command for the page already shown turns it even if it is new.
        assert_eq!(repeat_page(0, 3, Some(1), None), 1);
    }

    #[test]
    fn last_page_follows_its_id() {
        let ids = |items: &[&str]| items.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        let before = ids(&["a", "b", "c"]);
        assert_eq!(moved_page(&before, 1, &ids(&["b", "a"])), Some(0));
        assert_eq!(moved_page(&before, 2, &ids(&["a", "b"])), None);
        assert_eq!(moved_page(&before, 5, &before), None);
        assert_eq!(moved_page(&before, -1, &before), None);
    }

    #[test]
    fn quick_cells_follow_hotkey_positions() {
        let mut layout = LayoutPreset::initial();
        layout
            .quick_action_pages
            .push(QuickActionPage::new("second".into(), "Вторая".into()));
        layout.quick_action_pages[0].cells.insert(
            "KeyE".into(),
            QuickAction {
                name: "Имя".into(),
                action: "text:first".into(),
                icon: None,
            },
        );
        layout.quick_action_pages[1].cells.insert(
            "KeyT".into(),
            QuickAction {
                action: "text:second".into(),
                ..QuickAction::default()
            },
        );
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

    #[test]
    fn session_runs_a_choice_when_the_focus_leaves_its_popup() {
        let mut session = PopupSession::default();
        assert_eq!(session.shown(Popup::Emoji), None);
        assert_eq!(session.focus(Popup::Emoji, true).1, FocusChange::Entered);
        session.choose(Popup::Emoji, "text:x".into());
        assert!(session.timed_out().is_none());
        assert!(session.hidden(Popup::Emoji));
        assert!(!session.hidden(Popup::Emoji));
        // Focus of another popup does not run it.
        assert!(session.focus(Popup::Quick, false).0.is_none());
        let (action, change) = session.focus(Popup::Emoji, false);
        assert_eq!(action.map(|a| a.action).as_deref(), Some("text:x"));
        assert_eq!(change, FocusChange::None);
        assert!(!session.has_pending());
    }

    #[test]
    fn session_hides_a_popup_only_after_it_had_the_focus() {
        let mut session = PopupSession::default();
        session.shown(Popup::Quick);
        assert_eq!(
            session.focus(Popup::Quick, false).1,
            FocusChange::LeftBeforeEnter
        );
        session.focus(Popup::Quick, true);
        assert_eq!(session.focus(Popup::Quick, false).1, FocusChange::Left);
        assert_eq!(session.shown(Popup::Emoji), Some(Popup::Quick));
        assert_eq!(session.focused(), None);
    }
}
