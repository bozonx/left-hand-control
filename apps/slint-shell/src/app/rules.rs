use crate::{
    editor::{EditorHandle, KEY_CODES},
    ui::{RuleLayerChoice, RuleRow, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        auto_switch::AutoSwitchContext,
        diagnostics,
        model::{Layer, LayerRule},
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn list(value: &Option<Vec<String>>) -> String {
    value
        .as_ref()
        .map(|items| items.join(", "))
        .unwrap_or_default()
}
fn parse_list(value: &str) -> Option<Vec<String>> {
    let values: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    (!values.is_empty()).then_some(values)
}
fn optional(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}
fn timeout(value: &str) -> Result<Option<u64>, String> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        value
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| "Enter a non-negative integer".into())
    }
}
fn refresh(ui: &SettingsWindow, document: &ConfigDocument, selected: i32, fields: bool) {
    let rules = &document.layout().rules;
    ui.set_rule_rows(ModelRc::new(VecModel::from(
        rules
            .iter()
            .map(|rule| RuleRow {
                key: rule.key.clone().into(),
                tap: rule.tap_action.clone().unwrap_or_default().into(),
                hold: rule.hold_action.clone().unwrap_or_default().into(),
                double_tap: rule.double_tap_action.clone().into(),
                layer: document
                    .layout()
                    .layers
                    .iter()
                    .find(|layer| layer.id == rule.layer_id)
                    .map(|layer| layer.name.clone())
                    .unwrap_or_else(|| rule.layer_id.clone())
                    .into(),
                conditions: if rule.condition_game_mode.is_some()
                    || rule.condition_layouts.is_some()
                    || rule.condition_apps_whitelist.is_some()
                    || rule.condition_apps_blacklist.is_some()
                {
                    "Set".into()
                } else {
                    "".into()
                },
                hold_timeout: rule
                    .hold_timeout_ms
                    .map(|v| v.to_string())
                    .unwrap_or_default()
                    .into(),
                double_timeout: rule
                    .double_tap_timeout_ms
                    .map(|v| v.to_string())
                    .unwrap_or_default()
                    .into(),
                enabled: rule.is_enabled(),
            })
            .collect::<Vec<_>>(),
    )));
    let selected = if selected >= 0 && (selected as usize) < rules.len() {
        selected
    } else {
        -1
    };
    ui.set_selected_rule(selected);
    if !fields {
        return;
    }
    if let Some(rule) = rules.get(selected as usize) {
        ui.set_rule_enabled(rule.is_enabled());
        ui.set_rule_key(rule.key.clone().into());
        ui.set_rule_layer(rule.layer_id.clone().into());
        ui.set_rule_tap(rule.tap_action.clone().unwrap_or_default().into());
        ui.set_rule_swallow_tap(rule.tap_action.is_none());
        ui.set_rule_hold(rule.hold_action.clone().unwrap_or_default().into());
        ui.set_rule_swallow_hold(rule.hold_action.is_none());
        ui.set_rule_double_tap(rule.double_tap_action.clone().into());
        ui.set_rule_game_mode(rule.condition_game_mode.clone().unwrap_or_default().into());
        ui.set_rule_layouts(list(&rule.condition_layouts).into());
        ui.set_rule_apps_include(list(&rule.condition_apps_whitelist).into());
        ui.set_rule_apps_exclude(list(&rule.condition_apps_blacklist).into());
        ui.set_rule_isolate(rule.isolate.clone().unwrap_or_default().into());
        ui.set_rule_hold_for(rule.hold_for.clone().unwrap_or_default().into());
        ui.set_rule_hold_timeout(
            rule.hold_timeout_ms
                .map(|v| v.to_string())
                .unwrap_or_default()
                .into(),
        );
        ui.set_rule_double_timeout(
            rule.double_tap_timeout_ms
                .map(|v| v.to_string())
                .unwrap_or_default()
                .into(),
        );
    }
}
fn change(
    ui: &SettingsWindow,
    config: &Rc<RefCell<ConfigDocument>>,
    editor: &EditorHandle,
    selected: i32,
    fields: bool,
    edit: impl FnOnce(&mut Vec<LayerRule>),
) {
    let result = config
        .borrow_mut()
        .update_layout(|layout| edit(&mut layout.rules));
    match result {
        Ok(()) => {
            let document = config.borrow();
            refresh(ui, &document, selected, fields);
            editor.reload(&document);
            let issues = diagnostics::analyze_rules(&document.config());
            let status = match document.runtime_config(&AutoSwitchContext::current()) {
                Ok(runtime) => {
                    match lhc_core::mapper::runtime::update_config_if_running(&runtime.json) {
                        Ok(()) => "Saved".to_owned(),
                        Err(error) => format!("Saved, mapper update failed: {error}"),
                    }
                }
                Err(error) => format!("Saved; mapper cannot use these rules: {error}"),
            };
            ui.set_rule_status(if let Some(issue) = issues.first() {
                format!("{status}. {issue}").into()
            } else {
                status.into()
            });
            ui.set_config_status(
                crate::i18n::Msg::ConfigSaved(document.layout().rules.len()).to_ui(),
            );
        }
        Err(error) => ui.set_rule_status(format!("Save failed: {error}").into()),
    }
}
pub(super) fn bind(
    ui: &SettingsWindow,
    config: Option<Rc<RefCell<ConfigDocument>>>,
    editor: EditorHandle,
) {
    if let Some(config) = &config {
        refresh(ui, &config.borrow(), -1, true);
    }
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_refresh_rules(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            refresh(&ui, &config.borrow(), ui.get_selected_rule(), true);
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_select_rule(move |index| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            refresh(&ui, &config.borrow(), index, true);
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    let editor_copy = editor.clone();
    ui.on_add_rule(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let count = config.borrow().layout().rules.len();
            change(&ui, config, &editor_copy, count as i32, true, |rules| {
                let mut n = 1;
                let id = loop {
                    let id = format!("slint-rule-{n}");
                    if rules.iter().all(|r| r.id != id) {
                        break id;
                    }
                    n += 1;
                };
                rules.push(LayerRule::new(id, ""));
            });
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    let editor_copy = editor.clone();
    ui.on_remove_rule(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let index = ui.get_selected_rule();
            if index < 0 {
                return;
            }
            let next = index.min(config.borrow().layout().rules.len() as i32 - 2);
            change(&ui, config, &editor_copy, next, true, |rules| {
                rules.remove(index as usize);
            });
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    let editor_copy = editor.clone();
    ui.on_move_rule(move |direction| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let index = ui.get_selected_rule();
            let next = index + direction;
            if index < 0 || next < 0 || next >= config.borrow().layout().rules.len() as i32 {
                return;
            }
            change(&ui, config, &editor_copy, next, true, |rules| {
                rules.swap(index as usize, next as usize)
            });
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    let editor_copy = editor.clone();
    ui.on_create_rule_layer(move |name| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let name = name.trim();
            if name.is_empty() {
                ui.set_rule_status("Enter a layer name".into());
                return;
            }
            let id = lhc_core::profile::ids::generate("l_");
            let index = ui.get_selected_rule();
            let result = config.borrow_mut().update_layout(|layout| {
                layout.layers.push(Layer {
                    id: id.clone(),
                    name: name.into(),
                    description: None,
                });
                if let Some(rule) = layout.rules.get_mut(index as usize) {
                    rule.layer_id = id.clone();
                }
            });
            match result {
                Ok(()) => {
                    let document = config.borrow();
                    refresh(&ui, &document, index, true);
                    editor_copy.reload(&document);
                    ui.set_new_rule_layer_name("".into());
                    let status = match document.runtime_config(&AutoSwitchContext::current()) {
                        Ok(runtime) => {
                            match lhc_core::mapper::runtime::update_config_if_running(&runtime.json)
                            {
                                Ok(()) => "Layer saved".to_owned(),
                                Err(error) => format!("Layer saved, mapper update failed: {error}"),
                            }
                        }
                        Err(error) => {
                            format!("Layer saved; mapper cannot use these rules: {error}")
                        }
                    };
                    ui.set_rule_status(status.into());
                }
                Err(error) => ui.set_rule_status(format!("Save failed: {error}").into()),
            }
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_open_rule_dialog(move |index, field| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            refresh(&ui, &config.borrow(), index, true);
            ui.set_rule_dialog_field(field);
            let value = match field {
                1 => ui.get_rule_key(),
                2 => ui.get_rule_layer(),
                3 => ui.get_rule_tap(),
                4 => ui.get_rule_hold(),
                5 => ui.get_rule_double_tap(),
                12 => ui.get_rule_hold_timeout(),
                13 => ui.get_rule_double_timeout(),
                _ => "".into(),
            };
            ui.set_rule_dialog_value(value);
            let choices: Vec<_> = if field == 1 {
                KEY_CODES.iter().map(|s| (*s).into()).collect()
            } else if field == 3 || field == 4 || field == 5 {
                KEY_CODES
                    .iter()
                    .map(|s| (*s).into())
                    .chain(
                        ["Ctrl+KeyC", "Ctrl+KeyV", "Alt+Tab", "Escape", "Enter"]
                            .into_iter()
                            .map(Into::into),
                    )
                    .collect()
            } else {
                Vec::new()
            };
            ui.set_rule_picker_items(ModelRc::new(VecModel::from(choices)));
            let layers = config
                .borrow()
                .layout()
                .layers
                .iter()
                .map(|layer| RuleLayerChoice {
                    id: layer.id.clone().into(),
                    label: layer.name.clone().into(),
                })
                .collect::<Vec<_>>();
            ui.set_rule_layer_items(ModelRc::new(VecModel::from(layers)));
            ui.set_rule_dialog_open(true);
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_filter_rule_choices(move |query| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let query = query.to_lowercase();
            let field = ui.get_rule_dialog_field();
            if field == 2 {
                let filtered = config
                    .borrow()
                    .layout()
                    .layers
                    .iter()
                    .filter(|layer| {
                        layer.name.to_lowercase().contains(query.as_str())
                            || layer.id.to_lowercase().contains(query.as_str())
                    })
                    .map(|layer| RuleLayerChoice {
                        id: layer.id.clone().into(),
                        label: layer.name.clone().into(),
                    })
                    .collect::<Vec<_>>();
                ui.set_rule_layer_items(ModelRc::new(VecModel::from(filtered)));
                return;
            }
            let choices = if field == 1 {
                KEY_CODES
                    .iter()
                    .map(|key| (*key).to_owned())
                    .collect::<Vec<_>>()
            } else {
                KEY_CODES
                    .iter()
                    .copied()
                    .chain([
                        "Ctrl+KeyC",
                        "Ctrl+KeyV",
                        "Alt+Tab",
                        "text:Hello",
                        "delay:100",
                        "showQuickMenu",
                        "showEmojiMenu",
                    ])
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            let filtered = choices
                .into_iter()
                .filter(|value| value.to_lowercase().contains(query.as_str()))
                .map(Into::into)
                .collect::<Vec<_>>();
            ui.set_rule_picker_items(ModelRc::new(VecModel::from(filtered)));
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    let editor_copy = editor.clone();
    ui.on_choose_rule_value(move |value| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let index = ui.get_selected_rule();
            let field = ui.get_rule_dialog_field();
            if index < 0 {
                return;
            }
            let parsed = if field == 12 || field == 13 {
                match timeout(&value) {
                    Ok(v) => v,
                    Err(error) => {
                        ui.set_rule_status(error.into());
                        return;
                    }
                }
            } else {
                None
            };
            change(&ui, config, &editor_copy, index, true, |rules| {
                let rule = &mut rules[index as usize];
                match field {
                    1 => rule.key = value.into(),
                    2 => rule.layer_id = value.into(),
                    3 => rule.tap_action = Some(value.into()),
                    4 => rule.hold_action = Some(value.into()),
                    5 => rule.double_tap_action = value.into(),
                    12 => rule.hold_timeout_ms = parsed,
                    13 => rule.double_tap_timeout_ms = parsed,
                    _ => {}
                }
            });
            ui.set_rule_dialog_open(false);
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    let editor_copy = editor.clone();
    ui.on_apply_rule_conditions(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let index = ui.get_selected_rule();
            if index < 0 {
                return;
            }
            let game_mode = ui.get_rule_game_mode();
            let layouts = ui.get_rule_layouts();
            let include = ui.get_rule_apps_include();
            let exclude = ui.get_rule_apps_exclude();
            change(&ui, config, &editor_copy, index, true, |rules| {
                let rule = &mut rules[index as usize];
                rule.condition_game_mode = optional(&game_mode);
                rule.condition_layouts = parse_list(&layouts);
                rule.condition_apps_whitelist = parse_list(&include);
                rule.condition_apps_blacklist = parse_list(&exclude);
            });
            ui.set_rule_dialog_open(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_edit_rule(move |field, value| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config) {
            let index = ui.get_selected_rule();
            if index < 0 {
                return;
            }
            let parsed = match field {
                12 | 13 => match timeout(&value) {
                    Ok(v) => v,
                    Err(error) => {
                        ui.set_rule_status(error.into());
                        return;
                    }
                },
                _ => None,
            };
            change(&ui, config, &editor, index, false, |rules| {
                let rule = &mut rules[index as usize];
                match field {
                    0 => rule.enabled = (value != "true").then_some(false),
                    1 => rule.key = value.into(),
                    2 => rule.layer_id = value.into(),
                    3 => rule.tap_action = Some(value.into()),
                    4 => rule.hold_action = Some(value.into()),
                    5 => rule.double_tap_action = value.into(),
                    6 => rule.condition_game_mode = optional(&value),
                    7 => rule.condition_layouts = parse_list(&value),
                    8 => rule.condition_apps_whitelist = parse_list(&value),
                    9 => rule.condition_apps_blacklist = parse_list(&value),
                    10 => rule.isolate = optional(&value),
                    11 => rule.hold_for = optional(&value),
                    12 => rule.hold_timeout_ms = parsed,
                    13 => rule.double_tap_timeout_ms = parsed,
                    14 => {
                        rule.tap_action = (value != "true").then(|| String::from(ui.get_rule_tap()))
                    }
                    15 => {
                        rule.hold_action =
                            (value != "true").then(|| String::from(ui.get_rule_hold()))
                    }
                    _ => {}
                }
            });
        }
    });
}
