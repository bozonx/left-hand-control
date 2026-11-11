//! Emoji and quick action menus: validation, page reordering and command
//! placement.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuIssue {
    EmojiCell,
    UnknownKey,
    CommandId,
    DuplicateCommand,
    EmptyCommand,
}

use super::{
    actions::{self, Action},
    model::*,
};
use crate::config_document::{ConfigDocument, ConfigError};
use std::collections::BTreeMap;

/// Set or clear a quick action, preserving a custom name.
/// `previous_label` is the old action's catalog label in the current UI language.
pub fn set_quick_action(
    keymap: &mut KeymapPreset,
    page: i32,
    cell: i32,
    action: &str,
    label: &str,
    previous_label: &str,
) -> Result<(), ConfigError> {
    let out_of_range =
        || ConfigError::Invalid(format!("no quick action cell {cell} on page {page}"));
    let (Ok(page_index), Ok(cell_index)) = (usize::try_from(page), usize::try_from(cell)) else {
        return Err(out_of_range());
    };
    let (Some(page), Some(key)) = (
        keymap.quick_action_pages.get_mut(page_index),
        LEFT_HAND_HOTKEYS.get(cell_index),
    ) else {
        return Err(out_of_range());
    };
    if action.is_empty() {
        page.cells.remove(*key);
        return Ok(());
    }
    let item = page.cells.entry((*key).into()).or_default();
    let automatic = item.name.trim().is_empty()
        || item.name == item.action
        || (!previous_label.is_empty() && item.name == previous_label);
    if item.action.trim().is_empty() {
        *item = QuickAction {
            name: display_name(label, action).into(),
            action: action.to_string(),
        };
    } else {
        item.action = action.to_string();
        if automatic {
            item.name = display_name(label, action).into();
        }
    }
    Ok(())
}

impl ConfigDocument {
    pub fn save_menu_pages(
        &mut self,
        baseline: &KeymapPreset,
        candidate: &KeymapPreset,
    ) -> Result<(), ConfigError> {
        let current = self.keymap();
        let emoji_changed = baseline.emoji_pages != candidate.emoji_pages;
        let quick_changed = baseline.quick_action_pages != candidate.quick_action_pages;
        if (emoji_changed && current.emoji_pages != baseline.emoji_pages)
            || (quick_changed && current.quick_action_pages != baseline.quick_action_pages)
        {
            return Err(ConfigError::EditConflict);
        }
        // Validate against current commands, macros and the untouched menu.
        let mut merged = current.clone();
        if emoji_changed {
            merged.emoji_pages = candidate.emoji_pages.clone();
        }
        if quick_changed {
            merged.quick_action_pages = candidate.quick_action_pages.clone();
        }
        let candidate = &merged;
        for page in &candidate.emoji_pages {
            if page.cells.iter().any(|(key, value)| {
                !LEFT_HAND_HOTKEYS.contains(&key.as_str()) || value.chars().take(101).count() > 100
            }) {
                return Err(ConfigError::Menu(MenuIssue::EmojiCell));
            }
        }
        let config = ResolvedAppConfig::from_parts(
            self.settings().clone(),
            candidate.clone(),
            self.settings().editing_keymap_id.as_deref(),
        );
        for page in &candidate.quick_action_pages {
            if page
                .cells
                .keys()
                .any(|key| !LEFT_HAND_HOTKEYS.contains(&key.as_str()))
            {
                return Err(ConfigError::Menu(MenuIssue::UnknownKey));
            }
        }
        // A cell kept as saved (same page, key and action) is not checked
        // again, so an action that became unavailable elsewhere does not block
        // unrelated edits. A moved or edited cell is checked.
        let unchanged = |page_id: &str, key: &str, action: &str| {
            baseline
                .quick_action_pages
                .iter()
                .find(|page| page.id == page_id)
                .and_then(|page| page.cells.get(key))
                .is_some_and(|old| old.action == action)
        };
        for (page, key, item) in candidate.quick_action_pages.iter().flat_map(|page| {
            page.cells
                .iter()
                .map(move |(key, item)| (page.id.as_str(), key.as_str(), item))
        }) {
            if unchanged(page, key, &item.action) {
                continue;
            }
            if item.action.trim().is_empty() {
                continue;
            }
            let action = Action::parse(Some(&item.action));
            if let Some(issue) = actions::validate(&action, &config) {
                return Err(ConfigError::InvalidAction(issue));
            }
            #[cfg(target_os = "linux")]
            if matches!(action, Action::Keys(_))
                && crate::mapper::action::parse_action(&item.action).is_none()
            {
                return Err(ConfigError::Menu(MenuIssue::UnknownKey));
            }
        }
        self.update_keymap(|keymap| {
            keymap.emoji_pages = candidate.emoji_pages.clone();
            keymap.quick_action_pages = candidate.quick_action_pages.clone();
        })?;
        Ok(())
    }

    pub fn save_command(&mut self, mut command: Command) -> Result<(), ConfigError> {
        if !self.settings().commands_enabled {
            return Err(ConfigError::Invalid("Commands are disabled".into()));
        }
        if command.name.trim().is_empty() {
            command.name = command.script().trim().to_owned();
        }
        let mut commands = self.keymap().commands.clone();
        let index = commands
            .iter()
            .position(|item| item.id == command.id)
            .unwrap_or(commands.len());
        if index == commands.len() {
            commands.push(command);
        } else {
            commands[index] = command;
        }
        if let Some(issue) = command_issue(&commands, index) {
            return Err(ConfigError::Menu(issue));
        }
        self.update_keymap(|keymap| keymap.commands = commands)
    }
}

/// First problem of `commands[index]`, checked against the commands before it.
pub fn command_issue(commands: &[Command], index: usize) -> Option<MenuIssue> {
    let command = commands.get(index)?;
    if !super::ids::valid(&command.id) {
        return Some(MenuIssue::CommandId);
    }
    if commands[..index].iter().any(|c| c.id == command.id) {
        return Some(MenuIssue::DuplicateCommand);
    }
    if command.script().trim().is_empty() {
        return Some(MenuIssue::EmptyCommand);
    }
    None
}

/// Which of the two keymap menus an operation applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Emoji,
    Quick,
}

/// A menu page: an id, an optional caption and cells keyed by hotkey.
trait MenuPage {
    type Cell;
    fn empty() -> Self;
    fn name_mut(&mut self) -> &mut String;
    fn cells_mut(&mut self) -> &mut BTreeMap<String, Self::Cell>;
}

impl MenuPage for EmojiPage {
    type Cell = String;
    fn empty() -> Self {
        EmojiPage {
            id: super::ids::generate("emoji-"),
            name: String::new(),
            cells: BTreeMap::new(),
        }
    }
    fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }
    fn cells_mut(&mut self) -> &mut BTreeMap<String, String> {
        &mut self.cells
    }
}

impl MenuPage for QuickActionPage {
    type Cell = QuickAction;
    fn empty() -> Self {
        QuickActionPage::new(super::ids::generate("page-"), String::new())
    }
    fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }
    fn cells_mut(&mut self) -> &mut BTreeMap<String, QuickAction> {
        &mut self.cells
    }
}

/// Run a generic page operation on the pages of `kind`.
macro_rules! on_pages {
    ($keymap:expr, $kind:expr, $op:ident($($arg:expr),*)) => {
        match $kind {
            MenuKind::Emoji => $op(&mut $keymap.emoji_pages $(, $arg)*),
            MenuKind::Quick => $op(&mut $keymap.quick_action_pages $(, $arg)*),
        }
    };
}

/// Each menu has at least one page: the editor and `tecloid:show*Menu1` need it.
pub fn ensure_pages(keymap: &mut KeymapPreset) {
    fn ensure<P: MenuPage>(pages: &mut Vec<P>) {
        if pages.is_empty() {
            pages.push(P::empty());
        }
    }
    ensure(&mut keymap.emoji_pages);
    ensure(&mut keymap.quick_action_pages);
}

pub fn page_count(keymap: &KeymapPreset, kind: MenuKind) -> usize {
    match kind {
        MenuKind::Emoji => keymap.emoji_pages.len(),
        MenuKind::Quick => keymap.quick_action_pages.len(),
    }
}

/// Move a page and its cells together, preserving the order of other pages.
/// Returns whether anything moved.
///
/// `tecloid:showQuickMenuN` / `tecloid:showEmojiMenuN` open a page by position and
/// are deliberately left as they are: moving a page changes what those keys
/// open. See [`actions::MENU_PAGE_PICKER_COUNT`].
pub fn reorder_page(keymap: &mut KeymapPreset, kind: MenuKind, from: usize, to: usize) -> bool {
    fn reorder<P>(pages: &mut Vec<P>, from: usize, to: usize) -> bool {
        if from == to || from >= pages.len() || to >= pages.len() {
            return false;
        }
        let page = pages.remove(from);
        pages.insert(to, page);
        true
    }
    on_pages!(keymap, kind, reorder(from, to))
}

/// Where the page at `selected` ends up after moving `from` to `to`.
pub fn selection_after_reorder(selected: usize, from: usize, to: usize) -> usize {
    if selected == from {
        to
    } else if from < selected && selected <= to {
        selected - 1
    } else if to <= selected && selected < from {
        selected + 1
    } else {
        selected
    }
}

/// Remove a page; numbered `tecloid:` menu actions keep their numbers, so the
/// following pages move into the freed positions. The only page is cleared
/// instead (name and cells), keeping its id. Returns the page to select.
pub fn remove_page(keymap: &mut KeymapPreset, kind: MenuKind, index: usize) -> usize {
    fn remove<P: MenuPage>(pages: &mut Vec<P>, index: usize) -> usize {
        if pages.len() <= 1 {
            if let Some(page) = pages.first_mut() {
                page.name_mut().clear();
                page.cells_mut().clear();
            }
            return 0;
        }
        if index < pages.len() {
            pages.remove(index);
        }
        index.saturating_sub(1).min(pages.len() - 1)
    }
    on_pages!(keymap, kind, remove(index))
}

/// Set a page caption; surrounding whitespace is dropped.
pub fn rename_page(keymap: &mut KeymapPreset, kind: MenuKind, page: usize, name: &str) {
    fn rename<P: MenuPage>(pages: &mut [P], page: usize, name: &str) {
        if let Some(page) = pages.get_mut(page) {
            *page.name_mut() = name.trim().to_owned();
        }
    }
    on_pages!(keymap, kind, rename(page, name))
}

pub fn clear_cell(keymap: &mut KeymapPreset, kind: MenuKind, page: usize, cell: usize) {
    fn clear<P: MenuPage>(pages: &mut [P], page: usize, cell: usize) {
        if let (Some(page), Some(key)) = (pages.get_mut(page), LEFT_HAND_HOTKEYS.get(cell)) {
            page.cells_mut().remove(*key);
        }
    }
    on_pages!(keymap, kind, clear(page, cell))
}

/// Move the cell at `(page, cell)` `from` onto `to`, on the same or another
/// page; the target's previous contents move to the source. Other cells keep
/// their places.
pub fn move_cell(
    keymap: &mut KeymapPreset,
    kind: MenuKind,
    from: (usize, usize),
    to: (usize, usize),
) {
    fn swap<P: MenuPage>(pages: &mut [P], from: (usize, usize), to: (usize, usize)) {
        let (Some(source), Some(target)) =
            (LEFT_HAND_HOTKEYS.get(from.1), LEFT_HAND_HOTKEYS.get(to.1))
        else {
            return;
        };
        if from == to || from.0 >= pages.len() || to.0 >= pages.len() {
            return;
        }
        let value = pages[from.0].cells_mut().remove(*source);
        let previous = pages[to.0].cells_mut().remove(*target);
        if let Some(previous) = previous {
            pages[from.0].cells_mut().insert((*source).into(), previous);
        }
        if let Some(value) = value {
            pages[to.0].cells_mut().insert((*target).into(), value);
        }
    }
    on_pages!(keymap, kind, swap(from, to))
}

/// Copy a cell (page, cell) onto another, replacing the target. Copying an
/// empty cell clears the target.
pub fn copy_cell(
    keymap: &mut KeymapPreset,
    kind: MenuKind,
    from: (usize, usize),
    to: (usize, usize),
) {
    fn copy<P: MenuPage>(pages: &mut [P], from: (usize, usize), to: (usize, usize))
    where
        P::Cell: Clone,
    {
        let (Some(source), Some(target)) =
            (LEFT_HAND_HOTKEYS.get(from.1), LEFT_HAND_HOTKEYS.get(to.1))
        else {
            return;
        };
        if from == to || from.0 >= pages.len() || to.0 >= pages.len() {
            return;
        }
        match pages[from.0].cells_mut().get(*source).cloned() {
            Some(value) => {
                pages[to.0].cells_mut().insert((*target).into(), value);
            }
            None => {
                pages[to.0].cells_mut().remove(*target);
            }
        }
    }
    on_pages!(keymap, kind, copy(from, to))
}

/// Whether emoji cell text counts as empty. Only ordinary whitespace does:
/// a no-break space or other spacing characters are valid insertions.
pub fn emoji_text_is_blank(text: &str) -> bool {
    text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

/// Set an emoji cell to `text` as typed, or clear it when the text is blank.
/// Leading and trailing spaces are kept: snippets like `"TODO: "` need them.
pub fn set_emoji(keymap: &mut KeymapPreset, page: usize, cell: usize, text: &str) {
    let (Some(page), Some(key)) = (
        keymap.emoji_pages.get_mut(page),
        LEFT_HAND_HOTKEYS.get(cell),
    ) else {
        return;
    };
    if emoji_text_is_blank(text) {
        page.cells.remove(*key);
    } else {
        page.cells.insert((*key).into(), text.to_owned());
    }
}

/// Set the caption of an assigned quick action; surrounding whitespace is
/// dropped, and an empty caption shows the action.
pub fn set_quick_action_name(keymap: &mut KeymapPreset, page: usize, cell: usize, name: &str) {
    if let Some(item) = keymap
        .quick_action_pages
        .get_mut(page)
        .zip(LEFT_HAND_HOTKEYS.get(cell))
        .and_then(|(page, key)| page.cells.get_mut(*key))
    {
        item.name = name.trim().to_owned();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn copy_cell_keeps_source() {
        let mut keymap = KeymapPreset::default();
        keymap.emoji_pages.push(EmojiPage::empty());
        set_emoji(&mut keymap, 0, 0, "😀");
        copy_cell(&mut keymap, MenuKind::Emoji, (0, 0), (0, 1));
        let cells = &keymap.emoji_pages[0].cells;
        assert_eq!(
            cells.get(LEFT_HAND_HOTKEYS[0]).map(String::as_str),
            Some("😀")
        );
        assert_eq!(
            cells.get(LEFT_HAND_HOTKEYS[1]).map(String::as_str),
            Some("😀")
        );
    }

    use super::*;
    use crate::storage::StoragePaths;

    #[test]
    fn quick_action_edits_preserve_custom_names_and_clear_cells() {
        let mut keymap = KeymapPreset {
            quick_action_pages: vec![QuickActionPage::new("page".into(), "".into())],
            ..KeymapPreset::default()
        };
        set_quick_action(&mut keymap, 0, 0, "KeyA", "First", "").unwrap();
        assert_eq!(keymap.quick_action_pages[0].cells["KeyQ"].name, "First");
        let item = keymap.quick_action_pages[0].cells.get_mut("KeyQ").unwrap();
        item.name = "Custom".into();
        set_quick_action(&mut keymap, 0, 0, "KeyB", "Second", "").unwrap();
        let item = &keymap.quick_action_pages[0].cells["KeyQ"];
        assert_eq!(item.name, "Custom");
        set_quick_action(&mut keymap, 0, 0, "", "", "").unwrap();
        assert!(keymap.quick_action_pages[0].cells.is_empty());
        set_quick_action(&mut keymap, 0, 0, "KeyA", "", "").unwrap();
        set_quick_action(&mut keymap, 0, 0, "KeyB", "Second", "").unwrap();
        assert_eq!(keymap.quick_action_pages[0].cells["KeyQ"].name, "Second");
        let saved = keymap.clone();
        for (page, cell) in [(-1, 0), (1, 0), (0, -1), (0, 15)] {
            assert!(matches!(
                set_quick_action(&mut keymap, page, cell, "KeyC", "", ""),
                Err(ConfigError::Invalid(_))
            ));
            assert_eq!(keymap, saved);
        }
    }

    #[test]
    fn replacement_updates_catalog_labels_but_preserves_custom_labels() {
        let mut keymap = KeymapPreset::default();
        ensure_pages(&mut keymap);
        set_quick_action(&mut keymap, 0, 0, "macro:first", "First", "").unwrap();
        set_quick_action(&mut keymap, 0, 0, "macro:second", "Second", "First").unwrap();
        assert_eq!(keymap.quick_action_pages[0].cells["KeyQ"].name, "Second");
        set_quick_action_name(&mut keymap, 0, 0, "My shortcut");
        set_quick_action(&mut keymap, 0, 0, "macro:first", "First", "Second").unwrap();
        assert_eq!(
            keymap.quick_action_pages[0].cells["KeyQ"].name,
            "My shortcut"
        );
    }

    #[test]
    fn emoji_limit_counts_unicode_code_points_and_preserves_rejected_edits() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        // Supplementary-plane emoji use two UTF-16 units, but one code point.
        // Combining marks and ZWJ are separate code points, as the UI explains.
        for text in [
            "a".repeat(100),
            "🙂".repeat(100),
            "e\u{301}".repeat(50),
            "👩\u{200d}💻".repeat(33) + "!",
        ] {
            let baseline = doc.keymap().clone();
            let mut candidate = baseline.clone();
            ensure_pages(&mut candidate);
            set_emoji(&mut candidate, 0, 0, &text);
            doc.save_menu_pages(&baseline, &candidate).unwrap();
            assert_eq!(doc.keymap().emoji_pages[0].cells["KeyQ"], text);
            let baseline = doc.keymap().clone();
            set_emoji(&mut candidate, 0, 0, &(text + "!"));
            assert!(matches!(
                doc.save_menu_pages(&baseline, &candidate),
                Err(ConfigError::Menu(MenuIssue::EmojiCell))
            ));
            assert_eq!(doc.keymap(), &baseline);
        }
    }

    #[test]
    fn transfers_and_page_reorders_preserve_cells_and_action_metadata() {
        let mut keymap = KeymapPreset {
            emoji_pages: vec![
                EmojiPage {
                    id: "a".into(),
                    name: "A".into(),
                    cells: [("KeyQ".into(), "😀".into())].into(),
                },
                EmojiPage {
                    id: "b".into(),
                    name: "B".into(),
                    cells: [("KeyW".into(), "✨".into())].into(),
                },
            ],
            quick_action_pages: vec![
                QuickActionPage::new("a".into(), "A".into()),
                QuickActionPage::new("b".into(), "B".into()),
            ],
            ..KeymapPreset::default()
        };
        move_cell(&mut keymap, MenuKind::Emoji, (0, 0), (1, 1));
        assert_eq!(keymap.emoji_pages[0].cells["KeyQ"], "✨");
        assert_eq!(keymap.emoji_pages[1].cells["KeyW"], "😀");
        move_cell(&mut keymap, MenuKind::Emoji, (1, 1), (0, 2));
        assert!(!keymap.emoji_pages[1].cells.contains_key("KeyW"));
        assert_eq!(keymap.emoji_pages[0].cells["KeyE"], "😀");
        reorder_page(&mut keymap, MenuKind::Emoji, 0, 1);
        assert_eq!(keymap.emoji_pages[1].id, "a");
        assert_eq!(keymap.emoji_pages[1].cells["KeyE"], "😀");

        let source = QuickAction {
            name: "Label".into(),
            action: "text:hello".into(),
        };
        let previous = QuickAction {
            action: "text:previous".into(),
            ..QuickAction::default()
        };
        keymap.quick_action_pages[0]
            .cells
            .insert("KeyQ".into(), source.clone());
        keymap.quick_action_pages[1]
            .cells
            .insert("KeyW".into(), previous.clone());
        move_cell(&mut keymap, MenuKind::Quick, (0, 0), (1, 1));
        assert_eq!(keymap.quick_action_pages[1].cells["KeyW"], source);
        assert_eq!(keymap.quick_action_pages[0].cells["KeyQ"], previous);
        reorder_page(&mut keymap, MenuKind::Quick, 1, 0);
        assert_eq!(keymap.quick_action_pages[0].id, "b");
        assert_eq!(keymap.quick_action_pages[0].cells["KeyW"], source);
        assert_eq!(keymap.quick_action_pages[1].cells["KeyQ"], previous);
        let unchanged = keymap.clone();
        move_cell(&mut keymap, MenuKind::Quick, (99, 0), (0, 1));
        move_cell(&mut keymap, MenuKind::Emoji, (0, 15), (1, 0));
        reorder_page(&mut keymap, MenuKind::Quick, 0, 99);
        assert_eq!(keymap, unchanged);
    }

    #[test]
    fn moving_within_a_page_swaps_without_shifting() {
        let mut keymap = KeymapPreset {
            emoji_pages: vec![EmojiPage {
                id: "a".into(),
                name: String::new(),
                cells: [("KeyQ", "q"), ("KeyW", "w"), ("KeyE", "e")]
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .into(),
            }],
            ..KeymapPreset::default()
        };
        move_cell(&mut keymap, MenuKind::Emoji, (0, 0), (0, 2));
        let cells = &keymap.emoji_pages[0].cells;
        assert_eq!(cells["KeyQ"], "e");
        assert_eq!(cells["KeyW"], "w", "cells between stay in place");
        assert_eq!(cells["KeyE"], "q");
        move_cell(&mut keymap, MenuKind::Emoji, (0, 1), (0, 4));
        let cells = &keymap.emoji_pages[0].cells;
        assert!(!cells.contains_key("KeyW"), "an empty target moves back");
        assert_eq!(cells["KeyT"], "w");
    }

    #[test]
    fn removing_the_only_page_clears_it_and_keeps_its_id() {
        let mut keymap = KeymapPreset::default();
        ensure_pages(&mut keymap);
        let id = keymap.emoji_pages[0].id.clone();
        keymap.emoji_pages[0].name = "Mine".into();
        set_emoji(&mut keymap, 0, 0, "🙂");
        assert_eq!(remove_page(&mut keymap, MenuKind::Emoji, 0), 0);
        assert_eq!(keymap.emoji_pages.len(), 1);
        assert_eq!(keymap.emoji_pages[0].id, id);
        assert!(keymap.emoji_pages[0].name.is_empty());
        assert!(keymap.emoji_pages[0].cells.is_empty());

        keymap.emoji_pages.push(EmojiPage::empty());
        keymap.emoji_pages.push(EmojiPage::empty());
        assert_eq!(remove_page(&mut keymap, MenuKind::Emoji, 2), 1);
        assert_eq!(remove_page(&mut keymap, MenuKind::Emoji, 0), 0);
        assert_eq!(keymap.emoji_pages.len(), 1);
    }

    #[test]
    fn selection_follows_the_moved_page() {
        // Pages a b c d; selected index after moving `from` to `to`.
        for (selected, from, to, expected) in [
            (1, 1, 3, 3),
            (2, 1, 3, 1),
            (3, 1, 3, 2),
            (0, 1, 3, 0),
            (1, 3, 1, 2),
            (2, 3, 0, 3),
            (3, 3, 0, 0),
        ] {
            assert_eq!(
                selection_after_reorder(selected, from, to),
                expected,
                "{selected} {from}->{to}"
            );
        }
    }

    #[test]
    fn emoji_text_keeps_spaces_and_no_break_space() {
        let mut keymap = KeymapPreset::default();
        ensure_pages(&mut keymap);
        set_emoji(&mut keymap, 0, 0, "TODO: ");
        set_emoji(&mut keymap, 0, 1, "\u{a0}");
        set_emoji(&mut keymap, 0, 2, "x");
        set_emoji(&mut keymap, 0, 2, "  \t");
        let cells = &keymap.emoji_pages[0].cells;
        assert_eq!(cells["KeyQ"], "TODO: ");
        assert_eq!(cells["KeyW"], "\u{a0}");
        assert!(
            !cells.contains_key("KeyE"),
            "ordinary whitespace clears the cell"
        );
        set_emoji(&mut keymap, 5, 0, "out of range");
        set_emoji(&mut keymap, 0, 15, "out of range");
        assert_eq!(keymap.emoji_pages[0].cells.len(), 2);
    }

    #[test]
    fn names_are_trimmed_and_only_set_on_assigned_cells() {
        let mut keymap = KeymapPreset::default();
        ensure_pages(&mut keymap);
        rename_page(&mut keymap, MenuKind::Quick, 0, "  Media  ");
        assert_eq!(keymap.quick_action_pages[0].name, "Media");
        set_quick_action_name(&mut keymap, 0, 0, "Nothing here");
        assert!(keymap.quick_action_pages[0].cells.is_empty());
        set_quick_action(&mut keymap, 0, 0, "KeyA", "", "").unwrap();
        set_quick_action_name(&mut keymap, 0, 0, "   ");
        assert_eq!(keymap.quick_action_pages[0].cells["KeyQ"].name, "");
        clear_cell(&mut keymap, MenuKind::Quick, 0, 0);
        assert!(keymap.quick_action_pages[0].cells.is_empty());
    }

    #[test]
    fn page_actions_keep_positions_when_pages_move() {
        let mut keymap = KeymapPreset {
            quick_action_pages: ["a", "b", "c"]
                .map(|id| QuickActionPage::new(id.into(), String::new()))
                .into(),
            ..KeymapPreset::default()
        };
        let mut rule = KeyRule::new("r".into(), "KeyA");
        rule.tap_action = Some("tecloid:showQuickMenu1".into());
        keymap.rules.push(rule);
        reorder_page(&mut keymap, MenuKind::Quick, 0, 2);
        assert_eq!(keymap.quick_action_pages[0].id, "b");
        assert_eq!(remove_page(&mut keymap, MenuKind::Quick, 0), 0);
        assert_eq!(keymap.quick_action_pages[0].id, "c");
        assert_eq!(
            keymap.rules[0].tap_action.as_deref(),
            Some("tecloid:showQuickMenu1")
        );
    }

    #[test]
    fn command_switch_roundtrips_without_changing_assignments() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        assert!(!doc.settings().commands_enabled);
        let command = Command {
            id: "hello".into(),
            name: String::new(),
            posix_script: "printf hello".into(),
            powershell_script: "printf hello".into(),
            working_directory: None,
        };
        assert!(doc.save_command(command.clone()).is_err());
        doc.update_settings(|settings| settings.commands_enabled = true)
            .unwrap();
        doc.save_command(command).unwrap();
        doc.set_base_tap_action("KeyQ", "cmd:hello").unwrap();
        let keymap = doc.keymap().clone();
        doc.update_settings(|settings| settings.commands_enabled = false)
            .unwrap();
        doc.save_current_keymap_as("Saved").unwrap();
        let mut loaded = ConfigDocument::load(paths.clone()).unwrap();
        loaded.load_library_for_editing("Saved").unwrap();
        assert!(!loaded.settings().commands_enabled);
        assert_eq!(
            super::super::keymap_file::serialize(loaded.keymap()),
            super::super::keymap_file::serialize(&keymap)
        );
        assert_eq!(loaded.keymap().commands[0].name, "printf hello");
    }

    #[test]
    fn command_places_name_rule_triggers_not_the_hold_layer() {
        let mut tap = KeyRule::new("r1".into(), "F1");
        tap.layer_id = "sel".into();
        tap.tap_action = Some("cmd:ls".into());
        // Long hold is ignored while holding switches a layer.
        tap.hold_behavior = Some(HoldBehavior::Layer);
        tap.long_hold_action = "cmd:ls".into();
        let mut double = KeyRule::new("r2".into(), "KeyQ");
        double.enabled = Some(false);
        double.double_tap_action = "cmd:ls".into();
        let config = ResolvedAppConfig::from_parts(
            AppSettings::default(),
            KeymapPreset {
                layers: vec![Layer {
                    id: "sel".into(),
                    name: "Select".into(),
                    description: None,
                }],
                rules: vec![tap, double],
                layer_keymaps: [(
                    "sel".into(),
                    LayerKeymap {
                        extra_key_order: Vec::new(),
                        keys: [("KeyW".into(), Some("cmd:ls".into()))].into(),
                        ..Default::default()
                    },
                )]
                .into(),
                ..Default::default()
            },
            None,
        );
        let place = |kind, layer: &str, name: &str, enabled| CommandPlace {
            kind,
            layer: layer.into(),
            name: name.into(),
            enabled,
        };
        assert_eq!(
            command_places(&config, "cmd:ls"),
            vec![
                place(PlaceKind::RuleTap, "", "F1", true),
                place(PlaceKind::RuleDoubleTap, "", "KeyQ", false),
                place(PlaceKind::LayerKey, "Select", "KeyW", true),
            ]
        );
    }

    #[test]
    fn menu_save_merges_unrelated_edits_and_preserves_same_section_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        let baseline = doc.keymap().clone();
        let mut draft = baseline.clone();
        draft.emoji_pages[0].name = "Draft emoji".into();
        doc.update_keymap(|keymap| {
            keymap.description = Some("New description".into());
            keymap.quick_action_pages[0].name = "New quick menu".into();
        })
        .unwrap();
        doc.save_menu_pages(&baseline, &draft).unwrap();
        assert_eq!(doc.keymap().emoji_pages[0].name, "Draft emoji");
        assert_eq!(doc.keymap().quick_action_pages[0].name, "New quick menu");
        assert_eq!(doc.keymap().description.as_deref(), Some("New description"));
        let unchanged = doc.keymap().clone();
        draft.emoji_pages[0].name = "Conflicting emoji".into();
        assert!(matches!(
            doc.save_menu_pages(&baseline, &draft),
            Err(ConfigError::EditConflict)
        ));
        assert_eq!(doc.keymap(), &unchanged);
    }

    #[test]
    fn menu_save_validates_against_current_macros() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        let baseline = doc.keymap().clone();
        let mut draft = baseline.clone();
        doc.save_macros(
            vec![Macro {
                id: "newMacro".into(),
                name: String::new(),
                steps: vec![],
                step_pause_ms: None,
                modifier_delay_ms: None,
            }],
            &[],
        )
        .unwrap();
        draft.quick_action_pages[0].cells.insert(
            "KeyQ".into(),
            QuickAction {
                action: "macro:newMacro".into(),
                ..Default::default()
            },
        );
        doc.save_menu_pages(&baseline, &draft).unwrap();
        assert_eq!(doc.keymap().macros[0].id, "newMacro");
        assert_eq!(
            doc.keymap().quick_action_pages[0].cells["KeyQ"].action,
            "macro:newMacro"
        );
    }

    #[test]
    fn menu_save_rechecks_moved_cells_but_not_untouched_ones() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        // A saved cell whose macro no longer exists.
        doc.update_keymap(|keymap| {
            keymap.quick_action_pages[0].cells.insert(
                "KeyQ".into(),
                QuickAction {
                    action: "macro:gone".into(),
                    ..Default::default()
                },
            );
        })
        .unwrap();
        let baseline = doc.keymap().clone();

        let mut renamed = baseline.clone();
        renamed.quick_action_pages[0].name = "Renamed".into();
        doc.save_menu_pages(&baseline, &renamed).unwrap();

        let baseline = doc.keymap().clone();
        let mut moved = baseline.clone();
        let cell = moved.quick_action_pages[0].cells.remove("KeyQ").unwrap();
        moved.quick_action_pages[0]
            .cells
            .insert("KeyW".into(), cell);
        assert!(matches!(
            doc.save_menu_pages(&baseline, &moved),
            Err(ConfigError::InvalidAction(_))
        ));
    }

    #[test]
    fn command_overview_snapshot_owns_the_working_copy() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        doc.update_settings(|s| s.commands_enabled = true).unwrap();
        doc.save_command(Command {
            id: "test".into(),
            name: "Before".into(),
            posix_script: "true".into(),
            powershell_script: "exit 0".into(),
            working_directory: None,
        })
        .unwrap();
        doc.set_base_tap_action("KeyQ", "cmd:test").unwrap();
        let snapshot = doc.command_assignment_snapshot();
        doc.update_keymap(|keymap| keymap.commands[0].name = "After".into())
            .unwrap();
        let rows = std::thread::spawn(move || snapshot.load())
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(rows[0].command.name, "Before");
        assert_eq!(doc.command_assignments().unwrap()[0].command.name, "After");
    }

    #[test]
    fn command_overview_lists_only_assignments_across_saved_keymaps() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths.save_user_keymap("Test", r#"{"version": 1, "commands": [{"id": "hello", "sh": "printf hello"}, {"id": "unused", "sh": "true"}], "rules": [{"key": "KeyQ", "tap": "cmd:hello"}]}"#, true).unwrap();
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        doc.update_keymap(|keymap| {
            *keymap = super::super::keymap_file::parse(&paths.load_user_keymap("Test").unwrap())
                .unwrap()
                .unwrap()
        })
        .unwrap();
        doc.update_settings(|settings| settings.commands_enabled = true)
            .unwrap();
        paths.delete_user_keymap("Test").unwrap();
        assert!(doc.settings().commands_enabled);
        doc.save_current_keymap_as("First").unwrap();
        doc.save_current_keymap_as("Second").unwrap();
        let rows = doc.command_assignments().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|row| row.command.id == "hello" && row.usage.len() == 1)
        );
        assert_eq!(
            rows[0].places,
            vec![CommandPlace {
                kind: PlaceKind::RuleTap,
                layer: String::new(),
                name: "KeyQ".into(),
                enabled: true,
            }]
        );
        assert!(
            rows.iter()
                .any(|row| row.keymap_id.as_deref() == Some("user:First"))
        );
        assert!(
            rows.iter()
                .any(|row| row.keymap_id.as_deref() == Some("user:Second"))
        );
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAssignment {
    pub keymap_id: Option<String>,
    pub command: Command,
    pub usage: Vec<String>,
    /// Where the command is bound, with names instead of ids.
    pub places: Vec<CommandPlace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaceKind {
    RuleTap,
    RuleHold,
    RuleLongHold,
    RuleDoubleTap,
    LayerKey,
    Macro,
    QuickAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPlace {
    pub kind: PlaceKind,
    /// Layer name for layer keys, empty otherwise.
    pub layer: String,
    /// Trigger key, macro name or quick action name.
    pub name: String,
    /// False for a disabled rule, which the mapper skips.
    pub enabled: bool,
}

/// Every binding of `action`, named for people rather than by id.
pub fn command_places(config: &ResolvedAppConfig, action: &str) -> Vec<CommandPlace> {
    let layer_name = |id: &str| {
        config
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .map_or_else(|| id.to_owned(), |layer| layer.name.clone())
    };
    let mut places = Vec::new();
    for rule in &config.rules {
        // The mapper ignores the action of the hold mode that is not selected.
        let long_hold = rule.hold_behavior() == HoldBehavior::Action;
        for (kind, value) in [
            (PlaceKind::RuleTap, rule.tap_action.as_deref()),
            (
                PlaceKind::RuleHold,
                rule.hold_action.as_deref().filter(|_| !long_hold),
            ),
            (
                PlaceKind::RuleLongHold,
                long_hold.then_some(rule.long_hold_action.as_str()),
            ),
            (
                PlaceKind::RuleDoubleTap,
                Some(rule.double_tap_action.as_str()),
            ),
        ] {
            if value.map(str::trim) == Some(action) {
                places.push(CommandPlace {
                    kind,
                    layer: String::new(),
                    name: rule.key.clone(),
                    enabled: rule.is_enabled(),
                });
            }
        }
    }
    for (layer, map) in &config.layer_keymaps {
        for (key, value) in &map.keys {
            if value.as_deref() == Some(action) {
                places.push(CommandPlace {
                    kind: PlaceKind::LayerKey,
                    layer: layer_name(layer),
                    name: key.clone(),
                    enabled: true,
                });
            }
        }
    }
    for item in &config.macros {
        if item.steps.iter().any(|step| step.action.trim() == action) {
            places.push(CommandPlace {
                kind: PlaceKind::Macro,
                layer: String::new(),
                name: if item.name.is_empty() {
                    item.id.clone()
                } else {
                    item.name.clone()
                },
                enabled: true,
            });
        }
    }
    for page in &config.quick_action_pages {
        for (key, item) in &page.cells {
            if item.action.trim() == action {
                places.push(CommandPlace {
                    kind: PlaceKind::QuickAction,
                    layer: String::new(),
                    name: if item.name.is_empty() {
                        key.clone()
                    } else {
                        item.name.clone()
                    },
                    enabled: true,
                });
            }
        }
    }
    places
}

/// Owned input for reading command assignments away from the UI thread.
pub struct CommandAssignmentSnapshot {
    paths: crate::storage::StoragePaths,
    settings: AppSettings,
    keymap: KeymapPreset,
}

impl ConfigDocument {
    pub fn command_assignment_snapshot(&self) -> CommandAssignmentSnapshot {
        CommandAssignmentSnapshot {
            paths: self.paths().clone(),
            settings: self.settings().clone(),
            keymap: self.keymap().clone(),
        }
    }

    pub fn command_assignments(&self) -> Result<Vec<CommandAssignment>, ConfigError> {
        self.command_assignment_snapshot().load()
    }
}

impl CommandAssignmentSnapshot {
    pub fn load(self) -> Result<Vec<CommandAssignment>, ConfigError> {
        let mut keymaps = vec![(self.settings.editing_keymap_id.clone(), self.keymap)];
        for name in self.paths.list_user_keymaps().map_err(ConfigError::Io)? {
            let id = super::model::user_keymap_id(&name);
            if self.settings.editing_keymap_id.as_ref() == Some(&id) {
                continue;
            }
            let text = self
                .paths
                .load_user_keymap(&name)
                .map_err(ConfigError::Io)?;
            if let Some(keymap) = super::keymap_file::parse(&text).map_err(ConfigError::Parse)? {
                keymaps.push((Some(id), keymap));
            }
        }
        let mut rows = Vec::new();
        for (keymap_id, keymap) in keymaps {
            let config =
                ResolvedAppConfig::from_parts(self.settings.clone(), keymap, keymap_id.as_deref());
            for command in &config.commands {
                let action = format!("cmd:{}", command.id);
                let usage = super::macros::action_usage(&config, &action);
                if !usage.is_empty() {
                    rows.push(CommandAssignment {
                        keymap_id: keymap_id.clone(),
                        command: command.clone(),
                        usage,
                        places: command_places(&config, &action),
                    });
                }
            }
        }
        Ok(rows)
    }
}
