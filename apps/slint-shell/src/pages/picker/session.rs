//! Open, refresh and apply an action picker session.

use super::{
    Drafts, Session, assigns_command,
    catalog::{
        ACTION_CATEGORIES, KEY_CATEGORIES, catalog, category_for_value, execution_notice,
        is_key_category, load_desktop_shortcuts,
    },
    commands::command_notice,
    request::{PickerMode, filter, request},
};
use crate::{
    document::Document,
    i18n::Msg,
    pages::{keys, layers, macros, menus, one_line, rules},
    ui::{
        ActionPicker, Locale, PickerBehavior, PickerCategory, PickerCategoryCount, PickerItem,
        PickerTarget, SettingsWindow,
    },
};
use lhc_core::profile::actions::Action;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::RefCell;

pub(super) fn open(
    ui: &SettingsWindow,
    doc: &Document,
    state: &RefCell<Session>,
    drafts: &Drafts,
    selection: (PickerTarget, i32),
    value: slint::SharedString,
    key_only: bool,
) {
    let (target, index) = selection;
    load_desktop_shortcuts(ui.as_weak());
    let catalog = catalog(ui, &doc.read());
    let category = catalog
        .iter()
        .find(|item| item.value == value)
        .map(|item| item.category)
        .unwrap_or(match Action::parse(Some(value.as_str())) {
            Action::Text(_) => PickerCategory::Text,
            Action::KdeShortcut(_) => PickerCategory::DesktopShortcuts,
            Action::Pause(_) if target == PickerTarget::MacroStep => PickerCategory::Pause,
            _ => PickerCategory::Modifiers,
        });
    let request = request(ui, doc, target, index, key_only);
    let behavior = request.behavior(value.is_empty());
    let original = match behavior {
        PickerBehavior::Swallow => ui
            .global::<Locale>()
            .invoke_text(Msg::PickerSuppress.to_ui()),
        PickerBehavior::Native => ui
            .global::<Locale>()
            .invoke_text(request.native_label().to_ui()),
        PickerBehavior::Action => catalog
            .iter()
            .find(|item| item.value == value)
            .map(|item| item.label.clone())
            .or_else(|| {
                let Action::Command(id) = Action::parse(Some(value.as_str())) else {
                    return None;
                };
                let config = doc.read();
                let command = config.layout().commands.iter().find(|c| c.id == id)?;
                Some(command.display_name().into())
            })
            .unwrap_or_else(|| value.clone()),
    };
    let command = match Action::parse(Some(value.as_str())) {
        Action::Command(id) => drafts.command(ui, &id),
        _ => None,
    };
    *state.borrow_mut() = Session {
        identity: std::sync::Arc::new(()),
        target: Some((target, index)),
        catalog,
        command: command.clone(),
    };
    let picker = ui.global::<ActionPicker>();
    picker.set_command_help(if cfg!(target_os = "windows") {
        Msg::CommandHelpWindows.to_ui()
    } else if cfg!(target_os = "macos") {
        Msg::CommandHelpMacos.to_ui()
    } else {
        Msg::CommandHelpLinux.to_ui()
    });
    picker.set_commands_enabled(doc.read().settings().commands_enabled);
    picker.set_subject("".into());
    picker.set_command_name(
        command
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default()
            .into(),
    );
    picker.set_command_script(
        command
            .as_ref()
            .map(|c| c.script().to_owned())
            .unwrap_or_default()
            .into(),
    );
    picker.set_command_directory(
        command
            .and_then(|c| c.working_directory)
            .unwrap_or_default()
            .into(),
    );
    picker.set_command_notice(Msg::None.to_ui());
    picker.set_dialog_title(request.title);
    picker.set_layer_action(request.mode == PickerMode::LayerAction);
    picker.set_rule_action(matches!(
        request.mode,
        PickerMode::RuleTap | PickerMode::RuleHold
    ));
    picker.set_hold_modifiers(request.hold_modifiers);
    picker.set_allow_default(request.allow_default);
    picker.set_allow_swallow(request.allow_swallow);
    picker.set_default_behavior(request.default_behavior);
    picker.set_hold_action(request.mode == PickerMode::RuleHold);
    picker.set_hold_layer(request.hold_layer);
    picker.set_behavior(behavior);
    picker.set_ignore_key(behavior == PickerBehavior::Swallow);
    picker.set_touched(false);
    picker.set_error(Msg::None.to_ui());
    picker.set_key_only(request.key_only);
    picker.set_trigger_chords(request.mode == PickerMode::Trigger);
    picker.set_macro_step(request.mode == PickerMode::MacroStep);
    picker.set_original(original);
    picker.set_value(value);
    picker.set_query("".into());
    picker.set_capturing(false);
    picker.set_category(category);
    picker.set_opened(true);
    picker.invoke_refresh();
}

// Keep repeater instances (and their keyboard focus) when only row data changes.
fn update_model<T: Clone + PartialEq + 'static>(model: ModelRc<T>, rows: Vec<T>) -> ModelRc<T> {
    if let Some(existing) = model.as_any().downcast_ref::<VecModel<T>>()
        && existing.row_count() == rows.len()
    {
        for (index, row) in rows.into_iter().enumerate() {
            if existing.row_data(index).as_ref() != Some(&row) {
                existing.set_row_data(index, row);
            }
        }
        model
    } else {
        ModelRc::new(VecModel::from(rows))
    }
}

pub(super) fn refresh(ui: &SettingsWindow, doc: &Document, state: &RefCell<Session>) {
    let picker = ui.global::<ActionPicker>();
    state.borrow_mut().catalog = catalog(ui, &doc.read());
    let state = state.borrow();
    let filter = filter(ui, doc, &picker, state.target);
    let entries: Vec<&PickerItem> = state
        .catalog
        .iter()
        // Each action owns its command; existing ones are not offered.
        .filter(|entry| entry.category != PickerCategory::Commands && filter.admits(&entry.value))
        .collect();
    picker.set_categories(update_model(
        picker.get_categories(),
        KEY_CATEGORIES
            .into_iter()
            .chain(ACTION_CATEGORIES)
            .map(|category| PickerCategoryCount {
                category,
                count: entries.iter().filter(|e| e.category == category).count() as i32,
            })
            .collect::<Vec<_>>(),
    ));
    if picker.get_macro_step()
        && picker.get_category() == PickerCategory::Pause
        && !matches!(
            Action::parse(Some(picker.get_value().as_str())),
            Action::Pause(_)
        )
    {
        let pause = macros::picker_macro_pause(ui)
            .unwrap_or_else(|| doc.read().settings().default_macro_step_pause_ms)
            .clamp(
                lhc_core::profile::timing::PAUSE.min,
                lhc_core::profile::timing::PAUSE.max,
            );
        picker.set_value(
            Action::Pause(pause.to_string())
                .format()
                .unwrap_or_default()
                .into(),
        );
        picker.set_touched(true);
    }
    let value = picker.get_value();
    if let Some(command) = doc
        .read()
        .layout()
        .commands
        .iter()
        .find(|command| assigns_command(&value, &command.id))
        && state
            .command
            .as_ref()
            .is_none_or(|draft| draft.id != command.id)
    {
        picker.set_command_name(command.name.clone().into());
        picker.set_command_script(command.script().into());
        picker.set_command_directory(command.working_directory.clone().unwrap_or_default().into());
    }
    picker.set_command_notice(command_notice(&picker).to_ui());
    picker.set_value_category(category_for_value(&value));
    let action = Action::parse(Some(value.as_str()));
    let text = match &action {
        Action::Text(text) => text.as_str(),
        _ => "",
    };
    picker.set_text_content(text.into());
    picker.set_text_preview(one_line(text).into());
    picker.set_text_lines(if text.contains('\n') {
        text.split('\n').count() as i32
    } else {
        0
    });
    picker.set_pause_content(
        match &action {
            Action::Pause(ms) => ms.as_str(),
            _ => "",
        }
        .into(),
    );
    let query = picker.get_query().to_lowercase();
    let mut category = picker.get_category();
    if query.is_empty()
        && is_key_category(category)
        && !entries.iter().any(|entry| entry.category == category)
        && let Some(entry) = entries.iter().find(|entry| is_key_category(entry.category))
    {
        category = entry.category;
        picker.set_category(category);
    }
    picker.set_items(update_model(
        picker.get_items(),
        entries
            .into_iter()
            .filter(|entry| {
                if query.is_empty() {
                    entry.category == category
                } else {
                    entry.label.to_lowercase().contains(&query)
                        || entry.value.to_lowercase().contains(&query)
                }
            })
            .cloned()
            .collect::<Vec<_>>(),
    ));
    picker.set_notice(execution_notice(&action, &doc.read().config()).to_ui());
    let valid = match picker.get_behavior() {
        PickerBehavior::Native => picker.get_allow_default(),
        PickerBehavior::Swallow => picker.get_allow_swallow(),
        PickerBehavior::Action => match state
            .command
            .as_ref()
            .filter(|command| assigns_command(&value, &command.id))
        {
            Some(command) => {
                filter.admits(&value)
                    && doc.read().settings().commands_enabled
                    && !command.script().trim().is_empty()
            }
            None => filter.accepts(&value, Some(&doc.read())),
        },
    };
    picker.set_valid(valid);
}

pub(super) fn apply(
    ui: &SettingsWindow,
    doc: &Document,
    session: &RefCell<Session>,
    drafts: &Drafts,
) {
    let picker = ui.global::<ActionPicker>();
    picker.invoke_refresh();
    if !picker.get_valid() || picker.get_capturing() {
        return;
    }
    picker.set_error(Msg::None.to_ui());
    let value = if picker.get_behavior() == PickerBehavior::Action {
        picker.get_value()
    } else {
        "".into()
    };
    let command = session
        .borrow()
        .command
        .clone()
        .filter(|command| assigns_command(&value, &command.id));
    let (target, label) = {
        let state = session.borrow();
        let label = command
            .as_ref()
            .map(|command| command.display_name().into())
            .or_else(|| {
                state
                    .catalog
                    .iter()
                    .find(|item| item.value == value)
                    .map(|item| item.label.clone())
            })
            .or_else(|| {
                doc.read()
                    .layout()
                    .commands
                    .iter()
                    .find(|command| assigns_command(&value, &command.id))
                    .map(|command| command.name.clone().into())
            })
            .unwrap_or_default();
        (state.target, label)
    };
    let Some((target, index)) = target else {
        return;
    };
    let mut next_step = false;
    match target {
        PickerTarget::Rule => {
            if let Err(error) = rules::choose(ui, doc, &value, command) {
                picker.set_error(error.to_ui());
            }
        }
        PickerTarget::MacroStep => {
            if let Err(error) = macros::choose(ui, doc, &drafts.macros, index, &value, command) {
                picker.set_error(error.to_ui());
            }
        }
        PickerTarget::QuickAction => {
            if let Err(error) =
                menus::choose(ui, doc, &drafts.menus, index, &value, &label, command)
            {
                picker.set_error(error.to_ui());
            }
        }
        PickerTarget::LayerAction => {
            if picker.get_layer_action() {
                if let Err(error) = layers::assign(
                    ui,
                    doc,
                    index,
                    &value,
                    layers::Choice {
                        ignore: picker.get_ignore_key() && value.is_empty(),
                        hold_modifiers: picker.get_hold_modifiers(),
                        command,
                    },
                ) {
                    picker.set_error(error.to_ui());
                }
            } else {
                layers::choose_dialog_action(ui, &drafts.layers, value, command);
            }
        }
        PickerTarget::ExtraKey | PickerTarget::ExtraAction => {
            match layers::assign_extra(
                ui,
                doc,
                index,
                &value,
                target == PickerTarget::ExtraKey,
                layers::Choice {
                    ignore: picker.get_ignore_key() && value.is_empty(),
                    hold_modifiers: picker.get_hold_modifiers(),
                    command,
                },
            ) {
                Ok(next) => next_step = next,
                Err(error) => picker.set_error(error.to_ui()),
            }
        }
        PickerTarget::BaseKey => {
            if let Err(error) = keys::assign(ui, doc, index, &value, command) {
                picker.set_error(error.to_ui());
            }
        }
    }
    if picker.get_error().id.is_empty() {
        picker.invoke_dismiss_picker();
        if next_step {
            layers::pick_new_extra_action(ui);
        }
    }
}
