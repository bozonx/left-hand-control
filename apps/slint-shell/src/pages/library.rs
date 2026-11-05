//! Layout library page and the working copy toolbar.
//!
//! The working copy in memory is edited by the other pages;
//! the library holds named layouts. Library files are compared with the
//! contents last read so a change made elsewhere is never overwritten.

mod auto_rules;
mod metadata;
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
        AutoRuleView, GameCondition, LayoutLibrary, LayoutRow, LayoutSource, LibraryAction,
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
    /// Library layout whose menu or dialog is open.
    selected: Option<String>,
    /// Library layout the working copy came from and its saved contents.
    baseline: Option<(String, String)>,
    warning: Option<Msg>,
    /// System languages for rule warnings; re-read when the page is opened.
    languages: Option<Vec<String>>,
    /// Names the open rule dialog's target index refers to.
    rule_names: Vec<String>,
    rule_id: Option<String>,
    rule_missing_target: Option<String>,
    metadata: metadata::Loader,
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
        .editing_layout_id
        .as_deref()
        .and_then(user_layout_name)
        .map(str::to_owned)
}

fn summary(set: &LayoutConditionSet) -> (GameCondition, String) {
    let list = [set.layouts.join(", "), set.window_patterns.join(", ")]
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
        window_patterns: parse_list(apps),
        excluded_window_patterns: parse_list(excluded),
    }
}

/// Publish the ordered names immediately; file metadata arrives in the background.
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
    let previous = library.get_rows();
    let same_names = slint::Model::iter(&previous)
        .map(|row| row.name.to_string())
        .eq(names.iter().cloned());
    if !same_names {
        library.set_rows(ModelRc::new(VecModel::from(
            names
                .iter()
                .map(|name| {
                    slint::Model::iter(&previous)
                        .find(|row| row.name == name.as_str())
                        .unwrap_or_else(|| LayoutRow {
                            id: user_layout_id(name).into(),
                            name: name.clone().into(),
                            ..Default::default()
                        })
                })
                .collect::<Vec<_>>(),
        )));
    }
    let paths = config.paths().clone();
    state.metadata.load(ui, paths, names.clone());
    let selected = state
        .selected
        .as_ref()
        .and_then(|name| names.iter().position(|item| item == name));
    library.set_selected(selected.map_or(-1, |index| index as i32));
    if selected.is_none() {
        state.selected = None;
    }
    drop(config);
    refresh_auto(ui, document, state);
    refresh_context(ui, document);
    Ok(())
}

/// Automatic mode: the rules, the default and rules per layout. Targets
/// index the `names` already shown.
fn refresh_auto(ui: &SettingsWindow, document: &Document, state: &mut State) {
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
    // Querying the system is slow, so it happens once per page visit.
    let languages: &[String] = if rules.iter().any(|rule| !rule.conditions.layouts.is_empty()) {
        state.languages.get_or_insert_with(|| {
            lhc_core::keyboard_layout::available_layouts()
                .map(|layouts| layouts.into_iter().map(|layout| layout.code).collect())
                .unwrap_or_default()
        })
    } else {
        &[]
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
                    excluded: rule.conditions.excluded_window_patterns.join(", ").into(),
                    target,
                    missing,
                    enabled: rule.enabled,
                    shadowed_by: auto_switch::shadowed_by(rules, index, &ids)
                        .map_or(-1, |index| index as i32),
                    unknown_languages: unknown.join(", ").into(),
                }
            })
            .collect::<Vec<_>>(),
    )));
    let (default, default_missing) = target(&settings.auto_default_layout_id);
    library.set_auto_default(default);
    library.set_auto_default_missing(default_missing);
    let rows = library.get_rows();
    for (index, id) in ids.iter().enumerate() {
        if let Some(mut row) = slint::Model::row_data(&rows, index) {
            let count = rules
                .iter()
                .filter(|rule| rule.layout_id.as_ref() == Some(id))
                .count() as i32;
            if row.rule_count != count {
                row.rule_count = count;
                slint::Model::set_row_data(&rows, index, row);
            }
        }
    }
    drop(config);
    refresh_active(ui, document);
}

/// Combine an action result with a warning raised along the way: returns
/// an extra notification, if any, and the message to report.
fn merge_warning(result: Result<Msg, Msg>, warning: Option<Msg>) -> (Option<Msg>, Msg) {
    match (result, warning) {
        (Ok(message), Some(warning)) if message.is_none() => (None, warning),
        (Ok(message) | Err(message), warning) => (warning, message),
    }
}

/// Show an action result: success notices as notifications, anything else
/// in the page status.
fn report(ui: &SettingsWindow, state: &Shared, result: Result<Msg, Msg>) {
    let warning = state.borrow_mut().warning.take();
    let (extra, message) = merge_warning(result, warning);
    if let Some(extra) = extra {
        crate::notifications::send(ui, extra);
    }
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

/// Library name shown at `index`.
fn name_at(ui: &SettingsWindow, index: i32) -> Result<String, Msg> {
    usize::try_from(index)
        .ok()
        .and_then(|index| slint::Model::row_data(&ui.global::<LayoutLibrary>().get_names(), index))
        .map(|name| name.to_string())
        .ok_or(Msg::LibrarySelect)
}

/// File bytes of `name` as the user last saw it; read now if the card's
/// metadata has not arrived yet.
fn expected_bytes(document: &Document, state: &Shared, name: &str) -> Result<Vec<u8>, Msg> {
    match state.borrow().metadata.known(name) {
        Some(bytes) => Ok(bytes),
        None => document
            .read()
            .paths()
            .load_user_layout_bytes(name)
            .map_err(io),
    }
}

fn expected_text(document: &Document, state: &Shared, name: &str) -> Result<String, Msg> {
    String::from_utf8(expected_bytes(document, state, name)?)
        .map_err(|error| io(format!("layout UTF-8: {error}")))
}

/// Re-read a file this page has just written and remember it.
fn remember_saved(document: &Document, state: &Shared, name: &str) {
    match document.read().paths().load_user_layout_bytes(name) {
        Ok(bytes) => state.borrow().metadata.remember(name, Some(bytes)),
        Err(error) => {
            log::warn!("re-reading layout {name}: {error}");
            state.borrow().metadata.remember(name, None);
        }
    }
}

fn refresh_logged(ui: &SettingsWindow, document: &Document, state: &Shared) {
    if let Err(error) = refresh(ui, document, &mut state.borrow_mut()) {
        log::warn!("refreshing the layout library: {error:?}");
    }
}

/// Run a library change and refresh the page.
fn edit<T>(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    change: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
) -> Result<T, Msg> {
    let saved = document.edit_library(change).map_err(|error| {
        refresh_logged(ui, document, state);
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
            refresh_logged(ui, document, state);
            match error {
                ConfigError::EditConflict => Msg::LibraryChanged,
                other => config_error(other),
            }
        })?;
    refresh_auto(ui, document, &mut state.borrow_mut());
    Ok(saved.message(Msg::None))
}

/// Remember which layout a menu or dialog acts on. Reads no file, so a
/// broken layout can be selected (and deleted) like any other.
fn select(ui: &SettingsWindow, state: &Shared, index: i32) -> Result<(), Msg> {
    let name = name_at(ui, index)?;
    state.borrow_mut().selected = Some(name);
    ui.global::<LayoutLibrary>().set_selected(index);
    Ok(())
}

/// Dialog fields: game condition, languages, applications, exclusions.
fn condition_fields(set: Option<LayoutConditionSet>) -> (GameCondition, String, String, String) {
    set.map(|set| {
        (
            game_condition(set.game_mode.as_deref()),
            super::condition_list(&set.layouts),
            super::condition_list(&set.window_patterns),
            super::condition_list(&set.excluded_window_patterns),
        )
    })
    .unwrap_or_default()
}

fn library_action(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
    action: LibraryAction,
) -> Result<Msg, Msg> {
    let name = name_at(ui, index)?;
    let id = user_layout_id(&name);
    match action {
        LibraryAction::Activate => {
            edit(ui, document, state, |config| {
                config.update_settings(|settings| settings.manual_layout_id = Some(id))
            })?;
            Ok(Msg::LayoutActivated(name))
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
            Ok(Msg::None)
        }
    }
}

/// Replace the working copy with layout `name` and open its rules. The
/// caller has already asked about unsaved changes.
fn load(ui: &SettingsWindow, document: &Document, state: &Shared, name: &str) -> Result<Msg, Msg> {
    let is_current = current_name(&document.read()).as_deref() == Some(name);
    if !is_current {
        edit(ui, document, state, |config| {
            config.load_library_for_editing(name)
        })?;
        reset_context(ui, document, &mut state.borrow_mut());
    }
    ui.global::<LayoutLibrary>().invoke_discard_drafts();
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
    let copy_from = name_at(ui, copy_index).ok();
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
    let created = edit(ui, document, state, |config| {
        // The very first layout becomes active in single mode.
        let first = config.ordered_layout_ids()?.is_empty()
            && config.settings().layout_mode == LayoutMode::Manual;
        config.create_and_select_library_layout(name, description, source, first)
    })?;
    remember_saved(document, state, &created);
    library.set_dialog(LibraryDialog::None);
    // The layout exists now: a failure to open it is a warning, not a
    // failed creation. Unsaved changes were confirmed before the dialog.
    let opened = slint::Model::iter(&library.get_names())
        .position(|item| item == created)
        .ok_or(Msg::LibrarySelect)
        .and_then(|index| select(ui, state, index as i32))
        .and_then(|()| load(ui, document, state, &created));
    if let Err(error) = opened {
        let mut state = state.borrow_mut();
        state.warning = state.warning.take().or(Some(error));
    }
    Ok(Msg::LayoutCreated(created))
}

fn delete(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
) -> Result<Msg, Msg> {
    let name = name_at(ui, index)?;
    let expected = expected_bytes(document, state, &name)?;
    edit(ui, document, state, |config| {
        config.remove_library_layout(&name, &expected)
    })?;
    state.borrow().metadata.remember(&name, None);
    if state
        .borrow()
        .baseline
        .as_ref()
        .is_some_and(|(opened, _)| opened == &name)
    {
        state.borrow_mut().baseline = None;
    }
    ui.global::<LayoutLibrary>().set_dialog(LibraryDialog::None);
    Ok(Msg::LayoutDeleted(name))
}

/// Rename a layout; its file is kept byte-for-byte, so broken files too.
fn rename(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
    new_name: &str,
) -> Result<Msg, Msg> {
    let name = name_at(ui, index)?;
    let expected = expected_text(document, state, &name)?;
    let was_selected = state.borrow().selected.as_deref() == Some(name.as_str());
    let saved = edit(ui, document, state, |config| {
        config.edit_library_metadata(&name, new_name, None, &expected)
    })?;
    state.borrow().metadata.remember(&name, None);
    state
        .borrow()
        .metadata
        .remember(&saved, Some(expected.into_bytes()));
    if was_selected
        && let Some(index) = slint::Model::iter(&ui.global::<LayoutLibrary>().get_names())
            .position(|item| item == saved)
    {
        select(ui, state, index as i32)?;
    }
    update_baseline(&mut state.borrow_mut(), &name, &saved, None);
    Ok(Msg::None)
}

/// Follow our metadata edit without accepting unrelated changes on disk.
fn update_baseline(state: &mut State, name: &str, new_name: &str, description: Option<&str>) {
    let Some((opened, text)) = &mut state.baseline else {
        return;
    };
    if opened != name {
        return;
    }
    *opened = new_name.to_owned();
    if let Some(description) = description
        && let Ok(Some(mut layout)) = layout_file::parse(text)
    {
        layout.description =
            (!description.trim().is_empty()).then(|| description.trim().to_owned());
        *text = layout_file::serialize(&layout);
    }
}

fn set_description(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
    description: &str,
) -> Result<Msg, Msg> {
    let name = name_at(ui, index)?;
    let expected = expected_text(document, state, &name)?;
    edit(ui, document, state, |config| {
        config.edit_library_metadata(&name, &name, Some(description), &expected)
    })?;
    remember_saved(document, state, &name);
    update_baseline(&mut state.borrow_mut(), &name, &name, Some(description));
    Ok(Msg::None)
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let state: Shared = Rc::default();
    reset_context(ui, document, &mut state.borrow_mut());
    let library = ui.global::<LayoutLibrary>();
    library.on_row_names(|rows| super::strings(slint::Model::iter(&rows).map(|row| row.name)));
    library.on_copy_index_value(|name, names| {
        slint::Model::iter(&names)
            .position(|item| item == name)
            .map_or(-1, |index| index as i32)
    });
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
        let mut current = state.borrow_mut();
        current.languages = None;
        let result = refresh(&ui, &doc, &mut current).map(|()| Msg::None);
        drop(current);
        report(&ui, &state, result);
    });
    on!(ui, library.on_select, state = state => |ui, index| {
        if let Err(error) = select(&ui, &state, index) {
            report(&ui, &state, Err(error));
        }
    });
    on!(ui, library.on_set_automatic, doc = document, state = state => |ui, automatic| {
        let mode = if automatic {
            LayoutMode::Auto
        } else {
            LayoutMode::Manual
        };
        let result = edit_auto(&ui, &doc, &state, |settings| {
            auto_switch::set_mode(settings, mode)
        });
        report(&ui, &state, result);
    });
    on!(ui, library.on_action, doc = document, state = state => |ui, index, action| {
        let result = library_action(&ui, &doc, &state, index, action);
        report(&ui, &state, result);
    });
    library.on_suggest_name_value(|base, names| {
        let names: Vec<String> = slint::Model::iter(&names)
            .map(|name| name.to_string())
            .collect();
        lhc_core::profile::names::unique_layout_name(&base, &names)
            .unwrap_or_else(|_| base.to_string())
            .into()
    });
    library.on_name_error_value(|name, except, names| {
        let names: Vec<String> = slint::Model::iter(&names)
            .map(|name| name.to_string())
            .collect();
        let except = (!except.is_empty()).then_some(except.as_str());
        let result =
            lhc_core::profile::names::layout_name(&name, except, names.iter().map(String::as_str))
                .map_err(|issue| {
                    ConfigError::Name(lhc_core::profile::names::NameKind::Layout, issue)
                });
        super::name_error(result)
    });
    on!(ui, library.on_create, doc = document, state = state => |ui, name, description, source, copy_index| {
        let result = create(&ui, &doc, &state, &name, &description, source, copy_index);
        report(&ui, &state, result);
    });
    on!(ui, library.on_set_description, doc = document, state = state => |ui, index, description| {
        let result = set_description(&ui, &doc, &state, index, &description);
        let applied = result.is_ok();
        report(&ui, &state, result);
        applied
    });
    on!(ui, library.on_rename, doc = document, state = state => |ui, index, name| {
        let result = rename(&ui, &doc, &state, index, &name);
        let applied = result.is_ok();
        report(&ui, &state, result);
        applied
    });
    on!(ui, library.on_reorder, doc = document, state = state => |ui, from, to| {
        // Moving a card must not select or load it: take the name from the list.
        let names = ui.global::<LayoutLibrary>().get_names();
        let result = match super::move_indices(from, to, slint::Model::row_count(&names))
            .and_then(|(from, _)| slint::Model::row_data(&names, from))
        {
            Some(name) => {
                let id = user_layout_id(&name);
                edit(&ui, &doc, &state, |config| config.move_library_layout(&id, to - from))
                    .map(|()| Msg::None)
            }
            None => Ok(Msg::None),
        };
        report(&ui, &state, result);
    });
    on!(ui, library.on_discard_current, doc = document, state = state => |ui| {
        // Bind the name first: a `read()` borrow must not outlive this line.
        let name = current_name(&doc.read());
        let result = edit(&ui, &doc, &state, ConfigDocument::discard_working_layout).map(|()| {
            reset_context(&ui, &doc, &mut state.borrow_mut());
            ui.global::<LayoutLibrary>().invoke_discard_drafts();
            name.map_or(Msg::None, Msg::LayoutDiscarded)
        });
        report(&ui, &state, result);
    });
    on!(ui, library.on_activate_current, doc = document, state = state => |ui| {
        let id = doc.read().settings().editing_layout_id.clone();
        let result = id.ok_or(Msg::LibrarySelect).and_then(|id| {
            edit(&ui, &doc, &state, |config| {
                config.update_settings(|settings| settings.manual_layout_id = Some(id))
            })
        });
        let name = current_name(&doc.read()).unwrap_or_default();
        report(&ui, &state, result.map(|()| Msg::LayoutActivated(name)));
    });
    on!(ui, library.on_load, doc = document, state = state => |ui, index| {
        let result = name_at(&ui, index).and_then(|name| load(&ui, &doc, &state, &name));
        report(&ui, &state, result);
    });
    on!(ui, library.on_load_rule, doc = document, state = state => |ui, index, target| {
        load_rule(&ui, &doc, &state, index, target);
    });
    on!(ui, library.on_save_rule, doc = document, state = state => |ui| {
        let result = save_rule(&ui, &doc, &state);
        report(&ui, &state, result);
    });
    on!(ui, library.on_delete_rule, doc = document, state = state => |ui, index| {
        let result = delete_rule(&ui, &doc, &state, index);
        report(&ui, &state, result);
    });
    on!(ui, library.on_move_rule, doc = document, state = state => |ui, from, to| {
        let result = move_rule(&ui, &doc, &state, from, to);
        report(&ui, &state, result);
    });
    on!(ui, library.on_set_rule_enabled, doc = document, state = state => |ui, index, enabled| {
        let result = set_rule_enabled(&ui, &doc, &state, index, enabled);
        report(&ui, &state, result);
    });
    on!(ui, library.on_add_game_rule, doc = document, state = state => |ui| {
        let result = add_game_rule(&ui, &doc, &state);
        report(&ui, &state, result);
    });
    on!(ui, library.on_set_auto_default, doc = document, state = state => |ui, index| {
        let result = set_auto_default(&ui, &doc, &state, index);
        report(&ui, &state, result);
    });
    on!(ui, library.on_delete, doc = document, state = state => |ui, index| {
        let result = delete(&ui, &doc, &state, index);
        report(&ui, &state, result);
    });
    on!(ui, library.on_reset_context, doc = document, state = state => |ui| {
        reset_context(&ui, &doc, &mut state.borrow_mut());
    });
    on!(ui, library.on_refresh_context, doc = document => |ui| {
        refresh_context(&ui, &doc);
    });
    on!(ui, library.on_save_current, doc = document, state = state => |ui| {
        if !ui.global::<LayoutLibrary>().invoke_flush_drafts() {
            return;
        }
        let result = save_current(&ui, &doc, &state);
        report(&ui, &state, result);
    });
    on!(ui, library.on_overwrite_current, doc = document, state = state => |ui| {
        if !ui.global::<LayoutLibrary>().invoke_flush_drafts() {
            return;
        }
        let result = overwrite_current(&ui, &doc, &state);
        report(&ui, &state, result);
    });
    on!(ui, library.on_save_as, doc = document, state = state => |ui, name| {
        if !ui.global::<LayoutLibrary>().invoke_flush_drafts() {
            return;
        }
        let result = save_as(&ui, &doc, &state, &name);
        report(&ui, &state, result);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;

    fn bound_document() -> (tempfile::TempDir, Rc<Document>, SettingsWindow) {
        use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
        struct Platform;
        impl slint::platform::Platform for Platform {
            fn create_window_adapter(
                &self,
            ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
                Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
            }
        }
        slint::platform::set_platform(Box::new(Platform)).unwrap();
        let (dir, document) = super::super::test_document();
        document
            .edit_library(|config| config.save_current_layout_as("A"))
            .unwrap();
        document
            .edit_library(|config| config.save_current_layout_as("B"))
            .unwrap();
        document
            .edit_library(|config| config.load_library_for_editing("A"))
            .unwrap();
        let ui = SettingsWindow::new().unwrap();
        ui.global::<crate::ui::SettingsEditor>()
            .set_devices_loading(true);
        super::super::bind_document(&ui, &document);
        (dir, document, ui)
    }

    #[test]
    fn duplicate_keeps_its_source_after_save_as_reorders_the_library() {
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let paths = document.read().paths().clone();
        assert!(library.invoke_set_description(1, "source B".into()));
        document
            .edit(View::Rules, |config| {
                config.update_layout(|layout| layout.description = Some("mine A".into()))
            })
            .unwrap();
        library.invoke_open_create(LayoutSource::Copy, 1);
        assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
        paths
            .save_user_layout(
                "A",
                r#"{"schemaVersion":1,"description":"external A"}"#,
                true,
            )
            .unwrap();
        library.invoke_save_and_continue();
        assert_eq!(library.get_dialog(), LibraryDialog::Conflict);
        library.set_dialog(LibraryDialog::None);
        library.invoke_open_save_as("0".into());
        library.invoke_save_as("0".into());
        library.invoke_finish_save();
        assert_eq!(library.get_names().row_data(1).unwrap(), "A");
        assert_eq!(library.get_create_name(), "B (copy)");
        library.invoke_submit_create();
        assert_eq!(
            document.read().layout().description.as_deref(),
            Some("source B")
        );
        assert_eq!(library.get_current_label(), "B (copy)");
    }

    #[test]
    fn duplicate_reports_a_missing_source_instead_of_copying_its_old_neighbour() {
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        library.invoke_open_create(LayoutSource::Copy, 1);
        assert_eq!(library.get_dialog(), LibraryDialog::Create);
        document.read().paths().delete_user_layout("B").unwrap();
        library.invoke_refresh();
        library.invoke_submit_create();
        assert_eq!(library.get_dialog(), LibraryDialog::Create);
        assert_ne!(library.get_status().id, "");
        assert!(
            !document
                .read()
                .paths()
                .layout_path("B (copy)")
                .unwrap()
                .exists()
        );
    }

    #[test]
    fn delete_invalid_utf8_works_from_the_library_dialog() {
        let (_dir, document, ui) = bound_document();
        let paths = document.read().paths().clone();
        std::fs::write(paths.layout_path("Broken").unwrap(), [0xff, 0xfe]).unwrap();
        let library = ui.global::<LayoutLibrary>();
        library.invoke_refresh();
        let index = slint::Model::iter(&library.get_names())
            .position(|name| name == "Broken")
            .unwrap() as i32;
        library.invoke_open_delete(index);
        library.invoke_delete(library.get_selected());
        assert_eq!(library.get_dialog(), LibraryDialog::None);
        assert!(!paths.layout_path("Broken").unwrap().exists());
    }

    #[test]
    fn stale_metadata_does_not_replace_rows_or_conflict_baselines() {
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let loader = metadata::Loader::default();
        let paths = document.read().paths().clone();
        let names = vec!["A".into(), "B".into()];
        library.set_metadata_revision(20);
        let old = lhc_core::profile::library::load_library_entries(&paths, &names);
        paths
            .save_user_layout("A", r#"{"schemaVersion":1,"description":"new"}"#, true)
            .unwrap();
        loader.publish(
            &ui,
            20,
            &names,
            lhc_core::profile::library::load_library_entries(&paths, &names),
        );
        let expected = loader.known("A").unwrap();
        loader.publish(&ui, 19, &names, old);
        assert_eq!(library.get_rows().row_data(0).unwrap().description, "new");
        assert_eq!(loader.known("A").unwrap(), expected);
    }

    #[test]
    fn editing_missing_rule_targets_preserves_the_reference_until_explicitly_changed() {
        let (_dir, document, ui) = bound_document();
        document
            .edit(View::Library, |config| {
                config.update_settings(|settings| {
                    settings.auto_rules = vec![AutoRule {
                        id: "missing".into(),
                        enabled: true,
                        layout_id: Some("user:gone".into()),
                        conditions: Default::default(),
                    }];
                })
            })
            .unwrap();
        let library = ui.global::<LayoutLibrary>();
        library.invoke_refresh();
        library.invoke_open_rule(0, -1);
        assert!(library.get_rule_target_missing());
        library.set_rule_apps("kate".into());
        library.invoke_save_rule();
        assert_eq!(
            document.read().settings().auto_rules[0]
                .layout_id
                .as_deref(),
            Some("user:gone")
        );
        assert_eq!(
            ConfigDocument::load(document.read().paths().clone())
                .unwrap()
                .settings()
                .auto_rules[0]
                .layout_id
                .as_deref(),
            Some("user:gone")
        );
        library.invoke_open_rule(0, -1);
        library.set_rule_target(-1);
        library.set_rule_target_missing(false);
        library.invoke_save_rule();
        assert!(document.read().settings().auto_rules[0].layout_id.is_none());
    }

    #[test]
    fn open_rule_tracks_identity_and_picker_names_across_refreshes() {
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        library.invoke_open_rule(-1, 0);
        library.invoke_save_rule();
        library.invoke_open_rule(0, 0);
        library.invoke_reorder(0, 1);
        assert_eq!(library.get_rule_names().row_data(0).unwrap(), "A");
        library.invoke_add_game_rule();
        library.set_rule_apps("kate".into());
        library.invoke_save_rule();
        let config = document.read();
        assert_eq!(config.settings().auto_rules.len(), 2);
        assert_eq!(
            config.settings().auto_rules[1].layout_id.as_deref(),
            Some("user:A")
        );
        assert_eq!(
            config.settings().auto_rules[1].conditions.window_patterns,
            ["kate"]
        );
    }

    #[test]
    fn unrelated_metadata_edits_do_not_accept_external_changes_to_the_working_copy() {
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let paths = document.read().paths().clone();
        let mut theirs = document.read().layout().clone();
        theirs.description = Some("external".into());
        let theirs = layout_file::serialize(&theirs);
        document
            .edit(View::Rules, |config| {
                config.update_layout(|layout| layout.description = Some("mine".into()))
            })
            .unwrap();
        paths.save_user_layout("A", &theirs, true).unwrap();
        library.invoke_rename(1, "Renamed B".into());
        library.invoke_set_description(1, "B description".into());
        library.invoke_delete(1);
        library.invoke_save_current();
        assert_eq!(library.get_dialog(), LibraryDialog::Conflict);
        assert_eq!(paths.load_user_layout("A").unwrap(), theirs);
    }

    #[test]
    fn quitting_requires_a_decision_and_continues_after_a_successful_save() {
        use crate::ui::PendingLayoutAction;
        use std::cell::Cell;
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        assert!(ui.invoke_prepare_quit());
        document
            .edit(View::Rules, |config| config.create_layer("Unsaved", ""))
            .unwrap();
        assert!(!ui.invoke_prepare_quit());
        assert_eq!(library.get_pending_action(), PendingLayoutAction::Quit);
        assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
        let quit = Rc::new(Cell::new(false));
        let requested = quit.clone();
        library.on_quit(move || requested.set(true));
        library.invoke_cancel_dialog();
        assert!(!quit.get());
        assert!(!ui.invoke_prepare_quit());
        library.invoke_save_and_continue();
        assert!(quit.get());
        assert!(!library.get_dirty());
        assert_eq!(
            document.read().load_layout("user:A").unwrap().layers.len(),
            1
        );
    }

    #[test]
    fn failed_save_does_not_quit_and_can_be_retried() {
        use std::cell::Cell;
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let paths = document.read().paths().clone();
        document
            .edit(View::Rules, |config| config.create_layer("Unsaved", ""))
            .unwrap();
        let quit = Rc::new(Cell::new(false));
        let requested = quit.clone();
        library.on_quit(move || requested.set(true));
        assert!(!ui.invoke_prepare_quit());
        std::fs::remove_file(paths.layout_path("A").unwrap()).unwrap();
        std::fs::create_dir(paths.layout_path("A").unwrap()).unwrap();
        library.invoke_save_and_continue();
        assert_eq!(library.get_dialog(), LibraryDialog::Conflict);
        library.invoke_overwrite_current();
        library.invoke_finish_save();
        assert!(!quit.get());
        assert!(library.get_dirty());
        std::fs::remove_dir(paths.layout_path("A").unwrap()).unwrap();
        assert!(!ui.invoke_prepare_quit());
        library.invoke_save_and_continue();
        library.invoke_overwrite_current();
        library.invoke_finish_save();
        assert!(quit.get());
        assert!(!library.get_dirty());
    }

    #[test]
    fn quitting_can_explicitly_discard_an_invalid_draft_without_saving_it() {
        use crate::ui::MacroEditor;
        use std::cell::Cell;
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let macros = ui.global::<MacroEditor>();
        let original = document.read().paths().load_user_layout("A").unwrap();
        macros.invoke_add("Draft".into());
        macros.invoke_add_step(0, "macro:missing".into());
        let quit = Rc::new(Cell::new(false));
        let requested = quit.clone();
        library.on_quit(move || requested.set(true));
        assert!(!ui.invoke_prepare_quit());
        library.invoke_save_and_continue();
        assert!(!quit.get());
        library.invoke_continue_pending();
        assert!(quit.get());
        assert!(!macros.get_has_unsaved_changes());
        assert_eq!(
            document.read().paths().load_user_layout("A").unwrap(),
            original
        );
    }

    #[test]
    fn opening_or_failing_creation_keeps_the_current_page_drafts() {
        use crate::ui::MacroEditor;
        let (_dir, _document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let macros = ui.global::<MacroEditor>();
        macros.invoke_add("Draft".into());
        macros.invoke_add_step(0, "macro:missing".into());
        library.invoke_open_create(LayoutSource::Empty, 0);
        library.invoke_continue_pending();
        assert_eq!(library.get_dialog(), LibraryDialog::Create);
        assert!(macros.get_has_unsaved_changes());
        library.invoke_create("A".into(), "".into(), LayoutSource::Empty, 0);
        assert_eq!(library.get_dialog(), LibraryDialog::Create);
        assert!(macros.get_has_unsaved_changes());
        library.invoke_cancel_dialog();
        assert!(macros.get_has_unsaved_changes());
    }

    #[test]
    fn failed_switch_keeps_invalid_macro_drafts_until_a_successful_load() {
        use crate::ui::MacroEditor;
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let macros = ui.global::<MacroEditor>();
        macros.invoke_add("Draft".into());
        macros.invoke_add_step(0, "macro:missing".into());
        assert!(macros.get_has_unsaved_changes());
        library.invoke_edit(1);
        assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
        let paths = document.read().paths().clone();
        let target = paths.load_user_layout("B").unwrap();
        std::fs::remove_file(paths.layout_path("B").unwrap()).unwrap();
        library.invoke_continue_pending();
        assert!(macros.get_has_unsaved_changes());
        assert!(macros.get_has_errors());
        assert_eq!(
            macros
                .get_macros()
                .row_data(0)
                .unwrap()
                .steps
                .row_data(0)
                .unwrap()
                .action,
            "macro:missing"
        );
        paths.save_user_layout("B", &target, false).unwrap();
        library.invoke_refresh();
        library.invoke_edit(1);
        library.invoke_continue_pending();
        assert!(!macros.get_has_unsaved_changes());
        assert!(!macros.get_has_errors());
        assert_eq!(macros.get_macros().row_count(), 0);
    }

    #[test]
    fn discard_without_a_readable_saved_file_clears_working_copy_and_page_drafts() {
        use crate::ui::MacroEditor;
        let (_dir, document, ui) = bound_document();
        let library = ui.global::<LayoutLibrary>();
        let macros = ui.global::<MacroEditor>();
        macros.invoke_add("Draft".into());
        macros.invoke_add_step(0, "macro:missing".into());
        let paths = document.read().paths().clone();
        std::fs::remove_file(paths.layout_path("A").unwrap()).unwrap();
        library.invoke_discard_current();
        assert_eq!(library.get_current_label(), "");
        assert!(!library.get_dirty());
        assert!(!macros.get_has_unsaved_changes());
        assert!(!macros.get_has_errors());
        document
            .edit(View::Rules, |config| config.create_layer("Unnamed", ""))
            .unwrap();
        assert!(library.get_dirty());
        library.invoke_discard_current();
        assert!(!library.get_dirty());
        assert_eq!(document.read().layout(), &LayoutPreset::initial());
    }

    #[test]
    fn conditions_round_trip() {
        let set = condition(GameCondition::On, "us, ru", "", "kate");
        assert_eq!(set.layouts, ["us", "ru"]);
        assert_eq!(set.excluded_window_patterns, ["kate"]);
        assert_eq!(
            condition_fields(Some(set.clone())),
            (
                GameCondition::On,
                "us, ru".into(),
                String::new(),
                "kate".into()
            )
        );
        assert_eq!(summary(&set), (GameCondition::On, "us, ru".into()));
        assert!(condition(GameCondition::Any, " ", "", " ").is_empty());
    }

    #[test]
    fn warnings_never_hide_the_result() {
        let saved = || Msg::LayoutSaved("A".into());
        assert_eq!(merge_warning(Ok(saved()), None), (None, saved()));
        assert_eq!(
            merge_warning(Ok(saved()), Some(Msg::LibraryChanged)),
            (Some(Msg::LibraryChanged), saved())
        );
        assert_eq!(
            merge_warning(Ok(Msg::None), Some(Msg::LibraryChanged)),
            (None, Msg::LibraryChanged)
        );
        assert_eq!(
            merge_warning(Err(Msg::LibrarySelect), Some(Msg::LibraryChanged)),
            (Some(Msg::LibraryChanged), Msg::LibrarySelect)
        );
    }
}
