//! Automatic layout rule editing.

use super::*;

/// Only -1 explicitly requests passthrough; stale indices are errors.
fn target_id(names: &[String], index: i32) -> Result<Option<String>, Msg> {
    if index == -1 {
        return Ok(None);
    }
    usize::try_from(index)
        .ok()
        .and_then(|index| names.get(index))
        .map(|name| Some(user_layout_id(name)))
        .ok_or(Msg::LibraryChanged)
}

fn shown_names(ui: &SettingsWindow) -> Vec<String> {
    slint::Model::iter(&ui.global::<LayoutLibrary>().get_names())
        .map(|name| name.to_string())
        .collect()
}

/// Fill the rule dialog with rule `index`, or a new rule for `target`.
/// The names the dialog's target index refers to are kept until it is
/// saved, so a list refresh meanwhile cannot retarget the rule.
pub(super) fn load_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
    target: i32,
) {
    let library = ui.global::<LayoutLibrary>();
    let names = shown_names(ui);
    library.set_rule_names(super::super::strings(names.iter().cloned()));
    let rule = usize::try_from(index)
        .ok()
        .and_then(|index| document.read().settings().auto_rules.get(index).cloned());
    let (target, missing) = rule.as_ref().map_or((target, false), |rule| {
        rule.layout_id.as_ref().map_or((-1, false), |id| {
            names
                .iter()
                .position(|name| user_layout_id(name) == *id)
                .map_or((-1, true), |index| (index as i32, false))
        })
    });
    {
        let mut state = state.borrow_mut();
        state.rule_names = names;
        state.rule_id = rule.as_ref().map(|rule| rule.id.clone());
        state.rule_missing_target = rule.as_ref().and_then(|rule| rule.layout_id.clone());
    }
    let (game, layouts, apps, excluded) = condition_fields(rule.map(|rule| rule.conditions));
    library.set_rule_index(if rule_exists(document, index) {
        index
    } else {
        -1
    });
    library.set_rule_target(target);
    library.set_rule_target_missing(missing);
    library.set_rule_game(game);
    library.set_rule_layouts(layouts.into());
    library.set_rule_apps(apps.into());
    library.set_rule_excluded(excluded.into());
    library.set_status(Msg::None.to_ui());
    library.set_dialog(LibraryDialog::Rule);
}

fn rule_exists(document: &Document, index: i32) -> bool {
    usize::try_from(index).is_ok_and(|index| index < document.read().settings().auto_rules.len())
}

pub(super) fn save_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
) -> Result<Msg, Msg> {
    let library = ui.global::<LayoutLibrary>();
    let layout_id = {
        let state = state.borrow();
        if library.get_rule_target_missing() {
            Some(
                state
                    .rule_missing_target
                    .clone()
                    .ok_or(Msg::LibraryChanged)?,
            )
        } else {
            target_id(&state.rule_names, library.get_rule_target())?
        }
    };
    let conditions = condition(
        library.get_rule_game(),
        &library.get_rule_layouts(),
        &library.get_rule_apps(),
        &library.get_rule_excluded(),
    );
    check_patterns(&conditions)?;
    let index = state
        .borrow()
        .rule_id
        .as_ref()
        .map(|id| {
            document
                .read()
                .settings()
                .auto_rules
                .iter()
                .position(|rule| &rule.id == id)
                .ok_or(Msg::LibraryChanged)
        })
        .transpose()?;
    let message = edit_auto(ui, document, state, |settings| {
        match index.and_then(|index| settings.auto_rules.get_mut(index)) {
            Some(rule) => {
                rule.layout_id = layout_id;
                rule.conditions = conditions;
            }
            None => settings.auto_rules.insert(
                0,
                AutoRule {
                    id: ids::generate("rule-"),
                    enabled: true,
                    layout_id,
                    conditions,
                },
            ),
        }
    })?;
    library.set_dialog(LibraryDialog::None);
    Ok(message)
}

/// Refuse patterns that can never match (e.g. `title:` without text):
/// stored, they would block every layout from starting.
fn check_patterns(conditions: &LayoutConditionSet) -> Result<(), Msg> {
    let invalid = conditions
        .window_patterns
        .iter()
        .chain(&conditions.excluded_window_patterns)
        .find(|pattern| lhc_core::profile::app_match::AppMatcher::parse(pattern).is_none());
    match invalid {
        Some(pattern) => Err(Msg::Rule(lhc_core::profile::diagnostics::RuleIssue {
            code: lhc_core::profile::diagnostics::RuleIssueCode::InvalidCondition,
            rule_id: String::new(),
            trigger: Some(pattern.clone()),
        })),
        None => Ok(()),
    }
}

pub(super) fn delete_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
) -> Result<Msg, Msg> {
    let index = if index == ui.global::<LayoutLibrary>().get_rule_index() {
        state
            .borrow()
            .rule_id
            .as_ref()
            .map(|id| {
                document
                    .read()
                    .settings()
                    .auto_rules
                    .iter()
                    .position(|rule| &rule.id == id)
                    .ok_or(Msg::LibraryChanged)
            })
            .transpose()?
            .map(|index| index as i32)
            .unwrap_or(index)
    } else {
        index
    };
    let index = usize::try_from(index)
        .ok()
        .filter(|index| *index < document.read().settings().auto_rules.len())
        .ok_or(Msg::LibrarySelect)?;
    let message = edit_auto(ui, document, state, |settings| {
        settings.auto_rules.remove(index);
    })?;
    ui.global::<LayoutLibrary>().set_dialog(LibraryDialog::None);
    Ok(if matches!(message, Msg::None) {
        Msg::AutoRuleDeleted
    } else {
        message
    })
}

pub(super) fn move_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    from: i32,
    to: i32,
) -> Result<Msg, Msg> {
    let len = document.read().settings().auto_rules.len();
    if super::super::move_indices(from, to, len).is_none() {
        return Ok(Msg::None);
    }
    edit_auto(ui, document, state, |settings| {
        super::super::move_item(&mut settings.auto_rules, from, to);
    })
}

pub(super) fn set_rule_enabled(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
    enabled: bool,
) -> Result<Msg, Msg> {
    let index = usize::try_from(index)
        .ok()
        .filter(|index| *index < document.read().settings().auto_rules.len())
        .ok_or(Msg::LibrarySelect)?;
    edit_auto(ui, document, state, |settings| {
        settings.auto_rules[index].enabled = enabled;
    })
}

/// Ready-made rule: no layout while game mode is on.
pub(super) fn add_game_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
) -> Result<Msg, Msg> {
    let conditions = LayoutConditionSet {
        game_mode: Some(GameModeCondition::On),
        ..LayoutConditionSet::default()
    };
    // A second click must not stack identical rules.
    let exists = document
        .read()
        .settings()
        .auto_rules
        .iter()
        .any(|rule| rule.enabled && rule.layout_id.is_none() && rule.conditions == conditions);
    if exists {
        return Ok(Msg::None);
    }
    edit_auto(ui, document, state, |settings| {
        settings.auto_rules.insert(
            0,
            AutoRule {
                id: ids::generate("rule-"),
                enabled: true,
                layout_id: None,
                conditions,
            },
        );
    })
}

pub(super) fn set_auto_default(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
) -> Result<Msg, Msg> {
    let id = target_id(&shown_names(ui), index)?;
    edit_auto(ui, document, state, |settings| {
        settings.auto_default_layout_id = id
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_index_maps_to_id() {
        let names = vec!["A".to_owned(), "B".to_owned()];
        assert_eq!(target_id(&names, 1).unwrap(), Some(user_layout_id("B")));
        assert_eq!(target_id(&names, -1).unwrap(), None);
        assert!(target_id(&names, 2).is_err());
        assert!(target_id(&names, -2).is_err());
    }

    #[test]
    fn unusable_window_patterns_are_refused() {
        let set = |windows: &[&str]| LayoutConditionSet {
            window_patterns: windows.iter().map(|item| item.to_string()).collect(),
            ..LayoutConditionSet::default()
        };
        assert!(check_patterns(&set(&["kate", "title:YouTube"])).is_ok());
        assert!(check_patterns(&set(&["title:"])).is_err());
        let excluded = LayoutConditionSet {
            excluded_window_patterns: vec!["title:  ".into()],
            ..LayoutConditionSet::default()
        };
        assert!(check_patterns(&excluded).is_err());
    }
}
