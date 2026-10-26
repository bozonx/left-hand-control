//! Layout library page and the working copy toolbar.
//!
//! The working copy in memory is edited by the other pages;
//! the library holds named layouts. Library files are compared with the
//! contents last read so a change made elsewhere is never overwritten.

mod auto_rules;
mod working;
use auto_rules::{
    add_game_rule, delete_rule, load_rule, move_rule, save_rule, set_auto_default, set_rule_enabled,
};
pub use working::refresh_active;
use working::{overwrite_current, refresh_context, reset_context, save_as, save_current};

use super::{game_condition, game_condition_value, parse_list};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        AutoRuleView, GameCondition, LayoutLibrary, LayoutSource, LayoutSummary, LibraryAction,
        LibraryDialog, MenuKind, Page, SettingsWindow,
    },
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError},
    profile::{
        auto_switch::{self, AutoSwitchContext},
        bundled::BUNDLED,
        ids, layout_file,
        library::{LibrarySaveTarget, LibrarySource},
        model::{
            AppSettings, AutoRule, LayoutConditionSet, LayoutMode, LayoutPreset, user_layout_id,
            user_layout_name,
        },
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct State {
    /// Selected library layout and its file contents when it was selected.
    selected: Option<String>,
    known: Option<String>,
    /// Library layout the working copy came from and its saved contents.
    baseline: Option<(String, String)>,
    warning: Option<Msg>,
}

type Shared = Rc<RefCell<State>>;

fn io(error: String) -> Msg {
    Msg::Error(error)
}

fn config_error(error: ConfigError) -> Msg {
    Msg::from(&error)
}

/// Library name of the working copy, if it came from the library.
fn current_name(config: &ConfigDocument) -> Option<String> {
    config
        .settings()
        .current_layout_id
        .as_deref()
        .and_then(user_layout_name)
        .map(str::to_owned)
}

fn summary(set: &LayoutConditionSet) -> (GameCondition, String) {
    let list = [set.layouts.join(", "), set.apps.join(", ")]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (game_condition(set.game_mode.as_deref()), list)
}

fn condition(game: GameCondition, layouts: &str, apps: &str, excluded: &str) -> LayoutConditionSet {
    LayoutConditionSet {
        game_mode: game_condition_value(game),
        layouts: parse_list(layouts),
        apps: parse_list(apps),
        excluded_apps: parse_list(excluded),
    }
}

/// Show the library. Descriptions are read from the layout files.
fn refresh(ui: &SettingsWindow, document: &Document, state: &mut State) -> Result<(), Msg> {
    document.invalidate_library();
    let config = document.read();
    let library = ui.global::<LayoutLibrary>();
    let names: Vec<String> = config
        .ordered_layout_ids()
        .map_err(config_error)?
        .iter()
        .filter_map(|id| user_layout_name(id).map(str::to_owned))
        .collect();
    let presets: Vec<Option<LayoutPreset>> = names
        .iter()
        .map(|name| {
            config
                .paths()
                .load_user_layout(name)
                .ok()
                .and_then(|text| layout_file::parse(&text).ok().flatten())
        })
        .collect();
    library.set_descriptions(super::strings(presets.iter().map(|preset| {
        preset
            .as_ref()
            .and_then(|preset| preset.description.clone())
            .unwrap_or_default()
    })));
    library.set_summaries(ModelRc::new(VecModel::from(
        presets
            .into_iter()
            .map(|preset| {
                let preset = preset.unwrap_or_default();
                LayoutSummary {
                    rules: preset.rules.len() as i32,
                    layers: preset.layers.len() as i32,
                    emoji_pages: if preset
                        .emoji_pages
                        .iter()
                        .any(|page| page.cells.values().any(|value| !value.is_empty()))
                    {
                        preset.emoji_pages.len() as i32
                    } else {
                        0
                    },
                    action_pages: if preset
                        .quick_action_pages
                        .iter()
                        .any(|page| page.cells.values().any(|cell| !cell.action.is_empty()))
                    {
                        preset.quick_action_pages.len() as i32
                    } else {
                        0
                    },
                    macros: preset.macros.len() as i32,
                    commands: preset.used_commands() as i32,
                }
            })
            .collect::<Vec<_>>(),
    )));
    let selected = state
        .selected
        .as_ref()
        .and_then(|name| names.iter().position(|item| item == name));
    library.set_names(super::strings(names));
    library.set_selected(selected.map_or(-1, |index| index as i32));
    if selected.is_none() {
        state.selected = None;
        state.known = None;
    }
    drop(config);
    refresh_auto(ui, document);
    refresh_context(ui, document);
    Ok(())
}

/// Automatic mode: the rules, the default and rules per layout. Targets
/// index the `names` already shown.
fn refresh_auto(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let library = ui.global::<LayoutLibrary>();
    let settings = config.settings();
    library.set_automatic(settings.layout_mode == LayoutMode::Auto);
    let ids: Vec<String> = slint::Model::iter(&library.get_names())
        .map(|name| user_layout_id(&name))
        .collect();
    let target = |id: &Option<String>| -> (i32, bool) {
        match id {
            None => (-1, false),
            Some(id) => ids
                .iter()
                .position(|item| item == id)
                .map_or((-1, true), |index| (index as i32, false)),
        }
    };
    let rules = &settings.auto_rules;
    // Without layout detection every language would look unknown.
    let languages: Vec<String> = if rules.iter().any(|rule| !rule.conditions.layouts.is_empty()) {
        lhc_core::layout::available_layouts()
            .map(|layouts| layouts.into_iter().map(|layout| layout.short).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    library.set_auto_rules(ModelRc::new(VecModel::from(
        rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let (game, list) = summary(&rule.conditions);
                let (target, missing) = target(&rule.layout_id);
                let unknown: Vec<&str> = rule
                    .conditions
                    .layouts
                    .iter()
                    .filter(|code| !languages.is_empty() && !languages.contains(code))
                    .map(String::as_str)
                    .collect();
                AutoRuleView {
                    game,
                    list: list.into(),
                    excluded: rule.conditions.excluded_apps.join(", ").into(),
                    target,
                    missing,
                    enabled: rule.enabled,
                    shadowed_by: auto_switch::shadowed_by(rules, index)
                        .map_or(-1, |index| index as i32),
                    unknown_languages: unknown.join(", ").into(),
                }
            })
            .collect::<Vec<_>>(),
    )));
    let (default, default_missing) = target(&settings.auto_default_layout_id);
    library.set_auto_default(default);
    library.set_auto_default_missing(default_missing);
    library.set_rule_counts(ModelRc::new(VecModel::from(
        ids.iter()
            .map(|id| {
                rules
                    .iter()
                    .filter(|rule| rule.layout_id.as_ref() == Some(id))
                    .count() as i32
            })
            .collect::<Vec<_>>(),
    )));
    drop(config);
    refresh_active(ui, document);
}

/// Label and dirty flag of the working copy: it differs from its library
/// file. Both sides go through the same parser, so formatting differences
/// in the file do not count as changes.
fn report(ui: &SettingsWindow, state: &Shared, result: Result<Msg, Msg>) {
    let warning = state.borrow_mut().warning.take();
    let message = match result {
        Ok(message) => warning.unwrap_or(message),
        Err(error) => {
            if let Some(warning) = warning {
                crate::notifications::send(ui, warning);
            }
            error
        }
    };
    if matches!(
        message,
        Msg::LayoutCreated(_)
            | Msg::LayoutSaved(_)
            | Msg::LayoutDeleted(_)
            | Msg::AutoRuleDeleted
            | Msg::LayoutActivated(_)
            | Msg::LayoutDiscarded(_)
    ) {
        ui.global::<LayoutLibrary>().set_status(Msg::None.to_ui());
        crate::notifications::send(ui, message);
    } else {
        ui.global::<LayoutLibrary>()
            .set_status(crate::notifications::report(ui, &message));
    }
}

fn selected_name(state: &Shared) -> Result<String, Msg> {
    state.borrow().selected.clone().ok_or(Msg::LibrarySelect)
}

/// Run a library change and refresh the page.
fn edit<T>(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    change: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
) -> Result<T, Msg> {
    let saved = document.edit(View::Library, change).map_err(|error| {
        let _ = refresh(ui, document, &mut state.borrow_mut());
        match error {
            ConfigError::EditConflict => Msg::LibraryChanged,
            other => config_error(other),
        }
    })?;
    let refreshed = refresh(ui, document, &mut state.borrow_mut());
    let (value, warning) = saved.into_parts();
    let mut state = state.borrow_mut();
    state.warning = state.warning.take().or(warning).or_else(|| refreshed.err());
    Ok(value)
}

/// Change automatic-mode settings and refresh only the rules panel, so
/// the library list (and an open picker over it) is not rebuilt.
fn edit_auto(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    change: impl FnOnce(&mut AppSettings),
) -> Result<Msg, Msg> {
    let saved = document
        .edit(View::Library, |config| config.update_settings(change))
        .map_err(|error| {
            let _ = refresh(ui, document, &mut state.borrow_mut());
            match error {
                ConfigError::EditConflict => Msg::LibraryChanged,
                other => config_error(other),
            }
        })?;
    refresh_auto(ui, document);
    Ok(saved.message(Msg::None))
}

fn select(ui: &SettingsWindow, document: &Document, state: &Shared, index: i32) -> Result<(), Msg> {
    let library = ui.global::<LayoutLibrary>();
    let name = usize::try_from(index)
        .ok()
        .and_then(|index| slint::Model::row_data(&library.get_names(), index))
        .ok_or(Msg::LibrarySelect)?
        .to_string();
    let config = document.read();
    let text = config.paths().load_user_layout(&name).map_err(io)?;
    let layout = layout_file::parse(&text)
        .map_err(io)?
        .ok_or(Msg::LayoutEmpty)?;
    {
        let mut state = state.borrow_mut();
        state.selected = Some(name.clone());
        state.known = Some(text);
    }
    library.set_selected(index);
    library.set_name(name.into());
    library.set_description(layout.description.unwrap_or_default().into());
    Ok(())
}

/// Dialog fields: game condition, languages, applications, exclusions.
fn condition_fields(set: Option<LayoutConditionSet>) -> (GameCondition, String, String, String) {
    set.map(|set| {
        (
            game_condition(set.game_mode.as_deref()),
            super::condition_list(&set.layouts),
            super::condition_list(&set.apps),
            super::condition_list(&set.excluded_apps),
        )
    })
    .unwrap_or_default()
}

/// Selected file must still be what we read when it was selected.
fn check_unchanged(document: &Document, state: &Shared, name: &str) -> Result<String, Msg> {
    let current = document.read().paths().load_user_layout(name).map_err(io)?;
    if Some(&current) != state.borrow().known.as_ref() {
        return Err(Msg::LibraryChanged);
    }
    Ok(current)
}

fn library_action(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    action: LibraryAction,
) -> Result<Msg, Msg> {
    let library = ui.global::<LayoutLibrary>();
    let name = selected_name(state)?;
    let id = user_layout_id(&name);
    match action {
        LibraryAction::Activate => {
            edit(ui, document, state, |config| {
                config.update_settings(|settings| settings.manual_active_layout_id = Some(id))
            })?;
        }
        LibraryAction::MoveUp | LibraryAction::MoveDown => {
            let delta = if action == LibraryAction::MoveUp {
                -1
            } else {
                1
            };
            edit(ui, document, state, |config| {
                config.move_library_layout(&id, delta)
            })?;
        }
        LibraryAction::SaveDetails => {
            let expected = state.borrow().known.clone().ok_or(Msg::LibrarySelect)?;
            let new_name = library.get_name().to_string();
            let description = library.get_description().to_string();
            let saved = edit(ui, document, state, |config| {
                config.edit_library_metadata(&name, &new_name, &description, &expected)
            })?;
            let text = document
                .read()
                .paths()
                .load_user_layout(&saved)
                .map_err(io)?;
            {
                let mut state = state.borrow_mut();
                state.selected = Some(saved);
                state.known = Some(text);
            }
            refresh(ui, document, &mut state.borrow_mut())?;
            reset_context(ui, document, &mut state.borrow_mut());
        }
    }
    if action == LibraryAction::SaveDetails {
        library.set_dialog(LibraryDialog::None);
    }
    Ok(if action == LibraryAction::Activate {
        Msg::LayoutActivated(name)
    } else {
        Msg::None
    })
}

/// Replace the working copy with the selected layout and open its rules.
fn load(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let name = selected_name(state)?;
    let is_current = current_name(&document.read()).as_deref() == Some(name.as_str());
    if !is_current {
        edit(ui, document, state, |config| {
            config.load_library_for_editing(&name)
        })?;
        reset_context(ui, document, &mut state.borrow_mut());
    }
    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
    Ok(Msg::None)
}

fn create(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    name: &str,
    description: &str,
    source: LayoutSource,
    copy_index: i32,
) -> Result<Msg, Msg> {
    let library = ui.global::<LayoutLibrary>();
    let copy_from = usize::try_from(copy_index)
        .ok()
        .and_then(|index| slint::Model::row_data(&library.get_names(), index));
    let source = match source {
        LayoutSource::Bundled => LibrarySource::Bundled(
            usize::try_from(copy_index)
                .ok()
                .and_then(|index| BUNDLED.get(index))
                .ok_or(Msg::LibrarySelect)?,
        ),
        LayoutSource::Copy => LibrarySource::Copy(copy_from.as_deref().ok_or(Msg::LibrarySelect)?),
        LayoutSource::Empty => LibrarySource::Empty,
    };
    let activate_first =
        slint::Model::row_count(&library.get_names()) == 0 && !library.get_automatic();
    let created = edit(ui, document, state, |config| {
        let created = config.create_library_layout(name, description, source)?;
        if activate_first {
            config.update_settings(|settings| {
                settings.manual_active_layout_id = Some(user_layout_id(&created));
            })?;
        }
        Ok(created)
    })?;
    let index = slint::Model::iter(&library.get_names())
        .position(|item| item == created)
        .ok_or(Msg::LibrarySelect)?;
    library.set_dialog(LibraryDialog::None);
    select(ui, document, state, index as i32)?;
    library.invoke_edit(index as i32);
    Ok(Msg::LayoutCreated(created))
}

fn delete(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let name = selected_name(state)?;
    let current = check_unchanged(document, state, &name)?;
    edit(ui, document, state, |config| {
        config.remove_library_layout(&name, &current)
    })?;
    reset_context(ui, document, &mut state.borrow_mut());
    ui.global::<LayoutLibrary>().set_dialog(LibraryDialog::None);
    Ok(Msg::LayoutDeleted(name))
}

fn set_description(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
    description: &str,
) -> Result<Msg, Msg> {
    let library = ui.global::<LayoutLibrary>();
    let name = usize::try_from(index)
        .ok()
        .and_then(|index| slint::Model::row_data(&library.get_names(), index))
        .ok_or(Msg::LibrarySelect)?
        .to_string();
    let text = document
        .read()
        .paths()
        .load_user_layout(&name)
        .map_err(io)?;
    edit(ui, document, state, |config| {
        config.edit_library_metadata(&name, &name, description, &text)
    })?;
    if state.borrow().selected.as_deref() == Some(name.as_str()) {
        let text = document
            .read()
            .paths()
            .load_user_layout(&name)
            .map_err(io)?;
        state.borrow_mut().known = Some(text);
        library.set_description(description.trim().into());
    }
    reset_context(ui, document, &mut state.borrow_mut());
    Ok(Msg::None)
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let state: Shared = Rc::default();
    reset_context(ui, document, &mut state.borrow_mut());
    let library = ui.global::<LayoutLibrary>();
    library.set_bundled_names(super::strings(
        BUNDLED.iter().map(|bundled| bundled.display_name()),
    ));

    let result = refresh(ui, document, &mut state.borrow_mut()).map(|()| Msg::None);
    report(ui, &state, result);

    let weak = ui.as_weak();
    // Other pages change the working copy: only the toolbar depends on it.
    // The list itself is re-read when the library page is opened.
    document.subscribe(View::Library, move |document| {
        if let Some(ui) = weak.upgrade() {
            refresh_context(&ui, document);
            refresh_active(&ui, document);
        }
    });

    on!(ui, library.on_refresh, doc = document, state = state => |ui| {
        let result = refresh(&ui, &doc, &mut state.borrow_mut()).map(|()| Msg::None);
        report(&ui, &state, result);
    });
    on!(ui, library.on_select, doc = document, state = state => |ui, index| {
        let (doc, state) = (&doc, &state);
        if let Err(error) = select(&ui, doc, state, index) {
            report(&ui, state, Err(error));
        }
    });
    on!(ui, library.on_set_automatic, doc = document, state = state => |ui, automatic| {
        let (doc, state) = (&doc, &state);
        let mode = if automatic {
            LayoutMode::Auto
        } else {
            LayoutMode::Manual
        };
        report(
            &ui, state,
            edit_auto(&ui, doc, state, |settings| {
                auto_switch::set_mode(settings, mode)
            }),
        );
    });
    on!(ui, library.on_action, doc = document, state = state => |ui, action| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        library_action(&ui, doc, state, action)
    )
    });
    on!(ui, library.on_suggest_name, doc = document => |_ui, base| {
        let (doc,) = (&doc,);
        doc
        .read()
        .unique_library_name(&base)
        .unwrap_or_else(|_| base.to_string())
        .into()
    });
    on!(ui, library.on_name_error, doc = document => |_ui, name, except| {
        let (doc,) = (&doc,);
        let except = (!except.is_empty()).then_some(except.as_str());
        super::name_error(doc.read().checked_layout_name(&name, except))
    });
    on!(ui, library.on_create, doc = document, state = state => |ui, name, description, source, copy_index| {
        let (doc, state) = (&doc, &state);
        let result = create(&ui, doc, state, &name, &description, source, copy_index);
        report(&ui, state, result);
    });
    on!(ui, library.on_set_description, doc = document, state = state => |ui, index, description| {
        let (doc, state) = (&doc, &state);
        report(&ui, state, set_description(&ui, doc, state, index, &description))
    });
    on!(ui, library.on_rename, doc = document, state = state => |ui, index, name| {
        let (doc, state) = (&doc, &state);
        let result = select(&ui, doc, state, index).and_then(|()| {
            ui.global::<LayoutLibrary>().set_name(name);
            library_action(&ui, doc, state, LibraryAction::SaveDetails)
        });
        report(&ui, state, result);
    });
    on!(ui, library.on_reorder, doc = document, state = state => |ui, from, to| {
        let (doc, state) = (&doc, &state);
        // Moving a card must not select or load it: take the name from the list.
        let names = ui.global::<LayoutLibrary>().get_names();
        let result = match super::move_indices(from, to, slint::Model::row_count(&names))
            .and_then(|(from, _)| slint::Model::row_data(&names, from))
        {
            Some(name) => {
                let id = user_layout_id(&name);
                edit(&ui, doc, state, |config| config.move_library_layout(&id, to - from))
                    .map(|()| Msg::None)
            }
            None => Ok(Msg::None),
        };
        report(&ui, state, result);
    });
    on!(ui, library.on_discard_current, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        let name = current_name(&doc.read()).ok_or(Msg::LibrarySelect);
        let result = name.and_then(|name| {
            edit(&ui, doc, state, |config| {
                config.load_library_for_editing(&name)
            })?;
            reset_context(&ui, doc, &mut state.borrow_mut());
            Ok(Msg::LayoutDiscarded(name))
        });
        report(&ui, state, result);
    });
    on!(ui, library.on_activate_current, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        let result = doc
            .read()
            .settings()
            .current_layout_id
            .clone()
            .ok_or(Msg::LibrarySelect)
            .and_then(|id| {
                edit(&ui, doc, state, |config| {
                    config.update_settings(|settings| settings.manual_active_layout_id = Some(id))
                })
            });
        let name = current_name(&doc.read()).unwrap_or_default();
        report(&ui, state, result.map(|()| Msg::LayoutActivated(name)));
    });
    on!(ui, library.on_load, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        report(&ui, state, load(&ui, doc, state))
    });
    on!(ui, library.on_load_rule, doc = document => |ui, index, target| {
        let (doc,) = (&doc,);
        load_rule(
        &ui, doc, index, target
    )
    });
    on!(ui, library.on_save_rule, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        save_rule(&ui, doc, state)
    )
    });
    on!(ui, library.on_delete_rule, doc = document, state = state => |ui, index| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        delete_rule(&ui, doc, state, index)
    )
    });
    on!(ui, library.on_move_rule, doc = document, state = state => |ui, from, to| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        move_rule(&ui, doc, state, from, to)
    )
    });
    on!(ui, library.on_set_rule_enabled, doc = document, state = state => |ui, index, enabled| {
        let (doc, state) = (&doc, &state);
        report(
            &ui, state,
            set_rule_enabled(&ui, doc, state, index, enabled)
        )
    });
    on!(ui, library.on_add_game_rule, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        add_game_rule(&ui, doc, state)
    )
    });
    on!(ui, library.on_set_auto_default, doc = document, state = state => |ui, index| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        set_auto_default(&ui, doc, state, index)
    )
    });
    on!(ui, library.on_delete, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        delete(&ui, doc, state)
    )
    });
    on!(ui, library.on_reset_context, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        reset_context(
        &ui,
        doc,
        &mut state.borrow_mut()
    )
    });
    on!(ui, library.on_refresh_context, doc = document => |ui| {
        let (doc,) = (&doc,);
        refresh_context(
        &ui, doc
    )
    });
    on!(ui, library.on_save_current, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        save_current(&ui, doc, state)
    )
    });
    on!(ui, library.on_overwrite_current, doc = document, state = state => |ui| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        overwrite_current(&ui, doc, state)
    )
    });
    on!(ui, library.on_save_as, doc = document, state = state => |ui, name| {
        let (doc, state) = (&doc, &state);
        report(
        &ui, state,
        save_as(&ui, doc, state, &name)
    )
    });
}

#[cfg(test)]
#[path = "library/tests.rs"]
mod tests;
