//! Emoji menu, quick action menu and shell commands of the layout.
//!
//! The page edits a draft of the menus. Changes save as soon as the draft
//! is valid; typing saves after a short pause.

use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{CommandField, CommandRow, EmojiCategory, MenuCell, MenuEditor, MenuKind, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigError,
    profile::{
        ids,
        menus::{command_issue, empty_quick_action},
        model::*,
    },
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc, time::Duration};

const PAGE: usize = LEFT_HAND_HOTKEYS.len();
const TYPING_DELAY: Duration = Duration::from_millis(400);

#[derive(serde::Deserialize)]
struct Category {
    id: String,
    items: Vec<String>,
}

struct State {
    /// The menus as last saved or loaded.
    baseline: LayoutPreset,
    reference_ids: Vec<String>,
    layout: LayoutPreset,
    timer: Option<slint::Timer>,
}

type Shared = Rc<RefCell<State>>;

fn key_label(key: &str) -> SharedString {
    key.trim_start_matches("Key").into()
}

/// Every quick action page has `PAGE` slots and there is at least one
/// page of each menu.
fn normalize(layout: &mut LayoutPreset) {
    if layout.emoji_pages.is_empty() {
        layout.emoji_pages.push(EmojiPage {
            id: ids::generate("emoji_"),
            name: "1".into(),
            cells: Default::default(),
        });
    }
    let count = layout
        .quick_actions
        .len()
        .div_ceil(PAGE)
        .max(layout.quick_action_pages.len())
        .max(1);
    while layout.quick_action_pages.len() < count {
        layout.quick_action_pages.push(QuickActionPage {
            id: ids::generate("page_"),
            name: (layout.quick_action_pages.len() + 1).to_string(),
        });
    }
    layout
        .quick_actions
        .resize_with(count * PAGE, empty_quick_action);
}

fn page_count(kind: MenuKind, layout: &LayoutPreset) -> usize {
    match kind {
        MenuKind::Emoji => layout.emoji_pages.len(),
        MenuKind::Quick => layout.quick_action_pages.len(),
        MenuKind::Commands => 0,
    }
}

fn first_filled(layout: &LayoutPreset, page: usize) -> i32 {
    layout
        .quick_actions
        .get(page * PAGE..(page + 1) * PAGE)
        .and_then(|cells| cells.iter().position(|item| !item.action.trim().is_empty()))
        .map_or(-1, |i| i as i32)
}

fn command_row(config: &AppConfig, commands: &[Command], index: usize) -> CommandRow {
    let item = &commands[index];
    CommandRow {
        id: item.id.clone().into(),
        name: item.name.clone().into(),
        linux: item.linux.clone().into(),
        usage: super::strings(lhc_core::profile::macros::action_usage(
            config,
            &format!("cmd:{}", item.id),
        )),
        error: command_issue(commands, index)
            .map_or(Msg::None, Msg::MenuIssue)
            .to_ui(),
    }
}

fn config_of(layout: &LayoutPreset) -> AppConfig {
    AppConfig::from_parts(AppSettings::default(), layout.clone(), None)
}

fn has_command_errors(commands: &[Command]) -> bool {
    (0..commands.len()).any(|i| command_issue(commands, i).is_some())
}

/// Push the draft to the UI. `fields` also resets the bound text fields,
/// which must not happen while the user types into them.
fn refresh(ui: &SettingsWindow, layout: &LayoutPreset, fields: bool) {
    let e = ui.global::<MenuEditor>();
    let kind = e.get_kind();
    if kind == MenuKind::Commands {
        let config = config_of(layout);
        e.set_commands(ModelRc::new(VecModel::from(
            (0..layout.commands.len())
                .map(|i| command_row(&config, &layout.commands, i))
                .collect::<Vec<_>>(),
        )));
        e.set_has_errors(has_command_errors(&layout.commands));
        return;
    }
    let names: Vec<String> = if kind == MenuKind::Emoji {
        layout.emoji_pages.iter().map(|p| p.name.clone()).collect()
    } else {
        layout
            .quick_action_pages
            .iter()
            .map(|p| p.name.clone())
            .collect()
    };
    let p = (e.get_selected_page().max(0) as usize).min(names.len().saturating_sub(1));
    e.set_selected_page(p as i32);
    e.set_pages(super::strings(names.clone()));
    let c = e.get_selected_cell().clamp(-1, PAGE as i32 - 1);
    let cells: Vec<MenuCell> = if kind == MenuKind::Emoji {
        let c = c.max(0);
        e.set_selected_cell(c);
        let page = &layout.emoji_pages[p];
        if fields {
            e.set_value(
                page.cells
                    .get(LEFT_HAND_HOTKEYS[c as usize])
                    .cloned()
                    .unwrap_or_default()
                    .into(),
            );
        }
        LEFT_HAND_HOTKEYS
            .iter()
            .map(|key| MenuCell {
                key: key_label(key),
                value: page.cells.get(*key).cloned().unwrap_or_default().into(),
                name: SharedString::new(),
            })
            .collect()
    } else {
        e.set_selected_cell(c);
        if fields {
            let item = usize::try_from(c)
                .ok()
                .and_then(|c| layout.quick_actions.get(p * PAGE + c));
            e.set_value(item.map(|i| i.action.clone()).unwrap_or_default().into());
            e.set_name(item.map(|i| i.name.clone()).unwrap_or_default().into());
        }
        LEFT_HAND_HOTKEYS
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let item = &layout.quick_actions[p * PAGE + i];
                MenuCell {
                    key: key_label(key),
                    value: item.action.clone().into(),
                    name: if item.name.is_empty() {
                        item.action.clone()
                    } else {
                        item.name.clone()
                    }
                    .into(),
                }
            })
            .collect()
    };
    if fields {
        e.set_page_name(names[p].clone().into());
    }
    e.set_cells(ModelRc::new(VecModel::from(cells)));
}

/// Refresh command errors and usage in place, keeping focus in the rows.
fn annotate_commands(ui: &SettingsWindow, layout: &LayoutPreset) {
    let e = ui.global::<MenuEditor>();
    let commands = &layout.commands;
    let config = config_of(layout);
    let rows = e.get_commands();
    for index in 0..commands.len() {
        if let Some(mut row) = rows.row_data(index) {
            let fresh = command_row(&config, commands, index);
            row.id = fresh.id;
            row.name = fresh.name;
            row.linux = fresh.linux;
            row.usage = fresh.usage;
            row.error = fresh.error;
            rows.set_row_data(index, row);
        }
    }
    e.set_has_errors(has_command_errors(commands));
}

fn show_trust(ui: &SettingsWindow, document: &Document) {
    ui.global::<MenuEditor>()
        .set_trusted(document.read().commands_trusted());
}

/// Save the draft unless commands have errors.
fn save(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.timer = None;
    let e = ui.global::<MenuEditor>();
    let commands_valid = !has_command_errors(&state.layout.commands);
    if commands_valid {
        for index in 0..state.layout.commands.len() {
            let old = state.reference_ids[index].clone();
            let new = state.layout.commands[index].id.clone();
            if old != new {
                lhc_core::profile::menus::replace_command_references(
                    &mut state.layout,
                    &old,
                    Some(&new),
                );
                state.reference_ids[index] = new;
            }
        }
    }
    let baseline = state.baseline.clone();
    let mut candidate = state.layout.clone();
    if !commands_valid {
        candidate.commands = baseline.commands.clone();
        candidate.rules = baseline.rules.clone();
        candidate.layer_keymaps = baseline.layer_keymaps.clone();
        candidate.macros = baseline.macros.clone();
    }
    if candidate == baseline {
        e.set_status(
            if commands_valid {
                Msg::None
            } else {
                Msg::MenuDraft
            }
            .to_ui(),
        );
        return;
    }
    let result = document.edit(View::Menus, |config| {
        config.save_menu_pages(&baseline, &candidate)
    });
    let message = match result {
        Ok(saved) => {
            state.baseline = document.read().layout().clone();
            show_trust(ui, document);
            saved.message(if commands_valid {
                Msg::None
            } else {
                Msg::MenuDraft
            })
        }
        Err(error) => {
            if error == ConfigError::ExternalChange {
                reload(ui, document, state);
            }
            Msg::from(&error)
        }
    };
    e.set_status(message.to_ui());
}

fn reload(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.timer = None;
    state.baseline = document.read().layout().clone();
    state.layout = state.baseline.clone();
    state.reference_ids = state.layout.commands.iter().map(|c| c.id.clone()).collect();
    normalize(&mut state.layout);
    show_trust(ui, document);
    refresh(ui, &state.layout, true);
}

fn save_later(ui: &SettingsWindow, document: &Rc<Document>, state: &Shared) {
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), Rc::downgrade(state));
    timer.start(slint::TimerMode::SingleShot, TYPING_DELAY, move || {
        if let (Some(ui), Some(state)) = (weak.upgrade(), shared.upgrade()) {
            save(&ui, &doc, &mut state.borrow_mut());
        }
    });
    state.borrow_mut().timer = Some(timer);
}

fn action_name(label: &str, action: &str) -> String {
    if label.is_empty() { action } else { label }.into()
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let baseline = document.read().layout().clone();
    let mut layout = baseline.clone();
    normalize(&mut layout);
    let state: Shared = Rc::new(RefCell::new(State {
        reference_ids: layout.commands.iter().map(|c| c.id.clone()).collect(),
        baseline,
        layout,
        timer: None,
    }));
    let e = ui.global::<MenuEditor>();
    show_trust(ui, document);
    match serde_json::from_str::<Vec<Category>>(include_str!("../../ui/emoji-catalog.json")) {
        Ok(categories) => e.set_catalog(ModelRc::new(VecModel::from(
            categories
                .into_iter()
                .map(|c| EmojiCategory {
                    id: c.id.into(),
                    items: super::strings(c.items),
                })
                .collect::<Vec<_>>(),
        ))),
        Err(error) => log::error!("emoji catalog: {error}"),
    }

    // Reload when the menus changed elsewhere and no edit is pending.
    let weak = ui.as_weak();
    let shared = state.clone();
    document.subscribe(View::Menus, move |document| {
        let Some(ui) = weak.upgrade() else { return };
        // Busy: this page's own save is running and reloads by itself.
        let Ok(mut state) = shared.try_borrow_mut() else {
            return;
        };
        let mut saved = state.baseline.clone();
        normalize(&mut saved);
        if state.timer.is_none()
            && state.layout == saved
            && state.baseline != *document.read().layout()
        {
            reload(&ui, document, &mut state);
        } else {
            show_trust(&ui, document);
        }
    });

    // Run `edit` on the draft, save it, and push the result to the UI.
    let change = {
        let weak = ui.as_weak();
        let (doc, shared) = (document.clone(), state.clone());
        Rc::new(
            move |typing: bool, edit: &dyn Fn(&MenuEditor, &mut LayoutPreset)| {
                let Some(ui) = weak.upgrade() else { return };
                {
                    let mut state = shared.borrow_mut();
                    let commands = state.layout.commands.clone();
                    let ids = state.reference_ids.clone();
                    edit(&ui.global::<MenuEditor>(), &mut state.layout);
                    state.reference_ids = state
                        .layout
                        .commands
                        .iter()
                        .map(|command| {
                            commands
                                .iter()
                                .position(|old| old == command)
                                .map(|index| ids[index].clone())
                                .unwrap_or_else(|| command.id.clone())
                        })
                        .collect();
                    normalize(&mut state.layout);
                    ui.global::<MenuEditor>().set_editing_count(0);
                    if !typing {
                        save(&ui, &doc, &mut state);
                    }
                    refresh(&ui, &state.layout, !typing);
                    ui.global::<MenuEditor>().set_trusted(
                        doc.read().commands_trusted()
                            && state.layout.commands == doc.read().layout().commands,
                    );
                }
                if typing {
                    save_later(&ui, &doc, &shared);
                }
            },
        )
    };

    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    e.on_open(move |kind| {
        let Some(ui) = weak.upgrade() else { return };
        let mut state = shared.borrow_mut();
        if state.timer.is_some() {
            save(&ui, &doc, &mut state);
        }
        let e = ui.global::<MenuEditor>();
        e.set_editing_count(0);
        e.set_kind(kind);
        e.set_selected_page(0);
        if state.layout.commands == state.baseline.commands {
            reload(&ui, &doc, &mut state);
        }
        e.set_status(
            if state.layout.commands != state.baseline.commands {
                Msg::MenuDraft
            } else {
                Msg::None
            }
            .to_ui(),
        );
        e.set_selected_cell(if kind == MenuKind::Quick {
            first_filled(&state.layout, 0)
        } else {
            0
        });
        e.set_trusted(
            doc.read().commands_trusted() && state.layout.commands == doc.read().layout().commands,
        );
        e.set_has_errors(false);
        refresh(&ui, &state.layout, true);
    });

    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    e.on_refresh(move || {
        let Some(ui) = weak.upgrade() else { return };
        let mut state = shared.borrow_mut();
        state.timer = None;
        reload(&ui, &doc, &mut state);
        ui.global::<MenuEditor>().set_status(Msg::None.to_ui());
        refresh(&ui, &state.layout, true);
    });

    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_select_page(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let state = shared.borrow();
        let e = ui.global::<MenuEditor>();
        e.set_selected_page(index);
        e.set_selected_cell(if e.get_kind() == MenuKind::Quick {
            first_filled(&state.layout, index.max(0) as usize)
        } else {
            0
        });
        refresh(&ui, &state.layout, true);
    });
    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_select_cell(move |index| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<MenuEditor>().set_selected_cell(index);
            refresh(&ui, &shared.borrow().layout, true);
        }
    });

    let c = change.clone();
    e.on_add_page(move |name| {
        c(false, &|e, layout| {
            let index = if e.get_kind() == MenuKind::Emoji {
                layout.emoji_pages.push(EmojiPage {
                    id: ids::generate("emoji_"),
                    name: name.to_string(),
                    cells: Default::default(),
                });
                layout.emoji_pages.len() - 1
            } else {
                layout.quick_action_pages.push(QuickActionPage {
                    id: ids::generate("page_"),
                    name: name.to_string(),
                });
                layout.quick_action_pages.len() - 1
            };
            e.set_selected_page(index as i32);
            e.set_selected_cell(if e.get_kind() == MenuKind::Quick {
                -1
            } else {
                0
            });
        });
    });
    let c = change.clone();
    e.on_remove_page(move || {
        c(false, &|e, layout| {
            let p = e.get_selected_page().max(0) as usize;
            match e.get_kind() {
                MenuKind::Emoji if p < layout.emoji_pages.len() => {
                    layout.emoji_pages.remove(p);
                }
                MenuKind::Quick if p < layout.quick_action_pages.len() => {
                    layout.quick_action_pages.remove(p);
                    let end = ((p + 1) * PAGE).min(layout.quick_actions.len());
                    layout.quick_actions.drain((p * PAGE).min(end)..end);
                }
                _ => {}
            }
            e.set_selected_page(p.saturating_sub(1) as i32);
            e.set_selected_cell(if e.get_kind() == MenuKind::Quick {
                -1
            } else {
                0
            });
        });
    });
    let c = change.clone();
    e.on_move_page(move |delta| {
        c(false, &|e, layout| {
            let p = e.get_selected_page();
            let n = p + delta;
            if p < 0 || n < 0 || n as usize >= page_count(e.get_kind(), layout) {
                return;
            }
            let (p, n) = (p as usize, n as usize);
            if e.get_kind() == MenuKind::Emoji {
                layout.emoji_pages.swap(p, n);
            } else {
                layout.quick_action_pages.swap(p, n);
                for c in 0..PAGE {
                    layout.quick_actions.swap(p * PAGE + c, n * PAGE + c);
                }
            }
            e.set_selected_page(n as i32);
        });
    });
    let c = change.clone();
    e.on_rename_page(move || {
        c(true, &|e, layout| {
            let p = e.get_selected_page().max(0) as usize;
            let name = e.get_page_name().trim().to_owned();
            if name.is_empty() {
                return;
            }
            if e.get_kind() == MenuKind::Emoji {
                if let Some(page) = layout.emoji_pages.get_mut(p) {
                    page.name = name;
                }
            } else if let Some(page) = layout.quick_action_pages.get_mut(p) {
                page.name = name;
            }
        });
    });
    let c = change.clone();
    e.on_set_cell(move || {
        c(true, &|e, layout| {
            let p = e.get_selected_page().max(0) as usize;
            let key = LEFT_HAND_HOTKEYS[e.get_selected_cell().clamp(0, PAGE as i32 - 1) as usize];
            let value = e.get_value().trim().to_owned();
            if let Some(page) = layout.emoji_pages.get_mut(p) {
                if value.is_empty() {
                    page.cells.remove(key);
                } else {
                    page.cells.insert(key.into(), value);
                }
            }
        });
    });
    let c = change.clone();
    e.on_set_name(move || {
        c(true, &|e, layout| {
            let (Ok(page), Ok(cell)) = (
                usize::try_from(e.get_selected_page()),
                usize::try_from(e.get_selected_cell()),
            ) else {
                return;
            };
            if let Some(item) = layout.quick_actions.get_mut(page * PAGE + cell) {
                item.name = e.get_name().to_string();
            }
        });
    });
    let c = change.clone();
    e.on_set_action(move |cell, action, label| {
        c(false, &|e, layout| {
            let (Ok(page), Ok(cell_index)) = (
                usize::try_from(e.get_selected_page()),
                usize::try_from(cell),
            ) else {
                return;
            };
            let Some(item) = layout.quick_actions.get_mut(page * PAGE + cell_index) else {
                return;
            };
            let automatic = item.name.trim().is_empty() || item.name == item.action;
            if item.action.trim().is_empty() {
                *item = QuickAction {
                    name: action_name(&label, &action),
                    action: action.to_string(),
                    ..empty_quick_action()
                };
            } else {
                item.action = action.to_string();
                if automatic {
                    item.name = action_name(&label, &action);
                }
            }
            e.set_selected_cell(cell);
        });
    });
    let c = change.clone();
    e.on_clear_cell(move || {
        c(false, &|e, layout| {
            let p = e.get_selected_page().max(0) as usize;
            let cell = e.get_selected_cell().clamp(0, PAGE as i32 - 1) as usize;
            if e.get_kind() == MenuKind::Emoji {
                if let Some(page) = layout.emoji_pages.get_mut(p) {
                    page.cells.remove(LEFT_HAND_HOTKEYS[cell]);
                }
            } else if let Some(item) = layout.quick_actions.get_mut(p * PAGE + cell) {
                *item = empty_quick_action();
                e.set_selected_cell(-1);
            }
        });
    });
    let c = change.clone();
    e.on_move_cell(move |from, to| {
        c(false, &|e, layout| {
            if !(0..PAGE as i32).contains(&from) || !(0..PAGE as i32).contains(&to) || from == to {
                return;
            }
            let (from, to) = (from as usize, to as usize);
            let p = e.get_selected_page().max(0) as usize;
            if e.get_kind() == MenuKind::Emoji {
                let Some(page) = layout.emoji_pages.get_mut(p) else {
                    return;
                };
                let mut values: Vec<_> = LEFT_HAND_HOTKEYS
                    .iter()
                    .map(|key| page.cells.remove(*key))
                    .collect();
                let item = values.remove(from);
                values.insert(to, item);
                for (key, value) in LEFT_HAND_HOTKEYS.iter().zip(values) {
                    if let Some(value) = value {
                        page.cells.insert((*key).into(), value);
                    }
                }
            } else if p * PAGE + PAGE <= layout.quick_actions.len() {
                let item = layout.quick_actions.remove(p * PAGE + from);
                layout.quick_actions.insert(p * PAGE + to, item);
            }
            e.set_selected_cell(to as i32);
        });
    });

    let c = change.clone();
    e.on_add_command(move |name| {
        c(false, &|_, layout| {
            let mut id = ids::generate("cmd_");
            while layout.commands.iter().any(|c| c.id == id) {
                id = ids::generate("cmd_");
            }
            layout.commands.insert(
                0,
                Command {
                    id,
                    name: name.to_string(),
                    linux: String::new(),
                },
            );
        });
    });
    let c = change.clone();
    let shared = state.clone();
    e.on_remove_command(move |index| {
        let source = usize::try_from(index)
            .ok()
            .and_then(|index| shared.borrow().reference_ids.get(index).cloned());
        c(false, &|_, layout| {
            if let Ok(index) = usize::try_from(index)
                && index < layout.commands.len()
            {
                let id = source
                    .clone()
                    .unwrap_or_else(|| layout.commands[index].id.clone());
                layout.commands.remove(index);
                lhc_core::profile::menus::replace_command_references(layout, &id, None);
            }
        });
    });
    let c = change.clone();
    e.on_move_command(move |index, target| {
        c(false, &|_, layout| {
            let len = layout.commands.len() as i32;
            let next = target;
            if (0..len).contains(&index) && (0..len).contains(&next) {
                let item = layout.commands.remove(index as usize);
                layout.commands.insert(next as usize, item);
            }
        });
    });
    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    e.on_set_command(move |index, field, value| {
        let Some(ui) = weak.upgrade() else { return };
        {
            let mut state = shared.borrow_mut();
            let Some(index) = usize::try_from(index)
                .ok()
                .filter(|index| *index < state.layout.commands.len())
            else {
                return;
            };
            match field {
                CommandField::Id => {
                    let old = state.reference_ids[index].clone();
                    state.layout.commands[index].id = value.to_string();
                    if command_issue(&state.layout.commands, index).is_none() {
                        lhc_core::profile::menus::replace_command_references(
                            &mut state.layout,
                            &old,
                            Some(&value),
                        );
                        state.reference_ids[index] = value.to_string();
                    }
                }
                CommandField::Name => state.layout.commands[index].name = value.to_string(),
                CommandField::Linux => state.layout.commands[index].linux = value.to_string(),
            }
            annotate_commands(&ui, &state.layout);
            ui.global::<MenuEditor>().set_trusted(
                doc.read().commands_trusted()
                    && state.layout.commands == doc.read().layout().commands,
            );
        }
        save_later(&ui, &doc, &shared);
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    e.on_trust(move |approve| {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<MenuEditor>();
        {
            let mut state = state.borrow_mut();
            if state.timer.is_some() {
                save(&ui, &doc, &mut state);
            }
            if approve && state.layout.commands != doc.read().layout().commands {
                e.set_status(Msg::MenuSaveFirst.to_ui());
                return;
            }
        }
        let message = match doc.edit(View::Menus, |config| config.trust_commands(approve)) {
            Ok(saved) => saved.message(Msg::None),
            Err(error) => Msg::from(&error),
        };
        show_trust(&ui, &doc);
        e.set_status(message.to_ui());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_fills_pages() {
        let mut layout = LayoutPreset::default();
        normalize(&mut layout);
        assert_eq!(layout.emoji_pages.len(), 1);
        assert_eq!(layout.quick_action_pages.len(), 1);
        assert_eq!(layout.quick_actions.len(), PAGE);
        assert_eq!(first_filled(&layout, 0), -1);
        assert_eq!(first_filled(&layout, 5), -1, "missing pages are empty");
    }
}
