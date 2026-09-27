use crate::{
    i18n::Msg,
    ui::{LayerExtraRow, LayerKeyCell, Locale, SettingsWindow},
};
use lhc_core::{
    config_document::{ConfigDocument, KeyAssignment},
    profile::{
        actions::{self, Action, ActionName},
        auto_switch::AutoSwitchContext,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Layer key: code, US label, Linux evdev code (mirrors `utils/keys.ts`).
type KeyDef = (&'static str, &'static str, u16);

const LEFT_COLUMNS: usize = 6;
const RIGHT_COLUMNS: usize = 8;

const LEFT_HAND: [&[KeyDef]; 6] = [
    &[
        ("Escape", "Esc", 1),
        ("F1", "F1", 59),
        ("F2", "F2", 60),
        ("F3", "F3", 61),
        ("F4", "F4", 62),
        ("F5", "F5", 63),
    ],
    &[
        ("Backquote", "`", 41),
        ("Digit1", "1", 2),
        ("Digit2", "2", 3),
        ("Digit3", "3", 4),
        ("Digit4", "4", 5),
        ("Digit5", "5", 6),
    ],
    &[
        ("Tab", "Tab", 15),
        ("KeyQ", "Q", 16),
        ("KeyW", "W", 17),
        ("KeyE", "E", 18),
        ("KeyR", "R", 19),
        ("KeyT", "T", 20),
    ],
    &[
        ("CapsLock", "Caps", 58),
        ("KeyA", "A", 30),
        ("KeyS", "S", 31),
        ("KeyD", "D", 32),
        ("KeyF", "F", 33),
        ("KeyG", "G", 34),
    ],
    &[
        ("ShiftLeft", "Shift", 42),
        ("KeyZ", "Z", 44),
        ("KeyX", "X", 45),
        ("KeyC", "C", 46),
        ("KeyV", "V", 47),
        ("KeyB", "B", 48),
    ],
    &[
        ("ControlLeft", "Ctrl", 29),
        ("MetaLeft", "Meta", 125),
        ("AltLeft", "Alt", 56),
        ("Space", "Space", 57),
    ],
];

const RIGHT_HAND: [&[KeyDef]; 6] = [
    &[
        ("F6", "F6", 64),
        ("F7", "F7", 65),
        ("F8", "F8", 66),
        ("F9", "F9", 67),
        ("F10", "F10", 68),
        ("F11", "F11", 87),
        ("F12", "F12", 88),
        ("PrintScreen", "PrtSc", 210),
    ],
    &[
        ("Digit6", "6", 7),
        ("Digit7", "7", 8),
        ("Digit8", "8", 9),
        ("Digit9", "9", 10),
        ("Digit0", "0", 11),
        ("Minus", "-", 12),
        ("Equal", "=", 13),
        ("Backspace", "Bksp", 14),
    ],
    &[
        ("KeyY", "Y", 21),
        ("KeyU", "U", 22),
        ("KeyI", "I", 23),
        ("KeyO", "O", 24),
        ("KeyP", "P", 25),
        ("BracketLeft", "[", 26),
        ("BracketRight", "]", 27),
        ("Backslash", "\\", 43),
    ],
    &[
        ("KeyH", "H", 35),
        ("KeyJ", "J", 36),
        ("KeyK", "K", 37),
        ("KeyL", "L", 38),
        ("Semicolon", ";", 39),
        ("Quote", "'", 40),
        ("Enter", "Enter", 28),
    ],
    &[
        ("KeyN", "N", 49),
        ("KeyM", "M", 50),
        ("Comma", ",", 51),
        ("Period", ".", 52),
        ("Slash", "/", 53),
        ("ShiftRight", "Shift", 54),
    ],
    &[
        ("AltRight", "Alt", 100),
        ("MetaRight", "Meta", 126),
        ("ContextMenu", "Menu", 127),
        ("ControlRight", "Ctrl", 97),
    ],
];

/// Every layer key in index order: left hand rows, then right hand rows.
fn layer_keys() -> impl Iterator<Item = &'static KeyDef> {
    LEFT_HAND
        .iter()
        .chain(RIGHT_HAND.iter())
        .flat_map(|row| row.iter())
}

fn layer_key(index: i32) -> Option<&'static str> {
    usize::try_from(index)
        .ok()
        .and_then(|index| layer_keys().nth(index))
        .map(|key| key.0)
}

/// Display text and icon for an assigned action (see `ActionIcon` in `layers.slint`).
fn describe(action: &str, names: &HashMap<String, String>) -> (String, i32) {
    let icon = match Action::parse(Some(action)) {
        Action::Keys(_) => 1,
        Action::Macro(_) => 2,
        Action::Command(_) => 3,
        Action::System(_) => 4,
        Action::App(_) => 5,
        Action::Text(text) => return (text, 6),
        Action::Native | Action::Swallow | Action::Pause(_) => 0,
    };
    let label = names
        .get(action)
        .cloned()
        .unwrap_or_else(|| action.to_owned());
    (label, icon)
}

/// Display names of the catalog actions (macros, commands, built-ins).
fn action_names(ui: &SettingsWindow, document: &ConfigDocument) -> HashMap<String, String> {
    actions::catalog(&document.config())
        .into_iter()
        .filter_map(|entry| {
            let value = entry.action.format()?;
            let label = match entry.name {
                ActionName::Verbatim(name) if !name.is_empty() => name,
                ActionName::System { id, n } | ActionName::App { id, n } => ui
                    .global::<Locale>()
                    .invoke_text(Msg::PickerAction(id.into(), n).to_ui())
                    .to_string(),
                _ => return None,
            };
            Some((value, label))
        })
        .collect()
}

/// `(kind, label, icon)` of a layer assignment: 0 transparent, 1 swallow, 2 action.
fn assignment(
    value: Option<&Option<String>>,
    names: &HashMap<String, String>,
) -> (i32, String, i32) {
    match value {
        None => (0, String::new(), 0),
        Some(None) => (1, String::new(), 0),
        Some(Some(action)) if action.is_empty() => (0, String::new(), 0),
        Some(Some(action)) => {
            let (label, icon) = describe(action, names);
            (2, label, icon)
        }
    }
}

/// Grid slots of one hand; `first` is the index of its first key and
/// `offset` shifts the last row right (the left thumb row).
fn hand_cells(
    rows: &[&[KeyDef]],
    columns: usize,
    first: usize,
    offset: usize,
    lookup: &dyn Fn(&str) -> (i32, String, i32),
) -> Vec<LayerKeyCell> {
    let empty = LayerKeyCell {
        index: -1,
        ..Default::default()
    };
    let mut cells = Vec::with_capacity(rows.len() * columns);
    let mut index = first;
    for (row_index, row) in rows.iter().enumerate() {
        let skip = if row_index + 1 == rows.len() {
            offset
        } else {
            0
        };
        cells.extend(std::iter::repeat_n(empty.clone(), skip));
        for (code, label, numeric) in row.iter() {
            let (kind, action, icon) = lookup(code);
            cells.push(LayerKeyCell {
                index: index as i32,
                label: (*label).into(),
                code: (*code).into(),
                numeric: numeric.to_string().into(),
                kind,
                action: action.into(),
                icon,
            });
            index += 1;
        }
        cells.extend(std::iter::repeat_n(
            empty.clone(),
            columns - skip - row.len(),
        ));
    }
    cells
}

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
    let layer = layout.layers.get(selected as usize);
    ui.set_layer_description(
        layer
            .and_then(|layer| layer.description.clone())
            .unwrap_or_default()
            .into(),
    );
    let keymap = layer.and_then(|layer| layout.layer_keymaps.get(&layer.id));
    let names = action_names(ui, document);
    let lookup = |code: &str| assignment(keymap.and_then(|map| map.keys.get(code)), &names);
    let left_len: usize = LEFT_HAND.iter().map(|row| row.len()).sum();
    ui.set_layer_left_cells(ModelRc::new(VecModel::from(hand_cells(
        &LEFT_HAND,
        LEFT_COLUMNS,
        0,
        2,
        &lookup,
    ))));
    ui.set_layer_right_cells(ModelRc::new(VecModel::from(hand_cells(
        &RIGHT_HAND,
        RIGHT_COLUMNS,
        left_len,
        0,
        &lookup,
    ))));
    ui.set_layer_extras(ModelRc::new(VecModel::from(
        keymap
            .map(|map| {
                map.extras
                    .iter()
                    .map(|extra| {
                        let (kind, action, icon) = assignment(Some(&extra.action), &names);
                        LayerExtraRow {
                            key: extra.key.clone().into(),
                            kind,
                            action: action.into(),
                            icon,
                        }
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
                if let (Some(layer), Some(key)) = (layer, layer_key(index)) {
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
                    let Some(key) = layer_key(index) else {
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
    let config_copy = config.clone();
    ui.on_update_layer_description(move |description| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            let selected = ui.get_selected_layer();
            let Some((id, name)) = config
                .borrow()
                .layout()
                .layers
                .get(selected as usize)
                .map(|layer| (layer.id.clone(), layer.name.clone()))
            else {
                return;
            };
            let result = config
                .borrow_mut()
                .rename_layer(&id, &name, description.trim());
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
