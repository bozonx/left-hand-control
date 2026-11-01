//! Layers page: layer list, the keymap of the selected layer and its
//! extra keys.

use crate::layer_key_model::{action_names, assignment, hands};
use crate::{
    document::{Document, View},
    i18n::Msg,
    keyboard,
    ui::{
        ActionPicker, Assignment, LayerDialog, LayerExtraRow, LayersEditor, Locale, PickerTarget,
        SettingsWindow,
    },
};
use lhc_core::config_document::{ConfigDocument, ConfigError, KeyAssignment};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

/// Identity of a modal edit. A document change invalidates the draft rather
/// than letting its row index address a different layer or key.
#[derive(Clone)]
pub(super) struct EditTarget {
    revision: u64,
    layer_id: String,
}

impl EditTarget {
    fn new(document: &Document, layer_id: String) -> Self {
        Self {
            revision: document.revision(),
            layer_id,
        }
    }

    pub(super) fn check(&self, document: &Document, selected_id: &str) -> Result<(), Msg> {
        if self.revision != document.revision() || self.layer_id != selected_id {
            return Err(Msg::from(&ConfigError::EditConflict));
        }
        Ok(())
    }
}

pub(super) fn picker_target(ui: &SettingsWindow, document: &Document) -> EditTarget {
    EditTarget::new(
        document,
        ui.global::<LayersEditor>().get_selected_id().to_string(),
    )
}

/// Preserve repeater instances and their focus when the row count is unchanged.
fn update_model<T: Clone + PartialEq + 'static>(model: ModelRc<T>, rows: Vec<T>) -> ModelRc<T> {
    if let Some(existing) = model.as_any().downcast_ref::<VecModel<T>>()
        && existing.row_count() == rows.len()
    {
        for (index, row) in rows.into_iter().enumerate() {
            if existing.row_data(index).as_ref() != Some(&row) {
                existing.set_row_data(index, row);
            }
        }
        model
    } else {
        ModelRc::new(VecModel::from(rows))
    }
}

/// Index of the selected layer, if it exists.
fn selected(ui: &SettingsWindow, config: &ConfigDocument) -> Option<usize> {
    super::index(
        ui.global::<LayersEditor>().get_selected(),
        config.layout().layers.len(),
    )
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
    editor.set_selected_id(id.unwrap_or_default().into());
    refresh(ui, document);
}

fn refresh(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let layout = config.layout();
    let editor = ui.global::<LayersEditor>();
    editor.set_names(update_model(
        editor.get_names(),
        layout
            .layers
            .iter()
            .map(|layer| layer.name.clone().into())
            .collect(),
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
    let names = action_names(ui, &config.config());
    let (left, right) = hands(keymap, &names);
    editor.set_left_cells(update_model(editor.get_left_cells(), left));
    editor.set_right_cells(update_model(editor.get_right_cells(), right));
    editor.set_extras(update_model(
        editor.get_extras(),
        layer
            .and_then(|layer| layout.layer_keymaps.get(&layer.id))
            .map(|map| {
                ordered_extra_entries(map)
                    .into_iter()
                    .map(|(key, value)| {
                        let (kind, action, icon, category) = assignment(Some(&value), &names);
                        LayerExtraRow {
                            holds: map.hold_modifiers.contains(&key),
                            key: key.into(),
                            kind,
                            category,
                            action: action.into(),
                            icon,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    ));
    let triggers = layer
        .map(|layer| layout.layer_triggers(&layer.id))
        .unwrap_or_default();
    editor.set_activation(
        if triggers.is_empty() {
            Msg::LayerUnused
        } else {
            Msg::LayerTriggers(triggers.join(", "))
        }
        .to_ui(),
    );
}

/// Save `edit`, then refresh and report.
fn change<T>(
    ui: &SettingsWindow,
    document: &Document,
    edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
) -> Result<T, Msg> {
    super::change(
        ui,
        document,
        View::Layers,
        edit,
        || refresh(ui, document),
        |status| ui.global::<LayersEditor>().set_status(status),
    )
}

/// Fill the dialog for `dialog` on key `index`.
fn open_dialog(ui: &SettingsWindow, document: &Document, dialog: LayerDialog, index: i32) {
    let editor = ui.global::<LayersEditor>();
    let layer = {
        let config = document.read();
        selected(ui, &config).map(|i| config.layout().layers[i].clone())
    };
    let layer = layer.as_ref();
    if layer.is_none() && dialog != LayerDialog::Create {
        return;
    }
    editor.set_status(Msg::None.to_ui());
    let name = match (dialog, layer) {
        (LayerDialog::Duplicate, Some(layer)) => ui
            .global::<Locale>()
            .invoke_text(Msg::CopyName(layer.name.clone()).to_ui())
            .to_string(),
        _ => String::new(),
    };
    editor.set_dialog_name(name.into());
    editor.set_dialog_description(
        match dialog {
            LayerDialog::Duplicate => layer.and_then(|layer| layer.description.clone()),
            _ => None,
        }
        .unwrap_or_default()
        .into(),
    );
    editor.set_dialog_key("".into());
    let mut action = String::new();
    editor.set_assignment(Assignment::Transparent);
    if let (LayerDialog::EditKey, Some(layer)) = (dialog, layer) {
        let Some(key) = keyboard::layer_key(index) else {
            return;
        };
        match document.read().layer_key(&layer.id, key) {
            KeyAssignment::Transparent => {}
            KeyAssignment::Swallow => editor.set_assignment(Assignment::Swallow),
            KeyAssignment::Action(action_value) => {
                editor.set_assignment(Assignment::Action);
                action = action_value;
            }
        }
    }
    if dialog == LayerDialog::EditKey {
        editor.set_dialog(LayerDialog::None);
        ui.global::<ActionPicker>().invoke_open(
            PickerTarget::LayerAction,
            index,
            action.into(),
            false,
        );
    } else {
        editor.set_dialog(dialog);
    }
}

/// Apply the open dialog.
fn apply_dialog(
    ui: &SettingsWindow,
    document: &Document,
    target: Option<&EditTarget>,
) -> Result<(), Msg> {
    let editor = ui.global::<LayersEditor>();
    let target = target.ok_or_else(|| Msg::from(&ConfigError::EditConflict))?;
    target.check(document, &editor.get_selected_id())?;
    let name = editor.get_dialog_name().trim().to_owned();
    let description = editor.get_dialog_description().trim().to_owned();
    match editor.get_dialog() {
        LayerDialog::Create | LayerDialog::Duplicate => {
            let id = change(ui, document, |config| {
                if editor.get_dialog() == LayerDialog::Create {
                    config.create_layer(&name, &description)
                } else {
                    config.clone_layer(&target.layer_id, &name, &description)
                }
            })?;
            let index = document
                .read()
                .layout()
                .layers
                .iter()
                .position(|layer| layer.id == id);
            if let Some(index) = index {
                select_layer(ui, document, index as i32);
            }
        }
        LayerDialog::Delete => {
            change(ui, document, |config| config.delete_layer(&target.layer_id))?
        }
        LayerDialog::EditKey | LayerDialog::None => {
            return Err(Msg::from(&ConfigError::EditConflict));
        }
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

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let draft = Rc::new(RefCell::new(None));
    let remove_target = Rc::new(RefCell::new(None));
    let weak = ui.as_weak();
    let doc = document.clone();
    ui.global::<LayersEditor>().on_reorder(move |from, to| {
        let Some(ui) = weak.upgrade() else { return };
        let len = doc.read().layout().layers.len();
        let Some((from, to)) = super::move_indices(from, to, len) else {
            return;
        };
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
    let weak = ui.as_weak();
    let doc = document.clone();
    ui.global::<LayersEditor>().on_swap_keys(move |from, to| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        let (Some(from), Some(to)) = (keyboard::layer_key(from), keyboard::layer_key(to)) else {
            return;
        };
        let _ = change(&ui, &doc, |config| config.swap_layer_keys(&id, from, to));
    });
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
    editor.on_name_error(move |name, renaming| {
        let Some(ui) = weak.upgrade() else {
            return Msg::None.to_ui();
        };
        let except = renaming.then(|| selected_id(&ui, &doc)).flatten();
        super::name_error(doc.read().layer_name(&name, except.as_deref()))
    });
    on!(ui, editor.on_update_name, doc = document => |ui, name| {
        let Some(id) = selected_id(&ui, &doc) else {
            return false;
        };
        let description = ui.global::<LayersEditor>().get_description();
        change(&ui, &doc, |config| {
            config.rename_layer(&id, name.trim(), &description)
        }).is_ok()
    });
    on!(ui, editor.on_add_extra => |ui| {
        let editor = ui.global::<LayersEditor>();
        editor.set_dialog_key("".into());
        editor.set_assignment(Assignment::Transparent);
        ui.global::<ActionPicker>()
            .invoke_open(PickerTarget::ExtraKey, -1, "".into(), true);
    });
    on!(ui, editor.on_pick_extra, doc = document => |ui, index, key_only| {
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        let Some((key, value)) = extra_at(&doc, &id, index) else {
            return;
        };
        let editor = ui.global::<LayersEditor>();
        editor.set_dialog(LayerDialog::None);
        editor.set_dialog_key(key.clone().into());
        editor.set_assignment(if value.is_none() {
            Assignment::Swallow
        } else {
            Assignment::Action
        });
        let picker = ui.global::<ActionPicker>();
        if key_only {
            picker.invoke_open(PickerTarget::ExtraKey, index, key.into(), true);
        } else {
            picker.invoke_open(
                PickerTarget::ExtraAction,
                index,
                value.unwrap_or_default().into(),
                false,
            );
            picker.set_subject(key.into());
        }
    });
    on!(ui, editor.on_refresh, doc = document => |ui| {
        refresh(&ui, &doc);
    });

    on!(ui, editor.on_choose, doc = document => |ui, index| {
        select_layer(&ui, &doc, index);
    });

    on!(ui, editor.on_open_dialog, doc = document, draft = draft => |ui, dialog, index| {
        *draft.borrow_mut() = Some(picker_target(&ui, &doc));
        open_dialog(&ui, &doc, dialog, index);
    });

    on!(ui, editor.on_apply_dialog, doc = document, draft = draft => |ui| {
        let editor = ui.global::<LayersEditor>();
        match apply_dialog(&ui, &doc, draft.borrow().as_ref()) {
            Ok(()) => {
                editor.set_dialog(LayerDialog::None);
            },
            Err(error) => editor.set_status(crate::notifications::report(&ui, &error)),
        }
    });

    on!(ui, editor.on_update_description, doc = document => |ui, description| {
        let Some(id) = selected_id(&ui, &doc) else {
            return false;
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
        change(&ui, &doc, |config| {
            config.rename_layer(&id, &name, description.trim())
        }).is_ok()
    });

    on!(ui, editor.on_request_remove_extra, doc = document, target = remove_target => |ui, index| {
        let Some(id) = selected_id(&ui, &doc) else { return; };
        let Some((key, _)) = extra_at(&doc, &id, index) else { return; };
        *target.borrow_mut() = Some((EditTarget::new(&doc, id), key));
        ui.global::<LayersEditor>().set_delete_extra(index);
    });
    on!(ui, editor.on_remove_extra, doc = document, target = remove_target => |ui, _index| {
        let result = (|| {
            let target = target.borrow();
            let (target, key) = target.as_ref().ok_or_else(|| Msg::from(&ConfigError::EditConflict))?;
            target.check(&doc, &ui.global::<LayersEditor>().get_selected_id())?;
            change(&ui, &doc, |config| config.set_layer_key(&target.layer_id, key, KeyAssignment::Transparent))
        })();
        match result {
            Ok(()) => { *target.borrow_mut() = None; true }
            Err(error) => { ui.global::<LayersEditor>().set_status(crate::notifications::report(&ui, &error)); false }
        }
    });
}

/// What the picker may assign to a layer key or extra row.
pub(super) fn picker_request(
    ui: &SettingsWindow,
    document: &Document,
    target: PickerTarget,
    index: i32,
) -> super::picker::PickerRequest {
    let editor = ui.global::<LayersEditor>();
    let direct = matches!(
        target,
        PickerTarget::LayerAction | PickerTarget::ExtraAction
    );
    if !direct {
        return super::picker::PickerRequest::default();
    }
    // An extra row exists only while it has an assignment, so it is
    // removed with its delete button rather than by inheriting.
    let inherit = target != PickerTarget::ExtraAction;
    super::picker::PickerRequest {
        mode: super::picker::PickerMode::LayerAction,
        swallowed: editor.get_assignment() == Assignment::Swallow,
        hold_modifiers: holds_modifiers(ui, document, target, index),
        allow_default: inherit,
        default_behavior: if inherit {
            crate::ui::DefaultAssignmentBehavior::Inherit
        } else {
            crate::ui::DefaultAssignmentBehavior::Passthrough
        },
        allow_swallow: true,
        ..Default::default()
    }
}

/// Keys the picker cannot offer for a new extra row: layer keys and keys
/// that already have a row (except the one being edited).
pub(super) fn taken_extra_keys(ui: &SettingsWindow, document: &Document) -> Vec<String> {
    let editor = ui.global::<LayersEditor>();
    let selected = editor.get_selected_id();
    let current = editor.get_dialog_key();
    let config = document.read();
    keyboard::layer_keys()
        .map(str::to_owned)
        .chain(
            config
                .layout()
                .layer_keymaps
                .get(selected.as_str())
                .into_iter()
                .flat_map(|map| map.keys.keys())
                .filter(|key| key.as_str() != current.as_str())
                .cloned(),
        )
        .collect()
}

fn holds_modifiers(
    ui: &SettingsWindow,
    document: &Document,
    target: PickerTarget,
    index: i32,
) -> bool {
    let Some(id) = selected_id(ui, document) else {
        return false;
    };
    let key = match target {
        PickerTarget::LayerAction => keyboard::layer_key(index).map(str::to_owned),
        PickerTarget::ExtraAction => extra_at(document, &id, index).map(|(key, _)| key),
        _ => None,
    };
    key.is_some_and(|key| document.read().layer_key_holds_modifiers(&id, &key))
}

/// Picker options applied together with a layer assignment.
pub(super) struct Choice {
    pub ignore: bool,
    pub hold_modifiers: bool,
    pub command: Option<lhc_core::profile::model::Command>,
}

pub(super) fn assign(
    ui: &SettingsWindow,
    document: &Document,
    index: i32,
    value: &str,
    choice: Choice,
) -> Result<(), Msg> {
    let Choice {
        ignore,
        hold_modifiers,
        command,
    } = choice;
    let id = selected_id(ui, document)
        .ok_or_else(|| Msg::from(&lhc_core::config_document::ConfigError::EditConflict))?;
    let key = keyboard::layer_key(index)
        .ok_or_else(|| Msg::from(&lhc_core::config_document::ConfigError::EditConflict))?;
    let assignment = if ignore {
        KeyAssignment::Swallow
    } else if value.is_empty() {
        KeyAssignment::Transparent
    } else {
        KeyAssignment::Action(value.to_owned())
    };
    let action = matches!(assignment, KeyAssignment::Action(_));
    change(ui, document, |config| {
        config.edit_layout_with_commands(command, |config| {
            config.set_layer_key(&id, key, assignment)?;
            if action {
                config.set_layer_key_holds_modifiers(&id, key, hold_modifiers)?;
            }
            Ok(())
        })
    })
}

/// Apply a picker choice to extra row `index`; `-1` is a row being added.
/// Returns `true` when a key was chosen for a new row and its action has to
/// be picked next: nothing is saved until then.
pub(super) fn assign_extra(
    ui: &SettingsWindow,
    document: &Document,
    index: i32,
    value: &str,
    key_only: bool,
    choice: Choice,
) -> Result<bool, Msg> {
    let Choice {
        ignore,
        hold_modifiers,
        command,
    } = choice;
    let id = selected_id(ui, document)
        .ok_or_else(|| Msg::from(&lhc_core::config_document::ConfigError::EditConflict))?;
    let existing = extra_at(document, &id, index);
    let editor = ui.global::<LayersEditor>();
    if key_only {
        let previous = existing.as_ref().map(|(key, _)| key.as_str());
        let taken = previous != Some(value)
            && document
                .read()
                .layout()
                .layer_keymaps
                .get(&id)
                .is_some_and(|map| map.keys.contains_key(value));
        if !lhc_core::profile::key_catalog::valid_key(value)
            || keyboard::layer_keys().any(|code| code == value)
        {
            return Err(Msg::KeyCodeRequired);
        }
        if taken {
            return Err(Msg::from(&ConfigError::Invalid(
                "key already assigned".into(),
            )));
        }
        let Some((previous, action)) = existing else {
            editor.set_dialog_key(value.into());
            return Ok(true);
        };
        let action = action.map_or(KeyAssignment::Swallow, KeyAssignment::Action);
        change(ui, document, |config| {
            config.set_extra_key(&id, Some(&previous), value, action)
        })?;
        return Ok(false);
    }
    let action = if ignore {
        KeyAssignment::Swallow
    } else if value.is_empty() {
        return Err(Msg::ActionRequired);
    } else {
        KeyAssignment::Action(value.to_owned())
    };
    let (previous, key) = match existing {
        Some((key, _)) => (Some(key.clone()), key),
        None if index < 0 => (None, editor.get_dialog_key().to_string()),
        None => return Err(Msg::KeyCodeRequired),
    };
    let holds = matches!(action, KeyAssignment::Action(_)) && hold_modifiers;
    change(ui, document, |config| {
        config.edit_layout_with_commands(command, |config| {
            config.set_extra_key(&id, previous.as_deref(), &key, action)?;
            config.set_layer_key_holds_modifiers(&id, &key, holds)
        })
    })?;
    Ok(false)
}

/// Second step of adding an extra row: choose the action of the key picked
/// in the first one.
pub(super) fn pick_new_extra_action(ui: &SettingsWindow) {
    let key = ui.global::<LayersEditor>().get_dialog_key();
    let picker = ui.global::<ActionPicker>();
    picker.invoke_open(PickerTarget::ExtraAction, -1, "".into(), false);
    picker.set_subject(key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additional_keys_exclude_grid_keys_and_preserve_assignments() {
        let map = lhc_core::profile::model::LayerKeymap {
            keys: [
                ("KeyQ".into(), Some("KeyA".into())),
                ("KeyW".into(), None),
                ("Backspace".into(), Some("sys:screenOff".into())),
                ("F4".into(), None),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let entries = extra_entries(&map);
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&("Backspace".into(), Some("sys:screenOff".into()))));
        assert!(entries.contains(&("F4".into(), None)));
    }

    #[test]
    fn additional_key_suppression_is_preserved() {
        let layout = lhc_core::profile::layout_file::parse(
            r#"{"schemaVersion": 1, "layers": [{"id": "extra", "keys": {"BracketRight": null, "Backslash": null, "F4": "F4"}}]}"#
        ).unwrap().unwrap();
        let entries = extra_entries(&layout.layer_keymaps["extra"]);
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&("BracketRight".into(), None)));
        assert!(entries.contains(&("Backslash".into(), None)));
        assert!(entries.contains(&("F4".into(), Some("F4".into()))));
    }

    #[test]
    fn modal_target_rejects_selection_changes_and_document_edits() {
        let (_dir, document) = super::super::test_document();
        let layer = document
            .edit(View::Layers, |config| config.create_layer("First", ""))
            .unwrap()
            .value;
        let target = EditTarget::new(&document, layer.clone());
        assert!(target.check(&document, &layer).is_ok());
        assert_eq!(
            target.check(&document, "other").unwrap_err(),
            Msg::from(&ConfigError::EditConflict)
        );
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&layer, None, "F13", KeyAssignment::Swallow)
            })
            .unwrap();
        assert_eq!(
            target.check(&document, &layer).unwrap_err(),
            Msg::from(&ConfigError::EditConflict)
        );
        let target = EditTarget::new(&document, layer.clone());
        document
            .edit(View::Layers, |config| config.delete_layer(&layer))
            .unwrap();
        assert!(target.check(&document, &layer).is_err());
    }

    #[test]
    fn failed_edit_keeps_the_modal_target_valid_for_retry() {
        let (_dir, document) = super::super::test_document();
        let layer = document
            .edit(View::Layers, |config| config.create_layer("First", ""))
            .unwrap()
            .value;
        let target = EditTarget::new(&document, layer.clone());
        let before = document.read().layout().clone();
        assert!(
            document
                .edit(View::Layers, |config| {
                    config.set_layer_key(&layer, "KeyQ", KeyAssignment::Swallow)?;
                    config.set_layer_key(&layer, "KeyW", KeyAssignment::Action("bad-action".into()))
                })
                .is_err()
        );
        assert_eq!(*document.read().layout(), before);
        assert!(target.check(&document, &layer).is_ok());
    }

    #[test]
    fn same_size_refresh_reuses_models_and_updates_reordered_rows() {
        let original = Rc::new(VecModel::from(vec![
            "First".to_owned(),
            "Second".to_owned(),
        ]));
        let model = update_model(
            ModelRc::from(original.clone()),
            vec!["Second".into(), "Renamed".into()],
        );
        assert!(std::ptr::eq(
            model.as_any().downcast_ref::<VecModel<String>>().unwrap(),
            original.as_ref()
        ));
        assert_eq!(original.row_data(0).as_deref(), Some("Second"));
        assert_eq!(original.row_data(1).as_deref(), Some("Renamed"));
    }

    #[test]
    fn extra_order_is_stable_after_renaming_and_removal() {
        let mut layout = lhc_core::profile::model::LayoutPreset::default();
        layout.add_layer("extra", "Extra".into(), None);
        for key in ["F13", "F14", "F15"] {
            layout.set_extra_key("extra", None, key, KeyAssignment::Swallow);
        }
        layout
            .layer_keymaps
            .get_mut("extra")
            .unwrap()
            .extra_key_order = vec!["F15".into(), "F13".into(), "F14".into()];
        layout.set_extra_key("extra", Some("F15"), "F16", KeyAssignment::Swallow);
        layout.set_layer_key("extra", "F13", KeyAssignment::Transparent);
        assert_eq!(
            ordered_extra_entries(&layout.layer_keymaps["extra"])
                .into_iter()
                .map(|(key, _)| key)
                .collect::<Vec<_>>(),
            ["F16", "F14"]
        );
    }
}
