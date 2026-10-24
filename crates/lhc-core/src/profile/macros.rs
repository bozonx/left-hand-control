use super::{
    actions::{self, Action},
    model::{AppConfig, LayoutPreset, Macro},
};
use crate::config_document::{ConfigDocument, ConfigError};
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
    validate_loaded(config, item)
}

pub(crate) fn validate_step(config: &AppConfig, raw: &str) -> Result<(), ConfigError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(());
    }
    let action = Action::parse(Some(raw));
    if let Action::Pause(ms) = &action {
        return if !ms.is_empty()
            && ms.bytes().all(|c| c.is_ascii_digit())
            && ms
                .parse::<u64>()
                .is_ok_and(|v| super::timing::PAUSE.contains(v))
        {
            Ok(())
        } else {
            Err(ConfigError::Macro(MacroIssue::PauseRange))
        };
    }
    if let Action::Keys(keys) = &action
        && !actions::valid_held_key(keys)
    {
        return Err(ConfigError::Macro(MacroIssue::UnknownKey));
    }
    if let Some(issue) = actions::validate(&action, config) {
        return Err(ConfigError::InvalidAction(issue));
    }
    Ok(())
}

pub(crate) fn validate_loaded(config: &AppConfig, item: &Macro) -> Result<(), ConfigError> {
    if !super::ids::valid(&item.id) {
        return Err(ConfigError::Macro(MacroIssue::InvalidId));
    }
    if config.macros.iter().filter(|m| m.id == item.id).count() > 1 {
        return Err(ConfigError::Macro(MacroIssue::DuplicateId));
    }
    if item
        .step_pause_ms
        .is_some_and(|v| !super::timing::MACRO_DELAY.contains(v))
        || item
            .modifier_delay_ms
            .is_some_and(|v| !super::timing::MACRO_DELAY.contains(v))
    {
        return Err(ConfigError::Macro(MacroIssue::DelayRange));
    }
    for step in &item.steps {
        validate_step(config, &step.action)?;
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
            Vec::new()
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
            Some(rule.long_hold_action.as_str()),
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
    }
    for item in &config.macros {
        if item.steps.iter().any(|s| s.action.trim() == action) {
            places.push(format!("macro:{}", item.id));
        }
    }
    for (key, item) in config.quick_action_pages.iter().flat_map(|p| &p.cells) {
        if item.action.trim() == action {
            let name = if item.name.is_empty() {
                key
            } else {
                &item.name
            };
            places.push(format!("quick:{name}"));
        }
    }
    places
}

/// Point `macro:<old>` actions at the new ids. All renames apply at once, so
/// swapping two ids works.
fn rename_references(layout: &mut LayoutPreset, renames: &[(String, String)]) {
    let renames: Vec<_> = renames
        .iter()
        .filter(|(old, new)| old != new)
        .map(|(old, new)| (format!("macro:{old}"), format!("macro:{new}")))
        .collect();
    if renames.is_empty() {
        return;
    }
    layout.for_each_action_mut(|action| {
        if let Some((_, new)) = renames.iter().find(|(old, _)| action.trim() == old) {
            *action = new.clone();
        }
    });
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

    /// Replace all user macros at once; every macro must be valid.
    /// `renames` maps old macro ids to new ones; references `macro:<old>`
    /// elsewhere in the layout follow the new id.
    pub fn save_macros(
        &mut self,
        items: Vec<Macro>,
        renames: &[(String, String)],
    ) -> Result<(), ConfigError> {
        let mut candidate = self.config();
        candidate.macros = items.clone();
        for item in &items {
            validate(&candidate, item)?;
        }
        self.update_layout(|layout| {
            layout.macros = items;
            rename_references(layout, renames);
        })
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
            .save_user_layout(
                "Test",
                r#"{"schemaVersion": 1, "rules": [{"key": "KeyA", "tap": "macro:outer"}]}"#,
                true,
            )
            .unwrap();
        paths
            .save_config(r#"{"schemaVersion":1,"state":{"editedLayoutId":"user:Test"}}"#)
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
            .save_macro(
                None,
                item("outer", &["macro:inner", "builtin:copyLine", ""]),
            )
            .unwrap();
        assert_eq!(usage(&document.config(), "outer"), ["KeyA"]);
        assert_eq!(usage(&document.config(), "inner"), ["macro:outer"]);
        document.move_macro(0, 1).unwrap();
        let loaded = &document;
        assert_eq!(loaded.layout().macros[0].id, "inner");
        assert_eq!(loaded.layout().macros[0].steps[2].action, "text: hello ");
        assert_eq!(loaded.layout().macros[0].modifier_delay_ms, Some(0));
        assert_eq!(document.layout().rules, rules);
        assert_eq!(
            loaded.layout().rules[0].tap_action.as_deref(),
            Some("macro:outer")
        );
        document.remove_macro(1).unwrap();
        assert_eq!(document.layout().macros.len(), 1);
    }

    #[test]
    fn builtin_names_can_be_saved_and_renamed_without_changing_builtin_references() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths).unwrap();
        document
            .save_macro(None, item("copyLine", &["builtin:copyLine"]))
            .unwrap();
        document
            .save_macro(None, item("outer", &["macro:copyLine", "builtin:copyLine"]))
            .unwrap();
        assert_eq!(usage(&document.config(), "copyLine"), ["macro:outer"]);
        let mut layout = document.layout().clone();
        rename_references(&mut layout, &[("copyLine".into(), "custom".into())]);
        let user = layout.macros.iter().find(|m| m.id == "copyLine").unwrap();
        let outer = layout.macros.iter().find(|m| m.id == "outer").unwrap();
        assert_eq!(user.steps[0].action, "builtin:copyLine");
        assert_eq!(outer.steps[0].action, "macro:custom");
        assert_eq!(outer.steps[1].action, "builtin:copyLine");
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
            super::super::layout_file::serialize(document.layout()),
            super::super::layout_file::serialize(&before)
        );
        paths.save_config(r#"{"schemaVersion":1}"#).unwrap();
        document.save_macro(None, item("new", &[])).unwrap();
    }

    #[test]
    fn renamed_ids_carry_their_references() {
        let mut layout = LayoutPreset::initial();
        let mut rule = crate::profile::model::LayerRule::new("r1".into(), "KeyA");
        rule.tap_action = Some("macro:a".into());
        rule.hold_action = Some("macro:b".into());
        layout.rules.push(rule);
        layout.macros = vec![item("x", &["macro:a", "macro:other"])];
        let renames = [("a".into(), "b".into()), ("b".into(), "a".into())];
        rename_references(&mut layout, &renames);
        assert_eq!(layout.rules[0].tap_action.as_deref(), Some("macro:b"));
        assert_eq!(layout.rules[0].hold_action.as_deref(), Some("macro:a"));
        let steps: Vec<_> = layout.macros[0]
            .steps
            .iter()
            .map(|s| &s.action[..])
            .collect();
        assert_eq!(steps, ["macro:b", "macro:other"]);
    }
}
