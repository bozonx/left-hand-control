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
    ids,
    model::*,
};
use crate::config_document::{ConfigDocument, ConfigError};

impl ConfigDocument {
    pub fn save_menu_pages(
        &mut self,
        baseline: &LayoutPreset,
        candidate: &LayoutPreset,
    ) -> Result<(), ConfigError> {
        if self.layout() != baseline {
            return Err(ConfigError::ExternalChange);
        }
        for page in &candidate.emoji_pages {
            if page.cells.iter().any(|(key, value)| {
                !LEFT_HAND_HOTKEYS.contains(&key.as_str()) || value.encode_utf16().count() > 100
            }) {
                return Err(ConfigError::Menu(MenuIssue::EmojiCell));
            }
        }
        let config = AppConfig::from_parts(
            self.settings().clone(),
            candidate.clone(),
            self.settings().current_layout_id.as_deref(),
        );
        for item in &candidate.quick_actions {
            if baseline
                .quick_actions
                .iter()
                .any(|old| old.id == item.id && old.action == item.action)
            {
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
        self.ensure_files_unchanged()?;
        self.update_layout(|layout| {
            layout.emoji_pages = candidate.emoji_pages.clone();
            layout.quick_actions = candidate.quick_actions.clone();
            layout.quick_action_pages = candidate.quick_action_pages.clone();
        })?;
        Ok(())
    }

    pub fn save_command(&mut self, mut command: Command) -> Result<(), ConfigError> {
        if !self.settings().commands_enabled {
            return Err(ConfigError::Invalid("Commands are disabled".into()));
        }
        if command.name.trim().is_empty() {
            command.name = command.linux.trim().to_owned();
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
        self.ensure_files_unchanged()?;
        self.update_layout(|layout| layout.commands = commands)
    }
}

/// First problem of `commands[index]`, checked against the commands before it.
pub fn command_issue(commands: &[Command], index: usize) -> Option<MenuIssue> {
    let command = commands.get(index)?;
    if command.id.is_empty()
        || command.id.len() > 64
        || !command
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Some(MenuIssue::CommandId);
    }
    if commands[..index].iter().any(|c| c.id == command.id) {
        return Some(MenuIssue::DuplicateCommand);
    }
    if command.linux.trim().is_empty() {
        return Some(MenuIssue::EmptyCommand);
    }
    None
}

/// Move a page and its cells together, preserving the order of other pages.
pub fn reorder_page(layout: &mut LayoutPreset, emoji: bool, from: usize, to: usize) {
    if emoji {
        if from < layout.emoji_pages.len() && to < layout.emoji_pages.len() {
            let page = layout.emoji_pages.remove(from);
            layout.emoji_pages.insert(to, page);
        }
    } else {
        let size = LEFT_HAND_HOTKEYS.len();
        if from >= layout.quick_action_pages.len()
            || to >= layout.quick_action_pages.len()
            || layout.quick_actions.len() < layout.quick_action_pages.len() * size
        {
            return;
        }
        let page = layout.quick_action_pages.remove(from);
        layout.quick_action_pages.insert(to, page);
        let cells: Vec<_> = layout
            .quick_actions
            .drain(from * size..(from + 1) * size)
            .collect();
        layout.quick_actions.splice(to * size..to * size, cells);
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
    let size = LEFT_HAND_HOTKEYS.len();
    if source >= size || target >= size || source_page == target_page {
        return;
    }
    if emoji {
        if source_page >= layout.emoji_pages.len() || target_page >= layout.emoji_pages.len() {
            return;
        }
        let source_key = LEFT_HAND_HOTKEYS[source];
        let target_key = LEFT_HAND_HOTKEYS[target];
        let Some(value) = layout.emoji_pages[source_page].cells.remove(source_key) else {
            return;
        };
        if let Some(previous) = layout.emoji_pages[target_page]
            .cells
            .insert(target_key.into(), value)
        {
            layout.emoji_pages[source_page]
                .cells
                .insert(source_key.into(), previous);
        }
    } else {
        if source_page >= layout.quick_action_pages.len()
            || target_page >= layout.quick_action_pages.len()
        {
            return;
        }
        let from = source_page * size + source;
        let to = target_page * size + target;
        if from < layout.quick_actions.len()
            && to < layout.quick_actions.len()
            && !layout.quick_actions[from].action.is_empty()
        {
            layout.quick_actions.swap(from, to);
        }
    }
}

pub fn empty_quick_action() -> QuickAction {
    QuickAction {
        id: ids::generate("quick_"),
        name: String::new(),
        action: String::new(),
        icon: None,
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
                QuickActionPage {
                    id: "a".into(),
                    name: "A".into(),
                },
                QuickActionPage {
                    id: "b".into(),
                    name: "B".into(),
                },
            ],
            quick_actions: (0..30).map(|_| empty_quick_action()).collect(),
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

        layout.quick_actions[0] = QuickAction {
            id: "source".into(),
            name: "Label".into(),
            action: "text:hello".into(),
            icon: Some("icon".into()),
        };
        layout.quick_actions[16].action = "text:previous".into();
        let source = layout.quick_actions[0].clone();
        let previous = layout.quick_actions[16].clone();
        transfer_cell(&mut layout, false, 0, 0, 1, 1);
        assert_eq!(layout.quick_actions[16], source);
        assert_eq!(layout.quick_actions[0], previous);
        reorder_page(&mut layout, false, 1, 0);
        assert_eq!(layout.quick_action_pages[0].id, "b");
        assert_eq!(layout.quick_actions[1], source);
        assert_eq!(layout.quick_actions[15], previous);
        let unchanged = layout.clone();
        transfer_cell(&mut layout, false, 99, 0, 0, 1);
        transfer_cell(&mut layout, true, 0, 15, 1, 0);
        reorder_page(&mut layout, false, 0, 99);
        assert_eq!(layout, unchanged);
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
            linux: "printf hello".into(),
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
        assert!(
            !std::fs::read_to_string(paths.config_path())
                .unwrap()
                .contains("commandTrust")
        );
    }

    #[test]
    fn command_overview_lists_only_assignments_across_saved_layouts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths.save_user_layout("Test", "commands:\n  - id: hello\n    linux: printf hello\n  - id: unused\n    linux: true\nrules:\n  - key: KeyQ\n    tap: cmd:hello\n", true).unwrap();
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
                kind: PlaceKind::Rule,
                layer: String::new(),
                name: "KeyQ".into(),
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
    Rule,
    LayerKey,
    Macro,
    QuickAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandPlace {
    pub kind: PlaceKind,
    /// Layer name for rules and layer keys, empty for the base layer.
    pub layer: String,
    /// Key, macro name or quick action name.
    pub name: String,
}

/// Every binding of `action`, named for people rather than by id.
pub fn command_places(config: &AppConfig, action: &str) -> Vec<CommandPlace> {
    let layer_name = |id: &str| {
        config
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .map_or_else(|| id.to_owned(), |layer| layer.name.clone())
    };
    let mut places = Vec::new();
    for rule in &config.rules {
        if [
            rule.tap_action.as_deref(),
            rule.hold_action.as_deref(),
            Some(rule.long_hold_action.as_str()),
            Some(rule.double_tap_action.as_str()),
        ]
        .contains(&Some(action))
        {
            places.push(CommandPlace {
                kind: PlaceKind::Rule,
                layer: if rule.layer_id.is_empty() {
                    String::new()
                } else {
                    layer_name(&rule.layer_id)
                },
                name: rule.key.clone(),
            });
        }
    }
    for (layer, map) in &config.layer_keymaps {
        for (key, value) in &map.keys {
            if value.as_deref() == Some(action) {
                places.push(CommandPlace {
                    kind: PlaceKind::LayerKey,
                    layer: layer_name(layer),
                    name: key.clone(),
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
            });
        }
    }
    for item in &config.quick_actions {
        if item.action.trim() == action {
            places.push(CommandPlace {
                kind: PlaceKind::QuickAction,
                layer: String::new(),
                name: item.name.clone(),
            });
        }
    }
    places
}

impl ConfigDocument {
    pub fn command_assignments(&self) -> Result<Vec<CommandAssignment>, ConfigError> {
        let mut layouts = vec![(
            self.settings().current_layout_id.clone(),
            self.layout().clone(),
        )];
        for name in self.paths().list_user_layouts().map_err(ConfigError::Io)? {
            let id = super::model::user_layout_id(&name);
            if self.settings().current_layout_id.as_ref() == Some(&id) {
                continue;
            }
            let text = self
                .paths()
                .load_user_layout(&name)
                .map_err(ConfigError::Io)?;
            if let Some(layout) = super::layout_file::parse(&text).map_err(ConfigError::Parse)? {
                layouts.push((Some(id), layout));
            }
        }
        let mut rows = Vec::new();
        for (layout_id, layout) in layouts {
            let config =
                AppConfig::from_parts(self.settings().clone(), layout, layout_id.as_deref());
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
