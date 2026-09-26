use super::{
    actions::{self, Action},
    model::{AppConfig, Macro},
};
use crate::{
    config_document::{ConfigDocument, ConfigError},
    mapper::system_macros::SYSTEM_MACROS,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroIssue {
    InvalidId,
    DuplicateId,
    DelayRange,
    PauseRange,
    UnknownKey,
    Cycle,
}

pub fn validate(config: &AppConfig, item: &Macro) -> Result<(), ConfigError> {
    if item.id.is_empty()
        || item.id.len() > 64
        || !item
            .id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(ConfigError::Macro(MacroIssue::InvalidId));
    }
    if config.macros.iter().filter(|m| m.id == item.id).count() > 1
        || SYSTEM_MACROS.iter().any(|m| m.id == item.id)
    {
        return Err(ConfigError::Macro(MacroIssue::DuplicateId));
    }
    if item.step_pause_ms.is_some_and(|v| v > 2000)
        || item.modifier_delay_ms.is_some_and(|v| v > 2000)
    {
        return Err(ConfigError::Macro(MacroIssue::DelayRange));
    }
    for step in &item.steps {
        let raw = step.action.trim();
        if raw.is_empty() {
            continue;
        }
        let action = Action::parse(Some(raw));
        match action {
            Action::Pause(ref ms)
                if !ms.is_empty()
                    && ms.bytes().all(|c| c.is_ascii_digit())
                    && ms.parse::<u64>().is_ok_and(|v| v <= 10000) => {}
            Action::Pause(_) => return Err(ConfigError::Macro(MacroIssue::PauseRange)),
            _ => {
                if let Some(issue) = actions::validate(&action, config) {
                    return Err(ConfigError::InvalidAction(issue));
                }
                #[cfg(target_os = "linux")]
                if matches!(action, Action::Keys(_))
                    && crate::mapper::action::parse_action(raw).is_none()
                {
                    return Err(ConfigError::Macro(MacroIssue::UnknownKey));
                }
            }
        }
    }
    fn reaches(
        config: &AppConfig,
        source: &str,
        target: &str,
        visited: &mut HashSet<String>,
    ) -> bool {
        if source == target {
            return true;
        }
        if !visited.insert(target.into()) {
            return false;
        }
        let steps: Vec<&str> = if let Some(item) = config.macros.iter().find(|m| m.id == target) {
            item.steps.iter().map(|s| s.action.as_str()).collect()
        } else {
            SYSTEM_MACROS
                .iter()
                .find(|m| m.id == target)
                .map(|m| m.steps.to_vec())
                .unwrap_or_default()
        };
        steps
            .iter()
            .filter_map(|a| a.trim().strip_prefix("macro:").map(str::trim))
            .any(|next| reaches(config, source, next, visited))
    }
    if item
        .steps
        .iter()
        .filter_map(|s| s.action.trim().strip_prefix("macro:").map(str::trim))
        .any(|target| reaches(config, &item.id, target, &mut HashSet::new()))
    {
        return Err(ConfigError::Macro(MacroIssue::Cycle));
    }
    Ok(())
}

pub fn usage(config: &AppConfig, id: &str) -> Vec<String> {
    action_usage(config, &format!("macro:{id}"))
}

pub fn action_usage(config: &AppConfig, action: &str) -> Vec<String> {
    let mut places = Vec::new();
    for rule in &config.rules {
        if [
            rule.tap_action.as_deref(),
            rule.hold_action.as_deref(),
            Some(rule.double_tap_action.as_str()),
        ]
        .contains(&Some(action))
        {
            places.push(rule.key.clone());
        }
    }
    for (layer, map) in &config.layer_keymaps {
        for (key, value) in &map.keys {
            if value.as_deref() == Some(action) {
                places.push(format!("{layer}: {key}"));
            }
        }
        for extra in &map.extras {
            if extra.action.as_deref() == Some(action) {
                places.push(format!("{layer}: {}", extra.key));
            }
        }
    }
    for item in &config.macros {
        if item.steps.iter().any(|s| s.action.trim() == action) {
            places.push(format!("macro:{}", item.id));
        }
    }
    for item in &config.quick_actions {
        if item.action.trim() == action {
            places.push(format!("quick:{}", item.name));
        }
    }
    places
}

impl ConfigDocument {
    pub fn save_macro(&mut self, index: Option<usize>, item: Macro) -> Result<(), ConfigError> {
        let mut candidate = self.config();
        if let Some(index) = index {
            let old = candidate
                .macros
                .get_mut(index)
                .ok_or_else(|| ConfigError::Invalid("Unknown macro".into()))?;
            *old = item.clone();
        } else {
            candidate.macros.insert(0, item.clone());
        }
        validate(&candidate, &item)?;
        self.update_layout(|layout| layout.macros = candidate.macros)
    }

    pub fn remove_macro(&mut self, index: usize) -> Result<(), ConfigError> {
        if index >= self.layout().macros.len() {
            return Err(ConfigError::Invalid("Unknown macro".into()));
        }
        self.update_layout(|layout| {
            layout.macros.remove(index);
        })
    }

    pub fn move_macro(&mut self, index: usize, next: usize) -> Result<(), ConfigError> {
        if index >= self.layout().macros.len() || next >= self.layout().macros.len() {
            return Err(ConfigError::Invalid("Unknown macro".into()));
        }
        self.update_layout(|layout| {
            let item = layout.macros.remove(index);
            layout.macros.insert(next, item);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{profile::model::MacroStep, storage::StoragePaths};

    fn item(id: &str, actions: &[&str]) -> Macro {
        Macro {
            id: id.into(),
            name: id.into(),
            steps: actions
                .iter()
                .enumerate()
                .map(|(index, action)| MacroStep {
                    id: format!("step{index}"),
                    action: (*action).into(),
                })
                .collect(),
            step_pause_ms: None,
            modifier_delay_ms: Some(0),
        }
    }

    #[test]
    fn saves_reorders_and_removes_macros_without_changing_rules() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_current_layout("rules:\n  - key: KeyA\n    tap: macro:outer\n")
            .unwrap();
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        let rules = document.layout().rules.clone();
        document
            .save_macro(
                None,
                item("inner", &["Ctrl+KeyC", "pause:100", "text: hello "]),
            )
            .unwrap();
        document
            .save_macro(None, item("outer", &["macro:inner", "macro:copyLine", ""]))
            .unwrap();
        assert_eq!(usage(&document.config(), "outer"), ["KeyA"]);
        assert_eq!(usage(&document.config(), "inner"), ["macro:outer"]);
        document.move_macro(0, 1).unwrap();
        let loaded = ConfigDocument::load(paths.clone()).unwrap();
        assert_eq!(loaded.layout().macros[0].id, "inner");
        assert_eq!(loaded.layout().macros[0].steps[2].action, "text: hello ");
        assert_eq!(loaded.layout().macros[0].modifier_delay_ms, Some(0));
        assert_eq!(document.layout().rules, rules);
        assert_eq!(
            loaded.layout().rules[0].tap_action.as_deref(),
            Some("macro:outer")
        );
        document.remove_macro(1).unwrap();
        assert_eq!(
            ConfigDocument::load(paths).unwrap().layout().macros.len(),
            1
        );
    }

    #[test]
    fn rejects_invalid_edits_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        document.save_macro(None, item("inner", &["KeyA"])).unwrap();
        document
            .save_macro(None, item("outer", &["macro:inner"]))
            .unwrap();
        let before = document.layout().clone();
        for invalid in [
            item("", &[]),
            item("bad id", &[]),
            item("inner", &[]),
            item("copyLine", &[]),
            item("new", &["pause:10001"]),
            item("new", &["pause:-1"]),
            item("new", &["macro:missing"]),
            item("new", &["macro:new"]),
        ] {
            assert!(document.save_macro(None, invalid).is_err());
            assert_eq!(document.layout(), &before);
        }
        assert!(
            document
                .save_macro(Some(1), item("inner", &["macro:outer"]))
                .is_err()
        );
        let mut delay = item("new", &[]);
        delay.step_pause_ms = Some(2001);
        assert!(document.save_macro(None, delay).is_err());
        assert_eq!(
            super::super::layout_file::serialize(
                ConfigDocument::load(paths.clone()).unwrap().layout()
            ),
            super::super::layout_file::serialize(&before)
        );
        paths.save_current_layout("rules: []\n").unwrap();
        assert_eq!(
            document.save_macro(None, item("new", &[])),
            Err(ConfigError::ExternalChange)
        );
    }
}
