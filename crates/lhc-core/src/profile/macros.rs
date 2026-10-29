//! Macro editing: validation, usage lookup and reference renaming.

use super::macro_graph::{MacroGraph, MacroRef};
use super::{
    actions::{self, Action},
    model::{LayoutPreset, Macro, ResolvedAppConfig},
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

pub fn validate(config: &ResolvedAppConfig, item: &Macro) -> Result<(), ConfigError> {
    validate_loaded(config, item)
}

pub(crate) fn validate_step(config: &ResolvedAppConfig, raw: &str) -> Result<(), ConfigError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(());
    }
    let action = Action::parse(Some(raw));
    if let Action::Pause(ms) = &action {
        return if super::timing::PAUSE.parse(ms).is_some() {
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

pub(crate) fn validate_loaded(config: &ResolvedAppConfig, item: &Macro) -> Result<(), ConfigError> {
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
    let graph = MacroGraph::new(config.macros.iter().map(|item| {
        (
            item.id.as_str(),
            item.steps.iter().map(|step| step.action.as_str()).collect(),
        )
    }));
    if graph.has_cycle(&MacroRef::User(item.id.clone())) {
        return Err(ConfigError::Macro(MacroIssue::Cycle));
    }
    Ok(())
}

pub fn usage(config: &ResolvedAppConfig, id: &str) -> Vec<String> {
    action_usage(config, &format!("macro:{id}"))
}

pub fn action_usage(config: &ResolvedAppConfig, action: &str) -> Vec<String> {
    use super::action_walk::ActionLocation;
    let mut places = Vec::new();
    let mut visited = HashSet::new();
    let action = Action::parse(Some(action));
    config.for_each_action(|location, raw| {
        if Action::parse(Some(raw)) != action {
            return;
        }
        let place = match location {
            ActionLocation::Rule { index, .. } => {
                if !visited.insert((0, index)) {
                    return;
                }
                config.rules[index].key.clone()
            }
            ActionLocation::Layer { layer, key } => format!("{layer}: {key}"),
            ActionLocation::Macro { index, .. } => {
                if !visited.insert((1, index)) {
                    return;
                }
                format!("macro:{}", config.macros[index].id)
            }
            ActionLocation::QuickAction { page, key } => {
                let cell = &config.quick_action_pages[page].cells[key];
                let name = if cell.name.is_empty() {
                    key
                } else {
                    &cell.name
                };
                format!("quick:{name}")
            }
        };
        places.push(place);
    });
    places
}

/// Point `macro:<old>` actions at the new ids. All renames apply at once, so
/// swapping two ids works.
fn rename_references(layout: &mut LayoutPreset, renames: &[(String, String)]) {
    layout.for_each_action_mut(|raw| {
        if let Action::Macro(id) = Action::parse(Some(raw))
            && let Some((_, new)) = renames.iter().find(|(old, new)| old != new && *old == id)
            && let Some(renamed) = Action::Macro(new.clone()).format()
        {
            *raw = renamed;
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
    fn usage_and_renaming_share_typed_actions_and_preserve_separate_rules() {
        let mut layout = LayoutPreset::default();
        for index in 0..2 {
            let mut rule = crate::profile::model::KeyRule::new(format!("r{index}"), "KeyA");
            rule.tap_action = Some(" macro: a ".into());
            rule.long_hold_action = "macro:a".into();
            layout.rules.push(rule);
        }
        layout.macros = vec![item("outer", &[" macro: a ", "macro:a", "text:macro:a"])];
        let config = ResolvedAppConfig::from_parts(Default::default(), layout.clone(), None);
        assert_eq!(usage(&config, "a"), ["KeyA", "KeyA", "macro:outer"]);
        rename_references(&mut layout, &[("a".into(), "b".into())]);
        assert_eq!(layout.rules[0].tap_action.as_deref(), Some("macro:b"));
        assert_eq!(layout.macros[0].steps[0].action, "macro:b");
        assert_eq!(layout.macros[0].steps[2].action, "text:macro:a");
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
                item("outer", &["macro:inner", "builtin:copyLineText", ""]),
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
            .save_macro(None, item("copyLine", &["builtin:copyLineText"]))
            .unwrap();
        document
            .save_macro(
                None,
                item("outer", &["macro:copyLine", "builtin:copyLineText"]),
            )
            .unwrap();
        assert_eq!(usage(&document.config(), "copyLine"), ["macro:outer"]);
        let mut layout = document.layout().clone();
        rename_references(&mut layout, &[("copyLine".into(), "custom".into())]);
        let user = layout.macros.iter().find(|m| m.id == "copyLine").unwrap();
        let outer = layout.macros.iter().find(|m| m.id == "outer").unwrap();
        assert_eq!(user.steps[0].action, "builtin:copyLineText");
        assert_eq!(outer.steps[0].action, "macro:custom");
        assert_eq!(outer.steps[1].action, "builtin:copyLineText");
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
        let mut rule = crate::profile::model::KeyRule::new("r1".into(), "KeyA");
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
