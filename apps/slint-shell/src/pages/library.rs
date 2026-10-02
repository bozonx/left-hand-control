//! Layout library page and the working copy toolbar.
//!
//! The working copy (`current-layout.yaml`) is edited by the other pages;
//! the library holds named layouts. Library files are compared with the
//! contents last read so a change made elsewhere is never overwritten.

use super::{game_condition, game_condition_value, parse_list};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        GameCondition, LayoutConditionsView, LayoutLibrary, LayoutSource, LibraryAction,
        LibraryDialog, MenuKind, NameIssue, Page, SettingsWindow,
    },
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError},
    profile::{
        auto_switch::AutoSwitchContext,
        layout_file,
        library::LibrarySource,
        model::{LayoutConditionRule, LayoutConditionSet, LayoutMode, user_layout_id, user_layout_name},
    },
    storage::validate_layout_name,
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

fn summary(set: Option<&LayoutConditionSet>) -> (GameCondition, String) {
    let Some(set) = set else {
        return (GameCondition::Any, String::new());
    };
    let list = [set.layouts.join(", "), set.apps.join(", ")]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (game_condition(set.game_mode.as_deref()), list)
}

fn condition(game: GameCondition, layouts: &str, apps: &str) -> Option<LayoutConditionSet> {
    let set = LayoutConditionSet {
        game_mode: game_condition_value(game),
        layouts: parse_list(layouts),
        apps: parse_list(apps),
    };
    (set.game_mode.is_some() || !set.layouts.is_empty() || !set.apps.is_empty()).then_some(set)
}

fn name_issue(config: &ConfigDocument, name: &str) -> NameIssue {
    if name.trim().is_empty() {
        return NameIssue::Empty;
    }
    let Ok(name) = validate_layout_name(name) else {
        return NameIssue::Invalid;
    };
    let taken = config
        .paths()
        .list_user_layouts()
        .is_ok_and(|names| names.iter().any(|item| item.eq_ignore_ascii_case(&name)));
    if taken { NameIssue::Taken } else { NameIssue::Ok }
}

/// Show the library. Descriptions are read from the layout files.
fn refresh(ui: &SettingsWindow, document: &Document, state: &mut State) -> Result<(), Msg> {
    let config = document.read();
    let library = ui.global::<LayoutLibrary>();
    let names: Vec<String> = config
        .ordered_layout_ids()
        .map_err(config_error)?
        .iter()
        .filter_map(|id| user_layout_name(id).map(str::to_owned))
        .collect();
    library.set_automatic(config.settings().layout_mode == LayoutMode::Auto);
    let rules: Vec<LayoutConditionRule> = names
        .iter()
        .map(|name| {
            config
                .settings()
                .layout_conditions
                .get(&user_layout_id(name))
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    library.set_auto_included(ModelRc::new(VecModel::from(
        rules.iter().map(|rule| rule.enabled_in_auto).collect::<Vec<_>>(),
    )));
    library.set_conditions(ModelRc::new(VecModel::from(
        rules
            .iter()
            .map(|rule| {
                let (when_game, when_list) = summary(rule.whitelist.as_ref());
                let (unless_game, unless_list) = summary(rule.blacklist.as_ref());
                LayoutConditionsView {
                    when_game,
                    when_list: when_list.into(),
                    unless_game,
                    unless_list: unless_list.into(),
                }
            })
            .collect::<Vec<_>>(),
    )));
    library.set_descriptions(super::strings(names.iter().map(|name| {
        config
            .paths()
            .load_user_layout(name)
            .ok()
            .and_then(|text| layout_file::parse(&text).ok().flatten())
            .and_then(|layout| layout.description)
            .unwrap_or_default()
    })));
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
    refresh_context(ui, document);
    refresh_active(ui, document);
    Ok(())
}

/// Label and dirty flag of the working copy: it differs from its library
/// file. Both sides go through the same parser, so formatting differences
/// in the file do not count as changes.
fn refresh_context(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let library = ui.global::<LayoutLibrary>();
    let name = current_name(&config);
    library.set_current_label(name.clone().unwrap_or_default().into());
    let dirty = name.is_some_and(|name| {
        config
            .load_layout(&user_layout_id(&name))
            .ok()
            .map(|layout| layout_file::serialize(&layout))
            != Some(layout_file::serialize(config.layout()))
    });
    library.set_dirty(dirty);
}

/// Name of the layout the mapper uses now.
pub fn refresh_active(ui: &SettingsWindow, document: &Document) {
    let label = document
        .read()
        .active_layout_id(&AutoSwitchContext::current())
        .ok()
        .flatten()
        .and_then(|id| user_layout_name(&id).map(str::to_owned))
        .unwrap_or_default();
    ui.global::<LayoutLibrary>().set_active_label(label.into());
}

/// Remember the library file of the working copy as its saved state.
fn reset_context(ui: &SettingsWindow, document: &Document, state: &mut State) {
    let config = document.read();
    state.baseline = current_name(&config).and_then(|name| {
        let text = config.paths().load_user_layout(&name).ok()?;
        Some((name, text))
    });
    drop(config);
    refresh_context(ui, document);
}

fn report(ui: &SettingsWindow, result: Result<Msg, Msg>) {
    ui.global::<LayoutLibrary>()
        .set_status(result.unwrap_or_else(|error| error).to_ui());
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
            ConfigError::ExternalChange => Msg::LibraryChanged,
            other => config_error(other),
        }
    })?;
    refresh(ui, document, &mut state.borrow_mut())?;
    if let Err(error) = saved.runtime {
        return Err(Msg::SavedMapperNotUpdated(error));
    }
    Ok(saved.value)
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
    let rule = config
        .settings()
        .layout_conditions
        .get(&user_layout_id(&name))
        .cloned()
        .unwrap_or_default();
    {
        let mut state = state.borrow_mut();
        state.selected = Some(name.clone());
        state.known = Some(text);
    }
    library.set_selected(index);
    library.set_name(name.into());
    library.set_description(layout.description.unwrap_or_default().into());
    library.set_auto_enabled(rule.enabled_in_auto);
    let (white_game, white_layouts, white_apps) = condition_fields(rule.whitelist);
    let (black_game, black_layouts, black_apps) = condition_fields(rule.blacklist);
    library.set_white_game(white_game);
    library.set_white_layouts(white_layouts.into());
    library.set_white_apps(white_apps.into());
    library.set_black_game(black_game);
    library.set_black_layouts(black_layouts.into());
    library.set_black_apps(black_apps.into());
    Ok(())
}

fn condition_fields(set: Option<LayoutConditionSet>) -> (GameCondition, String, String) {
    set.map(|set| {
        (
            game_condition(set.game_mode.as_deref()),
            set.layouts.join(", "),
            set.apps.join(", "),
        )
    })
    .unwrap_or((GameCondition::Any, String::new(), String::new()))
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
            let delta = if action == LibraryAction::MoveUp { -1 } else { 1 };
            edit(ui, document, state, |config| config.move_library_layout(&id, delta))?;
        }
        LibraryAction::SaveConditions => {
            let rule = LayoutConditionRule {
                enabled_in_auto: library.get_auto_enabled(),
                whitelist: condition(
                    library.get_white_game(),
                    &library.get_white_layouts(),
                    &library.get_white_apps(),
                ),
                blacklist: condition(
                    library.get_black_game(),
                    &library.get_black_layouts(),
                    &library.get_black_apps(),
                ),
            };
            edit(ui, document, state, |config| {
                config.update_settings(|settings| {
                    settings.layout_conditions.insert(id, rule);
                })
            })?;
        }
        LibraryAction::SaveDetails => {
            let expected = state.borrow().known.clone().ok_or(Msg::LibrarySelect)?;
            let new_name = library.get_name().to_string();
            let description = library.get_description().to_string();
            let saved = edit(ui, document, state, |config| {
                config.edit_library_metadata(&name, &new_name, &description, &expected)
            })?;
            let text = document.read().paths().load_user_layout(&saved).map_err(io)?;
            {
                let mut state = state.borrow_mut();
                state.selected = Some(saved);
                state.known = Some(text);
            }
            refresh(ui, document, &mut state.borrow_mut())?;
            reset_context(ui, document, &mut state.borrow_mut());
        }
    }
    if matches!(action, LibraryAction::SaveConditions | LibraryAction::SaveDetails) {
        library.set_dialog(LibraryDialog::None);
    }
    Ok(Msg::LibrarySaved)
}

/// Replace the working copy with the selected layout and open its rules.
fn load(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let name = selected_name(state)?;
    let is_current = current_name(&document.read()).as_deref() == Some(name.as_str());
    if !is_current {
        edit(ui, document, state, |config| config.load_library_for_editing(&name))?;
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
        LayoutSource::Author => LibrarySource::IvanK,
        LayoutSource::Copy => LibrarySource::Copy(copy_from.as_deref().ok_or(Msg::LibrarySelect)?),
        LayoutSource::Empty => LibrarySource::Empty,
    };
    let created = edit(ui, document, state, |config| {
        config.create_library_layout(name, description, source)
    })?;
    let index = slint::Model::iter(&library.get_names())
        .position(|item| item == created)
        .ok_or(Msg::LibrarySelect)?;
    library.set_dialog(LibraryDialog::None);
    select(ui, document, state, index as i32)?;
    library.invoke_edit(index as i32);
    Ok(Msg::None)
}

/// Save the working copy back to its library file.
fn save_current(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let Some(name) = current_name(&document.read()) else {
        let library = ui.global::<LayoutLibrary>();
        library.set_save_as_name("".into());
        library.set_save_as_open(true);
        return Ok(Msg::None);
    };
    let current = document.read().paths().load_user_layout(&name).map_err(io)?;
    if state.borrow().baseline.as_ref() != Some(&(name.clone(), current)) {
        return Err(Msg::LibraryChanged);
    }
    let text = layout_file::serialize(document.read().layout());
    edit(ui, document, state, |config| {
        config
            .paths()
            .save_user_layout(&name, &text, true)
            .map_err(ConfigError::Io)
    })?;
    state.borrow_mut().baseline = Some((name, text));
    refresh_context(ui, document);
    Ok(Msg::LibrarySaved)
}

/// Save the working copy as a new library layout and continue editing it.
fn save_as(ui: &SettingsWindow, document: &Document, state: &Shared, name: &str) -> Result<Msg, Msg> {
    let text = layout_file::serialize(document.read().layout());
    let saved = edit(ui, document, state, |config| {
        let saved = config
            .paths()
            .save_user_layout(name, &text, false)
            .map_err(ConfigError::Io)?;
        config.update_settings(|settings| {
            let old = settings.current_layout_id.clone();
            settings.current_layout_id = Some(user_layout_id(&saved));
            if settings.manual_active_layout_id == old {
                settings.manual_active_layout_id = settings.current_layout_id.clone();
            }
        })?;
        Ok(saved)
    })?;
    state.borrow_mut().baseline = Some((saved, text));
    ui.global::<LayoutLibrary>().set_save_as_open(false);
    refresh(ui, document, &mut state.borrow_mut())?;
    Ok(Msg::LibrarySaved)
}

fn delete(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let name = selected_name(state)?;
    let current = check_unchanged(document, state, &name)?;
    edit(ui, document, state, |config| config.remove_library_layout(&name, &current))?;
    reset_context(ui, document, &mut state.borrow_mut());
    ui.global::<LayoutLibrary>().set_dialog(LibraryDialog::None);
    Ok(Msg::LibrarySaved)
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
    let text = document.read().paths().load_user_layout(&name).map_err(io)?;
    edit(ui, document, state, |config| {
        config.edit_library_metadata(&name, &name, description, &text)
    })?;
    if state.borrow().selected.as_deref() == Some(name.as_str()) {
        let text = document.read().paths().load_user_layout(&name).map_err(io)?;
        state.borrow_mut().known = Some(text);
        library.set_description(description.trim().into());
    }
    reset_context(ui, document, &mut state.borrow_mut());
    Ok(Msg::None)
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let state: Shared = Rc::default();
    reset_context(ui, document, &mut state.borrow_mut());
    report(ui, refresh(ui, document, &mut state.borrow_mut()).map(|()| Msg::None));
    let library = ui.global::<LayoutLibrary>();

    let weak = ui.as_weak();
    // Other pages change the working copy: only the toolbar depends on it.
    // The list itself is re-read when the library page is opened.
    document.subscribe(View::Library, move |document| {
        if let Some(ui) = weak.upgrade() {
            refresh_context(&ui, document);
            refresh_active(&ui, document);
        }
    });

    macro_rules! on {
        ($method:ident, |$ui:ident, $doc:ident, $state:ident $(, $arg:ident)*| $body:expr) => {{
            let weak = ui.as_weak();
            let doc = document.clone();
            let shared = state.clone();
            library.$method(move |$($arg),*| {
                let Some($ui) = weak.upgrade() else { return Default::default() };
                let ($doc, $state) = (&doc, &shared);
                $body
            });
        }};
    }

    on!(on_refresh, |ui, doc, state| report(
        &ui,
        refresh(&ui, doc, &mut state.borrow_mut()).map(|()| Msg::None)
    ));
    on!(on_select, |ui, doc, state, index| {
        if let Err(error) = select(&ui, doc, state, index) {
            report(&ui, Err(error));
        }
    });
    on!(on_set_automatic, |ui, doc, state, automatic| {
        let mode = if automatic { LayoutMode::Auto } else { LayoutMode::Manual };
        report(
            &ui,
            edit(&ui, doc, state, |config| {
                config.update_settings(|settings| settings.layout_mode = mode)
            })
            .map(|()| Msg::LibrarySaved),
        );
    });
    on!(on_action, |ui, doc, state, action| report(
        &ui,
        library_action(&ui, doc, state, action)
    ));
    on!(on_suggest_name, |_ui, doc, _state, base| doc
        .read()
        .unique_library_name(&base)
        .unwrap_or_else(|_| base.to_string())
        .into());
    on!(on_name_issue, |_ui, doc, _state, name| name_issue(&doc.read(), &name));
    on!(on_create, |ui, doc, state, name, description, source, copy_index| {
        if let Err(error) = create(&ui, doc, state, &name, &description, source, copy_index) {
            report(&ui, Err(error));
        }
    });
    on!(on_set_description, |ui, doc, state, index, description| report(
        &ui,
        set_description(&ui, doc, state, index, &description)
    ));
    on!(on_activate_current, |ui, doc, state| {
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
        report(&ui, result.map(|()| Msg::None));
    });
    on!(on_load, |ui, doc, state| report(&ui, load(&ui, doc, state)));
    on!(on_delete, |ui, doc, state| report(&ui, delete(&ui, doc, state)));
    on!(on_reset_context, |ui, doc, state| reset_context(
        &ui,
        doc,
        &mut state.borrow_mut()
    ));
    on!(on_refresh_context, |ui, doc, _state| refresh_context(&ui, doc));
    on!(on_save_current, |ui, doc, state| report(
        &ui,
        save_current(&ui, doc, state)
    ));
    on!(on_save_as, |ui, doc, state, name| report(
        &ui,
        save_as(&ui, doc, state, &name)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_round_trip() {
        let set = condition(GameCondition::On, "us, ru", "").unwrap();
        assert_eq!(set.layouts, ["us", "ru"]);
        assert_eq!(condition_fields(Some(set.clone())), (GameCondition::On, "us, ru".into(), String::new()));
        assert_eq!(summary(Some(&set)), (GameCondition::On, "us, ru".into()));
        assert!(condition(GameCondition::Any, " ", "").is_none());
    }
}
