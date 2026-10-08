//! Action picker: one dialog that chooses a key, shortcut or action for
//! every page and hands the value to the page that opened it.

use super::{keys, layers, rules};
use crate::{
    document::Document,
    i18n::Msg,
    ui::{
        ActionPicker, LayersEditor, Locale, MacroEditor, MenuEditor, PickerItem, PickerTarget,
        SettingsWindow,
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

/// Picker categories after the key categories of [`CATEGORIES`]; the
/// order matches the category list in `ui/action-picker.slint`.
const MACROS: i32 = CATEGORIES.len() as i32;
const SYSTEM_MACROS: i32 = MACROS + 1;
const COMMANDS: i32 = MACROS + 2;
const APP_ACTIONS: i32 = MACROS + 3;
const SYSTEM_ACTIONS: i32 = MACROS + 4;
/// Free-form categories without catalog entries.
const TEXT: i32 = MACROS + 5;
const PAUSE: i32 = MACROS + 6;
/// Categories with countable catalog entries.
const COUNTED: i32 = TEXT;

pub(crate) fn category_for_value(value: &str) -> i32 {
    for (prefix, category) in [
        ("macro:", MACROS),
        ("system-macro:", SYSTEM_MACROS),
        ("cmd:", COMMANDS),
        ("app:", APP_ACTIONS),
        ("sys:", SYSTEM_ACTIONS),
        ("text:", TEXT),
        ("pause:", PAUSE),
    ] {
        if value.starts_with(prefix) {
            return category;
        }
    }
    for key in value.split('+') {
        if let Some((index, _)) = CATEGORIES
            .iter()
            .enumerate()
            .find(|(_, keys)| keys.contains(&key))
        {
            return index as i32;
        }
    }
    5
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

fn catalog(ui: &SettingsWindow, config: &ConfigDocument) -> Vec<PickerItem> {
    let mut items: Vec<_> = CATEGORIES
        .iter()
        .enumerate()
        .flat_map(|(category, keys)| {
            keys.iter().map(move |key| PickerItem {
                value: (*key).into(),
                label: (*key).into(),
                category: category as i32,
                notice: Msg::None.to_ui(),
            })
        })
        .collect();
    let trusted = config.commands_trusted();
    let config = config.config();
    items.extend(actions::catalog(&config).into_iter().filter_map(|entry| {
        let category = match &entry.action {
            Action::Macro(id) if config.macros.iter().any(|m| m.id == *id) => MACROS,
            Action::Macro(_) => SYSTEM_MACROS,
            Action::Command(_) => COMMANDS,
            Action::App(_) => APP_ACTIONS,
            Action::System(_) => SYSTEM_ACTIONS,
            _ => return None,
        };
        let notice = match actions::execution_issue(&entry.action, &config, trusted) {
            Some(actions::ExecutionIssue::ApprovalRequired) => Msg::CommandApprovalRequired,
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
    items
}

fn valid(
    value: &str,
    key_only: bool,
    macro_step: bool,
    excluded: &str,
    config: Option<&ConfigDocument>,
) -> bool {
    if macro_step && value.trim().is_empty() {
        return false;
    }
    if key_only {
        return CATEGORIES.iter().any(|keys| keys.contains(&value));
    }
    if !excluded.is_empty() && value == format!("macro:{excluded}") {
        return false;
    }
    let action = Action::parse(Some(value));
    if let Action::Pause(ms) = &action {
        return macro_step && valid_pause(ms);
    }
    config.is_none_or(|config| actions::validate(&action, &config.config()).is_none())
}

/// The open picker: who receives the value and the catalog built on open.
#[derive(Default)]
struct Session {
    target: Option<(PickerTarget, i32)>,
    catalog: Vec<PickerItem>,
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let session = Rc::new(RefCell::new(Session::default()));
    let picker = ui.global::<ActionPicker>();

    let weak = ui.as_weak();
    picker.on_begin_capture(move || {
        CAPTURE_MODIFIERS.with(|state| state.borrow_mut().clear());
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(!picker.get_capturing());
        }
    });

    let weak = ui.as_weak();
    let (doc, state) = (document.clone(), session.clone());
    picker.on_open(move |target, index, value, key_only| {
        let Some(ui) = weak.upgrade() else { return };
        let catalog = catalog(&ui, &doc.read());
        let category = catalog
            .iter()
            .find(|item| item.value == value)
            .map(|item| item.category)
            .unwrap_or(if value.starts_with("text:") {
                TEXT
            } else if value.starts_with("pause:") && target == PickerTarget::MacroStep {
                PAUSE
            } else {
                0
            });
        *state.borrow_mut() = Session {
            target: Some((target, index)),
            catalog,
        };
        let picker = ui.global::<ActionPicker>();
        picker.set_layer_action(
            matches!(
                target,
                PickerTarget::LayerAction | PickerTarget::ExtraAction
            ) && ui.global::<LayersEditor>().get_dialog() == crate::ui::LayerDialog::None,
        );
        let rule_field = ui.global::<crate::ui::RulesEditor>().get_field();
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
        picker.set_error(Msg::None.to_ui());
        picker.set_key_only(key_only);
        picker.set_macro_step(target == PickerTarget::MacroStep);
        picker.set_original(value.clone());
        picker.set_value(value);
        picker.set_query("".into());
        picker.set_capturing(false);
        picker.set_category(category);
        picker.set_opened(true);
        picker.invoke_refresh();
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
        let entries: Vec<&PickerItem> = state
            .catalog
            .iter()
            .filter(|entry| {
                (!key_only || entry.category < MACROS)
                    && (excluded.is_empty() || entry.value != excluded_value)
            })
            .collect();
        picker.set_counts(ModelRc::new(VecModel::from(
            (0..COUNTED)
                .map(|category| entries.iter().filter(|e| e.category == category).count() as i32)
                .collect::<Vec<_>>(),
        )));
        let value = picker.get_value();
        picker.set_value_category(category_for_value(&value));
        picker.set_text_content(value.strip_prefix("text:").unwrap_or_default().into());
        picker.set_pause_content(value.strip_prefix("pause:").unwrap_or_default().into());
        let query = picker.get_query().to_lowercase();
        let category = picker.get_category();
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
        let notice = match actions::execution_issue(
            &Action::parse(Some(&value)),
            &doc.read().config(),
            doc.read().commands_trusted(),
        ) {
            Some(actions::ExecutionIssue::ApprovalRequired) => Msg::CommandApprovalRequired,
            Some(actions::ExecutionIssue::Unavailable) => Msg::ActionUnavailable,
            None => Msg::None,
        };
        picker.set_notice(notice.to_ui());
        picker.set_valid(valid(
            &value,
            key_only,
            picker.get_macro_step(),
            &excluded,
            Some(&doc.read()),
        ));
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
        let value = picker.get_value();
        let (target, label) = {
            let state = session.borrow();
            let label = state
                .catalog
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.label.clone())
                .unwrap_or_default();
            (state.target, label)
        };
        let Some((target, index)) = target else {
            return;
        };
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
            PickerTarget::LayerKey => ui.global::<LayersEditor>().set_dialog_key(value),
            PickerTarget::LayerAction => {
                if picker.get_layer_action() {
                    if let Err(error) = layers::assign(
                        &ui,
                        &doc,
                        index,
                        &value,
                        picker.get_ignore_key() && value.is_empty(),
                    ) {
                        picker.set_error(error.to_ui());
                    }
                } else {
                    ui.global::<LayersEditor>().set_dialog_action(value);
                }
            }
            PickerTarget::ExtraKey | PickerTarget::ExtraAction => {
                if let Err(error) = layers::assign_extra(
                    &ui,
                    &doc,
                    index,
                    &value,
                    target == PickerTarget::ExtraKey,
                    picker.get_ignore_key() && value.is_empty(),
                ) {
                    picker.set_error(error.to_ui());
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
        }
    });
}

/// Keyboard capture for the picker; returns `true` when `event` was used.
pub fn capture(ui: &SettingsWindow, event: &slint::winit_030::winit::event::WindowEvent) -> bool {
    use slint::winit_030::winit::{event::WindowEvent, keyboard::PhysicalKey};
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
    let PhysicalKey::Code(code) = event.physical_key else {
        return true;
    };
    if event.repeat {
        return true;
    }
    let value = CAPTURE_MODIFIERS.with(|state| {
        captured_key(
            code,
            event.state,
            picker.get_key_only(),
            &mut state.borrow_mut(),
        )
    });
    if let Some(value) = value {
        picker.set_value(value.into());
        picker.set_capturing(false);
        picker.invoke_refresh();
    }
    true
}

/// Config key code of a physical key. winit names its `KeyCode` variants
/// after the W3C `KeyboardEvent.code` values the config uses, so the
/// variant name is the code except where the config keeps older names.
fn key_name(code: slint::winit_030::winit::keyboard::KeyCode) -> String {
    let raw = format!("{code:?}");
    match raw.as_str() {
        "SuperLeft" => "MetaLeft".into(),
        "SuperRight" => "MetaRight".into(),
        "AudioVolumeUp" => "VolumeUp".into(),
        "AudioVolumeDown" => "VolumeDown".into(),
        "AudioVolumeMute" => "VolumeMute".into(),
        "MediaTrackNext" => "MediaNext".into(),
        "MediaTrackPrevious" => "MediaPrev".into(),
        "LaunchMediaPlayer" => "MediaSelect".into(),
        _ => raw,
    }
}

fn captured_key(
    code: slint::winit_030::winit::keyboard::KeyCode,
    state: slint::winit_030::winit::event::ElementState,
    key_only: bool,
    modifiers: &mut Vec<&'static str>,
) -> Option<String> {
    use slint::winit_030::winit::{event::ElementState, keyboard::KeyCode};
    let modifier = match code {
        KeyCode::ControlLeft | KeyCode::ControlRight => Some("Ctrl"),
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Some("Shift"),
        KeyCode::AltLeft | KeyCode::AltRight => Some("Alt"),
        KeyCode::SuperLeft | KeyCode::SuperRight => Some("Meta"),
        _ => None,
    };
    let key = key_name(code);
    let known = CATEGORIES.iter().any(|keys| keys.contains(&key.as_str()));
    if key_only {
        return (state == ElementState::Pressed && known).then_some(key);
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
            key
        } else {
            ["Ctrl", "Alt", "Shift", "Meta"]
                .into_iter()
                .filter(|m| modifiers.contains(m))
                .chain(std::iter::once(key.as_str()))
                .collect::<Vec<_>>()
                .join("+")
        };
        modifiers.clear();
        return Some(value);
    }
    None
}

thread_local! { static CAPTURE_MODIFIERS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_physical_keys_and_chords() {
        use slint::winit_030::winit::{
            event::ElementState::{Pressed, Released},
            keyboard::KeyCode as K,
        };
        let mut modifiers = Vec::new();
        assert_eq!(
            captured_key(K::ControlRight, Pressed, true, &mut modifiers).as_deref(),
            Some("ControlRight")
        );
        assert_eq!(
            captured_key(K::NumpadEnter, Pressed, true, &mut modifiers).as_deref(),
            Some("NumpadEnter")
        );
        assert_eq!(
            captured_key(K::ShiftLeft, Pressed, false, &mut modifiers),
            None
        );
        assert_eq!(
            captured_key(K::ControlRight, Pressed, false, &mut modifiers),
            None
        );
        assert_eq!(
            captured_key(K::KeyK, Pressed, false, &mut modifiers).as_deref(),
            Some("Ctrl+Shift+KeyK")
        );
        assert!(modifiers.is_empty());
        assert_eq!(
            captured_key(K::AltRight, Pressed, false, &mut modifiers),
            None
        );
        assert_eq!(
            captured_key(K::AltRight, Released, false, &mut modifiers).as_deref(),
            Some("AltRight")
        );
        assert_eq!(
            captured_key(K::AudioVolumeMute, Pressed, true, &mut modifiers).as_deref(),
            Some("VolumeMute")
        );
        assert_eq!(key_name(K::SuperLeft), "MetaLeft");
    }

    #[test]
    fn picker_modes_restrict_values() {
        assert!(valid("ControlRight", true, false, "", None));
        for value in ["", "Ctrl+KeyA", "macro:copyLine", "text:hello", "NotAKey"] {
            assert!(!valid(value, true, false, "", None));
        }
        assert!(!valid("pause:100", false, false, "", None));
        assert!(valid("pause:100", false, true, "", None));
        assert!(!valid("pause:10001", false, true, "", None));
        assert!(!valid("pause:0", false, true, "", None));
        assert!(!valid("macro:self", false, true, "self", None));
    }

    #[test]
    fn categories_follow_the_key_catalog() {
        assert_eq!(MACROS, 6);
        assert_eq!(PAUSE, 12);
        assert_eq!(category_for_value("CapsLock"), 0);
        assert_eq!(category_for_value("Ctrl+KeyV"), 1);
        assert_eq!(category_for_value("text:Ctrl+KeyV"), TEXT);
        assert_eq!(category_for_value("cmd:Ctrl+KeyV"), COMMANDS);
    }
}
