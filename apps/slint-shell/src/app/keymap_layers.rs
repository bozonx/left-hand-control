use crate::{
    editor::KEY_CODES,
    i18n::Msg,
    ui::{LayerExtraRow, SettingsWindow},
};
use lhc_core::{
    config_document::{ConfigDocument, KeyAssignment},
    profile::auto_switch::AutoSwitchContext,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn refresh(ui: &SettingsWindow, document: &ConfigDocument, selected: i32) {
    let layout = document.layout();
    ui.set_layer_names(ModelRc::new(VecModel::from(
        layout
            .layers
            .iter()
            .map(|layer| layer.name.clone().into())
            .collect::<Vec<_>>(),
    )));
    let selected = if selected >= 0 && (selected as usize) < layout.layers.len() {
        selected
    } else if layout.layers.is_empty() {
        -1
    } else {
        0
    };
    ui.set_selected_layer(selected);
    let Some(layer) = layout.layers.get(selected as usize) else {
        ui.set_layer_description("".into());
        ui.set_layer_actions(ModelRc::new(VecModel::from(vec![
            slint::SharedString::new();
            KEY_CODES.len()
        ])));
        ui.set_layer_extras(ModelRc::new(VecModel::<LayerExtraRow>::default()));
        return;
    };
    ui.set_layer_description(layer.description.clone().unwrap_or_default().into());
    let keymap = layout.layer_keymaps.get(&layer.id);
    ui.set_layer_actions(ModelRc::new(VecModel::from(
        KEY_CODES
            .iter()
            .map(|key| match keymap.and_then(|map| map.keys.get(*key)) {
                None => "".into(),
                Some(None) => "∅".into(),
                Some(Some(value)) => value.clone().into(),
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_layer_extras(ModelRc::new(VecModel::from(
        keymap
            .map(|map| {
                map.extras
                    .iter()
                    .map(|extra| LayerExtraRow {
                        key: extra.key.clone().into(),
                        action: extra.action.clone().unwrap_or_else(|| "∅".into()).into(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )));
}
fn changed(ui: &SettingsWindow, config: &Rc<RefCell<ConfigDocument>>, selected: i32) {
    let document = config.borrow();
    refresh(ui, &document, selected);
    let status = match document.runtime_config(&AutoSwitchContext::current()) {
        Ok(runtime) => match lhc_core::mapper::runtime::update_config_if_running(&runtime.json) {
            Ok(()) => Msg::LayerSaved,
            Err(error) => Msg::SavedMapperNotUpdated(error),
        },
        Err(error) => Msg::SavedMapperNotUpdated(error.to_string()),
    };
    ui.set_layer_status(status.to_ui());
    ui.set_config_status(crate::i18n::Msg::ConfigSaved(document.layout().rules.len()).to_ui());
    drop(document);
    ui.invoke_refresh_rules();
}
fn failure(ui: &SettingsWindow, error: lhc_core::config_document::ConfigError) {
    ui.set_layer_status(Msg::from(&error).to_ui());
}

pub(super) fn bind(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    ui.set_layer_key_codes(ModelRc::new(VecModel::from(
        KEY_CODES
            .iter()
            .map(|key| (*key).into())
            .collect::<Vec<_>>(),
    )));
    if let Some(config) = &config {
        refresh(ui, &config.borrow(), 0);
    }
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_refresh_layers(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            refresh(&ui, &config.borrow(), ui.get_selected_layer());
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_choose_layer(move |index| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            refresh(&ui, &config.borrow(), index);
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_open_layer_dialog(move |kind, index| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let document = config.borrow();
            let layer = document
                .layout()
                .layers
                .get(ui.get_selected_layer() as usize);
            ui.set_layer_status(Msg::None.to_ui());
            ui.set_layer_dialog_kind(kind);
            ui.set_layer_dialog_index(index);
            ui.set_layer_dialog_name(
                match kind {
                    2 => layer.map(|l| l.name.clone()).unwrap_or_default(),
                    3 => layer
                        .map(|l| format!("{} copy", l.name))
                        .unwrap_or_default(),
                    _ => String::new(),
                }
                .into(),
            );
            ui.set_layer_dialog_description(
                layer
                    .and_then(|l| l.description.as_deref())
                    .unwrap_or("")
                    .into(),
            );
            ui.set_layer_dialog_key("".into());
            ui.set_layer_dialog_action("".into());
            ui.set_layer_assignment_kind(0);
            if kind == 5 {
                if let (Some(layer), Some(key)) = (layer, KEY_CODES.get(index as usize)) {
                    match document.layer_key(&layer.id, key) {
                        KeyAssignment::Transparent => {}
                        KeyAssignment::Swallow => ui.set_layer_assignment_kind(1),
                        KeyAssignment::Action(action) => {
                            ui.set_layer_assignment_kind(2);
                            ui.set_layer_dialog_action(action.into());
                        }
                    }
                }
            } else if kind == 6 && index >= 0 {
                if let Some(extra) = layer
                    .and_then(|l| document.layout().layer_keymaps.get(&l.id))
                    .and_then(|map| map.extras.get(index as usize))
                {
                    ui.set_layer_dialog_key(extra.key.clone().into());
                    match &extra.action {
                        None => ui.set_layer_assignment_kind(1),
                        Some(action) => {
                            ui.set_layer_assignment_kind(if action.is_empty() { 0 } else { 2 });
                            ui.set_layer_dialog_action(action.clone().into());
                        }
                    }
                }
            }
            ui.set_layer_dialog_open(true);
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_apply_layer_dialog(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let kind = ui.get_layer_dialog_kind();
            let selected = ui.get_selected_layer();
            let index = ui.get_layer_dialog_index();
            let name = ui.get_layer_dialog_name().trim().to_owned();
            let description = ui.get_layer_dialog_description().trim().to_owned();
            let key = ui.get_layer_dialog_key().trim().to_owned();
            let action = ui.get_layer_dialog_action().to_string();
            let assignment_kind = ui.get_layer_assignment_kind();
            let layer_id = config
                .borrow()
                .layout()
                .layers
                .get(selected as usize)
                .map(|layer| layer.id.clone());
            if matches!(kind, 1..=3) && name.is_empty() {
                ui.set_layer_status(Msg::LayerNameRequired.to_ui());
                return;
            }
            let result = match kind {
                1 => {
                    let result = config.borrow_mut().create_layer(&name, &description);
                    if result.is_ok() {
                        ui.set_selected_layer(config.borrow().layout().layers.len() as i32 - 1);
                    }
                    result.map(|_| ())
                }
                2 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    config.borrow_mut().rename_layer(&id, &name, &description)
                }
                3 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    let result = config.borrow_mut().clone_layer(&id, &name, &description);
                    if result.is_ok() {
                        ui.set_selected_layer(config.borrow().layout().layers.len() as i32 - 1);
                    }
                    result.map(|_| ())
                }
                4 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    config.borrow_mut().delete_layer(&id)
                }
                5 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    let Some(key) = KEY_CODES.get(index as usize) else {
                        return;
                    };
                    let value = match assignment_kind {
                        0 => KeyAssignment::Transparent,
                        1 => KeyAssignment::Swallow,
                        _ => KeyAssignment::Action(action),
                    };
                    config.borrow_mut().set_layer_key(&id, key, value)
                }
                6 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    if key.is_empty() {
                        ui.set_layer_status(Msg::KeyCodeRequired.to_ui());
                        return;
                    }
                    let value = match assignment_kind {
                        0 => Some(String::new()),
                        1 => None,
                        _ => Some(action),
                    };
                    config.borrow_mut().set_layer_extra(
                        &id,
                        (index >= 0).then_some(index as usize),
                        &key,
                        value,
                    )
                }
                7 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    config.borrow_mut().clear_layer_keys(&id)
                }
                8 => {
                    let Some(id) = layer_id else {
                        return;
                    };
                    config.borrow_mut().clear_layer_extras(&id)
                }
                _ => return,
            };
            match result {
                Ok(()) => {
                    ui.set_layer_dialog_open(false);
                    changed(&ui, config, ui.get_selected_layer());
                }
                Err(error) => failure(&ui, error),
            }
        }
    });
    let weak = ui.as_weak();
    let config_copy = config.clone();
    ui.on_move_layer_extra(move |index, direction| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let selected = ui.get_selected_layer();
            let Some(id) = config
                .borrow()
                .layout()
                .layers
                .get(selected as usize)
                .map(|layer| layer.id.clone())
            else {
                return;
            };
            let next = index + direction;
            let len = config
                .borrow()
                .layout()
                .layer_keymaps
                .get(&id)
                .map_or(0, |map| map.extras.len()) as i32;
            if index < 0 || next < 0 || next >= len {
                return;
            }
            let result = config
                .borrow_mut()
                .move_layer_extra(&id, index as usize, next as usize);
            match result {
                Ok(()) => changed(&ui, config, selected),
                Err(error) => failure(&ui, error),
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_remove_layer_extra(move |index| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config) {
            let selected = ui.get_selected_layer();
            let Some(id) = config
                .borrow()
                .layout()
                .layers
                .get(selected as usize)
                .map(|layer| layer.id.clone())
            else {
                return;
            };
            if index < 0
                || index as usize
                    >= config
                        .borrow()
                        .layout()
                        .layer_keymaps
                        .get(&id)
                        .map_or(0, |map| map.extras.len())
            {
                return;
            }
            let result = config.borrow_mut().remove_layer_extra(&id, index as usize);
            match result {
                Ok(()) => changed(&ui, config, selected),
                Err(error) => failure(&ui, error),
            }
        }
    });
}
