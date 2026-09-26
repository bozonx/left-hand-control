use crate::{
    editor::EditorHandle,
    i18n::Msg,
    ui::{LayoutConditionsView, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{layout_file, library::LibrarySource, model::LayoutConditionSet},
    storage::validate_layout_name,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

struct Library {
    names: Rc<VecModel<SharedString>>,
    selected: Option<String>,
    known: Option<String>,
}

fn refresh(
    ui: &SettingsWindow,
    document: &ConfigDocument,
    state: &mut Library,
) -> Result<(), String> {
    let names: Vec<String> = document
        .ordered_layout_ids()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|id| id.trim_start_matches("user:").to_owned())
        .collect();
    ui.set_layout_mode_index(i32::from(
        document.settings().layout_mode == lhc_core::profile::model::LayoutMode::Auto,
    ));
    let rules: Vec<_> = names
        .iter()
        .map(|name| {
            document
                .settings()
                .layout_conditions
                .get(&format!("user:{name}"))
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    ui.set_layout_auto_included(ModelRc::new(VecModel::from(
        rules
            .iter()
            .map(|rule| rule.enabled_in_auto)
            .collect::<Vec<bool>>(),
    )));
    ui.set_layout_conditions(ModelRc::new(VecModel::from(
        rules
            .into_iter()
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
    ui.set_layout_descriptions(ModelRc::new(VecModel::from(
        names
            .iter()
            .map(|name| {
                document
                    .paths()
                    .load_user_layout(name)
                    .ok()
                    .and_then(|text| layout_file::parse(&text).ok().flatten())
                    .and_then(|layout| layout.description)
                    .unwrap_or_default()
                    .into()
            })
            .collect::<Vec<SharedString>>(),
    )));
    let selected = state
        .selected
        .as_ref()
        .and_then(|name| names.iter().position(|item| item == name));
    state
        .names
        .set_vec(names.into_iter().map(Into::into).collect::<Vec<_>>());
    ui.set_selected_layout(selected.map_or(-1, |index| index as i32));
    if selected.is_none() {
        state.selected = None;
        state.known = None;
        ui.set_layout_description("".into());
        ui.set_layout_auto_enabled(false);
        ui.set_layout_white_game(0);
        ui.set_layout_black_game(0);
        ui.set_layout_white_layouts("".into());
        ui.set_layout_black_layouts("".into());
        ui.set_layout_white_apps("".into());
        ui.set_layout_black_apps("".into());
    }
    Ok(())
}

/// Game-mode index (0 any, 1 on, 2 off) and the comma-joined layouts and apps.
fn summary(set: Option<&LayoutConditionSet>) -> (i32, String) {
    let Some(set) = set else {
        return (0, String::new());
    };
    let game = match set.game_mode.as_deref() {
        Some("on") => 1,
        Some("off") => 2,
        _ => 0,
    };
    (
        game,
        [set.layouts.join(", "), set.apps.join(", ")]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
    )
}

/// 0 valid, 1 empty, 2 invalid characters, 3 taken.
fn name_issue(document: &ConfigDocument, name: &str) -> i32 {
    if name.trim().is_empty() {
        return 1;
    }
    let Ok(name) = validate_layout_name(name) else {
        return 2;
    };
    let taken = document
        .paths()
        .list_user_layouts()
        .is_ok_and(|names| names.iter().any(|item| item.eq_ignore_ascii_case(&name)));
    if taken { 3 } else { 0 }
}

fn report(ui: &SettingsWindow, result: Result<String, String>) {
    ui.set_layout_status(
        match result {
            Ok(message) if message.is_empty() => Msg::None,
            Ok(_) => Msg::LibrarySaved,
            Err(error) if error == "Select a layout" => Msg::LibrarySelect,
            Err(error)
                if error.starts_with("Layout changed on disk")
                    || error
                        == lhc_core::config_document::ConfigError::ExternalChange.to_string() =>
            {
                Msg::LibraryChanged
            }
            Err(error) => Msg::Error(error),
        }
        .to_ui(),
    );
}

pub(super) fn bind(
    ui: &SettingsWindow,
    config: Option<Rc<RefCell<ConfigDocument>>>,
    editor: EditorHandle,
) {
    bind_toolbar(ui, config.clone());
    let state = Rc::new(RefCell::new(Library {
        names: Rc::new(VecModel::default()),
        selected: None,
        known: None,
    }));
    ui.set_layout_names(ModelRc::new(state.borrow().names.clone()));
    if let Some(config) = &config {
        report(
            ui,
            refresh(ui, &config.borrow(), &mut state.borrow_mut()).map(|_| String::new()),
        );
    }
    bind_controls(ui, config.clone(), state.clone());
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_refresh_layouts(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            report(
                &ui,
                refresh(&ui, &config.borrow(), &mut state_copy.borrow_mut()).map(|_| String::new()),
            );
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_select_layout(move |index| {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let Some(name) = usize::try_from(index)
            .ok()
            .and_then(|i| state_copy.borrow().names.row_data(i))
        else {
            return;
        };
        let name = name.to_string();
        let result = config
            .borrow()
            .paths()
            .load_user_layout(&name)
            .and_then(|text| {
                let layout = layout_file::parse(&text)?.ok_or("Layout is empty")?;
                let mut state = state_copy.borrow_mut();
                state.selected = Some(name.clone());
                state.known = Some(text);
                ui.set_selected_layout(index);
                ui.set_layout_name(name.clone().into());
                ui.set_layout_description(layout.description.unwrap_or_default().into());
                let document = config.borrow();
                let rule = document
                    .settings()
                    .layout_conditions
                    .get(&format!("user:{name}"))
                    .cloned()
                    .unwrap_or_default();
                ui.set_layout_auto_enabled(rule.enabled_in_auto);
                for (blacklist, set) in [(false, rule.whitelist), (true, rule.blacklist)] {
                    let (game, layouts, apps) = set
                        .map(|set| {
                            (
                                match set.game_mode.as_deref() {
                                    Some("on") => 1,
                                    Some("off") => 2,
                                    _ => 0,
                                },
                                set.layouts.join(", "),
                                set.apps.join(", "),
                            )
                        })
                        .unwrap_or_default();
                    if blacklist {
                        ui.set_layout_black_game(game);
                        ui.set_layout_black_layouts(layouts.into());
                        ui.set_layout_black_apps(apps.into());
                    } else {
                        ui.set_layout_white_game(game);
                        ui.set_layout_white_layouts(layouts.into());
                        ui.set_layout_white_apps(apps.into());
                    }
                }
                Ok(name)
            });
        report(&ui, result.map(|_| String::new()));
    });
    let config_copy = config.clone();
    ui.on_suggest_layout_name(move |base| {
        config_copy
            .as_ref()
            .and_then(|config| config.borrow().unique_library_name(&base).ok())
            .unwrap_or_else(|| base.to_string())
            .into()
    });
    let config_copy = config.clone();
    ui.on_layout_name_issue(move |name| {
        config_copy
            .as_ref()
            .map_or(0, |config| name_issue(&config.borrow(), &name))
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_create_layout(move |name, description, source, copy_index| {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let result = (|| {
            let copy_from = usize::try_from(copy_index)
                .ok()
                .and_then(|index| state_copy.borrow().names.row_data(index));
            let source = match source {
                1 => LibrarySource::IvanK,
                2 => LibrarySource::Copy(copy_from.as_deref().ok_or("Select a layout")?),
                _ => LibrarySource::Empty,
            };
            let name = config
                .borrow()
                .create_library_layout(&name, &description, source)
                .map_err(|error| error.to_string())?;
            refresh(&ui, &config.borrow(), &mut state_copy.borrow_mut())?;
            let index = state_copy
                .borrow()
                .names
                .iter()
                .position(|item| item == name)
                .ok_or("Select a layout")?;
            ui.set_library_dialog(0);
            ui.invoke_select_layout(index as i32);
            ui.invoke_load_layout();
            Ok(String::new())
        })();
        if result.is_err() {
            report(&ui, result);
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_set_layout_description(move |index, description| {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let result = (|| {
            let name = usize::try_from(index)
                .ok()
                .and_then(|index| state_copy.borrow().names.row_data(index))
                .ok_or("Select a layout")?
                .to_string();
            let text = config.borrow().paths().load_user_layout(&name)?;
            config
                .borrow_mut()
                .edit_library_metadata(&name, &name, &description, &text)
                .map_err(|error| error.to_string())?;
            let mut state = state_copy.borrow_mut();
            if state.selected.as_deref() == Some(name.as_str()) {
                state.known = Some(config.borrow().paths().load_user_layout(&name)?);
                ui.set_layout_description(description.trim().into());
            }
            refresh(&ui, &config.borrow(), &mut state)?;
            drop(state);
            ui.invoke_reset_layout_context();
            Ok(String::new())
        })();
        report(&ui, result);
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_save_layout(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let name = ui.get_layout_name().trim().to_owned();
        let result = (|| {
            let document = config.borrow();
            let selected = state_copy.borrow().selected.clone();
            let overwrite = selected.as_deref() == Some(name.as_str());
            if overwrite {
                let current = document.paths().load_user_layout(&name)?;
                if Some(current) != state_copy.borrow().known {
                    return Err("Layout changed on disk; select it again".into());
                }
            }
            let saved = document.paths().save_user_layout(
                &name,
                &layout_file::serialize(document.layout()),
                overwrite,
            )?;
            drop(document);
            let text = config.borrow().paths().load_user_layout(&saved)?;
            {
                let mut state = state_copy.borrow_mut();
                state.selected = Some(saved.clone());
                state.known = Some(text);
                refresh(&ui, &config.borrow(), &mut state)?;
            }
            ui.set_layout_name(saved.clone().into());
            if config.borrow().settings().current_layout_id.as_deref()
                == Some(format!("user:{saved}").as_str())
            {
                ui.invoke_reset_layout_context();
            }
            super::mapper::apply_runtime(&config.borrow())?;
            Ok(format!("Saved: {saved}"))
        })();
        report(&ui, result);
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_load_layout(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let result = (|| {
            let name = state_copy
                .borrow()
                .selected
                .clone()
                .ok_or("Select a layout")?;
            if config.borrow().settings().current_layout_id.as_deref()
                == Some(format!("user:{name}").as_str())
            {
                ui.invoke_navigate(3, 0);
                return Ok(String::new());
            }
            config
                .borrow_mut()
                .load_library_for_editing(&name)
                .map_err(|error| error.to_string())?;
            ui.invoke_reset_layout_context();
            editor.reload(&config.borrow());
            ui.invoke_refresh_rules();
            ui.invoke_refresh_layers();
            ui.global::<crate::ui::MacroEditor>().invoke_refresh();
            ui.set_config_status(
                crate::i18n::Msg::ConfigSaved(config.borrow().layout().rules.len()).to_ui(),
            );
            ui.invoke_navigate(3, 0);
            super::mapper::apply_runtime(&config.borrow())?;
            Ok(String::new())
        })();
        report(&ui, result);
    });
    let weak = ui.as_weak();
    ui.on_delete_layout(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config) else {
            return;
        };
        let result = (|| {
            let name = state.borrow().selected.clone().ok_or("Select a layout")?;
            let current = config.borrow().paths().load_user_layout(&name)?;
            if Some(current.as_str()) != state.borrow().known.as_deref() {
                return Err("Layout changed on disk; select it again".into());
            }
            config
                .borrow_mut()
                .remove_library_layout(&name, &current)
                .map_err(|error| error.to_string())?;
            super::mapper::apply_runtime(&config.borrow())?;
            ui.invoke_reset_layout_context();
            refresh(&ui, &config.borrow(), &mut state.borrow_mut())?;
            ui.set_library_dialog(0);
            Ok(format!("Deleted: {name}"))
        })();
        report(&ui, result);
    });
}

fn bind_toolbar(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    let known = Rc::new(RefCell::new(None::<(String, String)>));
    let weak = ui.as_weak();
    let document = config.clone();
    let baseline = known.clone();
    ui.on_reset_layout_context(move || {
        let (Some(ui), Some(document)) = (weak.upgrade(), &document) else {
            return;
        };
        let document = document.borrow();
        *baseline.borrow_mut() = document
            .settings()
            .current_layout_id
            .as_ref()
            .and_then(|id| {
                let name = id.strip_prefix("user:")?;
                document
                    .paths()
                    .load_user_layout(name)
                    .ok()
                    .map(|text| (name.to_owned(), text))
            });
        drop(document);
        ui.invoke_refresh_layout_context();
    });
    let weak = ui.as_weak();
    let document = config.clone();
    ui.on_refresh_layout_context(move || {
        let (Some(ui), Some(document)) = (weak.upgrade(), &document) else {
            return;
        };
        let document = document.borrow();
        let id = document.settings().current_layout_id.as_deref();
        ui.set_current_layout_label(id.unwrap_or("").trim_start_matches("user:").into());
        ui.set_layout_dirty(
            id.and_then(|id| document.load_layout(id).ok())
                .map(|layout| layout_file::serialize(&layout))
                .as_deref()
                != Some(layout_file::serialize(document.layout()).as_str()),
        );
    });
    ui.invoke_reset_layout_context();
    let weak = ui.as_weak();
    let document = config.clone();
    let baseline = known.clone();
    ui.on_save_current_layout(move || {
        let (Some(ui), Some(document)) = (weak.upgrade(), &document) else {
            return;
        };
        let name = document
            .borrow()
            .settings()
            .current_layout_id
            .as_deref()
            .and_then(|id| id.strip_prefix("user:"))
            .map(str::to_owned);
        let Some(name) = name else {
            ui.set_save_as_name("".into());
            ui.set_save_as_open(true);
            return;
        };
        let result = (|| {
            let document = document.borrow();
            let current = document.paths().load_user_layout(&name)?;
            if baseline.borrow().as_ref() != Some(&(name.clone(), current)) {
                return Err("Layout changed on disk; reload it before saving".to_owned());
            }
            let text = layout_file::serialize(document.layout());
            document.paths().save_user_layout(&name, &text, true)?;
            *baseline.borrow_mut() = Some((name, text));
            super::mapper::apply_runtime(&document)?;
            Ok(String::new())
        })();
        if let Err(error) = &result {
            ui.set_backend_error(crate::i18n::Msg::Error(error.clone()).to_ui());
        }
        report(&ui, result);
        ui.invoke_refresh_layout_context();
    });
    let weak = ui.as_weak();
    ui.on_save_layout_as(move |name| {
        let (Some(ui), Some(document)) = (weak.upgrade(), &config) else {
            return;
        };
        let result = (|| {
            let text = layout_file::serialize(document.borrow().layout());
            let saved = document
                .borrow()
                .paths()
                .save_user_layout(&name, &text, false)?;
            document
                .borrow_mut()
                .update_settings(|settings| {
                    let old_id = settings.current_layout_id.clone();
                    settings.current_layout_id = Some(format!("user:{saved}"));
                    if settings.manual_active_layout_id == old_id {
                        settings.manual_active_layout_id = settings.current_layout_id.clone();
                    }
                })
                .map_err(|error| error.to_string())?;
            *known.borrow_mut() = Some((saved, text));
            ui.set_save_as_open(false);
            ui.invoke_refresh_layouts();
            ui.invoke_refresh_layout_context();
            Ok(String::new())
        })();
        report(&ui, result);
    });
}

fn condition(
    game: i32,
    layouts: &str,
    apps: &str,
) -> Option<lhc_core::profile::model::LayoutConditionSet> {
    let parse = |value: &str| {
        value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let set = lhc_core::profile::model::LayoutConditionSet {
        game_mode: match game {
            1 => Some("on".into()),
            2 => Some("off".into()),
            _ => None,
        },
        layouts: parse(layouts),
        apps: parse(apps),
    };
    (set.game_mode.is_some() || !set.layouts.is_empty() || !set.apps.is_empty()).then_some(set)
}

fn bind_controls(
    ui: &SettingsWindow,
    config: Option<Rc<RefCell<ConfigDocument>>>,
    state: Rc<RefCell<Library>>,
) {
    let weak = ui.as_weak();
    let document = config.clone();
    ui.on_set_layout_mode(move |index| {
        let (Some(ui), Some(document)) = (weak.upgrade(), &document) else {
            return;
        };
        let result = document
            .borrow_mut()
            .update_settings(|settings| {
                settings.layout_mode = if index == 1 {
                    lhc_core::profile::model::LayoutMode::Auto
                } else {
                    lhc_core::profile::model::LayoutMode::Manual
                }
            })
            .map_err(|error| error.to_string());
        report(
            &ui,
            result
                .and_then(|_| super::mapper::apply_runtime(&document.borrow()))
                .map(|_| "saved".into()),
        );
        ui.invoke_refresh_layouts();
    });
    let weak = ui.as_weak();
    let document = config.clone();
    ui.on_activate_current_layout(move || {
        let (Some(ui), Some(document)) = (weak.upgrade(), &document) else {
            return;
        };
        let result = (|| {
            let id = document
                .borrow()
                .settings()
                .current_layout_id
                .clone()
                .ok_or("Select a layout")?;
            document
                .borrow_mut()
                .update_settings(|settings| settings.manual_active_layout_id = Some(id.clone()))
                .map_err(|error| error.to_string())?;
            super::mapper::apply_runtime(&document.borrow())?;
            ui.set_active_layout_label(id.trim_start_matches("user:").into());
            Ok(String::new())
        })();
        report(&ui, result);
    });
    let weak = ui.as_weak();
    ui.on_library_action(move |action| {
        let (Some(ui), Some(document)) = (weak.upgrade(), &config) else {
            return;
        };
        let result = (|| {
            let name = state.borrow().selected.clone().ok_or("Select a layout")?;
            let id = format!("user:{name}");
            match action {
                0 => {
                    document
                        .borrow_mut()
                        .update_settings(|settings| settings.manual_active_layout_id = Some(id))
                        .map_err(|error| error.to_string())?;
                    ui.set_active_layout_label(name.clone().into());
                }
                1 | 2 => document
                    .borrow_mut()
                    .move_library_layout(&id, if action == 1 { -1 } else { 1 })
                    .map_err(|error| error.to_string())?,
                3 => {
                    let rule = lhc_core::profile::model::LayoutConditionRule {
                        enabled_in_auto: ui.get_layout_auto_enabled(),
                        whitelist: condition(
                            ui.get_layout_white_game(),
                            &ui.get_layout_white_layouts(),
                            &ui.get_layout_white_apps(),
                        ),
                        blacklist: condition(
                            ui.get_layout_black_game(),
                            &ui.get_layout_black_layouts(),
                            &ui.get_layout_black_apps(),
                        ),
                    };
                    document
                        .borrow_mut()
                        .update_settings(|settings| {
                            settings.layout_conditions.insert(id, rule);
                        })
                        .map_err(|error| error.to_string())?;
                }
                4 => {
                    let expected = state.borrow().known.clone().ok_or("Select a layout")?;
                    let saved = document
                        .borrow_mut()
                        .edit_library_metadata(
                            &name,
                            &ui.get_layout_name(),
                            &ui.get_layout_description(),
                            &expected,
                        )
                        .map_err(|error| error.to_string())?;
                    state.borrow_mut().selected = Some(saved.clone());
                    state.borrow_mut().known =
                        Some(document.borrow().paths().load_user_layout(&saved)?);
                    ui.invoke_reset_layout_context();
                }
                _ => return Ok(String::new()),
            }
            super::mapper::apply_runtime(&document.borrow())?;
            refresh(&ui, &document.borrow(), &mut state.borrow_mut())?;
            if action == 3 || action == 4 {
                ui.set_library_dialog(0);
            }
            Ok("saved".into())
        })();
        report(&ui, result);
    });
}
