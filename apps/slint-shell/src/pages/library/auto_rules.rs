//! Automatic layout rule editing.

use super::*;

fn target_id(ui: &SettingsWindow, index: i32) -> Option<String> {
    usize::try_from(index)
        .ok()
        .and_then(|index| slint::Model::row_data(&ui.global::<LayoutLibrary>().get_names(), index))
        .map(|name| user_layout_id(&name))
}

/// Fill the rule dialog with rule `index`, or a new rule for `target`.
pub(super) fn load_rule(ui: &SettingsWindow, document: &Document, index: i32, target: i32) {
    let library = ui.global::<LayoutLibrary>();
    let rule = usize::try_from(index)
        .ok()
        .and_then(|index| document.read().settings().auto_rules.get(index).cloned());
    let (game, layouts, apps, excluded) = condition_fields(rule.map(|rule| rule.conditions));
    library.set_rule_index(if rule_exists(document, index) {
        index
    } else {
        -1
    });
    library.set_rule_target(target);
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
    let layout_id = target_id(ui, library.get_rule_target());
    let conditions = condition(
        library.get_rule_game(),
        &library.get_rule_layouts(),
        &library.get_rule_apps(),
        &library.get_rule_excluded(),
    );
    let index = usize::try_from(library.get_rule_index()).ok();
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

pub(super) fn delete_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
) -> Result<Msg, Msg> {
    let index = usize::try_from(index).map_err(|_| Msg::LibrarySelect)?;
    let message = edit_auto(ui, document, state, |settings| {
        if index < settings.auto_rules.len() {
            settings.auto_rules.remove(index);
        }
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
    let index = usize::try_from(index).map_err(|_| Msg::LibrarySelect)?;
    edit_auto(ui, document, state, |settings| {
        if let Some(rule) = settings.auto_rules.get_mut(index) {
            rule.enabled = enabled;
        }
    })
}

/// Ready-made rule: no layout while game mode is on.
pub(super) fn add_game_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
) -> Result<Msg, Msg> {
    edit_auto(ui, document, state, |settings| {
        settings.auto_rules.insert(
            0,
            AutoRule {
                id: ids::generate("rule-"),
                enabled: true,
                layout_id: None,
                conditions: LayoutConditionSet {
                    game_mode: Some("on".into()),
                    ..LayoutConditionSet::default()
                },
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
    let id = target_id(ui, index);
    edit_auto(ui, document, state, |settings| {
        settings.auto_default_layout_id = id
    })
}
