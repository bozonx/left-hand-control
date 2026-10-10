//! Layers page: layer list, the keymap of the selected layer and its
//! extra keys.

use super::{action_kind, picker::action_label};
use crate::{
    document::{Document, View},
    i18n::Msg,
    keyboard,
    ui::{
        ActionKind, ActionPicker, Assignment, LayerDialog, LayerExtraRow, LayerKeyCell,
        LayersEditor, Locale, PickerTarget, SettingsWindow,
    },
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError, KeyAssignment},
    profile::actions,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{collections::HashMap, rc::Rc};

/// Display names of catalog actions (macros, commands, built-ins).
fn action_names(ui: &SettingsWindow, config: &ConfigDocument) -> HashMap<String, String> {
    actions::catalog(&config.config())
        .into_iter()
        .filter_map(|entry| Some((entry.action.format()?, action_label(ui, entry.name)?)))
        .collect()
}

/// How a keymap value is shown: `None` no entry, `Some(None)` swallow.
fn assignment(
    value: Option<&Option<String>>,
    names: &HashMap<String, String>,
) -> (Assignment, String, ActionKind) {
    match value {
        None => (Assignment::Transparent, String::new(), ActionKind::None),
        Some(None) => (Assignment::Swallow, String::new(), ActionKind::None),
        Some(Some(action)) if action.is_empty() => {
            (Assignment::Transparent, String::new(), ActionKind::None)
        }
        Some(Some(action)) => {
            let kind = action_kind(action);
            let label = match actions::Action::parse(Some(action)) {
                actions::Action::Text(text) => text,
                _ => names.get(action).cloned().unwrap_or_else(|| action.clone()),
            };
            (Assignment::Action, label, kind)
        }
    }
}

/// Grid slots of one hand; `first` is the index of its first key and
/// `offset` shifts the last (thumb) row right.
fn hand_cells(
    rows: &[&[&str]],
    columns: usize,
    first: usize,
    offset: usize,
    lookup: &dyn Fn(&str) -> (Assignment, String, ActionKind),
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
        for code in row.iter() {
            let (kind, action, icon) = lookup(code);
            cells.push(LayerKeyCell {
                index: index as i32,
                label: keyboard::label(code).into(),
                code: (*code).into(),
                kind,
                category: super::picker::category_for_value(&action),
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

/// Index of the selected layer, if it exists.
fn selected(ui: &SettingsWindow, config: &ConfigDocument) -> Option<usize> {
    usize::try_from(ui.global::<LayersEditor>().get_selected())
        .ok()
        .filter(|index| *index < config.layout().layers.len())
}

fn selected_id(ui: &SettingsWindow, document: &Document) -> Option<String> {
    let config = document.read();
    selected(ui, &config).map(|index| config.layout().layers[index].id.clone())
}

fn select_layer(ui: &SettingsWindow, document: &Document, index: i32) {
    let id = document
        .read()
        .layout()
        .layers
        .get(index as usize)
        .map(|layer| layer.id.clone());
    let editor = ui.global::<LayersEditor>();
    editor.set_selected(index);
    editor.set_selected_id(id.clone().unwrap_or_default().into());
    if let Some(id) = id {
        document.update_ui_state(Some(&id), None);
    }
    refresh(ui, document);
}

fn refresh(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let layout = config.layout();
    let editor = ui.global::<LayersEditor>();
    editor.set_names(super::strings(
        layout.layers.iter().map(|layer| layer.name.clone()),
    ));
    let saved = editor.get_selected_id();
    let index = layout
        .layers
        .iter()
        .position(|layer| layer.id == saved.as_str())
        .or_else(|| selected(ui, &config))
        .or((!layout.layers.is_empty()).then_some(0));
    editor.set_selected(index.map_or(-1, |index| index as i32));
    let layer = index.map(|index| &layout.layers[index]);
    editor.set_selected_id(
        layer
            .map_or_else(String::new, |layer| layer.id.clone())
            .into(),
    );
    editor.set_description(
        layer
            .and_then(|layer| layer.description.clone())
            .unwrap_or_default()
            .into(),
    );
    let keymap = layer.and_then(|layer| layout.layer_keymaps.get(&layer.id));
    let names = action_names(ui, &config);
    let lookup = |code: &str| assignment(keymap.and_then(|map| map.keys.get(code)), &names);
    let left_len: usize = keyboard::LEFT_HAND.iter().map(|row| row.len()).sum();
    editor.set_left_cells(ModelRc::new(VecModel::from(hand_cells(
        &keyboard::LEFT_HAND,
        keyboard::LEFT_COLUMNS,
        0,
        2,
        &lookup,
    ))));
    editor.set_right_cells(ModelRc::new(VecModel::from(hand_cells(
        &keyboard::RIGHT_HAND,
        keyboard::RIGHT_COLUMNS,
        left_len,
        0,
        &lookup,
    ))));
    editor.set_extras(ModelRc::new(VecModel::from(
        layer
            .and_then(|layer| layout.layer_keymaps.get(&layer.id))
            .map(|map| {
                ordered_extra_entries(map)
                    .into_iter()
                    .map(|(key, value)| {
                        let (kind, action, icon) = assignment(Some(&value), &names);
                        LayerExtraRow {
                            key: key.into(),
                            kind,
                            category: super::picker::category_for_value(&action),
                            action: action.into(),
                            icon,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )));
}

/// Save `edit`, then refresh and report.
fn change<T>(
    ui: &SettingsWindow,
    document: &Document,
    edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
) -> Result<T, Msg> {
    let editor = ui.global::<LayersEditor>();
    match document.edit(View::Layers, edit) {
        Ok(saved) => {
            refresh(ui, document);
            editor.set_status(crate::notifications::report(ui, &saved.message(Msg::None)));
            Ok(saved.value)
        }
        Err(error) => {
            refresh(ui, document);
            let message = Msg::from(&error);
            editor.set_status(crate::notifications::report(ui, &message));
            Err(message)
        }
    }
}

/// Fill the dialog for `dialog` on key or extra row `index`.
fn open_dialog(ui: &SettingsWindow, document: &Document, dialog: LayerDialog, index: i32) {
    let editor = ui.global::<LayersEditor>();
    let config = document.read();
    let layer = selected(ui, &config).map(|i| &config.layout().layers[i]);
    if layer.is_none() && dialog != LayerDialog::Create {
        return;
    }
    editor.set_status(Msg::None.to_ui());
    editor.set_dialog_index(index);
    let name = match (dialog, layer) {
        (LayerDialog::Rename, Some(layer)) => layer.name.clone(),
        (LayerDialog::Duplicate, Some(layer)) => ui
            .global::<Locale>()
            .invoke_text(Msg::CopyName(layer.name.clone()).to_ui())
            .to_string(),
        _ => String::new(),
    };
    editor.set_dialog_name(name.into());
    editor.set_dialog_description(
        match dialog {
            LayerDialog::Rename | LayerDialog::Duplicate => {
                layer.and_then(|layer| layer.description.clone())
            }
            _ => None,
        }
        .unwrap_or_default()
        .into(),
    );
    editor.set_dialog_key("".into());
    editor.set_dialog_action("".into());
    editor.set_assignment(Assignment::Transparent);
    match (dialog, layer) {
        (LayerDialog::EditKey, Some(layer)) => {
            let Some(key) = keyboard::layer_key(index) else {
                return;
            };
            match config.layer_key(&layer.id, key) {
                KeyAssignment::Transparent => {}
                KeyAssignment::Swallow => editor.set_assignment(Assignment::Swallow),
                KeyAssignment::Action(action) => {
                    editor.set_assignment(Assignment::Action);
                    editor.set_dialog_action(action.into());
                }
            }
        }
        (LayerDialog::ExtraKey, Some(layer)) => {
            let extra = usize::try_from(index).ok().and_then(|index| {
                config
                    .layout()
                    .layer_keymaps
                    .get(&layer.id)
                    .and_then(|map| ordered_extra_entries(map).into_iter().nth(index))
            });
            if let Some(extra) = extra {
                editor.set_dialog_key(extra.0.into());
                match &extra.1 {
                    None => editor.set_assignment(Assignment::Swallow),
                    Some(action) if action.is_empty() => {}
                    Some(action) => {
                        editor.set_assignment(Assignment::Action);
                        editor.set_dialog_action(action.clone().into());
                    }
                }
            }
        }
        _ => {}
    }
    if dialog == LayerDialog::EditKey {
        editor.set_dialog(LayerDialog::None);
        ui.global::<ActionPicker>().invoke_open(
            PickerTarget::LayerAction,
            index,
            editor.get_dialog_action(),
            false,
        );
    } else {
        editor.set_dialog(dialog);
    }
}

/// Apply the open dialog.
fn apply_dialog(ui: &SettingsWindow, document: &Document) -> Result<(), Msg> {
    let editor = ui.global::<LayersEditor>();
    let dialog = editor.get_dialog();
    let index = editor.get_dialog_index();
    let name = editor.get_dialog_name().trim().to_owned();
    let description = editor.get_dialog_description().trim().to_owned();
    let key = editor.get_dialog_key().trim().to_owned();
    let action = editor.get_dialog_action().to_string();
    let assignment = editor.get_assignment();
    if matches!(dialog, LayerDialog::EditKey | LayerDialog::ExtraKey)
        && assignment == Assignment::Action
    {
        if action.trim().is_empty() {
            return Err(Msg::ActionRequired);
        }
        let parsed = actions::Action::parse(Some(&action));
        if matches!(&parsed, actions::Action::Text(text) if text.is_empty()) {
            return Err(Msg::TextEmpty);
        }
        if let Some(issue) = actions::validate(&parsed, &document.read().config()) {
            return Err(Msg::InvalidAction(issue));
        }
    }
    if matches!(
        dialog,
        LayerDialog::Create | LayerDialog::Rename | LayerDialog::Duplicate
    ) && name.is_empty()
    {
        return Err(Msg::LayerNameRequired);
    }
    if dialog == LayerDialog::Create {
        change(ui, document, |config| {
            config.create_layer(&name, &description)
        })?;
        let last = document.read().layout().layers.len() as i32 - 1;
        select_layer(ui, document, last);
        return Ok(());
    }
    let id = selected_id(ui, document).ok_or(Msg::None)?;
    match dialog {
        LayerDialog::Rename => change(ui, document, |config| {
            config.rename_layer(&id, &name, &description)
        })?,
        LayerDialog::Duplicate => {
            change(ui, document, |config| {
                config.clone_layer(&id, &name, &description)
            })?;
            let last = document.read().layout().layers.len() as i32 - 1;
            select_layer(ui, document, last);
        }
        LayerDialog::Delete => change(ui, document, |config| config.delete_layer(&id))?,
        LayerDialog::EditKey => {
            let key = keyboard::layer_key(index).ok_or(Msg::None)?;
            let value = match assignment {
                Assignment::Transparent => KeyAssignment::Transparent,
                Assignment::Swallow => KeyAssignment::Swallow,
                Assignment::Action => KeyAssignment::Action(action),
            };
            change(ui, document, |config| config.set_layer_key(&id, key, value))?
        }
        LayerDialog::ExtraKey => {
            if !lhc_core::profile::key_catalog::valid_key(&key) {
                return Err(Msg::KeyCodeRequired);
            }
            let value = match assignment {
                Assignment::Transparent => KeyAssignment::Transparent,
                Assignment::Swallow => KeyAssignment::Swallow,
                Assignment::Action => KeyAssignment::Action(action),
            };
            let previous = extra_at(document, &id, index).map(|(key, _)| key);
            change(ui, document, |config| {
                set_extra(config, &id, previous.as_deref(), &key, value)
            })?
        }
        LayerDialog::Create | LayerDialog::None => {}
    }
    Ok(())
}

fn extra_entries(map: &lhc_core::profile::model::LayerKeymap) -> Vec<(String, Option<String>)> {
    let mut entries: Vec<_> = map
        .keys
        .iter()
        .filter(|(key, _)| !keyboard::layer_keys().any(|code| code == key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let catalog: Vec<_> = lhc_core::profile::key_catalog::CATEGORIES
        .iter()
        .flat_map(|keys| keys.iter().copied())
        .collect();
    entries.sort_by_key(|(key, _)| {
        (
            catalog
                .iter()
                .position(|code| *code == key)
                .unwrap_or(usize::MAX),
            key.clone(),
        )
    });
    entries
}

fn ordered_extra_entries(
    map: &lhc_core::profile::model::LayerKeymap,
) -> Vec<(String, Option<String>)> {
    let mut entries = extra_entries(map);
    let order = &map.extra_key_order;
    entries.sort_by_key(|(key, _)| {
        order
            .iter()
            .position(|code| code == key)
            .unwrap_or(usize::MAX)
    });
    entries
}

fn extra_at(document: &Document, id: &str, index: i32) -> Option<(String, Option<String>)> {
    let index = usize::try_from(index).ok()?;
    document
        .read()
        .layout()
        .layer_keymaps
        .get(id)
        .and_then(|map| ordered_extra_entries(map).into_iter().nth(index))
}

fn set_extra(
    config: &mut ConfigDocument,
    id: &str,
    previous: Option<&str>,
    key: &str,
    value: KeyAssignment,
) -> Result<(), ConfigError> {
    if !lhc_core::profile::key_catalog::valid_key(key)
        || keyboard::layer_keys().any(|code| code == key)
    {
        return Err(ConfigError::Invalid("invalid additional key".into()));
    }
    if previous != Some(key)
        && config
            .layout()
            .layer_keymaps
            .get(id)
            .is_some_and(|map| map.keys.contains_key(key))
    {
        return Err(ConfigError::Invalid("key already assigned".into()));
    }
    config.update_layout(|layout| {
        let map = layout.layer_keymap_mut(id);
        if let Some(previous) = previous {
            for code in &mut map.extra_key_order {
                if code == previous {
                    *code = key.to_owned();
                }
            }
        }
        if value == KeyAssignment::Transparent {
            map.extra_key_order.retain(|code| code != key);
        }
        let keys = &mut map.keys;
        if let Some(previous) = previous {
            keys.remove(previous);
        }
        match value {
            KeyAssignment::Transparent => {
                keys.remove(key);
            }
            KeyAssignment::Swallow => {
                keys.insert(key.into(), None);
            }
            KeyAssignment::Action(action) => {
                keys.insert(key.into(), Some(action));
            }
        }
    })
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let weak = ui.as_weak();
    let doc = document.clone();
    ui.global::<LayersEditor>().on_reorder(move |from, to| {
        let Some(ui) = weak.upgrade() else { return };
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        let len = doc.read().layout().layers.len();
        if from >= len || to >= len {
            return;
        }
        let _ = change(&ui, &doc, |config| {
            config.update_layout(|layout| {
                let item = layout.layers.remove(from);
                layout.layers.insert(to, item);
            })
        });
    });
    let weak = ui.as_weak();
    let doc = document.clone();
    ui.global::<LayersEditor>()
        .on_reorder_extra(move |from, to| {
            let Some(ui) = weak.upgrade() else { return };
            let Some(id) = selected_id(&ui, &doc) else {
                return;
            };
            let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
                return;
            };
            let keys = ui
                .global::<LayersEditor>()
                .get_extras()
                .iter()
                .map(|row| row.key.to_string())
                .collect();
            let _ = change(&ui, &doc, |config| {
                config.reorder_extra_keys(&id, keys, from, to)
            });
        });
    ui.global::<LayersEditor>()
        .set_selected_id(document.selected_layer_id().into());
    refresh(ui, document);
    let weak = ui.as_weak();
    document.subscribe(View::Layers, move |document| {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, document);
        }
    });
    let editor = ui.global::<LayersEditor>();
    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_update_name(move |name| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        let description = ui.global::<LayersEditor>().get_description();
        let _ = change(&ui, &doc, |config| {
            config.rename_layer(&id, name.trim(), &description)
        });
    });
    let weak = ui.as_weak();
    editor.on_add_extra(move || {
        if let Some(ui) = weak.upgrade() {
            ui.global::<LayersEditor>().set_dialog_key("".into());
            ui.global::<ActionPicker>()
                .invoke_open(PickerTarget::ExtraKey, -1, "".into(), true);
        }
    });
    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_pick_extra(move |index, key_only| {
        let Some(ui) = weak.upgrade() else { return };
        open_dialog(&ui, &doc, LayerDialog::ExtraKey, index);
        let editor = ui.global::<LayersEditor>();
        editor.set_dialog(LayerDialog::None);
        ui.global::<ActionPicker>().invoke_open(
            if key_only {
                PickerTarget::ExtraKey
            } else {
                PickerTarget::ExtraAction
            },
            index,
            if key_only {
                editor.get_dialog_key()
            } else {
                editor.get_dialog_action()
            },
            key_only,
        );
    });
    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_update_extra_key(move |index, key| {
        let Some(ui) = weak.upgrade() else { return };
        if !lhc_core::profile::key_catalog::valid_key(key.trim()) {
            ui.global::<LayersEditor>()
                .set_status(crate::notifications::report(&ui, &Msg::KeyCodeRequired));
            return;
        }
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        if let Some((previous, action)) = extra_at(&doc, &id, index) {
            let value = action.map_or(KeyAssignment::Swallow, KeyAssignment::Action);
            let _ = change(&ui, &doc, |config| {
                set_extra(config, &id, Some(&previous), key.trim(), value)
            });
        }
    });
    editor.set_label_mode(document.label_mode());
    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_set_label_mode(move |mode| {
        if let Some(ui) = weak.upgrade() {
            let editor = ui.global::<LayersEditor>();
            doc.update_ui_state(None, Some(mode));
            editor.set_label_mode(mode);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &doc);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_choose(move |index| {
        if let Some(ui) = weak.upgrade() {
            select_layer(&ui, &doc, index);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_open_dialog(move |dialog, index| {
        if let Some(ui) = weak.upgrade() {
            open_dialog(&ui, &doc, dialog, index);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_apply_dialog(move || {
        let Some(ui) = weak.upgrade() else { return };
        let editor = ui.global::<LayersEditor>();
        match apply_dialog(&ui, &doc) {
            Ok(()) => editor.set_dialog(LayerDialog::None),
            Err(error) => editor.set_status(crate::notifications::report(&ui, &error)),
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_update_description(move |description| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        let name = {
            let config = doc.read();
            config
                .layout()
                .layers
                .iter()
                .find(|layer| layer.id == id)
                .map(|layer| layer.name.clone())
                .unwrap_or_default()
        };
        let _ = change(&ui, &doc, |config| {
            config.rename_layer(&id, &name, description.trim())
        });
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_remove_extra(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        if let Some((key, _)) = extra_at(&doc, &id, index) {
            if change(&ui, &doc, |config| {
                config.set_layer_key(&id, &key, KeyAssignment::Transparent)
            })
            .is_ok()
            {
                refresh(&ui, &doc);
            }
        }
    });
}

pub(super) fn assign(
    ui: &SettingsWindow,
    document: &Document,
    index: i32,
    value: &str,
    ignore: bool,
) -> Result<(), Msg> {
    let id = selected_id(ui, document).ok_or(Msg::None)?;
    let key = keyboard::layer_key(index).ok_or(Msg::None)?;
    let assignment = if ignore {
        KeyAssignment::Swallow
    } else if value.is_empty() {
        KeyAssignment::Transparent
    } else {
        KeyAssignment::Action(value.to_owned())
    };
    change(ui, document, |config| {
        config.set_layer_key(&id, key, assignment)
    })
}

pub(super) fn assign_extra(
    ui: &SettingsWindow,
    document: &Document,
    index: i32,
    value: &str,
    key_only: bool,
    ignore: bool,
) -> Result<(), Msg> {
    let id = selected_id(ui, document).ok_or(Msg::None)?;
    let existing = extra_at(document, &id, index);
    let (key, action) = if key_only {
        if !lhc_core::profile::key_catalog::valid_key(value) {
            return Err(Msg::KeyCodeRequired);
        }
        (
            value.to_owned(),
            existing.as_ref().map_or_else(
                || KeyAssignment::Action(value.to_owned()),
                |(_, action)| {
                    action
                        .clone()
                        .map_or(KeyAssignment::Swallow, KeyAssignment::Action)
                },
            ),
        )
    } else {
        let (key, _) = existing.clone().ok_or(Msg::KeyCodeRequired)?;
        (
            key,
            if ignore {
                KeyAssignment::Swallow
            } else if value.is_empty() {
                KeyAssignment::Transparent
            } else {
                KeyAssignment::Action(value.to_owned())
            },
        )
    };
    change(ui, document, |config| {
        set_extra(
            config,
            &id,
            existing.as_ref().map(|(key, _)| key.as_str()),
            &key,
            action,
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_additional_keys_preserve_actions_and_suppression() {
        let layout = lhc_core::profile::layout_file::parse(include_str!(
            "../../../../public/ivank-layout.yaml"
        ))
        .unwrap()
        .unwrap();
        for id in ["nav", "sel", "sym", "fkeys"] {
            assert_eq!(
                extra_entries(&layout.layer_keymaps[id]),
                vec![("BracketRight".into(), None), ("Backslash".into(), None),]
            );
        }
        let win = extra_entries(&layout.layer_keymaps["win"]);
        assert_eq!(win.len(), 3);
        assert!(win.contains(&("Backspace".into(), Some("sys:screenOff".into()))));
        assert!(win.contains(&("F4".into(), Some("F4".into()))));
        assert!(win.contains(&("BracketRight".into(), Some("sys:switchLayout7".into()))));
    }

    #[test]
    fn assignments_describe_keymap_values() {
        let names = HashMap::from([("macro:copyLine".to_owned(), "Copy line".to_owned())]);
        assert_eq!(assignment(None, &names).0, Assignment::Transparent);
        assert_eq!(assignment(Some(&None), &names).0, Assignment::Swallow);
        assert_eq!(
            assignment(Some(&Some("macro:copyLine".into())), &names),
            (Assignment::Action, "Copy line".into(), ActionKind::Macro)
        );
        assert_eq!(
            assignment(Some(&Some("text:hi".into())), &names),
            (Assignment::Action, "hi".into(), ActionKind::Text)
        );
    }

    #[test]
    fn hand_grids_index_every_key() {
        let lookup = |_: &str| (Assignment::Transparent, String::new(), ActionKind::None);
        let left = hand_cells(&keyboard::LEFT_HAND, keyboard::LEFT_COLUMNS, 0, 2, &lookup);
        assert_eq!(left.len(), 30);
        let thumb = &left[24..];
        assert_eq!(thumb[0].index, -1);
        assert_eq!(thumb[2].code, "ControlLeft");
        let indexed: Vec<_> = left.iter().filter(|cell| cell.index >= 0).collect();
        assert_eq!(
            indexed.len(),
            keyboard::LEFT_HAND.iter().map(|r| r.len()).sum::<usize>()
        );
        assert_eq!(keyboard::layer_key(indexed[1].index), Some("Digit1"));
    }
}
