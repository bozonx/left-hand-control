//! Rules page: triggers that activate layers or run actions.

use super::{game_condition, game_condition_value, parse_list};
use crate::{
    document::{Document, Saved, View},
    i18n::Msg,
    ui::{ActionPicker, PickerTarget, RuleDialog, RuleField, RuleLayerChoice, RuleRow, RulesEditor, SettingsWindow},
};
use lhc_core::profile::{
    diagnostics, ids,
    model::{Layer, LayerRule, LayoutPreset},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

fn join(value: &Option<Vec<String>>) -> String {
    value.as_ref().map(|items| super::condition_list(items)).unwrap_or_default()
}

fn optional_list(value: &str) -> Option<Vec<String>> {
    let items = parse_list(value);
    (!items.is_empty()).then_some(items)
}

fn optional(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn timeout_text(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// Empty means "use the default"; anything else must be a non-negative integer.
fn parse_timeout(value: &str) -> Result<Option<u64>, Msg> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value.parse().map(Some).map_err(|_| Msg::TimeoutInvalid)
}

fn layer_name(layout: &LayoutPreset, id: &str) -> String {
    layout
        .layers
        .iter()
        .find(|layer| layer.id == id)
        .map_or_else(|| id.to_owned(), |layer| layer.name.clone())
}

fn has_conditions(rule: &LayerRule) -> bool {
    rule.condition_game_mode.is_some()
        || rule.condition_layouts.is_some()
        || rule.condition_apps_whitelist.is_some()
        || rule.condition_apps_blacklist.is_some()
}

/// Selected rule index, or `None` when nothing valid is selected.
fn selected(ui: &SettingsWindow, document: &Document) -> Option<usize> {
    usize::try_from(ui.global::<RulesEditor>().get_selected())
        .ok()
        .filter(|index| *index < document.read().layout().rules.len())
}

/// Show the rules; with `fields` also load the selected rule into the
/// editable fields (never while the user types into them).
fn refresh(ui: &SettingsWindow, document: &Document, fields: bool) {
    let config = document.read();
    let layout = config.layout();
    let editor = ui.global::<RulesEditor>();
    editor.set_rows(ModelRc::new(VecModel::from(
        layout
            .rules
            .iter()
            .map(|rule| RuleRow {
                key: rule.key.clone().into(),
                tap: rule.tap_action.clone().unwrap_or_default().into(),
                hold: rule.hold_action.clone().unwrap_or_default().into(),
                double_tap: rule.double_tap_action.clone().into(),
                layer: layer_name(layout, &rule.layer_id).into(),
                has_conditions: has_conditions(rule),
                hold_timeout: timeout_text(rule.hold_timeout_ms).into(),
                double_timeout: timeout_text(rule.double_tap_timeout_ms).into(),
                enabled: rule.is_enabled(),
            })
            .collect::<Vec<_>>(),
    )));
    let index = usize::try_from(editor.get_selected())
        .ok()
        .filter(|index| *index < layout.rules.len());
    editor.set_selected(index.map_or(-1, |index| index as i32));
    let (true, Some(rule)) = (fields, index.and_then(|index| layout.rules.get(index))) else {
        return;
    };
    editor.set_enabled(rule.is_enabled());
    editor.set_key(rule.key.clone().into());
    editor.set_layer(rule.layer_id.clone().into());
    editor.set_tap(rule.tap_action.clone().unwrap_or_default().into());
    editor.set_swallow_tap(rule.tap_action.is_none());
    editor.set_hold(rule.hold_action.clone().unwrap_or_default().into());
    editor.set_swallow_hold(rule.hold_action.is_none());
    editor.set_double_tap(rule.double_tap_action.clone().into());
    editor.set_game_mode(game_condition(rule.condition_game_mode.as_deref()));
    editor.set_layouts(join(&rule.condition_layouts).into());
    editor.set_apps_include(join(&rule.condition_apps_whitelist).into());
    editor.set_apps_exclude(join(&rule.condition_apps_blacklist).into());
    editor.set_isolate(rule.isolate.clone().unwrap_or_default().into());
    editor.set_hold_for(rule.hold_for.clone().unwrap_or_default().into());
    editor.set_hold_timeout(timeout_text(rule.hold_timeout_ms).into());
    editor.set_double_timeout(timeout_text(rule.double_tap_timeout_ms).into());
}

/// Status after a saved change: mapper problems first, then rule warnings.
fn report<T>(ui: &SettingsWindow, document: &Document, saved: &Saved<T>, ok: Msg) -> Msg {
    let issue = diagnostics::analyze_rules(&document.read().config())
        .into_iter()
        .next();
    let message = match (&saved.runtime, issue) {
        (Err(_), _) => saved.message(ok),
        (Ok(()), Some(issue)) => Msg::Rule(issue),
        (Ok(()), None) => ok,
    };
    ui.global::<RulesEditor>().set_status(message.to_ui());
    message
}

/// Save `edit` of the layout's rules and refresh the page.
fn change(
    ui: &SettingsWindow,
    document: &Document,
    fields: bool,
    edit: impl FnOnce(&mut LayoutPreset),
) -> Result<(), Msg> {
    match document.edit(View::Rules, |config| config.update_layout(edit)) {
        Ok(saved) => {
            refresh(ui, document, fields);
            report(ui, document, &saved, Msg::RuleSaved);
            Ok(())
        }
        Err(error) => {
            refresh(ui, document, true);
            let message = Msg::from(&error);
            ui.global::<RulesEditor>().set_status(message.to_ui());
            Err(message)
        }
    }
}

/// Change rule `index`; a rule that vanished meanwhile is left alone.
fn change_rule(
    ui: &SettingsWindow,
    document: &Document,
    index: usize,
    fields: bool,
    edit: impl FnOnce(&mut LayerRule),
) -> Result<(), Msg> {
    change(ui, document, fields, |layout| {
        if let Some(rule) = layout.rules.get_mut(index) {
            edit(rule);
        }
    })
}

/// Set the property a dialog (or the picker) edits.
fn set_property(rule: &mut LayerRule, property: RuleDialog, value: &str) -> Result<(), Msg> {
    match property {
        RuleDialog::Key => rule.key = value.into(),
        RuleDialog::Layer => rule.layer_id = value.into(),
        RuleDialog::Tap => rule.tap_action = Some(value.into()),
        RuleDialog::Hold => rule.hold_action = Some(value.into()),
        RuleDialog::DoubleTap => rule.double_tap_action = value.into(),
        RuleDialog::HoldTimeout => rule.hold_timeout_ms = parse_timeout(value)?,
        RuleDialog::DoubleTimeout => rule.double_tap_timeout_ms = parse_timeout(value)?,
        RuleDialog::None
        | RuleDialog::Conditions
        | RuleDialog::Advanced
        | RuleDialog::Remove => {}
    }
    Ok(())
}

/// Set a property edited in place. Swallowing keeps `current` so turning
/// it off restores the previous action.
fn set_field(rule: &mut LayerRule, field: RuleField, value: &str, current: &str) {
    let on = value == "true";
    match field {
        RuleField::Enabled => rule.enabled = (!on).then_some(false),
        RuleField::SwallowTap => rule.tap_action = (!on).then(|| current.to_owned()),
        RuleField::SwallowHold => rule.hold_action = (!on).then(|| current.to_owned()),
        RuleField::Isolate => rule.isolate = optional(value),
        RuleField::HoldFor => rule.hold_for = optional(value),
    }
}

/// Apply `value` to the property `RulesEditor.field` of the selected rule.
pub(super) fn choose(ui: &SettingsWindow, document: &Document, value: &str) -> Result<(), Msg> {
    let editor = ui.global::<RulesEditor>();
    let property = editor.get_field();
    let index = selected(ui, document).ok_or(Msg::None)?;
    // Validate before saving so a bad value leaves the rule untouched.
    set_property(&mut LayerRule::new(String::new(), ""), property, value).inspect_err(|error| {
        editor.set_status(error.to_ui());
    })?;
    change_rule(ui, document, index, true, |rule| {
        let _ = set_property(rule, property, value);
    })?;
    editor.set_dialog(RuleDialog::None);
    editor.set_field(RuleDialog::None);
    Ok(())
}

fn layer_choices<'a>(layers: impl Iterator<Item = &'a Layer>) -> ModelRc<RuleLayerChoice> {
    ModelRc::new(VecModel::from(
        layers
            .map(|layer| RuleLayerChoice {
                id: layer.id.clone().into(),
                label: layer.name.clone().into(),
            })
            .collect::<Vec<_>>(),
    ))
}

fn unique_rule_id(rules: &[LayerRule]) -> String {
    (1..)
        .map(|n| format!("slint-rule-{n}"))
        .find(|id| rules.iter().all(|rule| rule.id != *id))
        .unwrap_or_else(|| ids::generate("rule_"))
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    refresh(ui, document, true);
    let weak = ui.as_weak();
    document.subscribe(View::Rules, move |document| {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, document, true);
        }
    });
    let editor = ui.global::<RulesEditor>();

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &doc, true);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_select(move |index| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<RulesEditor>().set_selected(index);
            refresh(&ui, &doc, true);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_add(move || {
        let Some(ui) = weak.upgrade() else { return };
        let count = doc.read().layout().rules.len();
        let result = change(&ui, &doc, true, |layout| {
            let id = unique_rule_id(&layout.rules);
            layout.rules.push(LayerRule::new(id, ""));
        });
        if result.is_ok() {
            ui.global::<RulesEditor>().set_selected(count as i32);
            refresh(&ui, &doc, true);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_remove(move || {
        let Some(ui) = weak.upgrade() else { return };
        let Some(index) = selected(&ui, &doc) else { return };
        let result = change(&ui, &doc, true, |layout| {
            if index < layout.rules.len() {
                layout.rules.remove(index);
            }
        });
        if result.is_ok() {
            let editor = ui.global::<RulesEditor>();
            let remaining = doc.read().layout().rules.len();
            let next = if remaining == 0 { -1 } else { index.min(remaining - 1) as i32 };
            editor.set_selected(next);
            editor.set_dialog(RuleDialog::None);
            refresh(&ui, &doc, true);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_move(move |from, to| {
        let Some(ui) = weak.upgrade() else { return };
        let len = doc.read().layout().rules.len();
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else { return };
        if from >= len || to >= len { return; }
        if change(&ui, &doc, true, |layout| {
            let item = layout.rules.remove(from);
            layout.rules.insert(to, item);
        }).is_ok() {
            ui.global::<RulesEditor>().set_selected(to as i32);
            refresh(&ui, &doc, true);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_create_layer(move |name| {
        let Some(ui) = weak.upgrade() else { return };
        let name = name.trim().to_owned();
        let editor = ui.global::<RulesEditor>();
        if name.is_empty() {
            editor.set_status(Msg::LayerNameRequired.to_ui());
            return;
        }
        let index = selected(&ui, &doc);
        let id = ids::generate("l_");
        let result = change(&ui, &doc, true, |layout| {
            layout.layers.push(Layer {
                id: id.clone(),
                name,
                description: None,
            });
            if let Some(rule) = index.and_then(|index| layout.rules.get_mut(index)) {
                rule.layer_id = id.clone();
            }
        });
        if result.is_ok() {
            editor.set_new_layer_name("".into());
            editor.set_dialog(RuleDialog::None);
            editor.set_field(RuleDialog::None);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_open_dialog(move |index, dialog| {
        let Some(ui) = weak.upgrade() else { return };
        let editor = ui.global::<RulesEditor>();
        editor.set_selected(index);
        refresh(&ui, &doc, true);
        if selected(&ui, &doc).is_none() {
            return;
        }
        editor.set_status(Msg::None.to_ui());
        editor.set_field(dialog);
        let value = match dialog {
            RuleDialog::Key => editor.get_key(),
            RuleDialog::Tap => editor.get_tap(),
            RuleDialog::Hold => editor.get_hold(),
            RuleDialog::DoubleTap => editor.get_double_tap(),
            RuleDialog::HoldTimeout => editor.get_hold_timeout(),
            RuleDialog::DoubleTimeout => editor.get_double_timeout(),
            _ => "".into(),
        };
        if matches!(
            dialog,
            RuleDialog::Key | RuleDialog::Tap | RuleDialog::Hold | RuleDialog::DoubleTap
        ) {
            ui.global::<ActionPicker>().invoke_open(
                PickerTarget::Rule,
                index,
                value,
                dialog == RuleDialog::Key,
            );
            return;
        }
        if dialog == RuleDialog::Layer {
            editor.set_layer_items(layer_choices(doc.read().layout().layers.iter()));
            editor.set_new_layer_name("".into());
            editor.set_dialog_value("".into());
        } else {
            editor.set_dialog_value(value);
        }
        editor.set_dialog(dialog);
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_filter_layers(move |query| {
        let Some(ui) = weak.upgrade() else { return };
        let query = query.to_lowercase();
        let config = doc.read();
        ui.global::<RulesEditor>().set_layer_items(layer_choices(
            config.layout().layers.iter().filter(|layer| {
                layer.name.to_lowercase().contains(&query) || layer.id.to_lowercase().contains(&query)
            }),
        ));
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_choose(move |value| {
        if let Some(ui) = weak.upgrade() {
            let _ = choose(&ui, &doc, &value);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_apply_conditions(move || {
        let Some(ui) = weak.upgrade() else { return };
        let Some(index) = selected(&ui, &doc) else { return };
        let editor = ui.global::<RulesEditor>();
        let game_mode = game_condition_value(editor.get_game_mode());
        let layouts = optional_list(&editor.get_layouts());
        let include = optional_list(&editor.get_apps_include());
        let exclude = optional_list(&editor.get_apps_exclude());
        let result = change_rule(&ui, &doc, index, true, |rule| {
            rule.condition_game_mode = game_mode;
            rule.condition_layouts = layouts;
            rule.condition_apps_whitelist = include;
            rule.condition_apps_blacklist = exclude;
        });
        if result.is_ok() {
            editor.set_dialog(RuleDialog::None);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_edit(move |field, value| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(index) = selected(&ui, &doc) else { return };
        let editor = ui.global::<RulesEditor>();
        let current = match field {
            RuleField::SwallowTap => editor.get_tap(),
            RuleField::SwallowHold => editor.get_hold(),
            _ => "".into(),
        };
        let _ = change_rule(&ui, &doc, index, false, |rule| {
            set_field(rule, field, &value, &current)
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_validate_and_apply() {
        let mut rule = LayerRule::new("r".into(), "");
        set_property(&mut rule, RuleDialog::Key, "CapsLock").unwrap();
        set_property(&mut rule, RuleDialog::HoldTimeout, " 250 ").unwrap();
        assert_eq!((rule.key.as_str(), rule.hold_timeout_ms), ("CapsLock", Some(250)));
        assert_eq!(
            set_property(&mut rule, RuleDialog::DoubleTimeout, "-1"),
            Err(Msg::TimeoutInvalid)
        );
        set_property(&mut rule, RuleDialog::HoldTimeout, "").unwrap();
        assert_eq!(rule.hold_timeout_ms, None);
    }

    #[test]
    fn swallow_toggles_keep_the_action() {
        let mut rule = LayerRule::new("r".into(), "KeyA");
        rule.tap_action = Some("KeyB".into());
        set_field(&mut rule, RuleField::SwallowTap, "true", "KeyB");
        assert_eq!(rule.tap_action, None);
        set_field(&mut rule, RuleField::SwallowTap, "false", "KeyB");
        assert_eq!(rule.tap_action.as_deref(), Some("KeyB"));
        set_field(&mut rule, RuleField::Enabled, "false", "");
        assert!(!rule.is_enabled());
        set_field(&mut rule, RuleField::Isolate, "  ", "");
        assert_eq!(rule.isolate, None);
    }

    #[test]
    fn rule_ids_are_unique() {
        let rules = vec![LayerRule::new("slint-rule-1".into(), "")];
        assert_eq!(unique_rule_id(&rules), "slint-rule-2");
    }
}
