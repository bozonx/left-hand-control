//! Rules page: triggers that activate layers or run actions.

use super::{game_condition, game_condition_value, parse_list};
use crate::{
    document::{Document, Saved, View},
    i18n::Msg,
    ui::{
        ActionPicker, HoldDetection, HoldRelease, PickerTarget, RuleDialog, RuleField,
        RuleLayerChoice, RuleRow, RulesEditor, SettingsWindow,
    },
};
use lhc_core::profile::{
    diagnostics, ids,
    model::{HoldBehavior, Layer, LayerRule, LayoutPreset, TapDecision},
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

fn join(value: &Option<Vec<String>>) -> String {
    value
        .as_ref()
        .map(|items| super::condition_list(items))
        .unwrap_or_default()
}

/// Up to [`CHIP_ITEMS`] entries for a rule-card chip, then `+N` for the rest.
fn chip_text(value: &Option<Vec<String>>) -> String {
    let items = value.as_deref().unwrap_or_default();
    let mut text = items
        .iter()
        .take(CHIP_ITEMS)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > CHIP_ITEMS {
        text.push_str(&format!(" +{}", items.len() - CHIP_ITEMS));
    }
    text
}

const CHIP_ITEMS: usize = 3;

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

/// Long-hold duration in milliseconds, as entered in a rule or in Settings.
pub(super) fn parse_long_hold(value: &str) -> Result<u64, Msg> {
    value
        .trim()
        .parse()
        .ok()
        .filter(|ms| (100..=60_000).contains(ms))
        .ok_or(Msg::LongHoldInvalid)
}

fn optional_long_hold(value: &str) -> Result<Option<u64>, Msg> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_long_hold(value).map(Some)
    }
}

fn hold_mode(rule: &LayerRule) -> i32 {
    match rule.hold_behavior() {
        HoldBehavior::None => 0,
        HoldBehavior::Layer => 1,
        HoldBehavior::Action => 2,
    }
}

fn hold_detection(rule: &LayerRule) -> HoldDetection {
    match rule.tap_decision {
        None => HoldDetection::Inherit,
        Some(TapDecision::PermissiveHold) => HoldDetection::Release,
        Some(TapDecision::HoldOnOtherKeyPress) => HoldDetection::Press,
    }
}

/// When the held key is pressed and the layer keys it applies to; a rule
/// with both lists set shows the stricter `only-with` list.
fn hold_release(rule: &LayerRule) -> (HoldRelease, String) {
    match (&rule.hold_for, &rule.isolate) {
        (Some(keys), _) => (HoldRelease::OnlyWith, keys.clone()),
        (None, Some(keys)) => (HoldRelease::Except, keys.clone()),
        (None, None) => (HoldRelease::Always, String::new()),
    }
}

/// Show `keys` (a comma-separated list) in the advanced dialog.
fn set_release_keys(editor: &RulesEditor, keys: &[String]) {
    editor.set_release_keys(keys.join(", ").into());
    editor.set_release_key_items(ModelRc::new(VecModel::from(
        keys.iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
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
    editor.set_default_long_hold_timeout(
        config
            .settings()
            .default_long_hold_timeout_ms
            .to_string()
            .into(),
    );
    editor.set_default_hold_timeout(config.settings().default_hold_timeout_ms.to_string().into());
    editor.set_default_hold_detection(if config.settings().tap_decision == "holdOnOtherKeyPress" {
        HoldDetection::Press
    } else {
        HoldDetection::Release
    });
    editor.set_default_double_timeout(
        config
            .settings()
            .default_double_tap_timeout_ms
            .to_string()
            .into(),
    );
    let rows = layout
        .rules
        .iter()
        .map(|rule| RuleRow {
            key: rule.key.clone().into(),
            tap: rule.tap_action.clone().unwrap_or_default().into(),
            hold: rule.hold_action.clone().unwrap_or_default().into(),
            long_hold: rule.long_hold_action.clone().into(),
            long_hold_timeout: timeout_text(rule.long_hold_timeout_ms).into(),
            hold_mode: hold_mode(rule),
            is_chord: rule.key.contains('+'),
            double_tap: rule.double_tap_action.clone().into(),
            layer: layer_name(layout, &rule.layer_id).into(),
            has_conditions: has_conditions(rule),
            game: game_condition(rule.condition_game_mode.as_deref()),
            languages: join(&rule.condition_layouts).into(),
            applications: chip_text(&rule.condition_apps_whitelist).into(),
            excluded: chip_text(&rule.condition_apps_blacklist).into(),
            hold_timeout: if hold_mode(rule) == 2 {
                String::new()
            } else {
                timeout_text(rule.hold_timeout_ms)
            }
            .into(),
            hold_detection: hold_detection(rule),
            double_timeout: timeout_text(if rule.key.contains('+') {
                None
            } else {
                rule.double_tap_timeout_ms
            })
            .into(),
            isolate: rule.isolate.clone().unwrap_or_default().into(),
            hold_for: rule.hold_for.clone().unwrap_or_default().into(),
            enabled: rule.is_enabled(),
            swallow_tap: rule.tap_action.is_none(),
            swallow_hold: rule.hold_action.is_none(),
            advanced: rule.isolate.is_some()
                || rule.hold_for.is_some()
                || rule.tap_decision.is_some(),
            key_category: super::picker::category_for_value(&rule.key),
            tap_category: super::picker::category_for_value(
                rule.tap_action.as_deref().unwrap_or_default(),
            ),
            hold_category: super::picker::category_for_value(
                rule.hold_action.as_deref().unwrap_or_default(),
            ),
            double_category: super::picker::category_for_value(&rule.double_tap_action),
            long_hold_category: super::picker::category_for_value(&rule.long_hold_action),
        })
        .collect::<Vec<_>>();
    let model = editor.get_rows();
    if let Some(model) = model.as_any().downcast_ref::<VecModel<RuleRow>>()
        && model.row_count() == rows.len()
    {
        for (index, row) in rows.into_iter().enumerate() {
            model.set_row_data(index, row);
        }
    } else {
        editor.set_rows(ModelRc::new(VecModel::from(rows)));
    }
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
    editor.set_long_hold(rule.long_hold_action.clone().into());
    editor.set_hold_mode(hold_mode(rule));
    let long_timeout = timeout_text(rule.long_hold_timeout_ms);
    editor.set_long_hold_timeout(long_timeout.clone().into());
    editor.set_original_long_hold_timeout(long_timeout.into());
    editor.set_swallow_hold(rule.hold_action.is_none());
    editor.set_is_chord(rule.key.contains('+'));
    editor.set_double_tap(rule.double_tap_action.clone().into());
    editor.set_game_mode(game_condition(rule.condition_game_mode.as_deref()));
    editor.set_layouts(join(&rule.condition_layouts).into());
    editor.set_apps_include(join(&rule.condition_apps_whitelist).into());
    editor.set_apps_exclude(join(&rule.condition_apps_blacklist).into());
    let (release, keys) = hold_release(rule);
    let keys = parse_list(&keys);
    editor.set_release_mode(release);
    set_release_keys(&editor, &keys);
    editor.set_original_release_mode(release);
    editor.set_original_release_keys(keys.join(", ").into());
    editor.set_original_hold(rule.hold_action.clone().unwrap_or_default().into());
    editor.set_original_swallow_hold(rule.hold_action.is_none());
    let decision = hold_detection(rule);
    editor.set_hold_detection(decision);
    editor.set_original_hold_detection(decision);
    editor.set_hold_timeout(timeout_text(rule.hold_timeout_ms).into());
    editor.set_double_timeout(timeout_text(rule.double_tap_timeout_ms).into());
    editor.set_original_hold_timeout(timeout_text(rule.hold_timeout_ms).into());
    editor.set_original_double_timeout(timeout_text(rule.double_tap_timeout_ms).into());
}

/// Status after a saved change: mapper problems first, then rule warnings.
fn report<T>(ui: &SettingsWindow, document: &Document, saved: &Saved<T>, ok: Msg) -> Msg {
    let issue = diagnostics::analyze_rules(&document.read().config())
        .into_iter()
        .max_by_key(|issue| issue.code.is_error());
    let message = match (&saved.runtime, issue) {
        (Err(_), _) => saved.message(ok),
        (Ok(()), Some(issue)) => Msg::Rule(issue),
        (Ok(()), None) => ok,
    };
    ui.global::<RulesEditor>()
        .set_status(crate::notifications::report(ui, &message));
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
            report(ui, document, &saved, Msg::None);
            Ok(())
        }
        Err(error) => {
            refresh(ui, document, true);
            let message = Msg::from(&error);
            ui.global::<RulesEditor>()
                .set_status(crate::notifications::report(ui, &message));
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
        RuleDialog::Layer => {
            if !value.is_empty() {
                set_hold_behavior(rule, HoldBehavior::Layer);
            }
            rule.layer_id = value.into();
        }
        RuleDialog::Tap => rule.tap_action = Some(value.into()),
        RuleDialog::Hold => rule.hold_action = Some(value.into()),
        RuleDialog::LongHold => {
            if !value.is_empty() {
                set_hold_behavior(rule, HoldBehavior::Action);
            }
            rule.long_hold_action = value.into();
        }
        RuleDialog::DoubleTap => rule.double_tap_action = value.into(),
        RuleDialog::HoldTimeout => rule.hold_timeout_ms = parse_timeout(value)?,
        RuleDialog::DoubleTimeout => rule.double_tap_timeout_ms = parse_timeout(value)?,
        RuleDialog::None
        | RuleDialog::Conditions
        | RuleDialog::Advanced
        | RuleDialog::Remove
        | RuleDialog::ReleaseKey => {}
    }
    Ok(())
}

/// Switch what holding the trigger does, dropping options of the other modes.
fn set_hold_behavior(rule: &mut LayerRule, behavior: HoldBehavior) {
    rule.hold_behavior = Some(behavior);
    if behavior != HoldBehavior::Layer {
        rule.layer_id.clear();
    }
    if behavior != HoldBehavior::Action {
        rule.long_hold_action.clear();
    }
    if behavior == HoldBehavior::None {
        rule.hold_action = Some(String::new());
        rule.isolate = None;
        rule.hold_for = None;
    }
    if behavior == HoldBehavior::Action {
        rule.hold_action = Some(String::new());
        rule.isolate = None;
        rule.hold_for = None;
    }
}

/// Set a property edited in place. Swallowing keeps `current` so turning
/// it off restores the previous action.
fn set_field(rule: &mut LayerRule, field: RuleField, value: &str, current: &str) {
    let on = value == "true";
    match field {
        RuleField::HoldMode => set_hold_behavior(
            rule,
            match value {
                "1" => HoldBehavior::Layer,
                "2" => HoldBehavior::Action,
                _ => HoldBehavior::None,
            },
        ),
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
    if property == RuleDialog::Key && !lhc_core::profile::key_catalog::valid_trigger(value) {
        return Err(Msg::KeyCodeRequired);
    }
    if property == RuleDialog::Key && editor.get_selected() == -1 {
        change(ui, document, true, |layout| {
            let id = unique_rule_id(&layout.rules);
            layout.rules.insert(0, LayerRule::new(id, value));
        })?;
        editor.set_selected(0);
        editor.set_created(0);
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(1400), move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<RulesEditor>().set_created(-1);
            }
        });
        refresh(ui, document, true);
        editor.set_field(RuleDialog::None);
        return Ok(());
    }
    if property == RuleDialog::Hold
        && !value.is_empty()
        && !lhc_core::profile::actions::valid_held_key(value)
    {
        return Err(Msg::ActionRequired);
    }
    // The advanced dialog keeps its keys as a draft until "Apply".
    if editor.get_dialog() == RuleDialog::Advanced
        && matches!(property, RuleDialog::Hold | RuleDialog::ReleaseKey)
    {
        if property == RuleDialog::Hold {
            if value.is_empty() {
                return Err(Msg::ActionRequired);
            }
            editor.set_hold(value.into());
            editor.set_swallow_hold(false);
        } else {
            if !lhc_core::profile::key_catalog::valid_key(value) {
                return Err(Msg::KeyCodeRequired);
            }
            let mut keys = parse_list(&editor.get_release_keys());
            if !keys.iter().any(|key| key == value) {
                keys.push(value.to_owned());
            }
            set_release_keys(&editor, &keys);
        }
        editor.set_field(RuleDialog::None);
        return Ok(());
    }
    let index = selected(ui, document).ok_or(Msg::None)?;
    // Validate before saving so a bad value leaves the rule untouched.
    set_property(&mut LayerRule::new(String::new(), ""), property, value).inspect_err(|error| {
        editor.set_status(crate::notifications::report(ui, error));
    })?;
    change_rule(ui, document, index, true, |rule| {
        let _ = set_property(rule, property, value);
        if ui.global::<ActionPicker>().get_ignore_key() && value.is_empty() {
            match property {
                RuleDialog::Tap => rule.tap_action = None,
                RuleDialog::Hold => rule.hold_action = None,
                _ => {}
            }
        }
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
    editor.on_add(move || {
        let Some(ui) = weak.upgrade() else { return };
        let editor = ui.global::<RulesEditor>();
        editor.set_selected(-1);
        editor.set_field(RuleDialog::Key);
        editor.set_dialog(RuleDialog::None);
        ui.global::<ActionPicker>()
            .invoke_open(PickerTarget::Rule, -1, "".into(), true);
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_remove(move || {
        let Some(ui) = weak.upgrade() else { return };
        let Some(index) = selected(&ui, &doc) else {
            return;
        };
        let result = change(&ui, &doc, true, |layout| {
            if index < layout.rules.len() {
                layout.rules.remove(index);
            }
        });
        if result.is_ok() {
            let editor = ui.global::<RulesEditor>();
            let remaining = doc.read().layout().rules.len();
            let next = if remaining == 0 {
                -1
            } else {
                index.min(remaining - 1) as i32
            };
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
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        if from >= len || to >= len {
            return;
        }
        if change(&ui, &doc, true, |layout| {
            let item = layout.rules.remove(from);
            layout.rules.insert(to, item);
        })
        .is_ok()
        {
            ui.global::<RulesEditor>().set_selected(to as i32);
            refresh(&ui, &doc, true);
        }
    });

    let doc = document.clone();
    editor.on_layer_name_error(move |name| super::name_error(doc.read().layer_name(&name, None)));

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_create_layer(move |name| {
        let Some(ui) = weak.upgrade() else { return };
        let editor = ui.global::<RulesEditor>();
        let checked = doc.read().layer_name(&name, None);
        let name = match checked {
            Ok(name) => name,
            Err(error) => {
                editor.set_status(crate::notifications::report(&ui, &Msg::from(&error)));
                return;
            }
        };
        let index = selected(&ui, &doc);
        let id = ids::generate("l_");
        let result = change(&ui, &doc, true, |layout| {
            layout.layers.push(Layer {
                id: id.clone(),
                name,
                description: None,
            });
            if let Some(rule) = index.and_then(|index| layout.rules.get_mut(index)) {
                set_hold_behavior(rule, HoldBehavior::Layer);
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
        // Keys picked for the advanced dialog must not reload its draft.
        let draft = editor.get_dialog() == RuleDialog::Advanced
            && matches!(dialog, RuleDialog::Hold | RuleDialog::ReleaseKey);
        editor.set_selected(index);
        if dialog != RuleDialog::Layer && !draft {
            refresh(&ui, &doc, true);
        }
        if selected(&ui, &doc).is_none() {
            return;
        }
        editor.set_status(Msg::None.to_ui());
        editor.set_field(dialog);
        let value = match dialog {
            RuleDialog::Key => editor.get_key(),
            RuleDialog::Tap => editor.get_tap(),
            RuleDialog::Hold => editor.get_hold(),
            RuleDialog::LongHold => editor.get_long_hold(),
            RuleDialog::DoubleTap => editor.get_double_tap(),
            RuleDialog::HoldTimeout => editor.get_hold_timeout(),
            RuleDialog::DoubleTimeout => editor.get_double_timeout(),
            _ => "".into(),
        };
        if matches!(
            dialog,
            RuleDialog::Key
                | RuleDialog::Tap
                | RuleDialog::Hold
                | RuleDialog::LongHold
                | RuleDialog::DoubleTap
                | RuleDialog::ReleaseKey
        ) {
            ui.global::<ActionPicker>().invoke_open(
                PickerTarget::Rule,
                index,
                value,
                matches!(dialog, RuleDialog::Key | RuleDialog::ReleaseKey),
            );
            return;
        }
        if dialog == RuleDialog::Layer {
            let config = doc.read();
            let layer_id = &config.layout().rules[index as usize].layer_id;
            editor.set_layer_id(layer_id.into());
            editor.set_layer_highlight(
                config
                    .layout()
                    .layers
                    .iter()
                    .position(|layer| &layer.id == layer_id)
                    .map_or(0, |index| index as i32 + 1),
            );
            editor.set_layer_items(layer_choices(config.layout().layers.iter()));
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
                layer.name.to_lowercase().contains(&query)
                    || layer.id.to_lowercase().contains(&query)
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
        let Some(index) = selected(&ui, &doc) else {
            return;
        };
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
    editor.on_apply_advanced(move || {
        let Some(ui) = weak.upgrade() else { return };
        let Some(index) = selected(&ui, &doc) else {
            return;
        };
        let editor = ui.global::<RulesEditor>();
        let timeouts = parse_timeout(&editor.get_hold_timeout()).and_then(|hold| {
            parse_timeout(&editor.get_double_timeout()).map(|double| (hold, double))
        });
        let (hold, double) = match timeouts {
            Ok(values) => values,
            Err(error) => {
                editor.set_status(crate::notifications::report(&ui, &error));
                return;
            }
        };
        // An unchanged value is kept even if it lies outside the range the editor offers.
        let long_hold = editor.get_long_hold_timeout();
        let long_hold_timeout = if long_hold == editor.get_original_long_hold_timeout() {
            parse_timeout(&long_hold)
        } else {
            optional_long_hold(&long_hold)
        };
        let long_hold_timeout = match long_hold_timeout {
            Ok(value) => value,
            Err(error) => {
                editor.set_status(crate::notifications::report(&ui, &error));
                return;
            }
        };
        let held = (!editor.get_swallow_hold()).then(|| editor.get_hold().to_string());
        // Release lists only apply to a key held together with a layer.
        let keys = optional(&editor.get_release_keys()).filter(|_| {
            editor.get_hold_mode() == 1 && held.as_deref().is_some_and(|key| !key.is_empty())
        });
        let (isolate, hold_for) = match editor.get_release_mode() {
            HoldRelease::Always => (None, None),
            HoldRelease::Except => (keys, None),
            HoldRelease::OnlyWith => (None, keys),
        };
        if change_rule(&ui, &doc, index, true, |rule| {
            rule.long_hold_timeout_ms = long_hold_timeout;
            rule.hold_timeout_ms = hold;
            rule.tap_decision = match editor.get_hold_detection() {
                HoldDetection::Release => Some(TapDecision::PermissiveHold),
                HoldDetection::Press => Some(TapDecision::HoldOnOtherKeyPress),
                HoldDetection::Inherit => None,
            };
            rule.double_tap_timeout_ms = double;
            if rule.hold_behavior() != HoldBehavior::Action {
                rule.hold_action = held;
            }
            rule.isolate = isolate;
            rule.hold_for = hold_for;
        })
        .is_ok()
        {
            editor.set_dialog(RuleDialog::None);
        }
    });

    let weak = ui.as_weak();
    editor.on_reset_advanced(move || {
        let Some(ui) = weak.upgrade() else { return };
        let editor = ui.global::<RulesEditor>();
        editor.set_hold(Default::default());
        editor.set_swallow_hold(false);
        editor.set_release_mode(HoldRelease::Always);
        set_release_keys(&editor, &[]);
        editor.set_hold_detection(HoldDetection::Inherit);
        editor.set_hold_timeout(Default::default());
        editor.set_long_hold_timeout(Default::default());
        editor.set_double_timeout(Default::default());
    });

    let weak = ui.as_weak();
    editor.on_remove_release_key(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let editor = ui.global::<RulesEditor>();
        let mut keys = parse_list(&editor.get_release_keys());
        if let Ok(index) = usize::try_from(index)
            && index < keys.len()
        {
            keys.remove(index);
            set_release_keys(&editor, &keys);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_edit(move |field, value| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(index) = selected(&ui, &doc) else {
            return;
        };
        let editor = ui.global::<RulesEditor>();
        let current = match field {
            RuleField::SwallowTap => editor.get_tap(),
            RuleField::SwallowHold => editor.get_hold(),
            _ => "".into(),
        };
        let _ = change_rule(&ui, &doc, index, false, |rule| {
            set_field(rule, field, &value, &current)
        });
        if field == RuleField::HoldMode {
            refresh(&ui, &doc, true);
        }
    });
}

#[cfg(test)]
#[path = "rules/tests.rs"]
mod tests;
