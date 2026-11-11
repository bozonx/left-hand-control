//! Layers page: layer list, the keymap of the selected layer and its
//! extra keys.
//!
//! The selected layer is identified by `LayersEditor.selected-id`; the
//! `selected` index only positions the tab strip and is derived from it.

use crate::layer_key_model::{action_names, assignment, hands};
use crate::{
    document::{Document, View},
    i18n::Msg,
    keyboard,
    ui::{
        ActionPicker, LayerDialog, LayerExtraRow, LayerTrigger, LayersEditor, Locale, PickerTarget,
        SettingsWindow,
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};
use tecloid_core::config::model::{Command, Layer, LayerKeymap};
use tecloid_core::config_document::{ConfigDocument, ConfigError, KeyAssignment};

fn conflict() -> Msg {
    Msg::from(&ConfigError::EditConflict)
}

/// The layer and keymap a modal edit started from.
type LayerState = Option<(Layer, Option<LayerKeymap>)>;

fn layer_state(config: &ConfigDocument, id: &str) -> LayerState {
    let keymap = config.keymap();
    let layer = keymap.layers.iter().find(|layer| layer.id == id)?;
    Some((layer.clone(), keymap.layer_keymaps.get(id).cloned()))
}

/// Identity of a modal edit. A change of the edited layer (or of the
/// selection) invalidates the draft rather than letting its row index
/// address a different layer or key; edits elsewhere leave it valid.
#[derive(Clone)]
pub(super) struct EditTarget {
    layer_id: String,
    state: LayerState,
}

impl EditTarget {
    fn new(document: &Document, layer_id: String) -> Self {
        let state = layer_state(&document.read(), &layer_id);
        Self { layer_id, state }
    }

    pub(super) fn check(&self, document: &Document, selected_id: &str) -> Result<(), Msg> {
        if self.layer_id != selected_id
            || layer_state(&document.read(), &self.layer_id) != self.state
        {
            return Err(conflict());
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

/// Layer to show after a change: the one with `saved_id`, else the one now
/// at the previously shown position (the last when it was past the end).
fn selection(layers: &[Layer], saved_id: &str, previous: i32) -> Option<usize> {
    if let Some(index) = layers.iter().position(|layer| layer.id == saved_id) {
        return Some(index);
    }
    let last = layers.len().checked_sub(1)?;
    Some(usize::try_from(previous).map_or(0, |index| index.min(last)))
}

/// Id of the selected layer, if it still exists.
fn selected_id(ui: &SettingsWindow, document: &Document) -> Option<String> {
    let id = ui.global::<LayersEditor>().get_selected_id().to_string();
    document
        .read()
        .keymap()
        .layers
        .iter()
        .any(|layer| layer.id == id)
        .then_some(id)
}

fn select_layer(ui: &SettingsWindow, document: &Document, index: i32) {
    let editor = ui.global::<LayersEditor>();
    if editor.get_dialog() == LayerDialog::None && editor.get_delete_extra() < 0 {
        editor.set_status(Msg::None.to_ui());
    }
    let id = usize::try_from(index).ok().and_then(|index| {
        document
            .read()
            .keymap()
            .layers
            .get(index)
            .map(|layer| layer.id.clone())
    });
    if let Some(id) = id {
        editor.set_selected_id(id.into());
    }
    refresh(ui, document);
}

fn refresh(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let keymap = config.keymap();
    let editor = ui.global::<LayersEditor>();
    editor.set_names(update_model(
        editor.get_names(),
        keymap
            .layers
            .iter()
            .map(|layer| layer.name.clone().into())
            .collect(),
    ));
    let index = selection(
        &keymap.layers,
        &editor.get_selected_id(),
        editor.get_selected(),
    );
    editor.set_selected(index.map_or(-1, |index| index as i32));
    let layer = index.map(|index| &keymap.layers[index]);
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
    let layer_keymap = layer.and_then(|layer| keymap.layer_keymaps.get(&layer.id));
    let names = action_names(ui, &config.config());
    let (left, right) = hands(layer_keymap, &names);
    editor.set_left_cells(update_model(editor.get_left_cells(), left));
    editor.set_right_cells(update_model(editor.get_right_cells(), right));
    // The keyboard grid keeps one Tab stop; move it to the first key when its key is gone.
    let key = editor.get_keyboard_key();
    let (left, right) = (editor.get_left_cells(), editor.get_right_cells());
    let keys: Vec<i32> = left
        .iter()
        .chain(right.iter())
        .map(|cell| cell.index)
        .collect();
    if !keys.contains(&key) {
        editor.set_keyboard_key(keys.first().copied().unwrap_or(-1));
    }
    let extras = layer_keymap.map_or_else(Vec::new, |map| {
        map.extra_entries()
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
            .collect()
    });
    editor.set_extras(update_model(editor.get_extras(), extras));
    let triggers = layer
        .map(|layer| keymap.layer_triggers(&layer.id))
        .unwrap_or_default();
    editor.set_triggers(update_model(
        editor.get_triggers(),
        triggers
            .into_iter()
            .map(|code| LayerTrigger {
                label: trigger_label(&code).into(),
                code: code.into(),
            })
            .collect(),
    ));
}

/// US-layout label of a trigger such as `CapsLock` or `ShiftLeft+KeyA`.
fn trigger_label(code: &str) -> String {
    code.split('+')
        .map(keyboard::label)
        .collect::<Vec<_>>()
        .join("+")
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

/// Report an error that did not come from [`change`], which reports its own.
fn fail(ui: &SettingsWindow, error: Msg) -> Msg {
    ui.global::<LayersEditor>()
        .set_status(crate::notifications::report(ui, &error));
    error
}

/// Fill and open `dialog` for the selected layer.
fn open_dialog(ui: &SettingsWindow, document: &Document, dialog: LayerDialog) -> bool {
    let layer = selected_id(ui, document).and_then(|id| {
        let config = document.read();
        config
            .keymap()
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .cloned()
    });
    let name = match (dialog, &layer) {
        (LayerDialog::Create, _) => String::new(),
        (LayerDialog::Duplicate, Some(layer)) => ui
            .global::<Locale>()
            .invoke_text(Msg::CopyName(layer.name.clone()).to_ui())
            .to_string(),
        (LayerDialog::Delete, Some(_)) => String::new(),
        _ => return false,
    };
    let name = match dialog {
        LayerDialog::Duplicate => document.read().free_layer_name(&name),
        _ => name,
    };
    let description = match dialog {
        LayerDialog::Duplicate => layer.and_then(|layer| layer.description),
        _ => None,
    };
    let editor = ui.global::<LayersEditor>();
    editor.set_status(Msg::None.to_ui());
    editor.set_dialog_name(name.into());
    editor.set_dialog_description(description.unwrap_or_default().into());
    editor.set_dialog(dialog);
    true
}

/// Open the action picker for layer key `index`.
fn edit_key(ui: &SettingsWindow, document: &Document, index: i32) {
    let (Some(id), Some(key)) = (selected_id(ui, document), keyboard::layer_key(index)) else {
        return;
    };
    ui.global::<LayersEditor>().set_status(Msg::None.to_ui());
    let action = match document.read().layer_key(&id, key) {
        KeyAssignment::Action(action) => action,
        KeyAssignment::Transparent | KeyAssignment::Swallow => String::new(),
    };
    ui.global::<ActionPicker>()
        .invoke_open(PickerTarget::LayerAction, index, action.into(), false);
}

/// Apply the open dialog.
fn apply_dialog(
    ui: &SettingsWindow,
    document: &Document,
    target: Option<&EditTarget>,
) -> Result<(), Msg> {
    let editor = ui.global::<LayersEditor>();
    let dialog = editor.get_dialog();
    let name = editor.get_dialog_name().trim().to_owned();
    let description = editor.get_dialog_description().trim().to_owned();
    // A new layer depends on nothing, so only copies and deletions check
    // that their source is unchanged.
    let target = match dialog {
        LayerDialog::Create => None,
        LayerDialog::Duplicate | LayerDialog::Delete => {
            let target = target.ok_or_else(|| fail(ui, conflict()))?;
            target
                .check(document, &editor.get_selected_id())
                .map_err(|error| fail(ui, error))?;
            Some(target.layer_id.as_str())
        }
        LayerDialog::None => return Err(fail(ui, conflict())),
    };
    match (dialog, target) {
        (LayerDialog::Delete, Some(id)) => change(ui, document, |config| config.delete_layer(id)),
        (_, source) => {
            let id = change(ui, document, |config| match source {
                Some(source) => config.clone_layer(source, &name, &description),
                None => config.create_layer(&name, &description),
            })?;
            editor.set_selected_id(id.into());
            refresh(ui, document);
            Ok(())
        }
    }
}

fn extra_at(document: &Document, id: &str, index: i32) -> Option<(String, Option<String>)> {
    let index = usize::try_from(index).ok()?;
    document
        .read()
        .keymap()
        .layer_keymaps
        .get(id)
        .and_then(|map| map.extra_entries().into_iter().nth(index))
}

/// Sources of the open modal dialogs, checked again when they are applied.
#[derive(Default)]
struct Modals {
    /// Layer behind the open create/duplicate/delete dialog.
    layer: Option<EditTarget>,
    /// Layer and key of the extra row whose removal awaits confirmation.
    remove_extra: Option<(EditTarget, String)>,
}

/// Close every page dialog and forget what it was opened for.
fn close_dialogs(ui: &SettingsWindow, modals: &RefCell<Modals>) {
    let editor = ui.global::<LayersEditor>();
    editor.set_dialog(LayerDialog::None);
    editor.set_delete_extra(-1);
    *modals.borrow_mut() = Modals::default();
}

/// Whether a failed dialog should close: its source changed, so a retry
/// would fail the same way and the page already shows the new state.
fn closes_on(error: &Msg) -> bool {
    *error == conflict()
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let modals = Rc::new(RefCell::new(Modals::default()));
    let editor = ui.global::<LayersEditor>();
    on!(ui, editor.on_reorder, doc = document => |ui, from, to| {
        let len = doc.read().keymap().layers.len();
        let Some((from, to)) = super::move_indices(from, to, len) else {
            return false;
        };
        change(&ui, &doc, |config| {
            config.update_keymap(|keymap| {
                let item = keymap.layers.remove(from);
                keymap.layers.insert(to, item);
            })
        })
        .is_ok()
    });
    on!(ui, editor.on_reorder_extra, doc = document => |ui, from, to| {
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
            return;
        };
        // The stored order, not the page model, which may lag behind it.
        let keys = doc
            .read()
            .keymap()
            .layer_keymaps
            .get(&id)
            .map(|map| map.extra_entries().into_iter().map(|(key, _)| key).collect())
            .unwrap_or_default();
        // Failures are reported by `change`; the row stays where it was.
        let _ = change(&ui, &doc, |config| {
            config.reorder_extra_keys(&id, keys, from, to)
        });
    });
    on!(ui, editor.on_swap_keys, doc = document => |ui, from, to| {
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        let (Some(from), Some(to)) = (keyboard::layer_key(from), keyboard::layer_key(to)) else {
            return;
        };
        // Failures are reported by `change`; both keys keep their assignments.
        let _ = change(&ui, &doc, |config| config.swap_layer_keys(&id, from, to));
    });
    refresh(ui, document);
    let weak = ui.as_weak();
    document.subscribe(View::Layers, move |document| {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, document);
        }
    });
    on!(ui, editor.on_name_error, doc = document => |ui, name, renaming| {
        let except = renaming.then(|| selected_id(&ui, &doc)).flatten();
        super::name_error(doc.read().layer_name(&name, except.as_deref()))
    });
    on!(ui, editor.on_update_name, doc = document => |ui, name| {
        let Some(id) = selected_id(&ui, &doc) else {
            return false;
        };
        change(&ui, &doc, |config| config.set_layer_name(&id, name.trim())).is_ok()
    });
    on!(ui, editor.on_update_description, doc = document => |ui, description| {
        let Some(id) = selected_id(&ui, &doc) else {
            return false;
        };
        change(&ui, &doc, |config| config.set_layer_description(&id, &description)).is_ok()
    });
    on!(ui, editor.on_add_extra, doc = document => |ui| {
        if selected_id(&ui, &doc).is_none() {
            return;
        }
        let editor = ui.global::<LayersEditor>();
        editor.set_status(Msg::None.to_ui());
        editor.set_dialog_key("".into());
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
        editor.set_status(Msg::None.to_ui());
        editor.set_dialog_key(key.clone().into());
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
        ui.global::<LayersEditor>().set_status(Msg::None.to_ui());
        refresh(&ui, &doc);
    });
    on!(ui, editor.on_choose, doc = document => |ui, index| {
        select_layer(&ui, &doc, index);
    });
    on!(ui, editor.on_open_dialog, doc = document, modals = modals => |ui, dialog| {
        if open_dialog(&ui, &doc, dialog) {
            modals.borrow_mut().layer = Some(picker_target(&ui, &doc));
        }
    });
    on!(ui, editor.on_close_dialog, modals = modals => |ui| {
        ui.global::<LayersEditor>().set_status(Msg::None.to_ui());
        close_dialogs(&ui, &modals);
    });
    on!(ui, editor.on_edit_key, doc = document => |ui, index| {
        edit_key(&ui, &doc, index);
    });
    on!(ui, editor.on_apply_dialog, doc = document, modals = modals => |ui| {
        let target = modals.borrow().layer.clone();
        match apply_dialog(&ui, &doc, target.as_ref()) {
            Err(error) if !closes_on(&error) => {}
            _ => close_dialogs(&ui, &modals),
        }
    });
    on!(ui, editor.on_request_remove_extra, doc = document, modals = modals => |ui, index| {
        let Some(id) = selected_id(&ui, &doc) else { return; };
        let Some((key, _)) = extra_at(&doc, &id, index) else { return; };
        modals.borrow_mut().remove_extra = Some((EditTarget::new(&doc, id), key));
        let editor = ui.global::<LayersEditor>();
        editor.set_status(Msg::None.to_ui());
        editor.set_delete_extra(index);
    });
    on!(ui, editor.on_remove_extra, doc = document, modals = modals => |ui| {
        let pending = modals.borrow().remove_extra.clone();
        let result = (|| {
            let (target, key) = pending.as_ref().ok_or_else(|| fail(&ui, conflict()))?;
            target
                .check(&doc, &ui.global::<LayersEditor>().get_selected_id())
                .map_err(|error| fail(&ui, error))?;
            change(&ui, &doc, |config| config.set_layer_key(&target.layer_id, key, KeyAssignment::Transparent))
        })();
        match result {
            Err(error) if !closes_on(&error) => {}
            _ => close_dialogs(&ui, &modals),
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
    let current = picked_key(ui, document, target, index);
    let (swallowed, hold_modifiers) = current.map_or((false, false), |(id, key)| {
        let config = document.read();
        (
            config.layer_key(&id, &key) == KeyAssignment::Swallow,
            config.layer_key_holds_modifiers(&id, &key),
        )
    });
    super::picker::PickerRequest {
        mode: super::picker::PickerMode::LayerAction,
        swallowed,
        hold_modifiers,
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
                .keymap()
                .layer_keymaps
                .get(selected.as_str())
                .into_iter()
                .flat_map(|map| map.keys.keys())
                .filter(|key| key.as_str() != current.as_str())
                .cloned(),
        )
        .collect()
}

/// Selected layer and the existing key the picker edits, if any.
fn picked_key(
    ui: &SettingsWindow,
    document: &Document,
    target: PickerTarget,
    index: i32,
) -> Option<(String, String)> {
    let id = selected_id(ui, document)?;
    let key = match target {
        PickerTarget::LayerAction => keyboard::layer_key(index).map(str::to_owned),
        PickerTarget::ExtraAction => extra_at(document, &id, index).map(|(key, _)| key),
        _ => None,
    }?;
    Some((id, key))
}

/// Picker options applied together with a layer assignment.
pub(super) struct Choice {
    pub ignore: bool,
    pub hold_modifiers: bool,
    pub command: Option<Command>,
}

/// Assignment of a layer grid key; an empty value inherits.
fn key_assignment(ignore: bool, value: &str) -> KeyAssignment {
    if ignore {
        KeyAssignment::Swallow
    } else if value.is_empty() {
        KeyAssignment::Transparent
    } else {
        KeyAssignment::Action(value.to_owned())
    }
}

/// Assignment of an extra row, which has no inheriting state.
fn extra_assignment(ignore: bool, value: &str) -> Result<KeyAssignment, Msg> {
    match key_assignment(ignore, value) {
        KeyAssignment::Transparent => Err(Msg::ActionRequired),
        assignment => Ok(assignment),
    }
}

/// Set a layer key or extra row together with its held modifiers; only an
/// action can hold them. `previous` replaces an extra row's key.
fn set_key(
    config: &mut ConfigDocument,
    id: &str,
    previous: Option<&str>,
    key: &str,
    assignment: KeyAssignment,
    choice: Choice,
) -> Result<(), ConfigError> {
    let holds = matches!(assignment, KeyAssignment::Action(_)) && choice.hold_modifiers;
    config.edit_keymap_with_commands(choice.command, |config| {
        if tecloid_core::config::layer_grid::contains(key) {
            config.set_layer_key(id, key, assignment.clone())?;
        } else {
            config.set_extra_key(id, previous, key, assignment.clone())?;
        }
        if matches!(assignment, KeyAssignment::Action(_)) {
            config.set_layer_key_holds_modifiers(id, key, holds)?;
        }
        Ok(())
    })
}

pub(super) fn assign(
    ui: &SettingsWindow,
    document: &Document,
    index: i32,
    value: &str,
    choice: Choice,
) -> Result<(), Msg> {
    let id = selected_id(ui, document).ok_or_else(conflict)?;
    let key = keyboard::layer_key(index).ok_or_else(conflict)?;
    let assignment = key_assignment(choice.ignore, value);
    change(ui, document, |config| {
        set_key(config, &id, None, key, assignment, choice)
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
    let id = selected_id(ui, document).ok_or_else(conflict)?;
    let existing = extra_at(document, &id, index);
    let editor = ui.global::<LayersEditor>();
    if key_only {
        let previous = existing.as_ref().map(|(key, _)| key.as_str());
        document
            .read()
            .check_extra_key(&id, previous, value)
            .map_err(|error| Msg::from(&error))?;
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
    let assignment = extra_assignment(choice.ignore, value)?;
    let (previous, key) = match existing {
        Some((key, _)) => (Some(key.clone()), key),
        None if index < 0 => (None, editor.get_dialog_key().to_string()),
        None => return Err(conflict()),
    };
    change(ui, document, |config| {
        set_key(config, &id, previous.as_deref(), &key, assignment, choice)
    })?;
    editor.set_dialog_key("".into());
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

    /// A bound layers page with layers `First` and `Second`, `Second` selected.
    fn page() -> (
        tempfile::TempDir,
        Rc<Document>,
        SettingsWindow,
        String,
        String,
    ) {
        let (dir, document) = super::super::test_document();
        let first = document
            .edit(View::Layers, |config| config.create_layer("First", ""))
            .unwrap()
            .value;
        let second = document
            .edit(View::Layers, |config| {
                config.create_layer("Second", "About")
            })
            .unwrap()
            .value;
        use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
        struct Platform;
        impl slint::platform::Platform for Platform {
            fn create_window_adapter(
                &self,
            ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
                Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
            }
        }
        // Each test thread needs its own platform; a reused thread already has one.
        let _ = slint::platform::set_platform(Box::new(Platform));
        let ui = SettingsWindow::new().unwrap();
        bind(&ui, &document);
        ui.global::<LayersEditor>().invoke_choose(1);
        (dir, document, ui, first, second)
    }

    fn choice(ignore: bool, hold_modifiers: bool) -> Choice {
        Choice {
            ignore,
            hold_modifiers,
            command: None,
        }
    }

    #[test]
    fn selection_follows_the_id_then_the_nearest_position() {
        let layers: Vec<_> = ["a", "b", "c"]
            .into_iter()
            .map(|id| Layer {
                id: id.into(),
                name: id.into(),
                description: None,
            })
            .collect();
        assert_eq!(selection(&layers, "c", 0), Some(2));
        assert_eq!(selection(&layers, "gone", 1), Some(1));
        assert_eq!(selection(&layers, "gone", 3), Some(2), "deleted last layer");
        assert_eq!(selection(&layers, "", -1), Some(0));
        assert_eq!(selection(&[], "a", 0), None);
    }

    #[test]
    fn grid_keys_inherit_when_empty_and_extra_rows_require_an_action() {
        assert_eq!(key_assignment(false, ""), KeyAssignment::Transparent);
        assert_eq!(key_assignment(true, "F1"), KeyAssignment::Swallow);
        assert_eq!(
            key_assignment(false, "F1"),
            KeyAssignment::Action("F1".into())
        );
        assert_eq!(extra_assignment(false, ""), Err(Msg::ActionRequired));
        assert_eq!(extra_assignment(true, ""), Ok(KeyAssignment::Swallow));
    }

    #[test]
    fn modal_target_rejects_selection_changes_and_edits_of_its_layer() {
        let (_dir, document) = super::super::test_document();
        let layer = document
            .edit(View::Layers, |config| config.create_layer("First", ""))
            .unwrap()
            .value;
        let target = EditTarget::new(&document, layer.clone());
        assert!(target.check(&document, &layer).is_ok());
        assert_eq!(target.check(&document, "other").unwrap_err(), conflict());
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&layer, None, "F13", KeyAssignment::Swallow)
            })
            .unwrap();
        assert_eq!(target.check(&document, &layer).unwrap_err(), conflict());
        let target = EditTarget::new(&document, layer.clone());
        document
            .edit(View::Layers, |config| config.delete_layer(&layer))
            .unwrap();
        assert!(target.check(&document, &layer).is_err());
    }

    #[test]
    fn modal_target_survives_edits_of_other_layers() {
        let (_dir, document) = super::super::test_document();
        let layer = document
            .edit(View::Layers, |config| config.create_layer("First", ""))
            .unwrap()
            .value;
        let target = EditTarget::new(&document, layer.clone());
        document
            .edit(View::Layers, |config| {
                let other = config.create_layer("Other", "")?;
                config.set_layer_key(&other, "KeyQ", KeyAssignment::Swallow)
            })
            .unwrap();
        assert!(target.check(&document, &layer).is_ok());
    }

    #[test]
    fn failed_edit_keeps_the_modal_target_valid_for_retry() {
        let (_dir, document) = super::super::test_document();
        let layer = document
            .edit(View::Layers, |config| config.create_layer("First", ""))
            .unwrap()
            .value;
        let target = EditTarget::new(&document, layer.clone());
        let before = document.read().keymap().clone();
        assert!(
            document
                .edit(View::Layers, |config| {
                    config.set_layer_key(&layer, "KeyQ", KeyAssignment::Swallow)?;
                    config.set_layer_key(&layer, "KeyW", KeyAssignment::Action("bad-action".into()))
                })
                .is_err()
        );
        assert_eq!(*document.read().keymap(), before);
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
    fn layer_keys_keep_held_modifiers_only_for_actions() {
        let (_dir, document, ui, _, second) = page();
        let q = keyboard::layer_keys()
            .position(|key| key == "KeyQ")
            .unwrap() as i32;
        assign(&ui, &document, q, "Alt+Tab", choice(false, true)).unwrap();
        assert!(document.read().layer_key_holds_modifiers(&second, "KeyQ"));
        let request = picker_request(&ui, &document, PickerTarget::LayerAction, q);
        assert!(request.hold_modifiers && !request.swallowed);
        assign(&ui, &document, q, "", choice(true, true)).unwrap();
        assert_eq!(
            document.read().layer_key(&second, "KeyQ"),
            KeyAssignment::Swallow
        );
        assert!(!document.read().layer_key_holds_modifiers(&second, "KeyQ"));
        assert!(picker_request(&ui, &document, PickerTarget::LayerAction, q).swallowed);
        assign(&ui, &document, q, "", choice(false, false)).unwrap();
        assert_eq!(
            document.read().layer_key(&second, "KeyQ"),
            KeyAssignment::Transparent
        );
        assert_eq!(
            assign(&ui, &document, -1, "F1", choice(false, false)),
            Err(conflict())
        );
    }

    #[test]
    fn adding_an_extra_row_saves_only_after_its_action_and_validates_the_key() {
        let (_dir, document, ui, _, second) = page();
        let editor = ui.global::<LayersEditor>();
        let rows = || document.read().keymap().layer_keymaps[&second].keys.len();
        assert_eq!(
            assign_extra(&ui, &document, -1, "KeyQ", true, choice(false, false)),
            Err(Msg::ExtraKeyOnGrid)
        );
        assert_eq!(
            assign_extra(&ui, &document, -1, "NoSuchKey", true, choice(false, false)),
            Err(Msg::KeyCodeRequired)
        );
        assert_eq!(
            assign_extra(&ui, &document, -1, "F13", true, choice(false, false)),
            Ok(true)
        );
        assert_eq!(rows(), 0, "the key alone is not saved");
        assert_eq!(editor.get_dialog_key(), "F13");
        assert_eq!(
            assign_extra(&ui, &document, -1, "", false, choice(false, false)),
            Err(Msg::ActionRequired)
        );
        assign_extra(&ui, &document, -1, "Alt+Tab", false, choice(false, true)).unwrap();
        assert_eq!(editor.get_dialog_key(), "");
        assert!(document.read().layer_key_holds_modifiers(&second, "F13"));
        assert_eq!(editor.get_extras().row_count(), 1);
        assert_eq!(
            assign_extra(&ui, &document, -1, "F13", true, choice(false, false)),
            Err(Msg::ExtraKeyTaken)
        );
    }

    #[test]
    fn changing_an_extra_rows_key_keeps_its_action_and_held_modifiers() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(
                    &second,
                    None,
                    "F13",
                    KeyAssignment::Action("Alt+Tab".into()),
                )?;
                config.set_layer_key_holds_modifiers(&second, "F13", true)?;
                config.set_extra_key(&second, None, "F14", KeyAssignment::Swallow)
            })
            .unwrap();
        let editor = ui.global::<LayersEditor>();
        // Edits from this page's own view do not notify it.
        editor.invoke_refresh();
        assert_eq!(editor.get_extras().row_data(0).unwrap().key, "F13");
        assert_eq!(
            assign_extra(&ui, &document, 0, "F14", true, choice(false, false)),
            Err(Msg::ExtraKeyTaken)
        );
        assert_eq!(
            assign_extra(&ui, &document, 0, "F15", true, choice(false, false)),
            Ok(false)
        );
        let config = document.read();
        assert_eq!(
            config.layer_key(&second, "F15"),
            KeyAssignment::Action("Alt+Tab".into())
        );
        assert!(config.layer_key_holds_modifiers(&second, "F15"));
        assert_eq!(config.layer_key(&second, "F13"), KeyAssignment::Transparent);
    }

    #[test]
    fn dialogs_create_duplicate_and_delete_layers() {
        let (_dir, document, ui, first, second) = page();
        let editor = ui.global::<LayersEditor>();
        editor.invoke_open_dialog(LayerDialog::Duplicate);
        assert_eq!(editor.get_dialog(), LayerDialog::Duplicate);
        assert_eq!(editor.get_dialog_description(), "About");
        let copy_name = editor.get_dialog_name();
        assert!(document.read().layer_name(&copy_name, None).is_ok());
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::None);
        let layers = document.read().keymap().layers.clone();
        assert_eq!(layers.len(), 3);
        assert_eq!(editor.get_selected_id(), layers[2].id.as_str());
        assert_eq!(editor.get_selected(), 2);

        // The copy is the last layer: deleting it selects the new last one.
        editor.invoke_open_dialog(LayerDialog::Delete);
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::None);
        assert_eq!(document.read().keymap().layers.len(), 2);
        assert_eq!(editor.get_selected_id(), second.as_str());

        // An edit of the layer behind an open dialog closes it unapplied:
        // confirming again would fail the same way.
        editor.invoke_open_dialog(LayerDialog::Delete);
        document
            .edit(View::Layers, |config| {
                config.rename_layer(&second, "Renamed", "")
            })
            .unwrap();
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::None);
        assert_eq!(editor.get_status().id, "config-edit-conflict");
        assert_eq!(document.read().keymap().layers.len(), 2);

        // Closing forgets the source: a stray apply cannot delete anything.
        editor.invoke_open_dialog(LayerDialog::Delete);
        editor.invoke_close_dialog();
        editor.set_dialog(LayerDialog::Delete);
        editor.invoke_apply_dialog();
        assert_eq!(document.read().keymap().layers.len(), 2);

        editor.invoke_open_dialog(LayerDialog::Create);
        assert_eq!(editor.get_dialog_name(), "");
        editor.set_dialog_name("First".into());
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::Create, "name taken");
        editor.set_dialog_name("Third".into());
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::None);
        assert_eq!(document.read().keymap().layers.len(), 3);
        assert_eq!(editor.get_selected(), 2);
        assert!(
            document
                .read()
                .keymap()
                .layers
                .iter()
                .any(|layer| layer.id == first)
        );
    }

    #[test]
    fn reorder_keeps_the_selected_layer_and_reports_no_move() {
        let (_dir, document, ui, _, second) = page();
        let editor = ui.global::<LayersEditor>();
        assert!(editor.invoke_reorder(1, 0));
        assert_eq!(document.read().keymap().layers[0].id, second);
        assert_eq!(editor.get_selected(), 0);
        assert_eq!(editor.get_selected_id(), second.as_str());
        assert!(!editor.invoke_reorder(0, 0));
        assert!(!editor.invoke_reorder(0, 5));
    }

    #[test]
    fn removing_an_extra_row_needs_an_unchanged_layer() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&second, None, "F13", KeyAssignment::Swallow)
            })
            .unwrap();
        let editor = ui.global::<LayersEditor>();
        editor.invoke_refresh();
        editor.invoke_remove_extra();
        assert_eq!(editor.get_extras().row_count(), 1, "nothing requested");
        editor.invoke_request_remove_extra(0);
        assert_eq!(editor.get_delete_extra(), 0);
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&second, None, "F14", KeyAssignment::Swallow)
            })
            .unwrap();
        editor.invoke_remove_extra();
        assert_eq!(
            editor.get_delete_extra(),
            -1,
            "a changed layer closes the confirmation"
        );
        assert_eq!(
            document.read().layer_key(&second, "F13"),
            KeyAssignment::Swallow
        );
        editor.invoke_request_remove_extra(0);
        editor.invoke_remove_extra();
        assert_eq!(editor.get_delete_extra(), -1);
        assert_eq!(editor.get_extras().row_count(), 1);
    }

    #[test]
    fn description_edits_do_not_revalidate_the_name() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                config.update_keymap(|keymap| keymap.layers[1].name = String::new())
            })
            .unwrap();
        assert!(
            ui.global::<LayersEditor>()
                .invoke_update_description(" New ".into())
        );
        assert_eq!(
            document.read().keymap().layers[1].description.as_deref(),
            Some("New")
        );
        assert_eq!(document.read().keymap().layers[1].id, second);
    }

    fn key_index(code: &str) -> i32 {
        keyboard::layer_keys().position(|key| key == code).unwrap() as i32
    }

    #[test]
    fn swapping_keys_exchanges_assignments_and_ignores_bad_indices() {
        let (_dir, document, ui, _, second) = page();
        let editor = ui.global::<LayersEditor>();
        let (q, w) = (key_index("KeyQ"), key_index("KeyW"));
        assign(&ui, &document, q, "F1", choice(false, false)).unwrap();
        editor.invoke_swap_keys(q, w);
        assert_eq!(
            document.read().layer_key(&second, "KeyQ"),
            KeyAssignment::Transparent
        );
        assert_eq!(
            document.read().layer_key(&second, "KeyW"),
            KeyAssignment::Action("F1".into())
        );
        let before = document.read().keymap().clone();
        editor.invoke_swap_keys(-1, q);
        editor.invoke_swap_keys(q, 10_000);
        assert_eq!(*document.read().keymap(), before);
    }

    #[test]
    fn reordering_extra_rows_uses_the_stored_order() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                for key in ["F13", "F14", "F15"] {
                    config.set_extra_key(&second, None, key, KeyAssignment::Swallow)?;
                }
                Ok(())
            })
            .unwrap();
        // The page model is stale (own-view edits do not notify it); the
        // stored order still decides what moves.
        let editor = ui.global::<LayersEditor>();
        editor.invoke_reorder_extra(2, 0);
        assert_eq!(
            document.read().keymap().layer_keymaps[&second].extra_key_order,
            ["F15", "F13", "F14"]
        );
        let rows: Vec<_> = editor
            .get_extras()
            .iter()
            .map(|row| row.key.to_string())
            .collect();
        assert_eq!(rows, ["F15", "F13", "F14"]);
        editor.invoke_reorder_extra(-1, 0);
        editor.invoke_reorder_extra(0, 9);
        assert_eq!(
            document.read().keymap().layer_keymaps[&second].extra_key_order,
            ["F15", "F13", "F14"]
        );
    }

    #[test]
    fn renaming_keeps_the_stored_description() {
        let (_dir, document, ui, first, second) = page();
        let editor = ui.global::<LayersEditor>();
        // A description the page has not shown yet must survive a rename.
        editor.set_description("stale".into());
        assert!(editor.invoke_update_name("  Renamed ".into()));
        let layer = document.read().keymap().layers[1].clone();
        assert_eq!(layer.name, "Renamed");
        assert_eq!(layer.description.as_deref(), Some("About"));
        assert!(!editor.invoke_update_name("First".into()), "taken");
        assert!(!editor.invoke_update_name("  ".into()), "empty");
        assert_eq!(document.read().keymap().layers[1].name, "Renamed");
        assert_eq!(document.read().keymap().layers[0].id, first);
        assert_eq!(layer.id, second);
    }

    #[test]
    fn taken_extra_keys_cover_the_grid_and_other_rows() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&second, None, "F13", KeyAssignment::Swallow)?;
                config.set_extra_key(&second, None, "F14", KeyAssignment::Swallow)
            })
            .unwrap();
        let editor = ui.global::<LayersEditor>();
        let taken = taken_extra_keys(&ui, &document);
        assert!(taken.iter().any(|key| key == "KeyQ"));
        assert!(taken.iter().any(|key| key == "F13") && taken.iter().any(|key| key == "F14"));
        editor.set_dialog_key("F13".into());
        let taken = taken_extra_keys(&ui, &document);
        assert!(
            !taken.iter().any(|key| key == "F13"),
            "the edited row may keep its key"
        );
        editor.set_selected_id("missing".into());
        assert!(
            !taken_extra_keys(&ui, &document)
                .iter()
                .any(|key| key == "F14")
        );
    }

    #[test]
    fn extra_rows_cannot_inherit_in_the_picker() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&second, None, "F13", KeyAssignment::Swallow)
            })
            .unwrap();
        ui.global::<LayersEditor>().invoke_refresh();
        let request = picker_request(&ui, &document, PickerTarget::ExtraAction, 0);
        assert!(!request.allow_default && request.swallowed);
        let request = picker_request(&ui, &document, PickerTarget::ExtraAction, -1);
        assert!(!request.allow_default && !request.swallowed, "a new row");
        let request = picker_request(&ui, &document, PickerTarget::LayerAction, key_index("KeyQ"));
        assert!(request.allow_default);
    }

    #[test]
    fn triggers_list_enabled_hold_rules_with_labels() {
        let (_dir, document, ui, _, second) = page();
        let editor = ui.global::<LayersEditor>();
        assert_eq!(editor.get_triggers().row_count(), 0);
        document
            .edit(View::Rules, |config| {
                config.update_keymap(|keymap| {
                    for key in ["CapsLock", "ShiftLeft+KeyA", "CapsLock"] {
                        let mut rule = tecloid_core::config::model::KeyRule::new(
                            format!("rule-{}", keymap.rules.len()),
                            key,
                        );
                        rule.layer_id = second.clone();
                        rule.hold_behavior = Some(tecloid_core::config::model::HoldBehavior::Layer);
                        keymap.rules.push(rule);
                    }
                })
            })
            .unwrap();
        let triggers: Vec<_> = editor
            .get_triggers()
            .iter()
            .map(|trigger| (trigger.code.to_string(), trigger.label.to_string()))
            .collect();
        assert_eq!(triggers.len(), 2, "duplicates are listed once");
        assert_eq!(triggers[1].0, "ShiftLeft+KeyA");
        assert!(triggers[1].1.ends_with("+A"), "{:?}", triggers[1]);
    }

    #[test]
    fn adding_an_extra_row_needs_a_layer() {
        let (_dir, document, ui, first, second) = page();
        for id in [first, second] {
            document
                .edit(View::Rules, |config| config.delete_layer(&id))
                .unwrap();
        }
        let editor = ui.global::<LayersEditor>();
        editor.set_dialog_key("F13".into());
        editor.invoke_add_extra();
        assert_eq!(editor.get_dialog_key(), "F13", "the picker was not opened");
    }
}
