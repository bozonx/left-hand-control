//! Action picker: one dialog that chooses a key, shortcut or action for
//! every page and hands the value to the page that opened it.

use super::{keys, layers, rules};
use crate::{
    document::Document,
    i18n::Msg,
    keyboard,
    ui::{
        ActionPicker, LayersEditor, Locale, MacroEditor, MenuEditor, PickerCategory,
        PickerCategoryCount, PickerItem, PickerTarget, SettingsWindow, ShortcutDesktop,
    },
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        actions::{self, Action, ActionName},
        key_catalog::CATEGORIES,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

/// Picker category of each group of [`CATEGORIES`], in the same order.
const KEY_CATEGORIES: [PickerCategory; CATEGORIES.len()] = [
    PickerCategory::Letters,
    PickerCategory::Modifiers,
    PickerCategory::ControlKeys,
    PickerCategory::Navigation,
    PickerCategory::Media,
    PickerCategory::SystemKeys,
    PickerCategory::Mouse,
    PickerCategory::OtherKeys,
];

/// Sidebar order of the non-key categories. Commands go last: one
/// command per action, not a catalog.
const ACTION_CATEGORIES: [PickerCategory; 8] = [
    PickerCategory::Macros,
    PickerCategory::BuiltinMacros,
    PickerCategory::App,
    PickerCategory::SystemActions,
    PickerCategory::DesktopShortcuts,
    PickerCategory::Text,
    PickerCategory::Pause,
    PickerCategory::Commands,
];

fn is_key_category(category: PickerCategory) -> bool {
    KEY_CATEGORIES.contains(&category)
}

pub(crate) fn category_for_value(value: &str) -> PickerCategory {
    for (prefix, category) in [
        ("macro:", PickerCategory::Macros),
        ("cmd:", PickerCategory::Commands),
        ("app:", PickerCategory::App),
        ("sys:", PickerCategory::SystemActions),
        ("kde:", PickerCategory::DesktopShortcuts),
        ("text:", PickerCategory::Text),
        ("pause:", PickerCategory::Pause),
    ] {
        if value.starts_with(prefix) {
            return category;
        }
    }
    // The last key of a chord is the main one; modifiers come first.
    for key in value.split('+').rev() {
        if let Some(index) = CATEGORIES.iter().position(|keys| keys.contains(&key)) {
            return KEY_CATEGORIES[index];
        }
    }
    PickerCategory::OtherKeys
}

/// Longest pause a macro step may wait, in milliseconds.
pub const MAX_PAUSE_MS: u32 = 10_000;

/// Whether `ms` is a valid macro pause.
pub fn valid_pause(ms: &str) -> bool {
    ms.trim()
        .parse::<u32>()
        .is_ok_and(|n| (1..=MAX_PAUSE_MS).contains(&n))
}

/// Display name of a catalog action in the current UI language.
pub(crate) fn action_label(ui: &SettingsWindow, name: ActionName) -> Option<String> {
    match name {
        ActionName::Verbatim(name) if !name.is_empty() => Some(name),
        ActionName::System { id, n } | ActionName::App { id, n } => Some(
            ui.global::<Locale>()
                .invoke_text(Msg::PickerAction(id.into(), n).to_ui())
                .to_string(),
        ),
        _ => None,
    }
}

/// A native global shortcut of the desktop environment as the picker and
/// key caps show it.
pub(crate) struct DesktopShortcutItem {
    /// Backend-prefixed id, e.g. `kde:component/shortcut`.
    pub value: String,
    pub component: String,
    pub label: String,
}

thread_local! {
    /// Global shortcuts of the running desktop session; empty until read
    /// and on desktops without a shortcut backend.
    pub(crate) static DESKTOP_SHORTCUTS: RefCell<Vec<DesktopShortcutItem>> = const { RefCell::new(Vec::new()) };
}

/// Desktop environment whose native shortcuts this session can list.
fn shortcut_desktop() -> ShortcutDesktop {
    #[cfg(target_os = "linux")]
    {
        use lhc_core::platform::linux;
        if matches!(linux::detect().desktop, linux::Desktop::Kde) {
            return ShortcutDesktop::Kde;
        }
    }
    ShortcutDesktop::None
}

/// Re-read the desktop's global shortcuts in the background and refresh
/// an open picker when they arrive.
fn load_desktop_shortcuts(ui: slint::Weak<SettingsWindow>) {
    let read: fn() -> Result<Vec<DesktopShortcutItem>, String> = match shortcut_desktop() {
        ShortcutDesktop::None => return,
        ShortcutDesktop::Kde => read_kde_shortcuts,
    };
    std::thread::spawn(move || {
        let shortcuts = match read() {
            Ok(shortcuts) => shortcuts,
            Err(error) => {
                log::warn!("[picker] cannot read desktop global shortcuts: {error}");
                return;
            }
        };
        let _ = slint::invoke_from_event_loop(move || {
            DESKTOP_SHORTCUTS.with(|cache| *cache.borrow_mut() = shortcuts);
            if let Some(ui) = ui.upgrade()
                && ui.global::<ActionPicker>().get_opened()
            {
                ui.global::<ActionPicker>().invoke_refresh();
            }
        });
    });
}

fn read_kde_shortcuts() -> Result<Vec<DesktopShortcutItem>, String> {
    #[cfg(target_os = "linux")]
    {
        Ok(lhc_core::platform::kde_shortcuts::list()?
            .into_iter()
            .map(|shortcut| DesktopShortcutItem {
                value: format!("kde:{}", shortcut.id()),
                component: shortcut.component_label,
                label: shortcut.label,
            })
            .collect())
    }
    #[cfg(not(target_os = "linux"))]
    Ok(Vec::new())
}

fn catalog(ui: &SettingsWindow, config: &ConfigDocument) -> Vec<PickerItem> {
    let mut items: Vec<_> = CATEGORIES
        .iter()
        .enumerate()
        .flat_map(|(category, keys)| {
            keys.iter().map(move |key| PickerItem {
                value: (*key).into(),
                label: (*key).into(),
                category: KEY_CATEGORIES[category],
                notice: Msg::None.to_ui(),
            })
        })
        .collect();
    let enabled = config.settings().commands_enabled;
    let config = config.config();
    items.extend(actions::catalog(&config).into_iter().filter_map(|entry| {
        let category = match &entry.action {
            Action::Macro(id) if config.macros.iter().any(|m| m.id == *id) => {
                PickerCategory::Macros
            }
            Action::Macro(_) => PickerCategory::BuiltinMacros,
            Action::Command(_) if enabled => PickerCategory::Commands,
            Action::Command(_) => return None,
            Action::App(_) => PickerCategory::App,
            Action::System(_) => PickerCategory::SystemActions,
            _ => return None,
        };
        let notice = match actions::execution_issue(&entry.action, &config) {
            Some(actions::ExecutionIssue::CommandsDisabled) => Msg::CommandsDisabled,
            Some(actions::ExecutionIssue::Unavailable) => Msg::ActionUnavailable,
            None => Msg::None,
        };
        let value = entry.action.format()?;
        let label = action_label(ui, entry.name).unwrap_or_else(|| value.clone());
        Some(PickerItem {
            value: value.into(),
            label: label.into(),
            category,
            notice: notice.to_ui(),
        })
    }));
    DESKTOP_SHORTCUTS.with(|cache| {
        items.extend(cache.borrow().iter().map(|shortcut| PickerItem {
            value: shortcut.value.as_str().into(),
            label: format!("{}: {}", shortcut.component, shortcut.label).into(),
            category: PickerCategory::DesktopShortcuts,
            notice: Msg::None.to_ui(),
        }));
    });
    items
}

fn valid(
    value: &str,
    key_only: bool,
    macro_step: bool,
    excluded: &str,
    config: Option<&ConfigDocument>,
) -> bool {
    if value.trim().is_empty() {
        return false;
    }
    if key_only {
        return CATEGORIES.iter().any(|keys| keys.contains(&value));
    }
    if !excluded.is_empty() && value == format!("macro:{excluded}") {
        return false;
    }
    let action = Action::parse(Some(value));
    if matches!(&action, Action::Keys(_)) && !actions::valid_held_key(value) {
        return false;
    }
    if matches!(&action, Action::Text(text) if text.is_empty()) {
        return false;
    }
    if let Action::Pause(ms) = &action {
        return macro_step && valid_pause(ms);
    }
    config.is_none_or(|config| actions::validate(&action, &config.config()).is_none())
}

/// Warning for the command being edited: a missing working directory or
/// a script file that cannot be executed.
fn command_notice(picker: &ActionPicker) -> Msg {
    use lhc_core::mapper::commands::{DraftIssue, draft_issue};
    if !picker.get_value().starts_with("cmd:") {
        return Msg::None;
    }
    let directory = picker.get_command_directory();
    match draft_issue(
        &picker.get_command_script(),
        Some(directory.as_str()).filter(|d| !d.trim().is_empty()),
    ) {
        Some(DraftIssue::MissingDirectory) => Msg::CommandDirectoryMissing,
        Some(DraftIssue::NotExecutable) => Msg::ScriptNotExecutable,
        None => Msg::None,
    }
}

/// The open picker: who receives the value and the catalog built on open.
#[derive(Default)]
struct Session {
    target: Option<(PickerTarget, i32)>,
    catalog: Vec<PickerItem>,
    command: Option<lhc_core::profile::model::Command>,
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let session = Rc::new(RefCell::new(Session::default()));
    let picker = ui.global::<ActionPicker>();
    picker.set_shortcut_desktop(shortcut_desktop());
    load_desktop_shortcuts(ui.as_weak());

    let weak = ui.as_weak();
    picker.on_begin_capture(move || {
        CAPTURE_MODIFIERS.with(|state| state.borrow_mut().clear());
        CAPTURE_TRIGGER.with(|state| state.borrow_mut().clear());
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(!picker.get_capturing());
        }
    });

    let weak = ui.as_weak();
    picker.on_select_behavior(move |behavior| {
        let Some(ui) = weak.upgrade() else { return };
        let picker = ui.global::<ActionPicker>();
        if (behavior == 0 && !picker.get_allow_default())
            || (behavior == 1 && !picker.get_allow_swallow())
            || !(0..=2).contains(&behavior)
        {
            return;
        }
        picker.set_behavior(behavior);
        picker.set_ignore_key(behavior == 1);
        picker.set_capturing(false);
        picker.set_error(Msg::None.to_ui());
        picker.invoke_refresh();
    });

    let weak = ui.as_weak();
    let (doc, state) = (document.clone(), session.clone());
    picker.on_open(move |target, index, value, key_only| {
        let Some(ui) = weak.upgrade() else { return };
        load_desktop_shortcuts(ui.as_weak());
        let catalog = catalog(&ui, &doc.read());
        let category = catalog
            .iter()
            .find(|item| item.value == value)
            .map(|item| item.category)
            .unwrap_or(if value.starts_with("text:") {
                PickerCategory::Text
            } else if value.starts_with("kde:") {
                PickerCategory::DesktopShortcuts
            } else if value.starts_with("pause:") && target == PickerTarget::MacroStep {
                PickerCategory::Pause
            } else {
                PickerCategory::Modifiers
            });
        *state.borrow_mut() = Session {
            target: Some((target, index)),
            catalog,
            command: None,
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
        picker.set_command_name("".into());
        picker.set_command_script("".into());
        picker.set_command_directory("".into());
        picker.set_command_notice(Msg::None.to_ui());
        picker.set_layer_action(
            matches!(
                target,
                PickerTarget::LayerAction | PickerTarget::ExtraAction
            ) && ui.global::<LayersEditor>().get_dialog() == crate::ui::LayerDialog::None,
        );
        let rule_field = ui.global::<crate::ui::RulesEditor>().get_field();
        picker.set_title_context(if target == PickerTarget::MacroStep {
            4
        } else if target == PickerTarget::Rule {
            match rule_field {
                crate::ui::RuleDialog::DoubleTap => 1,
                crate::ui::RuleDialog::Tap => 2,
                crate::ui::RuleDialog::Hold => 3,
                crate::ui::RuleDialog::LongHold => 5,
                _ => 0,
            }
        } else {
            0
        });
        let rule_action = target == PickerTarget::Rule
            && matches!(
                rule_field,
                crate::ui::RuleDialog::Tap | crate::ui::RuleDialog::Hold
            );
        picker.set_rule_action(rule_action);
        picker.set_ignore_key(
            (picker.get_layer_action()
                && ui.global::<LayersEditor>().get_assignment() == crate::ui::Assignment::Swallow)
                || (rule_action
                    && if rule_field == crate::ui::RuleDialog::Tap {
                        ui.global::<crate::ui::RulesEditor>().get_swallow_tap()
                    } else {
                        ui.global::<crate::ui::RulesEditor>().get_swallow_hold()
                    }),
        );
        let standalone_layer = picker.get_layer_action();
        picker.set_hold_modifiers(
            standalone_layer && layers::holds_modifiers(&ui, &doc, target, index),
        );
        // An extra row exists only while it has an assignment, so it is
        // removed with its delete button rather than by inheriting.
        let extra_action = target == PickerTarget::ExtraAction;
        let double_tap = target == PickerTarget::Rule
            && matches!(
                rule_field,
                crate::ui::RuleDialog::DoubleTap | crate::ui::RuleDialog::LongHold
            );
        // The advanced rule dialog chooses "trigger itself" / "nothing" for
        // the held key; the picker only picks the key.
        let held_key = rule_action && rule_field == crate::ui::RuleDialog::Hold;
        let allow_default = !key_only
            && ((standalone_layer && !extra_action)
                || (rule_action && !held_key)
                || (double_tap && rule_field != crate::ui::RuleDialog::LongHold)
                || matches!(target, PickerTarget::BaseKey));
        picker.set_allow_default(allow_default);

        picker.set_inherit_default(standalone_layer && !extra_action);
        picker.set_empty_default(double_tap);
        picker.set_quick_action(target == PickerTarget::QuickAction);
        picker.set_hold_action(rule_action && rule_field == crate::ui::RuleDialog::Hold);
        picker.set_hold_layer(
            picker.get_hold_action()
                && !ui.global::<crate::ui::RulesEditor>().get_layer().is_empty(),
        );
        picker.set_allow_swallow(
            standalone_layer || (rule_action && !held_key && !picker.get_hold_layer()),
        );
        picker.set_behavior(if picker.get_ignore_key() && picker.get_allow_swallow() {
            1
        } else if allow_default && (value.is_empty() || picker.get_ignore_key()) {
            0
        } else {
            2
        });
        picker.set_ignore_key(picker.get_behavior() == 1);
        picker.set_touched(false);
        picker.set_error(Msg::None.to_ui());
        picker.set_key_only(key_only);
        picker.set_trigger_chords(
            key_only && target == PickerTarget::Rule && rule_field == crate::ui::RuleDialog::Key,
        );
        picker.set_macro_step(target == PickerTarget::MacroStep);
        let original = if picker.get_behavior() == 1 {
            ui.global::<Locale>()
                .invoke_text(Msg::PickerSuppress.to_ui())
        } else if picker.get_behavior() == 0 {
            ui.global::<Locale>().invoke_text(
                if picker.get_inherit_default() {
                    Msg::PickerInherit
                } else if picker.get_quick_action() {
                    Msg::PickerEmptyCell
                } else if picker.get_empty_default() {
                    Msg::PickerNoAction
                } else if picker.get_hold_layer() {
                    Msg::PickerLayerOnly
                } else {
                    Msg::PickerNative
                }
                .to_ui(),
            )
        } else {
            ui.global::<ActionPicker>().get_original()
        };
        let original = if picker.get_behavior() == 2 {
            state
                .borrow()
                .catalog
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.label.clone())
                .unwrap_or_else(|| value.clone())
        } else {
            original
        };
        if value.starts_with("cmd:") && original == value {
            let config = doc.read();
            if let Some(command) = config
                .layout()
                .commands
                .iter()
                .find(|command| value == format!("cmd:{}", command.id))
            {
                picker.set_original(
                    if command.name.is_empty() {
                        command.script().to_owned()
                    } else {
                        command.name.clone()
                    }
                    .into(),
                );
            } else {
                picker.set_original(original);
            }
        } else {
            picker.set_original(original);
        }
        picker.set_value(value);
        picker.set_query("".into());
        picker.set_capturing(false);
        picker.set_category(category);
        picker.set_opened(true);
        picker.invoke_refresh();
    });

    let weak = ui.as_weak();
    let (doc, state) = (document.clone(), session.clone());
    picker.on_edit_command(move || {
        let Some(ui) = weak.upgrade() else { return };
        if !doc.read().settings().commands_enabled {
            return;
        }
        let picker = ui.global::<ActionPicker>();
        let value = picker.get_value().to_string();
        let id = value
            .strip_prefix("cmd:")
            .map(str::to_owned)
            .unwrap_or_else(|| lhc_core::profile::ids::generate("cmd_"));
        // Keep the script of the other operating systems.
        let mut command = doc
            .read()
            .layout()
            .commands
            .iter()
            .find(|command| command.id == id)
            .cloned()
            .unwrap_or_else(|| lhc_core::profile::model::Command {
                id: id.clone(),
                name: String::new(),
                linux: String::new(),
                windows: String::new(),
                working_directory: None,
            });
        command.name = picker.get_command_name().to_string();
        command.set_script(picker.get_command_script().to_string());
        command.working_directory = (!picker.get_command_directory().trim().is_empty())
            .then(|| picker.get_command_directory().trim().to_owned());
        state.borrow_mut().command = Some(command);
        picker.set_value(format!("cmd:{id}").into());
        picker.set_touched(true);
        picker.invoke_refresh();
    });

    for directory in [false, true] {
        let weak = ui.as_weak();
        let choose = move || {
            let Some(ui) = weak.upgrade() else { return };
            let title = ui
                .global::<Locale>()
                .invoke_text(
                    if directory {
                        Msg::ChooseCommandDirectory
                    } else {
                        Msg::ChooseCommandScript
                    }
                    .to_ui(),
                )
                .to_string();
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let picked = lhc_core::platform::file_chooser::pick(&title, directory);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let picker = ui.global::<ActionPicker>();
                    match picked {
                        Ok(Some(path)) => {
                            let path = path.to_string_lossy();
                            if directory {
                                picker.set_command_directory(path.as_ref().into());
                            } else {
                                let quoted = lhc_core::mapper::commands::invoke_path(&path);
                                let script = picker.get_command_script();
                                picker.set_command_script(if script.trim().is_empty() {
                                    quoted.into()
                                } else {
                                    format!("{} {quoted}", script.trim_end()).into()
                                });
                            }
                            picker.invoke_edit_command();
                        }
                        Ok(None) => {}
                        Err(error) => picker.set_error(Msg::ActionFailed(error).to_ui()),
                    }
                });
            });
        };
        if directory {
            picker.on_choose_command_directory(choose);
        } else {
            picker.on_choose_command_file(choose);
        }
    }

    let weak = ui.as_weak();
    picker.on_open_command_settings(move || {
        let Some(ui) = weak.upgrade() else { return };
        ui.global::<ActionPicker>().invoke_dismiss_picker();
        ui.global::<crate::ui::SettingsEditor>()
            .set_tab(crate::ui::SettingsTab::Commands);
        ui.invoke_navigate(crate::ui::Page::Settings, crate::ui::MenuKind::Emoji);
    });

    let weak = ui.as_weak();
    let (doc, state) = (document.clone(), session.clone());
    picker.on_refresh(move || {
        let Some(ui) = weak.upgrade() else { return };
        let picker = ui.global::<ActionPicker>();
        let excluded = if picker.get_macro_step() {
            let editor = ui.global::<MacroEditor>();
            slint::Model::row_data(
                &editor.get_macros(),
                editor.get_picker_macro().max(0) as usize,
            )
            .map(|row| row.id)
            .unwrap_or_default()
        } else {
            "".into()
        };
        let excluded_value = format!("macro:{excluded}");
        let key_only = picker.get_key_only();
        state.borrow_mut().catalog = catalog(&ui, &doc.read());
        let state = state.borrow();
        let unavailable_keys: Vec<String> = if state
            .target
            .is_some_and(|(target, _)| target == PickerTarget::ExtraKey)
        {
            let editor = ui.global::<LayersEditor>();
            let selected = editor.get_selected_id();
            let current = editor.get_dialog_key();
            let config = doc.read();
            keyboard::layer_keys()
                .map(str::to_owned)
                .chain(
                    config
                        .layout()
                        .layer_keymaps
                        .get(selected.as_str())
                        .into_iter()
                        .flat_map(|map| map.keys.keys())
                        .filter(|key| key.as_str() != current.as_str())
                        .cloned(),
                )
                .collect()
        } else {
            Vec::new()
        };
        let entries: Vec<&PickerItem> = state
            .catalog
            .iter()
            .filter(|entry| {
                (!key_only || is_key_category(entry.category))
                    // Each action owns its command; existing ones are not offered.
                    && entry.category != PickerCategory::Commands
                    && !unavailable_keys
                        .iter()
                        .any(|key| key == entry.value.as_str())
                    && (!picker.get_hold_action() || actions::valid_held_key(&entry.value))
                    && (!(key_only
                        && state
                            .target
                            .is_some_and(|(target, _)| target == PickerTarget::Rule))
                        || lhc_core::profile::key_catalog::valid_trigger(&entry.value))
                    && (excluded.is_empty() || entry.value != excluded_value)
            })
            .collect();
        picker.set_categories(ModelRc::new(VecModel::from(
            KEY_CATEGORIES
                .into_iter()
                .chain(ACTION_CATEGORIES)
                .map(|category| PickerCategoryCount {
                    category,
                    count: entries.iter().filter(|e| e.category == category).count() as i32,
                })
                .collect::<Vec<_>>(),
        )));
        if picker.get_macro_step()
            && picker.get_category() == PickerCategory::Pause
            && !matches!(
                Action::parse(Some(picker.get_value().as_str())),
                Action::Pause(_)
            )
        {
            let editor = ui.global::<MacroEditor>();
            let pause = slint::Model::row_data(
                &editor.get_macros(),
                editor.get_picker_macro().max(0) as usize,
            )
            .and_then(|row| row.step_pause.trim().parse::<u64>().ok())
            .unwrap_or_else(|| doc.read().settings().default_macro_step_pause_ms)
            .clamp(1, u64::from(MAX_PAUSE_MS));
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
            .find(|command| value == format!("cmd:{}", command.id))
            && state
                .command
                .as_ref()
                .is_none_or(|draft| draft.id != command.id)
        {
            picker.set_command_name(command.name.clone().into());
            picker.set_command_script(command.script().into());
            picker.set_command_directory(
                command.working_directory.clone().unwrap_or_default().into(),
            );
        }
        picker.set_command_notice(command_notice(&picker).to_ui());
        picker.set_value_category(category_for_value(&value));
        let text = value.strip_prefix("text:").unwrap_or_default();
        picker.set_text_content(text.into());
        picker.set_text_preview(super::one_line(text).into());
        picker.set_text_lines(if text.contains('\n') {
            text.split('\n').count() as i32
        } else {
            0
        });
        picker.set_pause_content(value.strip_prefix("pause:").unwrap_or_default().into());
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
        picker.set_items(ModelRc::new(VecModel::from(
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
        )));
        let notice =
            match actions::execution_issue(&Action::parse(Some(&value)), &doc.read().config()) {
                Some(actions::ExecutionIssue::CommandsDisabled) => Msg::CommandsDisabled,
                Some(actions::ExecutionIssue::Unavailable) => Msg::ActionUnavailable,
                None => Msg::None,
            };
        picker.set_notice(notice.to_ui());
        picker.set_valid(
            ((picker.get_behavior() == 0 && picker.get_allow_default())
                || (picker.get_behavior() == 1 && picker.get_allow_swallow()))
                || (picker.get_behavior() == 2
                    && state
                        .command
                        .as_ref()
                        .filter(|command| value == format!("cmd:{}", command.id))
                        .map_or_else(
                            || {
                                if key_only
                                    && state
                                        .target
                                        .is_some_and(|(target, _)| target == PickerTarget::Rule)
                                {
                                    lhc_core::profile::key_catalog::valid_trigger(&value)
                                } else {
                                    valid(
                                        &value,
                                        key_only,
                                        picker.get_macro_step(),
                                        &excluded,
                                        Some(&doc.read()),
                                    )
                                }
                            },
                            |command| {
                                doc.read().settings().commands_enabled
                                    && !command.script().trim().is_empty()
                            },
                        )
                    && (!picker.get_hold_action() || actions::valid_held_key(&value))
                    && (!(key_only
                        && state
                            .target
                            .is_some_and(|(target, _)| target == PickerTarget::Rule))
                        || lhc_core::profile::key_catalog::valid_trigger(&value))),
        );
    });

    let weak = ui.as_weak();
    let state = session.clone();
    picker.on_dismiss_picker(move || {
        state.borrow_mut().target = None;
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(false);
            picker.set_opened(false);
        }
    });

    let weak = ui.as_weak();
    let doc = document.clone();
    picker.on_apply(move || {
        let Some(ui) = weak.upgrade() else { return };
        let picker = ui.global::<ActionPicker>();
        picker.invoke_refresh();
        if !picker.get_valid() || picker.get_capturing() {
            return;
        }
        picker.set_error(Msg::None.to_ui());
        let value = if picker.get_behavior() == 2 {
            picker.get_value()
        } else {
            "".into()
        };
        let command = session
            .borrow()
            .command
            .clone()
            .filter(|command| value == format!("cmd:{}", command.id));
        if let Some(command) = command
            && let Err(error) = doc.edit(crate::document::View::Shell, |config| {
                config.save_command(command)
            })
        {
            picker.set_error(Msg::from(&error).to_ui());
            return;
        }
        let (target, label) = {
            let state = session.borrow();
            let label = state
                .catalog
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.label.clone())
                .or_else(|| {
                    doc.read()
                        .layout()
                        .commands
                        .iter()
                        .find(|command| value == format!("cmd:{}", command.id))
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
                if let Err(error) = rules::choose(&ui, &doc, &value) {
                    picker.set_error(error.to_ui());
                }
            }
            PickerTarget::MacroStep => {
                let editor = ui.global::<MacroEditor>();
                editor.invoke_set_step(editor.get_picker_macro(), index, value);
            }
            PickerTarget::QuickAction => ui
                .global::<MenuEditor>()
                .invoke_set_action(index, value, label),
            PickerTarget::LayerAction => {
                if picker.get_layer_action() {
                    if let Err(error) = layers::assign(
                        &ui,
                        &doc,
                        index,
                        &value,
                        picker.get_ignore_key() && value.is_empty(),
                        picker.get_hold_modifiers(),
                    ) {
                        picker.set_error(error.to_ui());
                    }
                } else {
                    ui.global::<LayersEditor>().set_dialog_action(value);
                }
            }
            PickerTarget::ExtraKey | PickerTarget::ExtraAction => {
                match layers::assign_extra(
                    &ui,
                    &doc,
                    index,
                    &value,
                    target == PickerTarget::ExtraKey,
                    picker.get_ignore_key() && value.is_empty(),
                    picker.get_hold_modifiers(),
                ) {
                    Ok(next) => next_step = next,
                    Err(error) => picker.set_error(error.to_ui()),
                }
            }
            PickerTarget::BaseKey => {
                if let Err(error) = keys::assign(&ui, &doc, index, &value) {
                    picker.set_error(error.to_ui());
                }
            }
        }
        if picker.get_error().id.is_empty() {
            picker.invoke_dismiss_picker();
            if next_step {
                layers::pick_new_extra_action(&ui);
            }
        }
    });
}

/// Keyboard capture for the picker; returns `true` to withhold the event from Slint.
pub fn capture(ui: &SettingsWindow, event: &slint::winit_030::winit::event::WindowEvent) -> bool {
    use slint::winit_030::winit::event::WindowEvent;
    let picker = ui.global::<ActionPicker>();
    if !picker.get_opened() || !picker.get_capturing() {
        return false;
    }
    if let WindowEvent::Focused(false) = event {
        picker.set_capturing(false);
        return false;
    }
    let WindowEvent::KeyboardInput { event, .. } = event else {
        return false;
    };
    let Some(key) = physical_key_name(event.physical_key) else {
        return true;
    };
    if event.repeat {
        return true;
    }
    let value = if picker.get_trigger_chords() {
        CAPTURE_TRIGGER.with(|state| captured_trigger(&key, event.state, &mut state.borrow_mut()))
    } else {
        CAPTURE_MODIFIERS.with(|state| {
            captured_key(
                &key,
                event.state,
                picker.get_key_only(),
                &mut state.borrow_mut(),
            )
        })
    };
    if let Some(value) = value {
        picker.set_touched(true);
        picker.set_value(value.into());
        picker.set_capturing(false);
        picker.invoke_refresh();
    }
    // Slint tracks modifiers from key events, not ModifiersChanged. Swallowing
    // a release here can leave Ctrl/Shift stuck when capture ends and block typing.
    lhc_core::profile::key_catalog::modifier_key(&key).is_none()
}

/// Config key code of a physical key. Keys winit cannot name on Linux
/// (most media, launch and laptop keys) arrive with their evdev code.
fn physical_key_name(key: slint::winit_030::winit::keyboard::PhysicalKey) -> Option<String> {
    use slint::winit_030::winit::keyboard::PhysicalKey;
    match key {
        PhysicalKey::Code(code) => Some(key_name(code)),
        #[cfg(target_os = "linux")]
        PhysicalKey::Unidentified(slint::winit_030::winit::keyboard::NativeKeyCode::Xkb(code)) => {
            u16::try_from(code)
                .ok()
                .and_then(lhc_core::mapper::keys::evdev_to_code)
                .map(str::to_owned)
        }
        PhysicalKey::Unidentified(_) => None,
    }
}

/// Config key code of a winit key code. winit names its `KeyCode` variants
/// after the W3C `KeyboardEvent.code` values the config uses, so the
/// variant name is the code except for the Meta keys.
fn key_name(code: slint::winit_030::winit::keyboard::KeyCode) -> String {
    let raw = format!("{code:?}");
    match raw.as_str() {
        "SuperLeft" => "MetaLeft".into(),
        "SuperRight" => "MetaRight".into(),
        _ => raw,
    }
}

fn captured_key(
    key: &str,
    state: slint::winit_030::winit::event::ElementState,
    key_only: bool,
    modifiers: &mut Vec<&'static str>,
) -> Option<String> {
    use slint::winit_030::winit::event::ElementState;
    let modifier = match key {
        "ControlLeft" | "ControlRight" => Some("Ctrl"),
        "ShiftLeft" | "ShiftRight" => Some("Shift"),
        "AltLeft" | "AltRight" => Some("Alt"),
        "MetaLeft" | "MetaRight" => Some("Meta"),
        _ => None,
    };
    let known = lhc_core::profile::key_catalog::valid_key(key);
    if key_only {
        return (state == ElementState::Pressed && known).then(|| key.to_owned());
    }
    if let Some(name) = modifier
        && state == ElementState::Pressed
    {
        if !modifiers.contains(&name) {
            modifiers.push(name);
        }
        return None;
    }
    if (state == ElementState::Pressed || modifier.is_some()) && known {
        let value = if modifier.is_some() {
            key.to_owned()
        } else {
            ["Ctrl", "Alt", "Shift", "Meta"]
                .into_iter()
                .filter(|m| modifiers.contains(m))
                .chain(std::iter::once(key))
                .collect::<Vec<_>>()
                .join("+")
        };
        modifiers.clear();
        return Some(value);
    }
    None
}

fn captured_trigger(
    key: &str,
    state: slint::winit_030::winit::event::ElementState,
    keys: &mut Vec<String>,
) -> Option<String> {
    if !lhc_core::profile::key_catalog::valid_trigger(key) {
        return None;
    }
    if state == slint::winit_030::winit::event::ElementState::Pressed {
        if !keys.iter().any(|k| k == key) {
            keys.push(key.to_owned());
        }
        None
    } else if keys.iter().any(|k| k == key) {
        let value = keys.join("+");
        keys.clear();
        Some(value)
    } else {
        None
    }
}

thread_local! { static CAPTURE_TRIGGER: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
thread_local! { static CAPTURE_MODIFIERS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }

#[cfg(test)]
#[path = "picker/tests.rs"]
mod tests;
