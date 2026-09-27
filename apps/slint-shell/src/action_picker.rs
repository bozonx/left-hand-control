use crate::ui::{ActionPicker, MacroEditor, MenuEditor, PickerItem, SettingsWindow};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        actions::{self, Action, ActionName},
        key_catalog::CATEGORIES,
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn catalog(ui: &SettingsWindow, config: Option<&ConfigDocument>) -> Vec<PickerItem> {
    let mut items: Vec<_> = CATEGORIES
        .iter()
        .enumerate()
        .flat_map(|(category, keys)| {
            keys.iter().map(move |key| PickerItem {
                value: (*key).into(),
                label: (*key).into(),
                category: category as i32,
            })
        })
        .collect();
    if let Some(config) = config {
        let config = config.config();
        items.extend(actions::catalog(&config).into_iter().filter_map(|entry| {
            let category = match &entry.action {
                Action::Macro(id) if config.macros.iter().any(|m| m.id == *id) => 6,
                Action::Macro(_) => 7,
                Action::Command(_) => 8,
                Action::App(_) => 9,
                Action::System(_) => 10,
                _ => return None,
            };
            let value = entry.action.format()?;
            let label = match entry.name {
                ActionName::Verbatim(name) if !name.is_empty() => name,
                ActionName::System { id, n } | ActionName::App { id, n } => ui
                    .global::<crate::ui::Locale>()
                    .invoke_text(crate::i18n::Msg::PickerAction(id.into(), n).to_ui())
                    .to_string(),
                _ => value.clone(),
            };
            Some(PickerItem {
                value: value.into(),
                label: label.into(),
                category,
            })
        }));
    }
    items
}

fn valid(
    value: &str,
    key_only: bool,
    macro_step: bool,
    excluded: &str,
    config: Option<&ConfigDocument>,
) -> bool {
    if key_only {
        return CATEGORIES.iter().any(|keys| keys.contains(&value));
    }
    if !excluded.is_empty() && value == format!("macro:{excluded}") {
        return false;
    }
    let action = Action::parse(Some(value));
    if let Action::Pause(ms) = &action {
        return macro_step && ms.parse::<u32>().is_ok_and(|n| (1..=10000).contains(&n));
    }
    config.is_none_or(|c| actions::validate(&action, &c.config()).is_none())
}

pub fn bind(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    let weak = ui.as_weak();
    ui.global::<ActionPicker>().on_begin_capture(move || {
        CAPTURE_MODIFIERS.with(|state| state.borrow_mut().clear());
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(!picker.get_capturing());
        }
    });
    let target = Rc::new(RefCell::new((0, 0)));
    let weak = ui.as_weak();
    let cfg = config.clone();
    let dest = target.clone();
    ui.global::<ActionPicker>()
        .on_open(move |kind, index, value, key_only| {
            let Some(ui) = weak.upgrade() else { return };
            *dest.borrow_mut() = (kind, index);
            let picker = ui.global::<ActionPicker>();
            picker.set_error(crate::i18n::Msg::None.to_ui());
            picker.set_key_only(key_only);
            picker.set_macro_step(kind == 2);
            picker.set_original(value.clone());
            picker.set_value(value.clone());
            picker.set_query("".into());
            picker.set_capturing(false);
            let entries = catalog(&ui, cfg.as_ref().map(|c| c.borrow()).as_deref());
            picker.set_category(entries.iter().find(|e| e.value == value).map_or(
                if value.starts_with("text:") {
                    11
                } else if value.starts_with("pause:") && kind == 2 {
                    12
                } else {
                    0
                },
                |e| e.category,
            ));
            picker.set_opened(true);
            picker.invoke_refresh();
        });
    let apply_config = config.clone();
    let weak = ui.as_weak();
    ui.global::<ActionPicker>().on_refresh(move || {
        let Some(ui) = weak.upgrade() else { return };
        let picker = ui.global::<ActionPicker>();
        let document = config.as_ref().map(|c| c.borrow());
        let excluded = if picker.get_macro_step() {
            let editor = ui.global::<MacroEditor>();
            editor
                .get_macros()
                .row_data(editor.get_picker_macro().max(0) as usize)
                .map(|row| row.id)
                .unwrap_or_default()
        } else {
            "".into()
        };
        let entries: Vec<_> = catalog(&ui, document.as_deref())
            .into_iter()
            .filter(|entry| {
                (!picker.get_key_only() || entry.category < 6)
                    && (excluded.is_empty() || entry.value != format!("macro:{excluded}"))
            })
            .collect();
        picker.set_counts(ModelRc::new(VecModel::from(
            (0..11)
                .map(|cat| entries.iter().filter(|e| e.category == cat).count() as i32)
                .collect::<Vec<_>>(),
        )));
        picker.set_text_content(
            picker
                .get_value()
                .strip_prefix("text:")
                .unwrap_or_default()
                .into(),
        );
        picker.set_pause_content(
            picker
                .get_value()
                .strip_prefix("pause:")
                .unwrap_or_default()
                .into(),
        );
        let query = picker.get_query().to_lowercase();
        picker.set_items(ModelRc::new(VecModel::from(
            entries
                .into_iter()
                .filter(|entry| {
                    if query.is_empty() {
                        entry.category == picker.get_category()
                    } else {
                        entry.label.to_lowercase().contains(&query)
                            || entry.value.to_lowercase().contains(&query)
                    }
                })
                .collect::<Vec<_>>(),
        )));
        picker.set_valid(valid(
            &picker.get_value(),
            picker.get_key_only(),
            picker.get_macro_step(),
            &excluded,
            document.as_deref(),
        ));
    });
    let weak = ui.as_weak();
    let close_target = target.clone();
    ui.global::<ActionPicker>().on_close(move || {
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(false);
            picker.set_opened(false);
            if close_target.borrow().0 == 6 {
                ui.invoke_cancel();
            }
        }
    });
    let weak = ui.as_weak();
    ui.global::<ActionPicker>().on_apply(move || {
        let Some(ui) = weak.upgrade() else { return };
        let picker = ui.global::<ActionPicker>();
        picker.invoke_refresh();
        if !picker.get_valid() || picker.get_capturing() {
            return;
        }
        picker.set_error(crate::i18n::Msg::None.to_ui());
        let value = picker.get_value();
        match *target.borrow() {
            (0, _) => ui.invoke_choose_rule_value(value),
            (6, _) => {
                ui.invoke_change_kind(3);
                ui.set_value(value);
                ui.invoke_save();
                if ui.get_editing() {
                    picker.set_error(ui.get_validation());
                    return;
                }
            }
            (1, _) => {
                ui.invoke_change_kind(3);
                ui.set_value(value);
                ui.invoke_validate();
            }
            (2, index) => {
                let editor = ui.global::<MacroEditor>();
                editor.invoke_set_step(editor.get_picker_macro(), index, value);
            }
            (3, index) => {
                let label = catalog(&ui, apply_config.as_ref().map(|c| c.borrow()).as_deref())
                    .into_iter()
                    .find(|item| item.value == value)
                    .map(|item| item.label)
                    .unwrap_or_default();
                ui.global::<MenuEditor>().invoke_set_action(index, value, label);
            }
            (4, _) => ui.set_layer_dialog_key(value),
            (5, _) => ui.set_layer_dialog_action(value),
            _ => {}
        }
        if picker.get_error().id.is_empty() {
            picker.invoke_close();
        }
    });
}

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
    let raw = format!("{code:?}");
    let key = match raw.as_str() {
        "SuperLeft" => "MetaLeft",
        "SuperRight" => "MetaRight",
        "AudioVolumeUp" => "VolumeUp",
        "AudioVolumeDown" => "VolumeDown",
        "AudioVolumeMute" => "VolumeMute",
        "MediaTrackNext" => "MediaNext",
        "MediaTrackPrevious" => "MediaPrev",
        "LaunchMediaPlayer" => "MediaSelect",
        "WakeUp" => "WakeUp",
        other => other,
    };
    if key_only {
        return (state == ElementState::Pressed
            && CATEGORIES.iter().any(|keys| keys.contains(&key)))
        .then(|| key.to_owned());
    }
    if let Some(name) = modifier
        && state == ElementState::Pressed
    {
        if !modifiers.contains(&name) {
            modifiers.push(name);
        }
        return None;
    }
    if (state == ElementState::Pressed || modifier.is_some())
        && CATEGORIES.iter().any(|keys| keys.contains(&key))
    {
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
            captured_key(K::Escape, Pressed, true, &mut modifiers).as_deref(),
            Some("Escape")
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
        assert!(!valid("macro:self", false, true, "self", None));
    }
}
