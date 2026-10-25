//! Emoji menu and quick action menu of the layout.
//!
//! The page edits a draft of the menus. Changes save as soon as the draft
//! is valid; typing saves after a short pause.

use super::autosave::Autosave;
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{EmojiCategory, MenuCell, MenuEditor, MenuKind, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigError,
    profile::{ids, model::*},
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

const PAGE: usize = LEFT_HAND_HOTKEYS.len();

#[derive(serde::Deserialize)]
struct Category {
    id: String,
    items: Vec<String>,
}

struct State {
    /// The menus as last saved or loaded.
    baseline: LayoutPreset,
    layout: LayoutPreset,
    autosave: Autosave,
}

type Shared = Rc<RefCell<State>>;

impl super::autosave::Draft for State {
    fn autosave(&self) -> &Autosave {
        &self.autosave
    }

    fn save(&mut self, ui: &SettingsWindow, document: &Document) {
        save(ui, document, self);
    }
}

fn key_label(key: &str) -> SharedString {
    key.trim_start_matches("Key").into()
}

/// There is at least one page of each menu.
fn normalize(layout: &mut LayoutPreset) {
    if layout.emoji_pages.is_empty() {
        layout.emoji_pages.push(EmojiPage {
            id: ids::generate("emoji-"),
            name: String::new(),
            cells: Default::default(),
        });
    }
    if layout.quick_action_pages.is_empty() {
        layout
            .quick_action_pages
            .push(QuickActionPage::new(ids::generate("page-"), String::new()));
    }
}

fn page_count(kind: MenuKind, layout: &LayoutPreset) -> usize {
    match kind {
        MenuKind::Emoji => layout.emoji_pages.len(),
        MenuKind::Quick => layout.quick_action_pages.len(),
    }
}

fn first_filled(layout: &LayoutPreset, page: usize) -> i32 {
    layout
        .quick_action_pages
        .get(page)
        .and_then(|page| {
            LEFT_HAND_HOTKEYS.iter().position(|key| {
                page.cells
                    .get(*key)
                    .is_some_and(|item| !item.action.trim().is_empty())
            })
        })
        .map_or(-1, |i| i as i32)
}

/// Swap the cells at hotkey indexes `from` and `to`; the other keys keep
/// their cells, as with a transfer between pages.
fn move_cell<V>(cells: &mut BTreeMap<String, V>, from: usize, to: usize) {
    let (Some(from), Some(to)) = (LEFT_HAND_HOTKEYS.get(from), LEFT_HAND_HOTKEYS.get(to)) else {
        return;
    };
    let source = cells.remove(*from);
    if let Some(target) = cells.remove(*to) {
        cells.insert((*from).into(), target);
    }
    if let Some(source) = source {
        cells.insert((*to).into(), source);
    }
}

/// Push the draft to the UI. `fields` also resets the bound text fields,
/// which must not happen while the user types into them.
fn refresh(ui: &SettingsWindow, layout: &LayoutPreset, fields: bool) {
    let e = ui.global::<MenuEditor>();
    let kind = e.get_kind();
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
    if e.get_pages()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>()
        != names
    {
        e.set_pages(super::strings(names.clone()));
    }
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
        let page = &layout.quick_action_pages[p];
        if fields {
            let item = usize::try_from(c)
                .ok()
                .and_then(|c| page.cells.get(LEFT_HAND_HOTKEYS[c]));
            e.set_value(item.map(|i| i.action.clone()).unwrap_or_default().into());
            e.set_name(item.map(|i| i.name.clone()).unwrap_or_default().into());
        }
        LEFT_HAND_HOTKEYS
            .iter()
            .map(|key| {
                let item = page.cells.get(*key).cloned().unwrap_or_default();
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

fn save(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.autosave.cancel();
    let e = ui.global::<MenuEditor>();
    let baseline = state.baseline.clone();
    let candidate = state.layout.clone();
    if candidate == baseline {
        e.set_status(Msg::None.to_ui());
        return;
    }
    let result = document.edit(View::Menus, |config| {
        config.save_menu_pages(&baseline, &candidate)
    });
    match &result {
        Ok(_) => state.baseline = document.read().layout().clone(),
        Err(ConfigError::EditConflict) => reload(ui, document, state),
        Err(_) => {}
    }
    let message = super::outcome(&result, Msg::None);
    e.set_status(crate::notifications::report(ui, &message));
}

fn reload(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.autosave.cancel();
    state.baseline = document.read().layout().clone();
    state.layout = state.baseline.clone();
    normalize(&mut state.layout);
    refresh(ui, &state.layout, true);
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let baseline = document.read().layout().clone();
    let mut layout = baseline.clone();
    normalize(&mut layout);
    let state: Shared = Rc::new(RefCell::new(State {
        baseline,
        layout,
        autosave: Autosave::default(),
    }));
    let e = ui.global::<MenuEditor>();
    on!(ui, e.on_flush_pending, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        super::autosave::flush(&ui, &doc, &mut *state);
    });
    match serde_json::from_str::<Vec<Category>>(include_str!("../../assets/emoji-catalog.json")) {
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
    super::autosave::subscribe(ui, document, View::Menus, &state, |ui, document, state| {
        let mut saved = state.baseline.clone();
        normalize(&mut saved);
        if !state.autosave.pending()
            && state.layout == saved
            && state.baseline != *document.read().layout()
        {
            reload(ui, document, state);
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
                    edit(&ui.global::<MenuEditor>(), &mut state.layout);
                    normalize(&mut state.layout);
                    ui.global::<MenuEditor>().set_editing_count(0);
                    if !typing {
                        save(&ui, &doc, &mut state);
                    }
                    refresh(&ui, &state.layout, !typing);
                }
                if typing {
                    super::autosave::schedule(&ui, &doc, &shared);
                }
            },
        )
    };

    on!(ui, e.on_open, doc = document, shared = state => |ui, kind| {
        let mut state = shared.borrow_mut();
        super::autosave::flush(&ui, &doc, &mut *state);
        let e = ui.global::<MenuEditor>();
        e.set_editing_count(0);
        e.set_kind(kind);
        e.set_selected_page(0);
        reload(&ui, &doc, &mut state);
        e.set_status(Msg::None.to_ui());
        e.set_selected_cell(if kind == MenuKind::Quick {
            first_filled(&state.layout, 0)
        } else {
            0
        });
        refresh(&ui, &state.layout, true);
    });

    on!(ui, e.on_refresh, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        state.autosave.cancel();
        reload(&ui, &doc, &mut state);
        ui.global::<MenuEditor>().set_status(Msg::None.to_ui());
        refresh(&ui, &state.layout, true);
    });

    on!(ui, e.on_select_page, shared = state => |ui, index| {
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
    on!(ui, e.on_select_cell, shared = state => |ui, index| {
        ui.global::<MenuEditor>().set_selected_cell(index);
        refresh(&ui, &shared.borrow().layout, true);
    });

    let c = change.clone();
    e.on_add_page(move |name| {
        c(false, &|e, layout| {
            let index = if e.get_kind() == MenuKind::Emoji {
                layout.emoji_pages.push(EmojiPage {
                    id: ids::generate("emoji-"),
                    name: name.to_string(),
                    cells: Default::default(),
                });
                layout.emoji_pages.len() - 1
            } else {
                layout.quick_action_pages.push(QuickActionPage::new(
                    ids::generate("page-"),
                    name.to_string(),
                ));
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
            lhc_core::profile::menus::remove_page(layout, e.get_kind() == MenuKind::Emoji, p);
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
            lhc_core::profile::menus::reorder_page(
                layout,
                e.get_kind() == MenuKind::Emoji,
                p as usize,
                n as usize,
            );
            e.set_selected_page(n);
        });
    });
    let c = change.clone();
    e.on_reorder_page(move |from, to| {
        c(false, &|e, layout| {
            let count = page_count(e.get_kind(), layout) as i32;
            if !(0..count).contains(&from) || !(0..count).contains(&to) || from == to {
                return;
            }
            lhc_core::profile::menus::reorder_page(
                layout,
                e.get_kind() == MenuKind::Emoji,
                from as usize,
                to as usize,
            );
            let selected = e.get_selected_page();
            e.set_selected_page(if selected == from {
                to
            } else if from < selected && selected <= to {
                selected - 1
            } else if to <= selected && selected < from {
                selected + 1
            } else {
                selected
            });
        });
    });
    let c = change.clone();
    e.on_transfer_cell(move |source_page, source, target| {
        c(false, &|e, layout| {
            let (Ok(source_page), Ok(source), Ok(target), Ok(target_page)) = (
                usize::try_from(source_page),
                usize::try_from(source),
                usize::try_from(target),
                usize::try_from(e.get_selected_page()),
            ) else {
                return;
            };
            lhc_core::profile::menus::transfer_cell(
                layout,
                e.get_kind() == MenuKind::Emoji,
                source_page,
                source,
                target_page,
                target,
            );
            e.set_selected_cell(target as i32);
        });
    });
    let c = change.clone();
    e.on_rename_page(move || {
        c(true, &|e, layout| {
            let p = e.get_selected_page().max(0) as usize;
            let name = e.get_page_name().trim().to_owned();
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
            if let Some(item) = layout
                .quick_action_pages
                .get_mut(page)
                .zip(LEFT_HAND_HOTKEYS.get(cell))
                .and_then(|(page, key)| page.cells.get_mut(*key))
            {
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
            let (Some(page), Some(key)) = (
                layout.quick_action_pages.get_mut(page),
                LEFT_HAND_HOTKEYS.get(cell_index),
            ) else {
                return;
            };
            if action.is_empty() {
                page.cells.remove(*key);
                e.set_selected_cell(-1);
                return;
            }
            let item = page.cells.entry((*key).into()).or_default();
            let automatic = item.name.trim().is_empty() || item.name == item.action;
            if item.action.trim().is_empty() {
                *item = QuickAction {
                    name: lhc_core::profile::model::display_name(&label, &action).into(),
                    action: action.to_string(),
                    icon: None,
                };
            } else {
                item.action = action.to_string();
                if automatic {
                    item.name = lhc_core::profile::model::display_name(&label, &action).into();
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
            } else if let Some(page) = layout.quick_action_pages.get_mut(p) {
                page.cells.remove(LEFT_HAND_HOTKEYS[cell]);
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
                if let Some(page) = layout.emoji_pages.get_mut(p) {
                    move_cell(&mut page.cells, from, to);
                }
            } else if let Some(page) = layout.quick_action_pages.get_mut(p) {
                move_cell(&mut page.cells, from, to);
            }
            e.set_selected_cell(to as i32);
        });
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
        assert_eq!(first_filled(&layout, 0), -1);
        assert_eq!(first_filled(&layout, 5), -1, "missing pages are empty");
    }

    #[test]
    fn move_cell_swaps_without_shifting() {
        let mut cells: BTreeMap<String, &str> = [("KeyQ", "q"), ("KeyW", "w"), ("KeyE", "e")]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        move_cell(&mut cells, 0, 2);
        assert_eq!(cells["KeyQ"], "e");
        assert_eq!(cells["KeyW"], "w", "cells between stay in place");
        assert_eq!(cells["KeyE"], "q");
        move_cell(&mut cells, 1, 4);
        assert!(!cells.contains_key("KeyW"), "an empty target moves back");
        assert_eq!(cells["KeyT"], "w");
    }
}
