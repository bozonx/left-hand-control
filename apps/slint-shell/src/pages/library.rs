//! Layout library page and the working copy toolbar.
//!
//! The working copy in memory is edited by the other pages;
//! the library holds named layouts. Library files are compared with the
//! contents last read so a change made elsewhere is never overwritten.

use super::{game_condition, game_condition_value, parse_list};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        AutoRuleView, GameCondition, LayoutLibrary, LayoutSource, LayoutSummary, LibraryAction,
        LibraryDialog, MenuKind, NameIssue, Page, SettingsWindow,
    },
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError},
    profile::{
        auto_switch::AutoSwitchContext,
        ids, layout_file,
        library::LibrarySource,
        model::{
            AutoRule, LayoutConditionSet, LayoutMode, LayoutPreset, user_layout_id,
            user_layout_name,
        },
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
    if taken {
        NameIssue::Taken
    } else {
        NameIssue::Ok
    }
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
    let ids: Vec<String> = names.iter().map(|name| user_layout_id(name)).collect();
    let target = |id: &Option<String>| -> (i32, bool) {
        match id {
            None => (-1, false),
            Some(id) => ids
                .iter()
                .position(|item| item == id)
                .map_or((-1, true), |index| (index as i32, false)),
        }
    };
    let rules = &config.settings().auto_rules;
    library.set_auto_rules(ModelRc::new(VecModel::from(
        rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let (game, list) = summary(Some(&rule.conditions));
                let (target, missing) = target(&rule.layout_id);
                AutoRuleView {
                    game,
                    list: list.into(),
                    target,
                    missing,
                    shadows: !missing && rule.conditions.is_empty() && index + 1 < rules.len(),
                }
            })
            .collect::<Vec<_>>(),
    )));
    let (default, default_missing) = target(&config.settings().auto_default_layout_id);
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
    library.set_descriptions(super::strings(names.iter().map(|name| {
        config
            .paths()
            .load_user_layout(name)
            .ok()
            .and_then(|text| layout_file::parse(&text).ok().flatten())
            .and_then(|layout| layout.description)
            .unwrap_or_default()
    })));
    library.set_summaries(ModelRc::new(VecModel::from(
        names
            .iter()
            .map(|name| {
                let preset = config
                    .paths()
                    .load_user_layout(name)
                    .ok()
                    .and_then(|text| layout_file::parse(&text).ok().flatten())
                    .unwrap_or_default();
                LayoutSummary {
                    rules: preset.rules.len() as i32,
                    layers: preset.layers.len() as i32,
                    emoji_pages: preset.emoji_pages.len() as i32,
                    action_pages: preset.quick_action_pages.len() as i32,
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
    let named = name.is_some();
    let saved = name
        .and_then(|name| config.load_layout(&user_layout_id(&name)).ok())
        .or_else(|| (!named).then(LayoutPreset::initial));
    let current = config.layout();
    let normalized = layout_file::parse(&layout_file::serialize(current))
        .ok()
        .flatten();
    let current = normalized.as_ref().unwrap_or(current);
    let dirty = saved
        .as_ref()
        .is_none_or(|saved| layout_file::serialize(saved) != layout_file::serialize(current));
    library.set_dirty(dirty);
    let changed = |section: fn(&LayoutPreset) -> LayoutPreset| {
        saved.as_ref().map_or(dirty, |saved| {
            layout_file::serialize(&section(saved)) != layout_file::serialize(&section(current))
        })
    };
    library.set_rules_dirty(changed(|layout| LayoutPreset {
        rules: layout.rules.clone(),
        ..Default::default()
    }));
    let base_dirty = saved.as_ref().map_or(dirty, |saved| {
        crate::keyboard::BASE
            .iter()
            .any(|key| saved.base_tap_action(key) != current.base_tap_action(key))
    });
    library.set_layers_dirty(
        base_dirty
            || changed(|layout| LayoutPreset {
                layers: layout.layers.clone(),
                layer_keymaps: layout.layer_keymaps.clone(),
                ..Default::default()
            }),
    );
    library.set_macros_dirty(changed(|layout| LayoutPreset {
        macros: layout.macros.clone(),
        ..Default::default()
    }));
    library.set_quick_dirty(changed(|layout| LayoutPreset {
        quick_actions: layout.quick_actions.clone(),
        quick_action_pages: layout.quick_action_pages.clone(),
        ..Default::default()
    }));
    library.set_emoji_dirty(changed(|layout| LayoutPreset {
        emoji_pages: layout.emoji_pages.clone(),
        ..Default::default()
    }));
    library.set_commands_dirty(changed(|layout| LayoutPreset {
        commands: layout.commands.clone(),
        ..Default::default()
    }));
}

/// Name of the layout the mapper uses now and the automatic rule that
/// chose it.
pub fn refresh_active(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let context = AutoSwitchContext::current();
    let label = config
        .active_layout_id(&context)
        .ok()
        .flatten()
        .and_then(|id| user_layout_name(&id).map(str::to_owned))
        .unwrap_or_default();
    let matched = config
        .auto_choice(&context)
        .ok()
        .and_then(|choice| choice.rule)
        .map_or(-1, |index| index as i32);
    drop(config);
    let library = ui.global::<LayoutLibrary>();
    library.set_active_label(label.into());
    library.set_auto_matched(matched);
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
    let message = result.unwrap_or_else(|error| error);
    if matches!(
        message,
        Msg::LayoutCreated(_)
            | Msg::LayoutSaved(_)
            | Msg::LayoutDeleted(_)
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

fn condition_fields(set: Option<LayoutConditionSet>) -> (GameCondition, String, String) {
    set.map(|set| {
        (
            game_condition(set.game_mode.as_deref()),
            super::condition_list(&set.layouts),
            super::condition_list(&set.apps),
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
    Ok(Msg::LayoutCreated(created))
}

/// Save the working copy back to its library file.
fn save_current(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let Some(name) = current_name(&document.read()) else {
        let library = ui.global::<LayoutLibrary>();
        library.set_save_as_name("".into());
        library.set_save_as_open(true);
        return Ok(Msg::None);
    };
    let current = document
        .read()
        .paths()
        .load_user_layout(&name)
        .map_err(io)?;
    if state.borrow().baseline.as_ref() != Some(&(name.clone(), current)) {
        return Err(Msg::LibraryChanged);
    }
    let text = edit(ui, document, state, |config| {
        config.update_layout(LayoutPreset::prune_commands)?;
        let text = layout_file::serialize(config.layout());
        config
            .paths()
            .save_user_layout(&name, &text, true)
            .map_err(ConfigError::Io)?;
        Ok(text)
    })?;
    state.borrow_mut().baseline = Some((name.clone(), text));
    refresh_context(ui, document);
    Ok(Msg::LayoutSaved(name))
}

/// Save the working copy as a new library layout and continue editing it.
fn save_as(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    name: &str,
) -> Result<Msg, Msg> {
    let (saved, text) = edit(ui, document, state, |config| {
        config.update_layout(LayoutPreset::prune_commands)?;
        let text = layout_file::serialize(config.layout());
        Ok((config.save_current_layout_as(name)?, text))
    })?;
    state.borrow_mut().baseline = Some((saved.clone(), text));
    ui.global::<LayoutLibrary>().set_save_as_open(false);
    refresh(ui, document, &mut state.borrow_mut())?;
    Ok(Msg::LayoutSaved(saved))
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

/// Layout id of the library row `index`; any other index means off.
fn target_id(ui: &SettingsWindow, index: i32) -> Option<String> {
    usize::try_from(index)
        .ok()
        .and_then(|index| slint::Model::row_data(&ui.global::<LayoutLibrary>().get_names(), index))
        .map(|name| user_layout_id(&name))
}

/// Fill the rule dialog with rule `index`, or a new rule for `target`.
fn load_rule(ui: &SettingsWindow, document: &Document, index: i32, target: i32) {
    let library = ui.global::<LayoutLibrary>();
    let rule = usize::try_from(index)
        .ok()
        .and_then(|index| document.read().settings().auto_rules.get(index).cloned());
    let (game, layouts, apps) = condition_fields(rule.map(|rule| rule.conditions));
    library.set_rule_index(if rule_exists(document, index) {
        index
    } else {
        -1
    });
    library.set_rule_target(target);
    library.set_rule_game(game);
    library.set_rule_layouts(layouts.into());
    library.set_rule_apps(apps.into());
    library.set_status(Msg::None.to_ui());
    library.set_dialog(LibraryDialog::Rule);
}

fn rule_exists(document: &Document, index: i32) -> bool {
    usize::try_from(index).is_ok_and(|index| index < document.read().settings().auto_rules.len())
}

fn save_rule(ui: &SettingsWindow, document: &Document, state: &Shared) -> Result<Msg, Msg> {
    let library = ui.global::<LayoutLibrary>();
    let layout_id = target_id(ui, library.get_rule_target());
    let conditions = condition(
        library.get_rule_game(),
        &library.get_rule_layouts(),
        &library.get_rule_apps(),
    )
    .unwrap_or_default();
    let index = usize::try_from(library.get_rule_index()).ok();
    edit(ui, document, state, |config| {
        config.update_settings(|settings| {
            match index.and_then(|index| settings.auto_rules.get_mut(index)) {
                Some(rule) => {
                    rule.layout_id = layout_id;
                    rule.conditions = conditions;
                }
                None => settings.auto_rules.push(AutoRule {
                    id: ids::generate("rule-"),
                    layout_id,
                    conditions,
                }),
            }
        })
    })?;
    library.set_dialog(LibraryDialog::None);
    refresh_active(ui, document);
    Ok(Msg::None)
}

fn delete_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
) -> Result<Msg, Msg> {
    let index = usize::try_from(index).map_err(|_| Msg::LibrarySelect)?;
    edit(ui, document, state, |config| {
        config.update_settings(|settings| {
            if index < settings.auto_rules.len() {
                settings.auto_rules.remove(index);
            }
        })
    })?;
    ui.global::<LayoutLibrary>().set_dialog(LibraryDialog::None);
    refresh_active(ui, document);
    Ok(Msg::None)
}

fn move_rule(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    from: i32,
    to: i32,
) -> Result<Msg, Msg> {
    let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) else {
        return Ok(Msg::None);
    };
    edit(ui, document, state, |config| {
        config.update_settings(|settings| {
            let rules = &mut settings.auto_rules;
            if from < rules.len() && to < rules.len() {
                let rule = rules.remove(from);
                rules.insert(to, rule);
            }
        })
    })?;
    refresh_active(ui, document);
    Ok(Msg::None)
}

fn set_auto_default(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    index: i32,
) -> Result<Msg, Msg> {
    let id = target_id(ui, index);
    edit(ui, document, state, |config| {
        config.update_settings(|settings| settings.auto_default_layout_id = id)
    })?;
    refresh_active(ui, document);
    Ok(Msg::None)
}

fn drag_target(rows: &[Option<(f32, f32)>], source: usize, offset: f32, count: usize) -> i32 {
    let Some(Some((y, height))) = rows.get(source) else {
        return source as i32;
    };
    let pointer = y + height / 2.0 + offset;
    rows.iter()
        .take(count)
        .enumerate()
        .filter_map(|(index, row)| {
            row.map(|(y, height)| (index, (y + height / 2.0 - pointer).abs()))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map_or(source as i32, |(index, _)| index as i32)
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let state: Shared = Rc::default();
    reset_context(ui, document, &mut state.borrow_mut());
    let library = ui.global::<LayoutLibrary>();

    let rows = Rc::new(RefCell::new(Vec::new()));
    let positions = rows.clone();
    library.on_row_position(move |index, y, height| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let mut rows = positions.borrow_mut();
        if rows.len() <= index {
            rows.resize(index + 1, None);
        }
        rows[index] = Some((y, height));
    });
    let weak = ui.as_weak();
    library.on_drag_target(move |source, offset| {
        let Some(ui) = weak.upgrade() else {
            return source;
        };
        let Ok(index) = usize::try_from(source) else {
            return source;
        };
        drag_target(
            &rows.borrow(),
            index,
            offset,
            slint::Model::row_count(&ui.global::<LayoutLibrary>().get_names()),
        )
    });

    report(
        ui,
        refresh(ui, document, &mut state.borrow_mut()).map(|()| Msg::None),
    );

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
        let mode = if automatic {
            LayoutMode::Auto
        } else {
            LayoutMode::Manual
        };
        report(
            &ui,
            edit(&ui, doc, state, |config| {
                config.update_settings(|settings| settings.layout_mode = mode)
            })
            .map(|()| Msg::None),
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
    on!(on_name_issue, |_ui, doc, _state, name| name_issue(
        &doc.read(),
        &name
    ));
    on!(on_create, |ui,
                    doc,
                    state,
                    name,
                    description,
                    source,
                    copy_index| {
        if let Err(error) = create(&ui, doc, state, &name, &description, source, copy_index) {
            report(&ui, Err(error));
        }
    });
    on!(on_set_description, |ui, doc, state, index, description| {
        report(&ui, set_description(&ui, doc, state, index, &description))
    });
    on!(on_rename, |ui, doc, state, index, name| {
        let result = select(&ui, doc, state, index).and_then(|()| {
            ui.global::<LayoutLibrary>().set_name(name);
            library_action(&ui, doc, state, LibraryAction::SaveDetails)
        });
        report(&ui, result);
    });
    on!(on_reorder, |ui, doc, state, from, to| {
        let result = select(&ui, doc, state, from).and_then(|()| {
            let id = user_layout_id(&selected_name(state)?);
            edit(&ui, doc, state, |config| {
                config.move_library_layout(&id, to - from)
            })
            .map(|()| Msg::None)
        });
        report(&ui, result);
    });
    on!(on_discard_current, |ui, doc, state| {
        let name = current_name(&doc.read()).ok_or(Msg::LibrarySelect);
        let result = name.and_then(|name| {
            edit(&ui, doc, state, |config| {
                config.load_library_for_editing(&name)
            })?;
            reset_context(&ui, doc, &mut state.borrow_mut());
            Ok(Msg::LayoutDiscarded(name))
        });
        report(&ui, result);
    });
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
        let name = current_name(&doc.read()).unwrap_or_default();
        report(&ui, result.map(|()| Msg::LayoutActivated(name)));
    });
    on!(on_load, |ui, doc, state| report(&ui, load(&ui, doc, state)));
    on!(on_load_rule, |ui, doc, _state, index, target| load_rule(
        &ui, doc, index, target
    ));
    on!(on_save_rule, |ui, doc, state| report(
        &ui,
        save_rule(&ui, doc, state)
    ));
    on!(on_delete_rule, |ui, doc, state, index| report(
        &ui,
        delete_rule(&ui, doc, state, index)
    ));
    on!(on_move_rule, |ui, doc, state, from, to| report(
        &ui,
        move_rule(&ui, doc, state, from, to)
    ));
    on!(on_set_auto_default, |ui, doc, state, index| report(
        &ui,
        set_auto_default(&ui, doc, state, index)
    ));
    on!(on_delete, |ui, doc, state| report(
        &ui,
        delete(&ui, doc, state)
    ));
    on!(on_reset_context, |ui, doc, state| reset_context(
        &ui,
        doc,
        &mut state.borrow_mut()
    ));
    on!(on_refresh_context, |ui, doc, _state| refresh_context(
        &ui, doc
    ));
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
    fn dragging_uses_actual_card_heights() {
        let rows = [Some((0.0, 200.0)), Some((200.0, 96.0)), Some((296.0, 96.0))];
        assert_eq!(drag_target(&rows, 0, 244.0, 3), 2);
        assert_eq!(drag_target(&rows, 2, -244.0, 3), 0);
        assert_eq!(drag_target(&rows, 0, 244.0, 2), 1);
    }

    #[test]
    fn conditions_round_trip() {
        let set = condition(GameCondition::On, "us, ru", "").unwrap();
        assert_eq!(set.layouts, ["us", "ru"]);
        assert_eq!(
            condition_fields(Some(set.clone())),
            (GameCondition::On, "us, ru".into(), String::new())
        );
        assert_eq!(summary(Some(&set)), (GameCondition::On, "us, ru".into()));
        assert!(condition(GameCondition::Any, " ", "").is_none());
    }
}
