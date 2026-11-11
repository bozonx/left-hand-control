//! Emoji menu and quick action menu of the profile.
//!
//! The page edits a draft of the menus. Changes save as soon as the draft
//! is valid; typing saves after a short pause.

use super::autosave::Autosave;
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{EmojiCategory, MenuCell, MenuEditor, MenuKind, SettingsWindow},
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};
use tecloid_core::config::{menus, model::*};

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
    baseline: ProfilePreset,
    profile: ProfilePreset,
    autosave: Autosave,
    commands: Vec<Command>,
}

type Shared = Rc<RefCell<State>>;

impl super::autosave::Draft for State {
    fn autosave(&self) -> &Autosave {
        &self.autosave
    }

    fn save(&mut self, ui: &SettingsWindow, document: &Document) {
        let _ = save(ui, document, self);
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

/// The draft of a profile: both menus always have a page.
fn draft(profile: &ProfilePreset) -> ProfilePreset {
    let mut profile = profile.clone();
    menus::ensure_pages(&mut profile);
    profile
}

/// Index of the first assigned quick action on `page`, or -1.
fn first_filled(profile: &ProfilePreset, page: usize) -> i32 {
    profile
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
fn initial_cell(kind: MenuKind, profile: &ProfilePreset, page: i32) -> i32 {
    match kind {
        MenuKind::Emoji => 0,
        MenuKind::Quick => usize::try_from(page).map_or(-1, |page| first_filled(profile, page)),
    }
}

/// Selected page and cell as indexes, if they point into the draft.
fn selection(e: &MenuEditor, profile: &ProfilePreset) -> Option<(usize, usize)> {
    let page = usize::try_from(e.get_selected_page()).ok()?;
    let cell = usize::try_from(e.get_selected_cell()).ok()?;
    (page < menus::page_count(profile, core_kind(e.get_kind())) && cell < PAGE)
        .then_some((page, cell))
}

/// Push the draft to the UI. `fields` also resets the bound text fields,
/// which must not happen while the user types into them.
fn refresh(ui: &SettingsWindow, profile: &ProfilePreset, fields: bool) {
    let e = ui.global::<MenuEditor>();
    let kind = e.get_kind();
    let names: Vec<String> = match kind {
        MenuKind::Emoji => profile.emoji_pages.iter().map(|p| p.name.clone()).collect(),
        MenuKind::Quick => profile
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
            let page = &profile.emoji_pages[p];
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
            let page = &profile.quick_action_pages[p];
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
        || state.profile.emoji_pages != state.baseline.emoji_pages
        || state.profile.quick_action_pages != state.baseline.quick_action_pages
}

/// Persist the menu draft independently of the Slint editor.
fn persist(
    document: &Document,
    state: &mut State,
) -> Result<Option<crate::document::Saved<()>>, tecloid_core::config_document::ConfigError> {
    state.commands.retain(|command| {
        state
            .profile
            .quick_action_pages
            .iter()
            .flat_map(|page| page.cells.values())
            .any(|cell| {
                matches!(tecloid_core::config::actions::Action::parse(Some(&cell.action)),
                tecloid_core::config::actions::Action::Command(id) if id == command.id)
            })
    });
    let baseline = state.baseline.clone();
    let candidate = state.profile.clone();
    if !menu_changed(state) {
        return Ok(None);
    }
    let result = document.edit(View::Menus, |config| {
        config.edit_profile_with_commands(state.commands.clone(), |config| {
            config.save_menu_pages(&baseline, &candidate)
        })
    });
    if result.is_ok() {
        state.commands.clear();
        state.baseline = document.read().profile().clone();
        state.profile = draft(&state.baseline);
    }
    result.map(Some)
}

fn save(ui: &SettingsWindow, document: &Document, state: &mut State) -> Result<(), Msg> {
    state.autosave.cancel();
    let result = persist(document, state);
    let e = ui.global::<MenuEditor>();
    e.set_has_unsaved_changes(menu_changed(state));
    let outcome = result.as_ref().map(|_| ()).map_err(Msg::from);
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
    outcome
}

fn reload(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.autosave.cancel();
    state.commands.clear();
    ui.global::<MenuEditor>().set_preview_visible(false);
    ui.global::<MenuEditor>().invoke_cancel_mark();
    state.baseline = document.read().profile().clone();
    state.profile = draft(&state.baseline);
    ui.global::<MenuEditor>().set_has_unsaved_changes(false);
    refresh(ui, &state.profile, true);
}

pub(super) fn command(state: &RefCell<State>, id: &str) -> Option<Command> {
    state
        .borrow()
        .commands
        .iter()
        .find(|command| command.id == id)
        .cloned()
}

fn selection_for_quick(profile: &ProfilePreset, page: i32, cell: i32) -> Option<&QuickAction> {
    profile
        .quick_action_pages
        .get(usize::try_from(page).ok()?)?
        .cells
        .get(*LEFT_HAND_HOTKEYS.get(usize::try_from(cell).ok()?)?)
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
    let previous_label = selection_for_quick(&state.profile, e.get_selected_page(), cell)
        .map(|item| {
            let settings = document.read().settings().clone();
            let mut config = ResolvedAppConfig::from_parts(
                settings.clone(),
                state.profile.clone(),
                settings.editing_profile_id.as_deref(),
            );
            for command in &state.commands {
                super::stage_command(&mut config.commands, command.clone());
            }
            super::picker::menu_label(ui, &config, &item.action)
        })
        .unwrap_or_default();
    tecloid_core::config::menus::set_quick_action(
        &mut state.profile,
        e.get_selected_page(),
        cell,
        action,
        label,
        &previous_label,
    )
    .map_err(|error| Msg::from(&error))?;
    e.set_selected_cell(if action.is_empty() { -1 } else { cell });
    if let Some(command) = command {
        super::stage_command(&mut state.commands, command);
    }
    let result = save(ui, document, &mut state);
    refresh(ui, &state.profile, true);
    result
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) -> Shared {
    let baseline = document.read().profile().clone();
    let profile = draft(&baseline);
    let state: Shared = Rc::new(RefCell::new(State {
        baseline,
        profile,
        autosave: Autosave::default(),
        commands: Vec::new(),
    }));
    let e = ui.global::<MenuEditor>();
    on!(ui, e.on_flush_pending, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        let _ = save(&ui, &doc, &mut state);
        !menu_changed(&state)
    });
    on!(ui, e.on_show_page, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        if save(&ui, &doc, &mut state).is_err() {
            return false;
        }
        refresh(&ui, &state.profile, true);
        true
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
            && state.profile == draft(&state.baseline)
            && state.baseline != *document.read().profile()
        {
            let baseline = document.read().profile().clone();
            let profile = draft(&baseline);
            let changed = state.profile.emoji_pages != profile.emoji_pages
                || state.profile.quick_action_pages != profile.quick_action_pages;
            state.baseline = baseline;
            state.profile = profile;
            if changed {
                ui.global::<MenuEditor>().invoke_cancel_mark();
                refresh(ui, &state.profile, true);
            }
        }
    });

    // Run `edit` on the draft, save it, and push the result to the UI.
    let change = {
        let weak = ui.as_weak();
        let (doc, shared) = (document.clone(), state.clone());
        Rc::new(
            move |typing: bool, edit: &dyn Fn(&MenuEditor, &mut ProfilePreset)| {
                let Some(ui) = weak.upgrade() else { return };
                {
                    let mut state = shared.borrow_mut();
                    edit(&ui.global::<MenuEditor>(), &mut state.profile);
                    menus::ensure_pages(&mut state.profile);
                    ui.global::<MenuEditor>()
                        .set_has_unsaved_changes(menu_changed(&state));
                    if !typing {
                        let _ = save(&ui, &doc, &mut state);
                    }
                    refresh(&ui, &state.profile, !typing);
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
        e.set_preview_visible(false);
        e.set_kind(kind);
        e.set_selected_page(0);
        if !menu_changed(&state) {
            reload(&ui, &doc, &mut state);
            e.set_status(Msg::None.to_ui());
        }
        e.set_selected_cell(initial_cell(kind, &state.profile, 0));
        refresh(&ui, &state.profile, true);
    });

    on!(ui, e.on_refresh, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        state.autosave.cancel();
        reload(&ui, &doc, &mut state);
        ui.global::<MenuEditor>().set_status(Msg::None.to_ui());
        refresh(&ui, &state.profile, true);
    });

    on!(ui, e.on_select_page, shared = state => |ui, index| {
        let state = shared.borrow();
        let e = ui.global::<MenuEditor>();
        e.set_selected_page(index);
        e.set_selected_cell(initial_cell(e.get_kind(), &state.profile, index));
        refresh(&ui, &state.profile, true);
    });
    on!(ui, e.on_select_cell, shared = state => |ui, index| {
        ui.global::<MenuEditor>().set_selected_cell(index);
        refresh(&ui, &shared.borrow().profile, true);
    });

    let c = change.clone();
    e.on_add_page(move |name| {
        c(false, &|e, profile| {
            let kind = e.get_kind();
            let index = match kind {
                MenuKind::Emoji => {
                    profile.emoji_pages.push(EmojiPage {
                        id: tecloid_core::config::ids::generate("emoji-"),
                        name: name.trim().to_owned(),
                        cells: Default::default(),
                    });
                    profile.emoji_pages.len() - 1
                }
                MenuKind::Quick => {
                    profile.quick_action_pages.push(QuickActionPage::new(
                        tecloid_core::config::ids::generate("page-"),
                        name.trim().to_owned(),
                    ));
                    profile.quick_action_pages.len() - 1
                }
            };
            e.set_selected_page(index as i32);
            e.set_selected_cell(initial_cell(kind, profile, index as i32));
        });
    });
    let c = change.clone();
    e.on_remove_page(move || {
        c(false, &|e, profile| {
            let Ok(page) = usize::try_from(e.get_selected_page()) else {
                return;
            };
            let kind = e.get_kind();
            e.invoke_cancel_mark();
            let selected = menus::remove_page(profile, core_kind(kind), page);
            e.set_selected_page(selected as i32);
            e.set_selected_cell(initial_cell(kind, profile, selected as i32));
        });
    });
    let c = change.clone();
    e.on_reorder_page(move |from, to| {
        c(false, &|e, profile| {
            let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
                return;
            };
            if menus::reorder_page(profile, core_kind(e.get_kind()), from, to) {
                e.invoke_cancel_mark();
                let selected = e.get_selected_page().max(0) as usize;
                e.set_selected_page(menus::selection_after_reorder(selected, from, to) as i32);
            }
        });
    });
    let c = change.clone();
    e.on_move_cell(move |source_page, source, target| {
        c(false, &|e, profile| {
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
                profile,
                core_kind(e.get_kind()),
                (source_page, source),
                (target_page, target),
            );
            e.set_selected_cell(target as i32);
        });
    });
    let c = change.clone();
    e.on_copy_cell(move |source_page, source, target| {
        c(false, &|e, profile| {
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
            menus::copy_cell(
                profile,
                core_kind(e.get_kind()),
                (source_page, source),
                (target_page, target),
            );
            e.set_selected_cell(target as i32);
        });
    });
    let c = change.clone();
    e.on_rename_page(move || {
        c(true, &|e, profile| {
            if let Ok(page) = usize::try_from(e.get_selected_page()) {
                menus::rename_page(profile, core_kind(e.get_kind()), page, &e.get_page_name());
            }
        });
    });
    let c = change.clone();
    e.on_set_cell(move || {
        c(true, &|e, profile| {
            if e.get_kind() != MenuKind::Emoji {
                return;
            }
            if let Some((page, cell)) = selection(e, profile) {
                menus::set_emoji(profile, page, cell, &e.get_value());
            }
        });
    });
    let c = change.clone();
    e.on_set_name(move || {
        c(true, &|e, profile| {
            if e.get_kind() != MenuKind::Quick {
                return;
            }
            if let Some((page, cell)) = selection(e, profile) {
                menus::set_quick_action_name(profile, page, cell, &e.get_name());
            }
        });
    });
    on!(ui, e.on_set_action, doc = document, shared = state => |ui, cell, action, label| {
        if let Err(error) = choose(&ui, &doc, &shared, cell, &action, &label, None) {
            let e = ui.global::<MenuEditor>();
            e.set_status(crate::notifications::report_changed(&ui, &error, e.get_status()));
        }
    });
    let c = change.clone();
    e.on_clear_cell(move || {
        c(false, &|e, profile| {
            let kind = e.get_kind();
            if let Some((page, cell)) = selection(e, profile) {
                menus::clear_cell(profile, core_kind(kind), page, cell);
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
