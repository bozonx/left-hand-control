//! Macro editing: validation, usage lookup and reference renaming.

use super::macro_graph::{MacroGraph, MacroRef};
use super::{
    actions::{self, Action},
    model::{Macro, ProfilePreset, ResolvedAppConfig},
};
use crate::config_document::{ConfigDocument, ConfigError};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroIssue {
    InvalidId,
    DuplicateId,
    DelayRange,
    PauseRange,
    UnknownKey,
    Cycle,
    ExpansionLimit,
    InUse,
}

/// A single analysis shared by all rows of the editor and diagnostics.
pub struct MacroAnalysis {
    pub errors: Vec<Option<ConfigError>>,
    pub step_errors: Vec<Vec<Option<ConfigError>>>,
    pub usage: HashMap<String, Vec<String>>,
}

pub fn analyze(config: &ResolvedAppConfig) -> MacroAnalysis {
    let graph = MacroGraph::new(config.macros.iter().map(|item| {
        (
            item.id.as_str(),
            item.steps.iter().map(|step| step.action.as_str()).collect(),
        )
    }));
    let expansions = graph.expansion_issues();
    let cycles = graph.cycle_members();
    let mut counts = HashMap::new();
    for item in &config.macros {
        *counts.entry(item.id.as_str()).or_insert(0usize) += 1;
    }
    let step_errors: Vec<Vec<_>> = config
        .macros
        .iter()
        .map(|item| {
            item.steps
                .iter()
                .map(|step| validate_step(config, &step.action).err())
                .collect()
        })
        .collect();
    let errors = config
        .macros
        .iter()
        .zip(&step_errors)
        .map(|(item, steps)| {
            let issue = if !super::ids::valid(&item.id) {
                Some(MacroIssue::InvalidId)
            } else if counts[item.id.as_str()] > 1 {
                Some(MacroIssue::DuplicateId)
            } else if item
                .step_pause_ms
                .is_some_and(|v| !super::timing::MACRO_DELAY.contains(v))
                || item
                    .modifier_delay_ms
                    .is_some_and(|v| !super::timing::MACRO_DELAY.contains(v))
            {
                Some(MacroIssue::DelayRange)
            } else if cycles.contains(&MacroRef::User(item.id.clone())) {
                Some(MacroIssue::Cycle)
            } else if expansions.contains_key(&MacroRef::User(item.id.clone())) {
                Some(MacroIssue::ExpansionLimit)
            } else {
                None
            };
            issue
                .map(ConfigError::Macro)
                .or_else(|| steps.iter().flatten().next().cloned())
        })
        .collect();
    let mut usage: HashMap<String, Vec<String>> = HashMap::new();
    let mut visited = HashSet::new();
    config.for_each_action(|location, raw| {
        if let Action::Macro(id) = Action::parse(Some(raw))
            && !matches!(location, super::action_walk::ActionLocation::Macro { index, .. } if config.macros[index].id == id)
            && let Some(place) = usage_place(config, location, &mut visited, &id)
        {
            usage.entry(id).or_default().push(place);
        }
    });
    MacroAnalysis {
        errors,
        step_errors,
        usage,
    }
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
    let mut candidate = config.clone();
    let index = candidate.macros.iter().position(|m| m.id == item.id);
    let index = if let Some(index) = index {
        candidate.macros[index] = item.clone();
        index
    } else {
        candidate.macros.push(item.clone());
        candidate.macros.len() - 1
    };
    analyze(&candidate).errors[index]
        .clone()
        .map_or(Ok(()), Err)
}

fn usage_place(
    config: &ResolvedAppConfig,
    location: super::action_walk::ActionLocation<'_>,
    visited: &mut HashSet<(String, u8, usize)>,
    id: &str,
) -> Option<String> {
    use super::action_walk::ActionLocation;
    Some(match location {
        ActionLocation::Rule { index, .. } => {
            if !visited.insert((id.into(), 0, index)) {
                return None;
            }
            config.rules[index].key.clone()
        }
        ActionLocation::Layer { layer, key } => format!("{layer}: {key}"),
        ActionLocation::Macro { index, .. } => {
            if !visited.insert((id.into(), 1, index)) {
                return None;
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
    })
}

pub fn usage(config: &ResolvedAppConfig, id: &str) -> Vec<String> {
    action_usage(config, &format!("macro:{id}"))
}

/// References that would remain after deleting this macro. Its own steps
/// disappear with it, so a self-reference must not prevent recovery by deletion.
pub fn deletion_usage(config: &ResolvedAppConfig, id: &str) -> Vec<String> {
    let mut remaining = config.clone();
    remaining.macros.retain(|item| item.id != id);
    usage(&remaining, id)
}

pub fn action_usage(config: &ResolvedAppConfig, action: &str) -> Vec<String> {
    let mut places = Vec::new();
    let mut visited = HashSet::new();
    let action = Action::parse(Some(action));
    config.for_each_action(|location, raw| {
        if Action::parse(Some(raw)) == action
            && let Some(place) = usage_place(config, location, &mut visited, "")
        {
            places.push(place);
        }
    });
    places
}

/// Point `macro:<old>` actions at the new ids. All renames apply at once, so
/// swapping two ids works.
fn rename_references(profile: &mut ProfilePreset, renames: &[(String, String)]) {
    profile.for_each_action_mut(|raw| rename_action(raw, renames));
}

fn rename_action(raw: &mut String, renames: &[(String, String)]) {
    if let Action::Macro(id) = Action::parse(Some(raw))
        && let Some((_, new)) = renames.iter().find(|(old, new)| old != new && *old == id)
        && let Some(renamed) = Action::Macro(new.clone()).format()
    {
        *raw = renamed;
    }
}

/// Preview a macro transaction, including incoming references everywhere in the profile.
pub fn preview(
    config: &ResolvedAppConfig,
    items: Vec<Macro>,
    renames: &[(String, String)],
) -> ResolvedAppConfig {
    let mut candidate = config.clone();
    candidate.macros = items;
    candidate.for_each_action_mut(|raw| rename_action(raw, renames));
    candidate
}

impl ConfigDocument {
    pub fn save_macro(&mut self, index: Option<usize>, item: Macro) -> Result<(), ConfigError> {
        let mut items = self.profile().macros.clone();
        let renames = if let Some(index) = index {
            let old = items
                .get_mut(index)
                .ok_or_else(|| ConfigError::Invalid("Unknown macro".into()))?;
            let rename = (old.id.clone(), item.id.clone());
            *old = item;
            vec![rename]
        } else {
            items.insert(0, item);
            Vec::new()
        };
        self.save_macros(items, &renames)
    }

    /// Replace all user macros at once; every macro must be valid.
    /// `renames` maps old macro ids to new ones; references `macro:<old>`
    /// elsewhere in the profile follow the new id.
    pub fn save_macros(
        &mut self,
        items: Vec<Macro>,
        renames: &[(String, String)],
    ) -> Result<(), ConfigError> {
        let candidate = preview(&self.config(), items.clone(), renames);
        if let Some(error) = analyze(&candidate).errors.into_iter().flatten().next() {
            return Err(error);
        }
        self.update_profile(|profile| {
            profile.macros = items;
            rename_references(profile, renames);
        })
    }

    pub fn remove_macro(&mut self, index: usize) -> Result<(), ConfigError> {
        if index >= self.profile().macros.len() {
            return Err(ConfigError::Invalid("Unknown macro".into()));
        }
        if !deletion_usage(&self.config(), &self.profile().macros[index].id).is_empty() {
            return Err(ConfigError::Macro(MacroIssue::InUse));
        }
        self.update_profile(|profile| {
            profile.macros.remove(index);
        })
    }

    pub fn move_macro(&mut self, index: usize, next: usize) -> Result<(), ConfigError> {
        if index >= self.profile().macros.len() || next >= self.profile().macros.len() {
            return Err(ConfigError::Invalid("Unknown macro".into()));
        }
        self.update_profile(|profile| {
            let item = profile.macros.remove(index);
            profile.macros.insert(next, item);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::model::MacroStep, storage::StoragePaths};

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
        let mut profile = ProfilePreset::default();
        for index in 0..2 {
            let mut rule = crate::config::model::KeyRule::new(format!("r{index}"), "KeyA");
            rule.tap_action = Some(" macro: a ".into());
            rule.long_hold_action = "macro:a".into();
            profile.rules.push(rule);
        }
        profile.macros = vec![item("outer", &[" macro: a ", "macro:a", "text:macro:a"])];
        let config = ResolvedAppConfig::from_parts(Default::default(), profile.clone(), None);
        assert_eq!(usage(&config, "a"), ["KeyA", "KeyA", "macro:outer"]);
        rename_references(&mut profile, &[("a".into(), "b".into())]);
        assert_eq!(profile.rules[0].tap_action.as_deref(), Some("macro:b"));
        assert_eq!(profile.macros[0].steps[0].action, "macro:b");
        assert_eq!(profile.macros[0].steps[2].action, "text:macro:a");
    }

    #[test]
    fn saves_reorders_and_removes_macros_without_changing_rules() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_user_profile(
                "Test",
                r#"{"version": 1, "rules": [{"key": "KeyA", "tap": "macro:outer"}]}"#,
                true,
            )
            .unwrap();
        paths.save_config(r#"{"version":1}"#).unwrap();
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        document.load_library_for_editing("Test").unwrap();
        let rules = document.profile().rules.clone();
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
        assert_eq!(loaded.profile().macros[0].id, "inner");
        assert_eq!(loaded.profile().macros[0].steps[2].action, "text: hello ");
        assert_eq!(loaded.profile().macros[0].modifier_delay_ms, Some(0));
        assert_eq!(document.profile().rules, rules);
        assert_eq!(
            loaded.profile().rules[0].tap_action.as_deref(),
            Some("macro:outer")
        );
        assert_eq!(
            document.remove_macro(1),
            Err(ConfigError::Macro(MacroIssue::InUse))
        );
        document
            .update_profile(|profile| profile.rules.clear())
            .unwrap();
        document.remove_macro(1).unwrap();
        assert_eq!(document.profile().macros.len(), 1);
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
        let mut profile = document.profile().clone();
        rename_references(&mut profile, &[("copyLine".into(), "custom".into())]);
        let user = profile.macros.iter().find(|m| m.id == "copyLine").unwrap();
        let outer = profile.macros.iter().find(|m| m.id == "outer").unwrap();
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
        let before = document.profile().clone();
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
            assert_eq!(document.profile(), &before);
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
            super::super::profile_file::serialize(document.profile()),
            super::super::profile_file::serialize(&before)
        );
        paths.save_config(r#"{"version":1}"#).unwrap();
        document.save_macro(None, item("new", &[])).unwrap();
    }

    #[test]
    fn renamed_ids_carry_their_references() {
        let mut profile = ProfilePreset::initial();
        let mut rule = crate::config::model::KeyRule::new("r1".into(), "KeyA");
        rule.tap_action = Some("macro:a".into());
        rule.hold_action = Some("macro:b".into());
        profile.rules.push(rule);
        profile.macros = vec![item("x", &["macro:a", "macro:other"])];
        let renames = [("a".into(), "b".into()), ("b".into(), "a".into())];
        rename_references(&mut profile, &renames);
        assert_eq!(profile.rules[0].tap_action.as_deref(), Some("macro:b"));
        assert_eq!(profile.rules[0].hold_action.as_deref(), Some("macro:a"));
        let steps: Vec<_> = profile.macros[0]
            .steps
            .iter()
            .map(|s| &s.action[..])
            .collect();
        assert_eq!(steps, ["macro:b", "macro:other"]);
    }
    #[test]
    fn save_transaction_renames_nested_references_before_validation_and_supports_swaps() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths).unwrap();
        document
            .save_macros(vec![item("a", &["KeyA"]), item("b", &["macro:a"])], &[])
            .unwrap();
        document
            .save_macros(
                vec![item("c", &["KeyA"]), item("b", &["macro:a"])],
                &[("a".into(), "c".into())],
            )
            .unwrap();
        assert_eq!(document.profile().macros[1].steps[0].action, "macro:c");
        document
            .save_macros(
                vec![item("b", &["KeyA"]), item("c", &["macro:c"])],
                &[("c".into(), "b".into()), ("b".into(), "c".into())],
            )
            .unwrap();
        assert_eq!(document.profile().macros[1].steps[0].action, "macro:b");
        assert!(
            analyze(&document.config())
                .errors
                .iter()
                .all(Option::is_none)
        );
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn editor_mapper_and_diagnostics_agree_on_expansion_limits() {
        let mut profile = ProfilePreset::default();
        for count in [1000, 1001] {
            profile.macros = vec![item("large", &vec!["KeyA"; count])];
            let config = ResolvedAppConfig::from_parts(Default::default(), profile.clone(), None);
            let accepted = validate(&config, &config.macros[0]).is_ok();
            let mapper = crate::mapper::config::MapperConfig::try_from(&config).unwrap();
            assert_eq!(accepted, count == 1000);
            assert_eq!(
                crate::mapper::validation::validate_config(&mapper).is_ok(),
                accepted
            );
            assert_eq!(
                super::super::diagnostics::analyze(&config)
                    .iter()
                    .all(|i| !i.code.is_error()),
                accepted
            );
        }
        profile.macros = (0..12)
            .map(|i| {
                item(
                    &format!("m{i}"),
                    &[if i == 11 {
                        "KeyA".into()
                    } else {
                        format!("macro:m{}", i + 1)
                    }]
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                )
            })
            .collect();
        let config = ResolvedAppConfig::from_parts(Default::default(), profile, None);
        assert_eq!(
            analyze(&config).errors[0],
            Some(ConfigError::Macro(MacroIssue::ExpansionLimit))
        );
    }
    #[test]
    fn self_references_do_not_prevent_deleting_an_invalid_macro() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths).unwrap();
        document
            .update_profile(|profile| profile.macros = vec![item("self", &["macro:self"])])
            .unwrap();
        assert!(deletion_usage(&document.config(), "self").is_empty());
        assert!(!analyze(&document.config()).usage.contains_key("self"));
        document.remove_macro(0).unwrap();
        assert!(document.profile().macros.is_empty());
    }
    #[test]
    fn preview_renames_incoming_references_in_every_section_without_mutating_source() {
        let mut profile = ProfilePreset::initial();
        let mut rule = crate::config::model::KeyRule::new("rule".into(), "KeyA");
        rule.tap_action = Some("macro:a".into());
        profile.rules.push(rule);
        profile
            .layer_keymaps
            .entry("nav".into())
            .or_default()
            .keys
            .insert("KeyB".into(), Some("macro:a".into()));
        profile.quick_action_pages[0].cells.insert(
            "KeyC".into(),
            crate::config::model::QuickAction {
                name: "quick".into(),
                action: "macro:a".into(),
            },
        );
        profile.macros = vec![item("a", &["KeyA"]), item("outer", &["macro:a"])];
        let config = ResolvedAppConfig::from_parts(Default::default(), profile, None);
        let renamed = preview(
            &config,
            vec![item("new", &["KeyA"]), item("outer", &["macro:a"])],
            &[("a".into(), "new".into())],
        );
        assert_eq!(deletion_usage(&renamed, "new").len(), 4);
        assert!(usage(&renamed, "a").is_empty());
        assert_eq!(usage(&config, "a").len(), 4);
        assert!(analyze(&renamed).errors.iter().all(Option::is_none));
    }
}
