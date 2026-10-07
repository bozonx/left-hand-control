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
        for index in 0..candidate.commands.len() {
            if let Some(issue) = command_issue(&candidate.commands, index) {
                return Err(ConfigError::Menu(issue));
            }
        }
        self.update_layout(|layout| {
            layout.emoji_pages = candidate.emoji_pages.clone();
            layout.quick_actions = candidate.quick_actions.clone();
            layout.quick_action_pages = candidate.quick_action_pages.clone();
            layout.commands = candidate.commands.clone();
            layout.rules = candidate.rules.clone();
            layout.layer_keymaps = candidate.layer_keymaps.clone();
            layout.macros = candidate.macros.clone();
        })
    }

    pub fn commands_trusted(&self) -> bool {
        let config: crate::mapper_config::AppConfig =
            serde_json::from_str(&self.config().to_json()).expect("serialized config");
        config.settings.commands_trusted(&config.commands)
    }

    pub fn trust_commands(&mut self, approve: bool) -> Result<(), ConfigError> {
        let config: crate::mapper_config::AppConfig =
            serde_json::from_str(&self.config().to_json())
                .map_err(|e| ConfigError::Parse(e.to_string()))?;
        let key = self
            .settings()
            .current_layout_id
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "custom".into());
        let fingerprint = crate::mapper_config::command_fingerprint(&config.commands);
        self.update_settings(|settings| {
            if approve {
                settings.command_trust.insert(
                    key,
                    CommandTrustEntry {
                        fingerprint,
                        trusted_at: String::new(),
                    },
                );
            } else {
                settings.command_trust.remove(&key);
            }
        })
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

pub fn replace_command_references(layout: &mut LayoutPreset, old: &str, new: Option<&str>) {
    let is_source = |value: &str| matches!(Action::parse(Some(value)), Action::Command(id) if id == old);
    let target = new.map(|id| format!("cmd:{id}")).unwrap_or_default();
    let replace = |value: &mut String| {
        if is_source(value) {
            *value = target.clone();
        }
    };
    for rule in &mut layout.rules {
        if let Some(value) = &mut rule.tap_action {
            replace(value);
        }
        if let Some(value) = &mut rule.hold_action {
            replace(value);
        }
        replace(&mut rule.double_tap_action);
    }
    for map in layout.layer_keymaps.values_mut() {
        for value in map.keys.values_mut().flatten() {
            replace(value);
        }
        for extra in &mut map.extras {
            if let Some(value) = &mut extra.action {
                replace(value);
            }
        }
    }
    for item in &mut layout.macros {
        if new.is_none() {
            item.steps.retain(|step| !is_source(&step.action));
        } else {
            for step in &mut item.steps { replace(&mut step.action); }
        }
    }
    for item in &mut layout.quick_actions {
        replace(&mut item.action);
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
    fn renames_and_removes_command_dependencies_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths.save_current_layout(r#"
commands:
  - id: old
    linux: printf hello
rules:
  - key: KeyA
    tap: cmd:old
    dtap: cmd:old
layers:
  - id: nav
    name: Navigation
    keys:
      KeyB: cmd:old
    extras:
      - key: F13
        action: cmd:old
macros:
  - id: sequence
    steps:
      - action: cmd:old
      - action: text:hello
quickActions:
  - id: quick
    action: cmd:old
"#).unwrap();
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        let baseline = doc.layout().clone();
        let mut candidate = baseline.clone();
        let before = super::super::macros::action_usage(&doc.config(), "cmd:old").len();
        assert!(before >= 4);
        candidate.commands[0].id = "new".into();
        replace_command_references(&mut candidate, "old", Some("new"));
        doc.save_menu_pages(&baseline, &candidate).unwrap();
        assert!(super::super::macros::action_usage(&doc.config(), "cmd:old").is_empty());
        assert_eq!(super::super::macros::action_usage(&doc.config(), "cmd:new").len(), before);
        assert_eq!(ConfigDocument::load(paths).unwrap().layout(), &candidate);
        let baseline = candidate.clone();
        candidate.commands.clear();
        replace_command_references(&mut candidate, "new", None);
        doc.save_menu_pages(&baseline, &candidate).unwrap();
        assert!(super::super::macros::action_usage(&doc.config(), "cmd:new").is_empty());
        assert_eq!(doc.layout().macros[0].steps.len(), 1);
    }

    #[test]
    fn menus_roundtrip_and_trust_invalidation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        let baseline = doc.layout().clone();
        let mut next = baseline.clone();
        next.commands.push(Command {
            id: "hello".into(),
            name: "Привет".into(),
            linux: "printf hello".into(),
        });
        next.quick_actions.push(QuickAction {
            id: "q".into(),
            name: "Запуск".into(),
            action: "cmd:hello".into(),
            icon: Some("★".into()),
        });
        next.emoji_pages[0].cells.insert("KeyQ".into(), "👨‍👩‍👧‍👦".into());
        doc.save_menu_pages(&baseline, &next).unwrap();
        assert!(!doc.commands_trusted());
        doc.trust_commands(true).unwrap();
        assert!(
            ConfigDocument::load(paths.clone())
                .unwrap()
                .commands_trusted()
        );
        let baseline = doc.layout().clone();
        next.commands[0].linux = "printf changed".into();
        doc.save_menu_pages(&baseline, &next).unwrap();
        assert!(!doc.commands_trusted());
        assert_eq!(ConfigDocument::load(paths.clone()).unwrap().layout(), &next);
        let before = doc.layout().clone();
        next.commands[0].id = "bad id".into();
        assert!(doc.save_menu_pages(&before, &next).is_err());
        next = before.clone();
        next.emoji_pages[0]
            .cells
            .insert("KeyQ".into(), "😀".repeat(51));
        assert!(doc.save_menu_pages(&before, &next).is_err());
        next = before.clone();
        next.quick_actions[0].action = "macro:missing".into();
        assert!(doc.save_menu_pages(&before, &next).is_err());
        assert_eq!(doc.layout(), &before);
        paths.save_current_layout("rules: []\n").unwrap();
        assert_eq!(
            doc.save_menu_pages(&before, &before),
            Err(ConfigError::ExternalChange)
        );
    }
}
