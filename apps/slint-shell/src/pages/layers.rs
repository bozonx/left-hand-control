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
        ActionPicker, LayerDialog, LayerExtraRow, LayersEditor, Locale, PickerTarget,
        SettingsWindow,
    },
};
use lhc_core::config_document::{ConfigDocument, ConfigError, KeyAssignment};
use lhc_core::profile::model::{Command, Layer, LayerKeymap};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

fn conflict() -> Msg {
    Msg::from(&ConfigError::EditConflict)
}

/// The layer and keymap a modal edit started from.
type LayerState = Option<(Layer, Option<LayerKeymap>)>;

fn layer_state(config: &ConfigDocument, id: &str) -> LayerState {
    let layout = config.layout();
    let layer = layout.layers.iter().find(|layer| layer.id == id)?;
    Some((layer.clone(), layout.layer_keymaps.get(id).cloned()))
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
        .layout()
        .layers
        .iter()
        .any(|layer| layer.id == id)
        .then_some(id)
}

fn select_layer(ui: &SettingsWindow, document: &Document, index: i32) {
    let id = usize::try_from(index).ok().and_then(|index| {
        document
            .read()
            .layout()
            .layers
            .get(index)
            .map(|layer| layer.id.clone())
    });
    if let Some(id) = id {
        ui.global::<LayersEditor>().set_selected_id(id.into());
    }
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
    let index = selection(
        &layout.layers,
        &editor.get_selected_id(),
        editor.get_selected(),
    );
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
    let extras = keymap.map_or_else(Vec::new, |map| {
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
            .collect()
    });
    editor.set_extras(update_model(editor.get_extras(), extras));
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
            .layout()
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

/// Extra keys of a layer (every key off the grid) in display order: the
/// saved order first, then the rest in key catalog order.
fn ordered_extra_entries(map: &LayerKeymap) -> Vec<(String, Option<String>)> {
    let mut entries: Vec<_> = map
        .keys
        .iter()
        .filter(|(key, _)| !lhc_core::profile::layer_grid::contains(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if entries.is_empty() {
        return entries;
    }
    let catalog: HashMap<&str, usize> = lhc_core::profile::key_catalog::CATEGORIES
        .iter()
        .flat_map(|keys| keys.iter().copied())
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect();
    let order = &map.extra_key_order;
    entries.sort_by_cached_key(|(key, _)| {
        (
            order
                .iter()
                .position(|code| code == key)
                .unwrap_or(usize::MAX),
            catalog.get(key.as_str()).copied().unwrap_or(usize::MAX),
            key.clone(),
        )
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
    let editor = ui.global::<LayersEditor>();
    on!(ui, editor.on_reorder, doc = document => |ui, from, to| {
        let len = doc.read().layout().layers.len();
        let Some((from, to)) = super::move_indices(from, to, len) else {
            return false;
        };
        change(&ui, &doc, |config| {
            config.update_layout(|layout| {
                let item = layout.layers.remove(from);
                layout.layers.insert(to, item);
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
        let keys = ui
            .global::<LayersEditor>()
            .get_extras()
            .iter()
            .map(|row| row.key.to_string())
            .collect();
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
        let description = ui.global::<LayersEditor>().get_description();
        change(&ui, &doc, |config| {
            config.rename_layer(&id, name.trim(), &description)
        }).is_ok()
    });
    on!(ui, editor.on_update_description, doc = document => |ui, description| {
        let Some(id) = selected_id(&ui, &doc) else {
            return false;
        };
        change(&ui, &doc, |config| config.set_layer_description(&id, &description)).is_ok()
    });
    on!(ui, editor.on_add_extra => |ui| {
        ui.global::<LayersEditor>().set_dialog_key("".into());
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
        ui.global::<LayersEditor>().set_dialog_key(key.clone().into());
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
    on!(ui, editor.on_open_dialog, doc = document, draft = draft => |ui, dialog| {
        if open_dialog(&ui, &doc, dialog) {
            *draft.borrow_mut() = Some(picker_target(&ui, &doc));
        }
    });
    on!(ui, editor.on_edit_key, doc = document => |ui, index| {
        edit_key(&ui, &doc, index);
    });
    on!(ui, editor.on_apply_dialog, doc = document, draft = draft => |ui| {
        if apply_dialog(&ui, &doc, draft.borrow().as_ref()).is_ok() {
            ui.global::<LayersEditor>().set_dialog(LayerDialog::None);
            *draft.borrow_mut() = None;
        }
    });
    on!(ui, editor.on_request_remove_extra, doc = document, target = remove_target => |ui, index| {
        let Some(id) = selected_id(&ui, &doc) else { return; };
        let Some((key, _)) = extra_at(&doc, &id, index) else { return; };
        *target.borrow_mut() = Some((EditTarget::new(&doc, id), key));
        ui.global::<LayersEditor>().set_delete_extra(index);
    });
    on!(ui, editor.on_remove_extra, doc = document, target = remove_target => |ui| {
        let result = (|| {
            let target = target.borrow();
            let (target, key) = target.as_ref().ok_or_else(|| fail(&ui, conflict()))?;
            target
                .check(&doc, &ui.global::<LayersEditor>().get_selected_id())
                .map_err(|error| fail(&ui, error))?;
            change(&ui, &doc, |config| config.set_layer_key(&target.layer_id, key, KeyAssignment::Transparent))
        })();
        if result.is_ok() {
            *target.borrow_mut() = None;
        }
        result.is_ok()
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
    config.edit_layout_with_commands(choice.command, |config| {
        if lhc_core::profile::layer_grid::contains(key) {
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
    use lhc_core::profile::model::LayoutPreset;

    fn keymap(keys: &[(&str, Option<&str>)]) -> LayerKeymap {
        LayerKeymap {
            keys: keys
                .iter()
                .map(|(key, value)| ((*key).into(), value.map(str::to_owned)))
                .collect(),
            ..Default::default()
        }
    }

    fn keys(entries: Vec<(String, Option<String>)>) -> Vec<String> {
        entries.into_iter().map(|(key, _)| key).collect()
    }

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
    fn extra_keys_exclude_grid_keys_and_preserve_assignments() {
        let map = keymap(&[
            ("KeyQ", Some("KeyA")),
            ("KeyW", None),
            ("Backspace", Some("sys:screenOff")),
            ("F4", None),
        ]);
        let entries = ordered_extra_entries(&map);
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&("Backspace".into(), Some("sys:screenOff".into()))));
        assert!(entries.contains(&("F4".into(), None)));
    }

    #[test]
    fn extra_key_suppression_is_preserved() {
        let layout = lhc_core::profile::layout_file::parse(
            r#"{"schemaVersion": 1, "layers": [{"id": "extra", "keys": {"BracketRight": null, "Backslash": null, "F4": "F4"}}]}"#
        ).unwrap().unwrap();
        let entries = ordered_extra_entries(&layout.layer_keymaps["extra"]);
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&("BracketRight".into(), None)));
        assert!(entries.contains(&("F4".into(), Some("F4".into()))));
    }

    #[test]
    fn unordered_extra_keys_follow_the_saved_order_then_the_catalog() {
        let mut map = keymap(&[("F2", None), ("F14", None), ("F1", None), ("F13", None)]);
        map.extra_key_order = vec!["F14".into(), "F13".into()];
        assert_eq!(
            keys(ordered_extra_entries(&map)),
            ["F14", "F13", "F1", "F2"]
        );
    }

    #[test]
    fn extra_order_is_stable_after_renaming_and_removal() {
        let mut layout = LayoutPreset::default();
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
            keys(ordered_extra_entries(&layout.layer_keymaps["extra"])),
            ["F16", "F14"]
        );
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
        let rows = || document.read().layout().layer_keymaps[&second].keys.len();
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
        let layers = document.read().layout().layers.clone();
        assert_eq!(layers.len(), 3);
        assert_eq!(editor.get_selected_id(), layers[2].id.as_str());
        assert_eq!(editor.get_selected(), 2);

        // The copy is the last layer: deleting it selects the new last one.
        editor.invoke_open_dialog(LayerDialog::Delete);
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::None);
        assert_eq!(document.read().layout().layers.len(), 2);
        assert_eq!(editor.get_selected_id(), second.as_str());

        // An edit of the layer behind an open dialog cancels it.
        editor.invoke_open_dialog(LayerDialog::Delete);
        document
            .edit(View::Layers, |config| {
                config.rename_layer(&second, "Renamed", "")
            })
            .unwrap();
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::Delete);
        assert_eq!(editor.get_status().id, "config-edit-conflict");
        assert_eq!(document.read().layout().layers.len(), 2);

        editor.invoke_open_dialog(LayerDialog::Create);
        assert_eq!(editor.get_dialog_name(), "");
        editor.set_dialog_name("First".into());
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::Create, "name taken");
        editor.set_dialog_name("Third".into());
        editor.invoke_apply_dialog();
        assert_eq!(editor.get_dialog(), LayerDialog::None);
        assert_eq!(document.read().layout().layers.len(), 3);
        assert_eq!(editor.get_selected(), 2);
        assert!(
            document
                .read()
                .layout()
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
        assert_eq!(document.read().layout().layers[0].id, second);
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
        assert!(!editor.invoke_remove_extra(), "nothing requested");
        editor.invoke_request_remove_extra(0);
        assert_eq!(editor.get_delete_extra(), 0);
        document
            .edit(View::Layers, |config| {
                config.set_extra_key(&second, None, "F14", KeyAssignment::Swallow)
            })
            .unwrap();
        assert!(!editor.invoke_remove_extra());
        editor.invoke_request_remove_extra(0);
        assert!(editor.invoke_remove_extra());
        assert_eq!(editor.get_extras().row_count(), 1);
    }

    #[test]
    fn description_edits_do_not_revalidate_the_name() {
        let (_dir, document, ui, _, second) = page();
        document
            .edit(View::Layers, |config| {
                config.update_layout(|layout| layout.layers[1].name = String::new())
            })
            .unwrap();
        assert!(
            ui.global::<LayersEditor>()
                .invoke_update_description(" New ".into())
        );
        assert_eq!(
            document.read().layout().layers[1].description.as_deref(),
            Some("New")
        );
        assert_eq!(document.read().layout().layers[1].id, second);
    }
}
