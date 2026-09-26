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
        for (index, command) in candidate.commands.iter().enumerate() {
            if command.id.is_empty()
                || command.id.len() > 64
                || !command
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(ConfigError::Menu(MenuIssue::CommandId));
            }
            if candidate.commands[..index]
                .iter()
                .any(|c| c.id == command.id)
            {
                return Err(ConfigError::Menu(MenuIssue::DuplicateCommand));
            }
            if command.linux.trim().is_empty() {
                return Err(ConfigError::Menu(MenuIssue::EmptyCommand));
            }
        }
        self.update_layout(|layout| {
            layout.emoji_pages = candidate.emoji_pages.clone();
            layout.quick_actions = candidate.quick_actions.clone();
            layout.quick_action_pages = candidate.quick_action_pages.clone();
            layout.commands = candidate.commands.clone();
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
