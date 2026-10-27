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

impl ConfigDocument {
    pub fn save_menu_pages(
        &mut self,
        baseline: &LayoutPreset,
        candidate: &LayoutPreset,
    ) -> Result<(), ConfigError> {
        let current = self.layout();
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
                !LEFT_HAND_HOTKEYS.contains(&key.as_str()) || value.encode_utf16().count() > 100
            }) {
                return Err(ConfigError::Menu(MenuIssue::EmojiCell));
            }
        }
        let config = ResolvedAppConfig::from_parts(
            self.settings().clone(),
            candidate.clone(),
            self.settings().editing_layout_id.as_deref(),
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
        let unchanged = |action: &str| {
            baseline
                .quick_action_pages
                .iter()
                .flat_map(|page| page.cells.values())
                .any(|old| old.action == action)
        };
        for item in candidate
            .quick_action_pages
            .iter()
            .flat_map(|page| page.cells.values())
        {
            // Actions kept from the saved menus are not checked again.
            if unchanged(&item.action) {
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
        self.update_layout(|layout| {
            layout.emoji_pages = candidate.emoji_pages.clone();
            layout.quick_action_pages = candidate.quick_action_pages.clone();
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
        let mut commands = self.layout().commands.clone();
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
        self.update_layout(|layout| layout.commands = commands)
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

/// Move a page and its cells together, preserving the order of other pages.
///
/// `app:showQuickMenuN` / `app:showEmojiMenuN` open a page by position and
/// are deliberately left as they are: moving a page changes what those keys
/// open. See [`actions::MENU_PAGE_PICKER_COUNT`].
pub fn reorder_page(layout: &mut LayoutPreset, emoji: bool, from: usize, to: usize) {
    fn reorder<T>(pages: &mut Vec<T>, from: usize, to: usize) {
        if from < pages.len() && to < pages.len() {
            let page = pages.remove(from);
            pages.insert(to, page);
        }
    }
    if emoji {
        reorder(&mut layout.emoji_pages, from, to);
    } else {
        reorder(&mut layout.quick_action_pages, from, to);
    }
}

/// Remove a page; numbered `app:` menu actions keep their numbers, so the
/// following pages move into the freed positions.
pub fn remove_page(layout: &mut LayoutPreset, emoji: bool, index: usize) {
    if emoji {
        if index < layout.emoji_pages.len() {
            layout.emoji_pages.remove(index);
        }
    } else if index < layout.quick_action_pages.len() {
        layout.quick_action_pages.remove(index);
    }
}

/// Transfer a cell between pages, returning the destination's contents to its source.
pub fn transfer_cell(
    layout: &mut LayoutPreset,
    emoji: bool,
    source_page: usize,
    source: usize,
    target_page: usize,
    target: usize,
) {
    fn transfer<T, V>(
        pages: &mut [T],
        cells: impl Fn(&mut T) -> &mut BTreeMap<String, V>,
        (source_page, source): (usize, &str),
        (target_page, target): (usize, &str),
    ) {
        if source_page >= pages.len() || target_page >= pages.len() {
            return;
        }
        let Some(value) = cells(&mut pages[source_page]).remove(source) else {
            return;
        };
        if let Some(previous) = cells(&mut pages[target_page]).insert(target.into(), value) {
            cells(&mut pages[source_page]).insert(source.into(), previous);
        }
    }
    if source_page == target_page {
        return;
    }
    let (Some(source), Some(target)) =
        (LEFT_HAND_HOTKEYS.get(source), LEFT_HAND_HOTKEYS.get(target))
    else {
        return;
    };
    let source = (source_page, *source);
    let target = (target_page, *target);
    if emoji {
        transfer(&mut layout.emoji_pages, |p| &mut p.cells, source, target);
    } else {
        transfer(
            &mut layout.quick_action_pages,
            |p| &mut p.cells,
            source,
            target,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StoragePaths;

    #[test]
    fn transfers_and_page_reorders_preserve_cells_and_action_metadata() {
        let mut layout = LayoutPreset {
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
            ..LayoutPreset::default()
        };
        transfer_cell(&mut layout, true, 0, 0, 1, 1);
        assert_eq!(layout.emoji_pages[0].cells["KeyQ"], "✨");
        assert_eq!(layout.emoji_pages[1].cells["KeyW"], "😀");
        transfer_cell(&mut layout, true, 1, 1, 0, 2);
        assert!(!layout.emoji_pages[1].cells.contains_key("KeyW"));
        assert_eq!(layout.emoji_pages[0].cells["KeyE"], "😀");
        reorder_page(&mut layout, true, 0, 1);
        assert_eq!(layout.emoji_pages[1].id, "a");
        assert_eq!(layout.emoji_pages[1].cells["KeyE"], "😀");

        let source = QuickAction {
            name: "Label".into(),
            action: "text:hello".into(),
            icon: Some("icon".into()),
        };
        let previous = QuickAction {
            action: "text:previous".into(),
            ..QuickAction::default()
        };
        layout.quick_action_pages[0]
            .cells
            .insert("KeyQ".into(), source.clone());
        layout.quick_action_pages[1]
            .cells
            .insert("KeyW".into(), previous.clone());
        transfer_cell(&mut layout, false, 0, 0, 1, 1);
        assert_eq!(layout.quick_action_pages[1].cells["KeyW"], source);
        assert_eq!(layout.quick_action_pages[0].cells["KeyQ"], previous);
        reorder_page(&mut layout, false, 1, 0);
        assert_eq!(layout.quick_action_pages[0].id, "b");
        assert_eq!(layout.quick_action_pages[0].cells["KeyW"], source);
        assert_eq!(layout.quick_action_pages[1].cells["KeyQ"], previous);
        let unchanged = layout.clone();
        transfer_cell(&mut layout, false, 99, 0, 0, 1);
        transfer_cell(&mut layout, true, 0, 15, 1, 0);
        reorder_page(&mut layout, false, 0, 99);
        assert_eq!(layout, unchanged);
    }

    #[test]
    fn page_actions_keep_positions_when_pages_move() {
        let mut layout = LayoutPreset {
            quick_action_pages: ["a", "b", "c"]
                .map(|id| QuickActionPage::new(id.into(), String::new()))
                .into(),
            ..LayoutPreset::default()
        };
        let mut rule = KeyRule::new("r".into(), "KeyA");
        rule.tap_action = Some("app:showQuickMenu1".into());
        layout.rules.push(rule);
        reorder_page(&mut layout, false, 0, 2);
        assert_eq!(layout.quick_action_pages[0].id, "b");
        remove_page(&mut layout, false, 0);
        assert_eq!(layout.quick_action_pages[0].id, "c");
        assert_eq!(
            layout.rules[0].tap_action.as_deref(),
            Some("app:showQuickMenu1")
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
        let layout = doc.layout().clone();
        doc.update_settings(|settings| settings.commands_enabled = false)
            .unwrap();
        doc.save_current_layout_as("Saved").unwrap();
        let loaded = ConfigDocument::load(paths.clone()).unwrap();
        assert!(!loaded.settings().commands_enabled);
        assert_eq!(
            super::super::layout_file::serialize(loaded.layout()),
            super::super::layout_file::serialize(&layout)
        );
        assert_eq!(loaded.layout().commands[0].name, "printf hello");
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
            LayoutPreset {
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
        let baseline = doc.layout().clone();
        let mut draft = baseline.clone();
        draft.emoji_pages[0].name = "Draft emoji".into();
        doc.update_layout(|layout| {
            layout.description = Some("New description".into());
            layout.quick_action_pages[0].name = "New quick menu".into();
        })
        .unwrap();
        doc.save_menu_pages(&baseline, &draft).unwrap();
        assert_eq!(doc.layout().emoji_pages[0].name, "Draft emoji");
        assert_eq!(doc.layout().quick_action_pages[0].name, "New quick menu");
        assert_eq!(doc.layout().description.as_deref(), Some("New description"));
        let unchanged = doc.layout().clone();
        draft.emoji_pages[0].name = "Conflicting emoji".into();
        assert!(matches!(
            doc.save_menu_pages(&baseline, &draft),
            Err(ConfigError::EditConflict)
        ));
        assert_eq!(doc.layout(), &unchanged);
    }

    #[test]
    fn menu_save_validates_against_current_macros() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        let baseline = doc.layout().clone();
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
        assert_eq!(doc.layout().macros[0].id, "newMacro");
        assert_eq!(
            doc.layout().quick_action_pages[0].cells["KeyQ"].action,
            "macro:newMacro"
        );
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
        doc.update_layout(|layout| layout.commands[0].name = "After".into())
            .unwrap();
        let rows = std::thread::spawn(move || snapshot.load())
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(rows[0].command.name, "Before");
        assert_eq!(doc.command_assignments().unwrap()[0].command.name, "After");
    }

    #[test]
    fn command_overview_lists_only_assignments_across_saved_layouts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths.save_user_layout("Test", r#"{"schemaVersion": 1, "commands": [{"id": "hello", "unix": "printf hello"}, {"id": "unused", "unix": "true"}], "rules": [{"key": "KeyQ", "tap": "cmd:hello"}]}"#, true).unwrap();
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        doc.update_layout(|layout| {
            *layout = super::super::layout_file::parse(&paths.load_user_layout("Test").unwrap())
                .unwrap()
                .unwrap()
        })
        .unwrap();
        doc.update_settings(|settings| settings.commands_enabled = true)
            .unwrap();
        paths.delete_user_layout("Test").unwrap();
        assert!(doc.settings().commands_enabled);
        doc.save_current_layout_as("First").unwrap();
        doc.save_current_layout_as("Second").unwrap();
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
                .any(|row| row.layout_id.as_deref() == Some("user:First"))
        );
        assert!(
            rows.iter()
                .any(|row| row.layout_id.as_deref() == Some("user:Second"))
        );
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAssignment {
    pub layout_id: Option<String>,
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
    layout: LayoutPreset,
}

impl ConfigDocument {
    pub fn command_assignment_snapshot(&self) -> CommandAssignmentSnapshot {
        CommandAssignmentSnapshot {
            paths: self.paths().clone(),
            settings: self.settings().clone(),
            layout: self.layout().clone(),
        }
    }

    pub fn command_assignments(&self) -> Result<Vec<CommandAssignment>, ConfigError> {
        self.command_assignment_snapshot().load()
    }
}

impl CommandAssignmentSnapshot {
    pub fn load(self) -> Result<Vec<CommandAssignment>, ConfigError> {
        let mut layouts = vec![(self.settings.editing_layout_id.clone(), self.layout)];
        for name in self.paths.list_user_layouts().map_err(ConfigError::Io)? {
            let id = super::model::user_layout_id(&name);
            if self.settings.editing_layout_id.as_ref() == Some(&id) {
                continue;
            }
            let text = self
                .paths
                .load_user_layout(&name)
                .map_err(ConfigError::Io)?;
            if let Some(layout) = super::layout_file::parse(&text).map_err(ConfigError::Parse)? {
                layouts.push((Some(id), layout));
            }
        }
        let mut rows = Vec::new();
        for (layout_id, layout) in layouts {
            let config =
                ResolvedAppConfig::from_parts(self.settings.clone(), layout, layout_id.as_deref());
            for command in &config.commands {
                let action = format!("cmd:{}", command.id);
                let usage = super::macros::action_usage(&config, &action);
                if !usage.is_empty() {
                    rows.push(CommandAssignment {
                        layout_id: layout_id.clone(),
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
