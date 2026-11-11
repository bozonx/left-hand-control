//! Open, refresh and apply an action picker session.

use super::{
    Drafts, Session, assigns_command,
    catalog::{
        ACTION_CATEGORIES, KEY_CATEGORIES, SHORTCUTS_GENERATION, catalog, category_for_value,
        execution_notice, is_key_category, load_desktop_shortcuts,
    },
    commands::command_notice,
    request::{PickerMode, filter, request},
};
use crate::{
    document::Document,
    i18n::Msg,
    pages::{keys, layers, macros, menus, one_line, rules},
    ui::{
        ActionPicker, PickerBehavior, PickerCategory, PickerItem, PickerSidebarEntry, PickerTarget,
        SettingsWindow,
    },
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use tecloid_core::config::{actions::Action, model::Command};

/// Category shown when the picker opens with `value`.
pub(super) fn initial_category(
    value: &str,
    catalog: &[PickerItem],
    target: PickerTarget,
) -> PickerCategory {
    if let Some(item) = catalog.iter().find(|item| item.value == value) {
        return item.category;
    }
    if value.is_empty() {
        return PickerCategory::Modifiers;
    }
    match category_for_value(value) {
        PickerCategory::Pause if target != PickerTarget::MacroStep => PickerCategory::Modifiers,
        category => category,
    }
}

/// Lines of a `text:` action when it has several, 0 otherwise. A single
/// trailing line break does not start a new line; `\r\n` counts once.
pub(super) fn text_lines(text: &str) -> i32 {
    let lines = text.lines().count();
    if lines > 1 { lines as i32 } else { 0 }
}

/// Whether every word of a lowercase search matches the label or value of
/// an item, so `ctrl c` finds `Ctrl+KeyC`.
pub(super) fn matches_query(words: &[&str], label: &str, value: &str) -> bool {
    let (label, value) = (label.to_lowercase(), value.to_lowercase());
    words
        .iter()
        .all(|word| label.contains(word) || value.contains(word))
}

/// Whether the category is a list of catalog items rather than an editor.
pub(super) fn is_listed(category: PickerCategory) -> bool {
    is_key_category(category)
        || matches!(
            category,
            PickerCategory::Macros
                | PickerCategory::BuiltinMacros
                | PickerCategory::App
                | PickerCategory::SystemActions
                | PickerCategory::DesktopShortcuts
                | PickerCategory::AppActions
        )
}

/// What decides which sidebar categories the open picker shows.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SidebarOptions {
    pub development: bool,
    pub commands_enabled: bool,
    pub key_only: bool,
    pub hold_action: bool,
    pub macro_step: bool,
    pub shortcuts_failed: bool,
}

/// Every category in sidebar order with its visibility; `count` gives the
/// number of listed items of a category.
pub(super) fn sidebar(
    options: SidebarOptions,
    count: impl Fn(PickerCategory) -> usize,
) -> Vec<PickerSidebarEntry> {
    let actions = !options.key_only && !options.hold_action;
    KEY_CATEGORIES
        .into_iter()
        .chain(ACTION_CATEGORIES)
        .map(|category| {
            let key = is_key_category(category);
            let allowed = match category {
                PickerCategory::Macros
                | PickerCategory::Mouse
                | PickerCategory::SystemActions
                | PickerCategory::DesktopShortcuts
                | PickerCategory::AppActions => options.development,
                PickerCategory::Commands => options.development && options.commands_enabled,
                _ => true,
            };
            let shown = if key {
                count(category) > 0
            } else {
                actions
                    && match category {
                        PickerCategory::Pause => options.macro_step,
                        PickerCategory::DesktopShortcuts => {
                            count(category) > 0 || options.shortcuts_failed
                        }
                        PickerCategory::AppActions => count(category) > 0,
                        _ => true,
                    }
            };
            let first_action = if options.development {
                PickerCategory::Macros
            } else {
                PickerCategory::BuiltinMacros
            };
            PickerSidebarEntry {
                category,
                key,
                heading: allowed
                    && actions
                    && (category == PickerCategory::Letters || category == first_action),
                visible: allowed && shown,
            }
        })
        .collect()
}

/// Index of the item `delta` steps from `current`, kept inside the list.
pub(super) fn step_index(current: Option<usize>, len: usize, delta: i32) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let Some(current) = current else {
        return Some(0);
    };
    let next = (current as i64 + i64::from(delta)).clamp(0, len as i64 - 1);
    Some(next as usize)
}

/// Display name of `value`: its command, catalog item or saved command.
pub(super) fn label_for(
    value: &str,
    catalog: &[PickerItem],
    draft: Option<&Command>,
    commands: &[Command],
) -> SharedString {
    draft
        .filter(|command| assigns_command(value, &command.id))
        .map(|command| command.display_name().into())
        .or_else(|| {
            catalog
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.label.clone())
        })
        .or_else(|| {
            commands
                .iter()
                .find(|command| assigns_command(value, &command.id))
                .map(|command| command.display_name().into())
        })
        .unwrap_or_default()
}

/// Show `error` for the current value; editing the value clears it.
fn fail(picker: &ActionPicker, state: &RefCell<Session>, error: Msg) {
    picker.set_error(error.to_ui());
    state.borrow_mut().error_value = Some(picker.get_value());
}

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
    if target != PickerTarget::Rule {
        let rules = ui.global::<crate::ui::RulesEditor>();
        rules.set_field(crate::ui::RuleDialog::None);
        rules.set_adding(false);
    }
    load_desktop_shortcuts(ui.as_weak());
    let request = request(ui, doc, target, index, key_only);
    ui.global::<ActionPicker>()
        .set_trigger_chords(request.mode == PickerMode::Trigger);
    let catalog = catalog(ui, &drafts.config(doc, Some(target)));
    // The catalog generation is read after building, so shortcuts arriving
    // later still trigger a rebuild.
    let category = initial_category(&value, &catalog, target);
    let behavior = request.behavior(value.is_empty());
    let command = match Action::parse(Some(value.as_str())) {
        Action::Command(id) => drafts.command(&id),
        _ => None,
    };
    let generation = state.borrow().next_generation();
    *state.borrow_mut() = Session {
        generation,
        target: Some((target, index)),
        initial: value.clone(),
        catalog,
        catalog_shortcuts: SHORTCUTS_GENERATION.with(std::cell::Cell::get),
        error_value: None,
        command: command.clone(),
        layer_target: matches!(
            target,
            PickerTarget::LayerAction | PickerTarget::ExtraKey | PickerTarget::ExtraAction
        )
        .then(|| layers::picker_target(ui, doc)),
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
    picker.set_command_error(Msg::None.to_ui());
    picker.set_choosing_file(false);
    picker.set_capture_unsupported(false);
    picker.set_capture_status(crate::ui::CaptureStatus::None);
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
    picker.set_touched(false);
    picker.set_error(Msg::None.to_ui());
    picker.set_key_only(request.key_only);
    picker.set_trigger_chords(request.mode == PickerMode::Trigger);
    picker.set_macro_step(request.mode == PickerMode::MacroStep);
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

pub(super) fn refresh(
    ui: &SettingsWindow,
    doc: &Document,
    state: &RefCell<Session>,
    drafts: &Drafts,
) {
    let picker = ui.global::<ActionPicker>();
    {
        let mut state = state.borrow_mut();
        let shortcuts = SHORTCUTS_GENERATION.with(std::cell::Cell::get);
        if state.catalog_shortcuts != shortcuts {
            state.catalog = catalog(
                ui,
                &drafts.config(doc, state.target.map(|(target, _)| target)),
            );
            state.catalog_shortcuts = shortcuts;
        }
        if state
            .error_value
            .as_ref()
            .is_some_and(|value| *value != picker.get_value())
        {
            picker.set_error(Msg::None.to_ui());
            state.error_value = None;
        }
    }
    let state = state.borrow();
    let (development, commands_enabled) = {
        let config = doc.read();
        let settings = config.settings();
        (settings.in_development, settings.commands_enabled)
    };
    let filter = filter(ui, doc, &picker, state.target, &state.initial);
    let entries: Vec<&PickerItem> = state
        .catalog
        .iter()
        // Each action owns its command; existing ones are not offered.
        .filter(|entry| entry.category != PickerCategory::Commands && filter.admits(&entry.value))
        .collect();
    let sidebar = sidebar(
        SidebarOptions {
            development,
            commands_enabled,
            key_only: picker.get_key_only(),
            hold_action: picker.get_hold_action(),
            macro_step: picker.get_macro_step(),
            shortcuts_failed: picker.get_shortcuts_failed(),
        },
        |category| entries.iter().filter(|e| e.category == category).count(),
    );
    let value = picker.get_value();
    let command = doc
        .read()
        .profile()
        .commands
        .iter()
        .find(|command| assigns_command(&value, &command.id))
        .cloned();
    if let Some(command) = command
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
    picker.set_text_lines(text_lines(text));
    picker.set_pause_content(
        match &action {
            Action::Pause(ms) => ms.as_str(),
            _ => "",
        }
        .into(),
    );
    let query = picker.get_query().trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut category = picker.get_category();
    let visible = |category: PickerCategory| {
        sidebar
            .iter()
            .any(|entry| entry.category == category && entry.visible)
    };
    // A category the target does not offer falls back to the first one it does.
    if query.is_empty()
        && !visible(category)
        && let Some(entry) = sidebar
            .iter()
            .filter(|entry| entry.visible)
            .find(|entry| entry.key)
            .or_else(|| sidebar.iter().find(|entry| entry.visible))
    {
        category = entry.category;
        picker.set_category(category);
    }
    picker.set_category_is_key(is_key_category(category));
    picker.set_category_listed(is_listed(category));
    picker.set_sidebar(update_model(picker.get_sidebar(), sidebar));
    picker.set_items(update_model(
        picker.get_items(),
        entries
            .into_iter()
            .filter(|entry| {
                if query.is_empty() {
                    entry.category == category
                } else {
                    matches_query(&words, &entry.label, &entry.value)
                }
            })
            .cloned()
            .collect::<Vec<_>>(),
    ));
    if picker.get_keyboard_item() < 0
        || picker.get_keyboard_item() as usize >= picker.get_items().row_count()
    {
        picker.set_keyboard_item(0);
    }
    let config = drafts.config(doc, state.target.map(|(target, _)| target));
    picker.set_notice(execution_notice(&action, &config).to_ui());
    let valid = match picker.get_behavior() {
        PickerBehavior::Native => picker.get_allow_default(),
        PickerBehavior::Swallow => picker.get_allow_swallow(),
        PickerBehavior::Action => match state
            .command
            .as_ref()
            .filter(|command| assigns_command(&value, &command.id))
        {
            Some(command) => {
                filter.admits(&value) && commands_enabled && !command.script().trim().is_empty()
            }
            None => filter.accepts(&value, Some(&config)),
        },
    };
    picker.set_valid(valid);
}

/// Show `category`; entering Pause in a macro step starts with the
/// macro's step pause.
pub(super) fn select_category(ui: &SettingsWindow, doc: &Document, category: PickerCategory) {
    let picker = ui.global::<ActionPicker>();
    picker.set_category(category);
    picker.set_query("".into());
    if picker.get_macro_step()
        && category == PickerCategory::Pause
        && !matches!(
            Action::parse(Some(picker.get_value().as_str())),
            Action::Pause(_)
        )
    {
        let default = doc.read().settings().default_macro_step_pause_ms;
        let pause = macros::picker_macro_pause(ui).unwrap_or(default).clamp(
            tecloid_core::config::timing::PAUSE.min,
            tecloid_core::config::timing::PAUSE.max,
        );
        picker.set_value(
            Action::Pause(pause.to_string())
                .format()
                .unwrap_or_default()
                .into(),
        );
        picker.set_touched(true);
    }
    picker.invoke_refresh();
}

/// Move the selection `delta` items through the shown list.
pub(super) fn step_selection(ui: &SettingsWindow, delta: i32) {
    let picker = ui.global::<ActionPicker>();
    let items = picker.get_items();
    let value = picker.get_value();
    let current = items.iter().position(|item| item.value == value);
    let Some(next) = step_index(current, items.row_count(), delta) else {
        return;
    };
    let Some(item) = items.row_data(next) else {
        return;
    };
    picker.set_touched(true);
    picker.set_value(item.value);
    picker.invoke_refresh();
}

pub(super) fn apply(
    ui: &SettingsWindow,
    doc: &Document,
    session: &RefCell<Session>,
    drafts: &Drafts,
) {
    let picker = ui.global::<ActionPicker>();
    let layer_check = session.borrow().layer_target.as_ref().map(|target| {
        target.check(
            doc,
            &ui.global::<crate::ui::LayersEditor>().get_selected_id(),
        )
    });
    if let Some(Err(error)) = layer_check {
        fail(&picker, session, error);
        return;
    }
    picker.invoke_refresh();
    if !picker.get_valid() || picker.get_capturing() {
        return;
    }
    picker.set_error(Msg::None.to_ui());
    let swallow = picker.get_behavior() == PickerBehavior::Swallow;
    let value = if picker.get_behavior() == PickerBehavior::Action {
        picker.get_value()
    } else {
        "".into()
    };
    let (target, command, label) = {
        let state = session.borrow();
        let command = state
            .command
            .clone()
            .filter(|command| assigns_command(&value, &command.id));
        let label = label_for(
            &value,
            &state.catalog,
            command.as_ref(),
            &doc.read().profile().commands,
        );
        (state.target, command, label)
    };
    let Some((target, index)) = target else {
        return;
    };
    let choice = || layers::Choice {
        ignore: swallow,
        hold_modifiers: picker.get_hold_modifiers(),
        command: command.clone(),
    };
    let result = match target {
        PickerTarget::Rule => rules::choose(ui, doc, &value, command.clone()).map(|()| false),
        PickerTarget::MacroStep => {
            macros::choose(ui, doc, &drafts.macros, index, &value, command.clone()).map(|()| false)
        }
        PickerTarget::QuickAction => menus::choose(
            ui,
            doc,
            &drafts.menus,
            index,
            &value,
            &label,
            command.clone(),
        )
        .map(|()| false),
        PickerTarget::LayerAction => {
            layers::assign(ui, doc, index, &value, choice()).map(|()| false)
        }
        PickerTarget::ExtraKey | PickerTarget::ExtraAction => layers::assign_extra(
            ui,
            doc,
            index,
            &value,
            target == PickerTarget::ExtraKey,
            choice(),
        ),
        PickerTarget::BaseKey => {
            keys::assign(ui, doc, index, &value, command.clone()).map(|()| false)
        }
    };
    match result {
        Ok(next_step) => {
            picker.invoke_dismiss_picker();
            if next_step {
                layers::pick_new_extra_action(ui);
            }
        }
        Err(error) => fail(&picker, session, error),
    }
}
