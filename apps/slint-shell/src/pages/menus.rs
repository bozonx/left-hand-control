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
use lhc_core::profile::{menus, model::*};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

const PAGE: usize = LEFT_HAND_HOTKEYS.len();

/// Palette categories that `ui/menus.slint` has titles for.
const CATEGORY_IDS: [&str; 8] = [
    "smileys", "people", "symbols", "work", "nature", "food", "travel", "utf",
];

#[derive(serde::Deserialize)]
struct Category {
    id: String,
    items: Vec<String>,
}

pub(super) struct State {
    /// The menus as last saved or loaded.
    baseline: LayoutPreset,
    layout: LayoutPreset,
    autosave: Autosave,
    commands: Vec<Command>,
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

/// The bundled emoji palette; a unit test keeps it parseable.
fn catalog() -> Result<Vec<Category>, serde_json::Error> {
    serde_json::from_str(include_str!("../../assets/emoji-catalog.json"))
}

fn key_label(key: &str) -> SharedString {
    key.trim_start_matches("Key").into()
}

fn core_kind(kind: MenuKind) -> menus::MenuKind {
    match kind {
        MenuKind::Emoji => menus::MenuKind::Emoji,
        MenuKind::Quick => menus::MenuKind::Quick,
    }
}

/// The draft of a layout: both menus always have a page.
fn draft(layout: &LayoutPreset) -> LayoutPreset {
    let mut layout = layout.clone();
    menus::ensure_pages(&mut layout);
    layout
}

/// Index of the first assigned quick action on `page`, or -1.
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

/// The cell to select when a page opens: emoji pages always edit a cell,
/// quick action pages start on the first assigned one.
fn initial_cell(kind: MenuKind, layout: &LayoutPreset, page: i32) -> i32 {
    match kind {
        MenuKind::Emoji => 0,
        MenuKind::Quick => usize::try_from(page).map_or(-1, |page| first_filled(layout, page)),
    }
}

/// Selected page and cell as indexes, if they point into the draft.
fn selection(e: &MenuEditor, layout: &LayoutPreset) -> Option<(usize, usize)> {
    let page = usize::try_from(e.get_selected_page()).ok()?;
    let cell = usize::try_from(e.get_selected_cell()).ok()?;
    (page < menus::page_count(layout, core_kind(e.get_kind())) && cell < PAGE)
        .then_some((page, cell))
}

/// Push the draft to the UI. `fields` also resets the bound text fields,
/// which must not happen while the user types into them.
fn refresh(ui: &SettingsWindow, layout: &LayoutPreset, fields: bool) {
    let e = ui.global::<MenuEditor>();
    let kind = e.get_kind();
    let names: Vec<String> = match kind {
        MenuKind::Emoji => layout.emoji_pages.iter().map(|p| p.name.clone()).collect(),
        MenuKind::Quick => layout
            .quick_action_pages
            .iter()
            .map(|p| p.name.clone())
            .collect(),
    };
    if names.is_empty() {
        log::error!("menu draft has no {kind:?} pages");
        return;
    }
    let p = (e.get_selected_page().max(0) as usize).min(names.len() - 1);
    e.set_selected_page(p as i32);
    if !e
        .get_pages()
        .iter()
        .map(|name| name.to_string())
        .eq(names.iter().cloned())
    {
        e.set_pages(super::strings(names.clone()));
    }
    // Emoji pages always edit a cell; quick action pages may have none selected.
    let min = if kind == MenuKind::Emoji { 0 } else { -1 };
    let c = e.get_selected_cell().clamp(min, PAGE as i32 - 1);
    e.set_selected_cell(c);
    let selected = usize::try_from(c).ok().map(|c| LEFT_HAND_HOTKEYS[c]);
    let cells: Vec<MenuCell> = match kind {
        MenuKind::Emoji => {
            let page = &layout.emoji_pages[p];
            if fields {
                let value = selected.and_then(|key| page.cells.get(key));
                e.set_value(value.cloned().unwrap_or_default().into());
            }
            LEFT_HAND_HOTKEYS
                .iter()
                .map(|key| MenuCell {
                    key: key_label(key),
                    value: page.cells.get(*key).cloned().unwrap_or_default().into(),
                    name: SharedString::new(),
                    category: Default::default(),
                })
                .collect()
        }
        MenuKind::Quick => {
            let page = &layout.quick_action_pages[p];
            if fields {
                let item = selected.and_then(|key| page.cells.get(key));
                e.set_value(item.map(|i| i.action.clone()).unwrap_or_default().into());
                e.set_name(item.map(|i| i.name.clone()).unwrap_or_default().into());
            }
            LEFT_HAND_HOTKEYS
                .iter()
                .map(|key| {
                    let item = page.cells.get(*key).cloned().unwrap_or_default();
                    MenuCell {
                        key: key_label(key),
                        name: display_name(&item.name, &item.action).into(),
                        category: super::picker::category_for_value(&item.action),
                        value: item.action.into(),
                    }
                })
                .collect()
        }
    };
    if fields {
        e.set_page_name(names[p].clone().into());
    }
    e.set_only_page_empty(
        names.len() == 1 && names[0].is_empty() && cells.iter().all(|cell| cell.value.is_empty()),
    );
    let model = e.get_cells();
    if let Some(existing) = model.as_any().downcast_ref::<VecModel<MenuCell>>()
        && existing.row_count() == cells.len()
    {
        for (index, cell) in cells.into_iter().enumerate() {
            if existing.row_data(index).as_ref() != Some(&cell) {
                existing.set_row_data(index, cell);
            }
        }
    } else {
        e.set_cells(ModelRc::new(VecModel::from(cells)));
    }
}

fn menu_changed(state: &State) -> bool {
    !state.commands.is_empty()
        || state.layout.emoji_pages != state.baseline.emoji_pages
        || state.layout.quick_action_pages != state.baseline.quick_action_pages
}

/// Persist the menu draft independently of the Slint editor.
fn persist(
    document: &Document,
    state: &mut State,
) -> Result<Option<crate::document::Saved<()>>, lhc_core::config_document::ConfigError> {
    state.commands.retain(|command| {
        state
            .layout
            .quick_action_pages
            .iter()
            .flat_map(|page| page.cells.values())
            .any(|cell| {
                matches!(lhc_core::profile::actions::Action::parse(Some(&cell.action)),
                lhc_core::profile::actions::Action::Command(id) if id == command.id)
            })
    });
    let baseline = state.baseline.clone();
    let candidate = state.layout.clone();
    if !menu_changed(state) {
        return Ok(None);
    }
    let result = document.edit(View::Menus, |config| {
        config.edit_layout_with_commands(state.commands.clone(), |config| {
            config.save_menu_pages(&baseline, &candidate)
        })
    });
    if result.is_ok() {
        state.commands.clear();
        state.baseline = document.read().layout().clone();
        state.layout = draft(&state.baseline);
    }
    result.map(Some)
}

fn save(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.autosave.cancel();
    let result = persist(document, state);
    let e = ui.global::<MenuEditor>();
    e.set_has_unsaved_changes(menu_changed(state));
    let message = match result {
        Ok(Some(saved)) => saved.message(Msg::None),
        Ok(None) => Msg::None,
        Err(error) => Msg::from(&error),
    };
    // Autosave repeats while the user types: notify only when the status changes.
    e.set_status(crate::notifications::report_changed(
        ui,
        &message,
        e.get_status(),
    ));
}

fn reload(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.autosave.cancel();
    state.commands.clear();
    state.baseline = document.read().layout().clone();
    state.layout = draft(&state.baseline);
    ui.global::<MenuEditor>().set_has_unsaved_changes(false);
    refresh(ui, &state.layout, true);
}

pub(super) fn command(state: &RefCell<State>, id: &str) -> Option<Command> {
    state
        .borrow()
        .commands
        .iter()
        .find(|command| command.id == id)
        .cloned()
}

pub(super) fn choose(
    ui: &SettingsWindow,
    document: &Document,
    state: &RefCell<State>,
    cell: i32,
    action: &str,
    label: &str,
    command: Option<Command>,
) -> Result<(), Msg> {
    let mut state = state.borrow_mut();
    let e = ui.global::<MenuEditor>();
    lhc_core::profile::menus::set_quick_action(
        &mut state.layout,
        e.get_selected_page(),
        cell,
        action,
        label,
    )
    .map_err(|error| Msg::from(&error))?;
    e.set_selected_cell(if action.is_empty() { -1 } else { cell });
    if let Some(command) = command {
        super::stage_command(&mut state.commands, command);
    }
    save(ui, document, &mut state);
    refresh(ui, &state.layout, true);
    Ok(())
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) -> Shared {
    let baseline = document.read().layout().clone();
    let layout = draft(&baseline);
    let state: Shared = Rc::new(RefCell::new(State {
        baseline,
        layout,
        autosave: Autosave::default(),
        commands: Vec::new(),
    }));
    let e = ui.global::<MenuEditor>();
    on!(ui, e.on_flush_pending, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        save(&ui, &doc, &mut state);
        !menu_changed(&state)
    });
    match catalog() {
        Ok(categories) => e.set_catalog(ModelRc::new(VecModel::from(
            categories
                .into_iter()
                .filter(|c| {
                    // The page has no title for other categories.
                    let known = CATEGORY_IDS.contains(&c.id.as_str());
                    if !known {
                        log::error!("emoji catalog: unknown category {}", c.id);
                    }
                    known
                })
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
        if !state.autosave.pending()
            && state.commands.is_empty()
            && state.layout == draft(&state.baseline)
            && state.baseline != *document.read().layout()
        {
            let baseline = document.read().layout().clone();
            let layout = draft(&baseline);
            let changed = state.layout.emoji_pages != layout.emoji_pages
                || state.layout.quick_action_pages != layout.quick_action_pages;
            state.baseline = baseline;
            state.layout = layout;
            if changed {
                refresh(ui, &state.layout, true);
            }
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
                    menus::ensure_pages(&mut state.layout);
                    ui.global::<MenuEditor>()
                        .set_has_unsaved_changes(menu_changed(&state));
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
        e.set_kind(kind);
        e.set_selected_page(0);
        if !menu_changed(&state) {
            reload(&ui, &doc, &mut state);
            e.set_status(Msg::None.to_ui());
        }
        e.set_selected_cell(initial_cell(kind, &state.layout, 0));
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
        e.set_selected_cell(initial_cell(e.get_kind(), &state.layout, index));
        refresh(&ui, &state.layout, true);
    });
    on!(ui, e.on_select_cell, shared = state => |ui, index| {
        ui.global::<MenuEditor>().set_selected_cell(index);
        refresh(&ui, &shared.borrow().layout, true);
    });

    let c = change.clone();
    e.on_add_page(move |name| {
        c(false, &|e, layout| {
            let kind = e.get_kind();
            let index = match kind {
                MenuKind::Emoji => {
                    layout.emoji_pages.push(EmojiPage {
                        id: lhc_core::profile::ids::generate("emoji-"),
                        name: name.trim().to_owned(),
                        cells: Default::default(),
                    });
                    layout.emoji_pages.len() - 1
                }
                MenuKind::Quick => {
                    layout.quick_action_pages.push(QuickActionPage::new(
                        lhc_core::profile::ids::generate("page-"),
                        name.trim().to_owned(),
                    ));
                    layout.quick_action_pages.len() - 1
                }
            };
            e.set_selected_page(index as i32);
            e.set_selected_cell(initial_cell(kind, layout, index as i32));
        });
    });
    let c = change.clone();
    e.on_remove_page(move || {
        c(false, &|e, layout| {
            let Ok(page) = usize::try_from(e.get_selected_page()) else {
                return;
            };
            let kind = e.get_kind();
            let selected = menus::remove_page(layout, core_kind(kind), page);
            e.set_selected_page(selected as i32);
            e.set_selected_cell(initial_cell(kind, layout, selected as i32));
        });
    });
    let c = change.clone();
    e.on_reorder_page(move |from, to| {
        c(false, &|e, layout| {
            let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
                return;
            };
            if menus::reorder_page(layout, core_kind(e.get_kind()), from, to) {
                let selected = e.get_selected_page().max(0) as usize;
                e.set_selected_page(menus::selection_after_reorder(selected, from, to) as i32);
            }
        });
    });
    let c = change.clone();
    e.on_move_cell(move |source_page, source, target| {
        c(false, &|e, layout| {
            let (Ok(source_page), Ok(source), Ok(target), Ok(target_page)) = (
                usize::try_from(source_page),
                usize::try_from(source),
                usize::try_from(target),
                usize::try_from(e.get_selected_page()),
            ) else {
                return;
            };
            if target >= PAGE {
                return;
            }
            menus::move_cell(
                layout,
                core_kind(e.get_kind()),
                (source_page, source),
                (target_page, target),
            );
            e.set_selected_cell(target as i32);
        });
    });
    let c = change.clone();
    e.on_rename_page(move || {
        c(true, &|e, layout| {
            if let Ok(page) = usize::try_from(e.get_selected_page()) {
                menus::rename_page(layout, core_kind(e.get_kind()), page, &e.get_page_name());
            }
        });
    });
    let c = change.clone();
    e.on_set_cell(move || {
        c(true, &|e, layout| {
            if e.get_kind() != MenuKind::Emoji {
                return;
            }
            if let Some((page, cell)) = selection(e, layout) {
                menus::set_emoji(layout, page, cell, &e.get_value());
            }
        });
    });
    let c = change.clone();
    e.on_set_name(move || {
        c(true, &|e, layout| {
            if e.get_kind() != MenuKind::Quick {
                return;
            }
            if let Some((page, cell)) = selection(e, layout) {
                menus::set_quick_action_name(layout, page, cell, &e.get_name());
            }
        });
    });
    on!(ui, e.on_set_action, doc = document, shared = state => |ui, cell, action, label| {
        if let Err(error) = choose(&ui, &doc, &shared, cell, &action, &label, None) {
            ui.global::<MenuEditor>().set_status(crate::notifications::report(&ui, &error));
        }
    });
    let c = change.clone();
    e.on_clear_cell(move || {
        c(false, &|e, layout| {
            let kind = e.get_kind();
            if let Some((page, cell)) = selection(e, layout) {
                menus::clear_cell(layout, core_kind(kind), page, cell);
                if kind == MenuKind::Quick {
                    e.set_selected_cell(-1);
                }
            }
        });
    });
    state
}

#[cfg(test)]
#[path = "menus/tests.rs"]
mod tests;
