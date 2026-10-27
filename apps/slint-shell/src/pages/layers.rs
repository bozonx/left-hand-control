//! Layers page: layer list, the keymap of the selected layer and its
//! extra keys.

use super::{action_kind, picker::action_label};
use crate::{
    document::{Document, View},
    i18n::Msg,
    keyboard,
    ui::{
        ActionKind, ActionPicker, Assignment, LayerDialog, LayerExtraRow, LayerKeyCell,
        LayersEditor, Locale, PickerCategory, PickerTarget, SettingsWindow,
    },
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError, KeyAssignment},
    profile::{
        actions,
        model::{LayerKeymap, LayoutPreset, ResolvedAppConfig},
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// A command chosen inside a layer-key dialog, pending its Apply button.
#[derive(Default)]
pub(super) struct Draft {
    command: Option<lhc_core::profile::model::Command>,
    target: Option<(String, i32)>,
}

pub(super) fn command(
    ui: &SettingsWindow,
    state: &RefCell<Draft>,
    id: &str,
) -> Option<lhc_core::profile::model::Command> {
    let editor = ui.global::<LayersEditor>();
    let state = state.borrow();
    let target = (
        editor.get_selected_id().to_string(),
        editor.get_dialog_index(),
    );
    state
        .command
        .as_ref()
        .filter(|command| {
            editor.get_dialog() == LayerDialog::EditKey
                && state.target.as_ref() == Some(&target)
                && command.id == id
        })
        .cloned()
}

pub(super) fn choose_dialog_action(
    ui: &SettingsWindow,
    state: &RefCell<Draft>,
    action: slint::SharedString,
    command: Option<lhc_core::profile::model::Command>,
) {
    let editor = ui.global::<LayersEditor>();
    let mut state = state.borrow_mut();
    state.command = command;
    state.target = Some((
        editor.get_selected_id().to_string(),
        editor.get_dialog_index(),
    ));
    editor.set_dialog_action(action);
}

/// Display names of catalog actions (macros, commands, built-ins).
fn action_names(ui: &SettingsWindow, config: &ResolvedAppConfig) -> HashMap<String, String> {
    let mut names: HashMap<String, String> = actions::catalog(config)
        .into_iter()
        .filter_map(|entry| Some((entry.action.format()?, action_label(ui, entry.name)?)))
        .collect();
    super::picker::DESKTOP_SHORTCUTS.with(|cache| {
        names.extend(
            cache
                .borrow()
                .iter()
                .map(|shortcut| (shortcut.value.clone(), shortcut.label.clone())),
        );
    });
    names
}

/// How a keymap value is shown: `None` no entry, `Some(None)` swallow.
fn assignment(
    value: Option<&Option<String>>,
    names: &HashMap<String, String>,
) -> (Assignment, String, ActionKind, PickerCategory) {
    match value {
        None => (
            Assignment::Transparent,
            String::new(),
            ActionKind::None,
            PickerCategory::None,
        ),
        Some(None) => (
            Assignment::Swallow,
            String::new(),
            ActionKind::None,
            PickerCategory::None,
        ),
        Some(Some(action)) if action.is_empty() => (
            Assignment::Transparent,
            String::new(),
            ActionKind::None,
            PickerCategory::None,
        ),
        Some(Some(action)) => {
            let kind = action_kind(action);
            let label = match actions::Action::parse(Some(action)) {
                actions::Action::Text(text) => super::one_line(&text),
                actions::Action::KdeShortcut(id) => {
                    names.get(action).cloned().unwrap_or_else(|| {
                        actions::kde_shortcut_parts(&id)
                            .map_or(id.clone(), |(_, shortcut)| shortcut.to_owned())
                    })
                }
                _ => names.get(action).cloned().unwrap_or_else(|| action.clone()),
            };
            (
                Assignment::Action,
                label,
                kind,
                super::picker::category_for_value(action),
            )
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
    lookup: &dyn Fn(&str) -> (Assignment, String, ActionKind, PickerCategory),
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
            let (kind, action, icon, category) = lookup(code);
            cells.push(LayerKeyCell {
                index: index as i32,
                label: keyboard::label(code).into(),
                code: (*code).into(),
                kind,
                emoji: category == PickerCategory::Text && is_emoji(&action),
                category,
                action: action.into(),
                icon,
                holds: false,
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

/// Both hand grids of a layer with `keymap`.
fn hands(
    keymap: Option<&LayerKeymap>,
    names: &HashMap<String, String>,
) -> (Vec<LayerKeyCell>, Vec<LayerKeyCell>) {
    let lookup = |code: &str| assignment(keymap.and_then(|map| map.keys.get(code)), names);
    let left_len: usize = keyboard::LEFT_HAND.iter().map(|row| row.len()).sum();
    let mark_holds = |mut cells: Vec<LayerKeyCell>| {
        if let Some(map) = keymap {
            for cell in &mut cells {
                cell.holds = map.hold_modifiers.contains(cell.code.as_str());
            }
        }
        cells
    };
    (
        mark_holds(hand_cells(
            &keyboard::LEFT_HAND,
            keyboard::LEFT_COLUMNS,
            0,
            2,
            &lookup,
        )),
        mark_holds(hand_cells(
            &keyboard::RIGHT_HAND,
            keyboard::RIGHT_COLUMNS,
            left_len,
            0,
            &lookup,
        )),
    )
}

/// The keys of layer `id` of the running `layout` for the layer hint, or
/// `None` when the layout has no such layer.
pub(crate) fn hint(
    ui: &SettingsWindow,
    config: &ConfigDocument,
    layout: &LayoutPreset,
    id: &str,
) -> Option<crate::layer_hint_model::LayerHint> {
    let layer = layout.layers.iter().find(|layer| layer.id == id)?;
    let names = action_names(
        ui,
        &ResolvedAppConfig::from_parts(config.settings().clone(), layout.clone(), None),
    );
    let (left, right) = hands(layout.layer_keymaps.get(id), &names);
    Some(crate::layer_hint_model::LayerHint::from_cells(
        layer.name.clone(),
        &left,
        &right,
    ))
}

/// Whether `text` consists only of emoji, so the key shows it in the colour
/// emoji font; mixed text keeps the regular font, whose digits stay plain.
fn is_emoji(text: &str) -> bool {
    let pictographic = |c: char| {
        matches!(c as u32,
            0x1F000..=0x1FAFF | 0x2300..=0x23FF | 0x2600..=0x27BF | 0x2B00..=0x2BFF)
    };
    // Joiners, variation selectors, skin tones and tag characters only extend a glyph.
    let modifier = |c: char| {
        matches!(
            c as u32,
            0x200D | 0xFE0E | 0xFE0F | 0x20E3 | 0xE0020..=0xE007F
        )
    };
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        // U+FE0F asks for emoji presentation of an otherwise textual symbol, like ℹ️.
        if pictographic(c) || chars.peek() == Some(&'\u{FE0F}') {
            any = true;
        } else if !modifier(c) {
            return false;
        }
    }
    any
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
    let names = action_names(ui, &config.config());
    let (left, right) = hands(keymap, &names);
    editor.set_left_cells(ModelRc::new(VecModel::from(left)));
    editor.set_right_cells(ModelRc::new(VecModel::from(right)));
    editor.set_extras(ModelRc::new(VecModel::from(
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
    )));
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
    if let (LayerDialog::EditKey, Some(layer)) = (dialog, layer) {
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
fn apply_dialog(
    ui: &SettingsWindow,
    document: &Document,
    command: Option<lhc_core::profile::model::Command>,
) -> Result<(), Msg> {
    let editor = ui.global::<LayersEditor>();
    let dialog = editor.get_dialog();
    let index = editor.get_dialog_index();
    let name = editor.get_dialog_name().trim().to_owned();
    let description = editor.get_dialog_description().trim().to_owned();
    let action = editor.get_dialog_action().to_string();
    let assignment = editor.get_assignment();
    if dialog == LayerDialog::EditKey && assignment == Assignment::Action {
        if action.trim().is_empty() {
            return Err(Msg::ActionRequired);
        }
        let parsed = actions::Action::parse(Some(&action));
        if matches!(&parsed, actions::Action::Text(text) if text.is_empty()) {
            return Err(Msg::TextEmpty);
        }
        let mut candidate = document.read().config();
        if let Some(command) = command.as_ref() {
            super::stage_command(&mut candidate.commands, command.clone());
        }
        if let Some(issue) = actions::validate(&parsed, &candidate) {
            return Err(Msg::InvalidAction(issue));
        }
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
            change(ui, document, |config| {
                config.edit_layout_with_commands(command, |config| {
                    config.set_layer_key(&id, key, value)
                })
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

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) -> Rc<RefCell<Draft>> {
    let draft = Rc::new(RefCell::new(Draft::default()));
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
            return;
        };
        let description = ui.global::<LayersEditor>().get_description();
        let _ = change(&ui, &doc, |config| {
            config.rename_layer(&id, name.trim(), &description)
        });
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
        *draft.borrow_mut() = Draft::default();
        open_dialog(&ui, &doc, dialog, index);
    });

    on!(ui, editor.on_apply_dialog, doc = document, draft = draft => |ui| {
        let editor = ui.global::<LayersEditor>();
        let pending = match actions::Action::parse(Some(&editor.get_dialog_action())) {
            actions::Action::Command(id) if editor.get_assignment() == Assignment::Action => command(&ui, &draft, &id),
            _ => None,
        };
        match apply_dialog(&ui, &doc, pending) {
            Ok(()) => {
                *draft.borrow_mut() = Draft::default();
                editor.set_dialog(LayerDialog::None);
            },
            Err(error) => editor.set_status(crate::notifications::report(&ui, &error)),
        }
    });

    on!(ui, editor.on_update_description, doc = document => |ui, description| {
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

    on!(ui, editor.on_remove_extra, doc = document => |ui, index| {
        let Some(id) = selected_id(&ui, &doc) else {
            return;
        };
        if let Some((key, _)) = extra_at(&doc, &id, index) {
            let _ = change(&ui, &doc, |config| {
                config.set_layer_key(&id, &key, KeyAssignment::Transparent)
            });
        }
    });
    draft
}

/// Whether the layer key `index` of picker `target` keeps its modifiers
/// held until the layer is released.
/// What the picker may assign to a layer key or extra row.
pub(super) fn picker_request(
    ui: &SettingsWindow,
    document: &Document,
    target: PickerTarget,
    index: i32,
) -> super::picker::PickerRequest {
    let editor = ui.global::<LayersEditor>();
    // Inside a layer dialog the picker only fills the dialog's draft.
    let direct = matches!(
        target,
        PickerTarget::LayerAction | PickerTarget::ExtraAction
    ) && editor.get_dialog() == LayerDialog::None;
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
    fn only_emoji_text_uses_the_emoji_font() {
        assert!(is_emoji("👍"));
        assert!(is_emoji("⚡"));
        assert!(is_emoji("👍🏽"));
        assert!(is_emoji("❤️"));
        assert!(is_emoji("ℹ️"));
        assert!(is_emoji("👨‍👩‍👧"));
        assert!(!is_emoji("≠"));
        assert!(!is_emoji("№"));
        assert!(!is_emoji("ℹ"));
        assert!(!is_emoji("ok 👍"));
        assert!(!is_emoji(""));
    }

    #[test]
    fn bundled_additional_keys_preserve_actions_and_suppression() {
        let layout = lhc_core::profile::bundled::BUNDLED[0].layout().unwrap();
        for id in ["navigation", "select", "symbols", "f-keys"] {
            assert!(extra_entries(&layout.layer_keymaps[id]).is_empty());
        }
        assert_eq!(
            extra_entries(&layout.layer_keymaps["desktop"]),
            vec![("Backspace".into(), Some("sys:screenOff".into()))]
        );
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
    fn assignments_describe_keymap_values() {
        let names = HashMap::from([("macro:copyLine".to_owned(), "Copy line".to_owned())]);
        assert_eq!(assignment(None, &names).0, Assignment::Transparent);
        assert_eq!(assignment(Some(&None), &names).0, Assignment::Swallow);
        assert_eq!(
            assignment(Some(&Some("macro:copyLine".into())), &names),
            (
                Assignment::Action,
                "Copy line".into(),
                ActionKind::Macro,
                PickerCategory::Macros
            )
        );
        assert_eq!(
            assignment(Some(&Some("text:hi".into())), &names),
            (
                Assignment::Action,
                "hi".into(),
                ActionKind::Text,
                PickerCategory::Text
            )
        );
    }

    #[test]
    fn hand_grids_index_every_key() {
        let lookup = |_: &str| {
            (
                Assignment::Transparent,
                String::new(),
                ActionKind::None,
                PickerCategory::None,
            )
        };
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
