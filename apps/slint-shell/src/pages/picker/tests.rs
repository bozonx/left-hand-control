//! Regression tests for picker.

use super::*;

#[test]
fn captures_trigger_combinations_including_two_modifiers() {
    use slint::winit_030::winit::event::ElementState::{Pressed, Released};
    let mut keys = Vec::new();
    assert_eq!(captured_trigger("ShiftLeft", Pressed, &mut keys), None);
    assert_eq!(captured_trigger("ControlRight", Pressed, &mut keys), None);
    assert_eq!(
        captured_trigger("ShiftLeft", Released, &mut keys).as_deref(),
        Some("ShiftLeft+ControlRight")
    );
    assert!(keys.is_empty());
    assert_eq!(captured_trigger("F13", Pressed, &mut keys), None);
    assert_eq!(
        captured_trigger("F13", Released, &mut keys).as_deref(),
        Some("F13")
    );
}

#[test]
fn captures_physical_keys_and_chords() {
    use slint::winit_030::winit::{
        event::ElementState::{Pressed, Released},
        keyboard::KeyCode as K,
    };
    let mut modifiers = Vec::new();
    assert_eq!(
        captured_key("ControlRight", Pressed, true, &mut modifiers).as_deref(),
        Some("ControlRight")
    );
    assert_eq!(
        captured_key("NumpadEnter", Pressed, true, &mut modifiers).as_deref(),
        Some("NumpadEnter")
    );
    assert_eq!(
        captured_key("ShiftLeft", Pressed, false, &mut modifiers),
        None
    );
    assert_eq!(
        captured_key("ControlRight", Pressed, false, &mut modifiers),
        None
    );
    assert_eq!(
        captured_key("KeyK", Pressed, false, &mut modifiers).as_deref(),
        Some("Ctrl+Shift+KeyK")
    );
    assert!(modifiers.is_empty());
    assert_eq!(
        captured_key("AltRight", Pressed, false, &mut modifiers),
        None
    );
    assert_eq!(
        captured_key("AltRight", Released, false, &mut modifiers).as_deref(),
        Some("AltRight")
    );
    assert_eq!(
        captured_key(&key_name(K::AudioVolumeMute), Pressed, true, &mut modifiers).as_deref(),
        Some("AudioVolumeMute")
    );
    assert_eq!(key_name(K::SuperLeft), "MetaLeft");
    assert_eq!(key_name(K::LaunchApp1), "LaunchApp1");
    assert_eq!(
        physical_key_name(slint::winit_030::winit::keyboard::PhysicalKey::Code(
            K::NumpadEqual
        ))
        .as_deref(),
        Some("NumpadEqual")
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        physical_key_name(
            slint::winit_030::winit::keyboard::PhysicalKey::Unidentified(
                slint::winit_030::winit::keyboard::NativeKeyCode::Xkb(140)
            )
        )
        .as_deref(),
        Some("LaunchApp2")
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
    assert!(!valid("pause:0", false, true, "", None));
    assert!(!valid("macro:self", false, true, "self", None));
}

#[test]
fn categories_follow_the_key_catalog() {
    assert_eq!(category_for_value("CapsLock"), PickerCategory::Modifiers);
    assert_eq!(category_for_value("ArrowUp"), PickerCategory::Navigation);
    assert_eq!(category_for_value("Numpad5"), PickerCategory::Navigation);
    assert_eq!(category_for_value("Digit5"), PickerCategory::Letters);
    assert_eq!(category_for_value("Escape"), PickerCategory::ControlKeys);
    assert_eq!(category_for_value("Ctrl+KeyV"), PickerCategory::Letters);
    assert_eq!(
        category_for_value("ShiftLeft+F13"),
        PickerCategory::ControlKeys
    );
    assert_eq!(category_for_value("KeyV"), PickerCategory::Letters);
    assert_eq!(category_for_value("F13"), PickerCategory::ControlKeys);
    assert_eq!(
        category_for_value("MicrophoneMuteToggle"),
        PickerCategory::Media
    );
    assert_eq!(
        category_for_value("TouchpadToggle"),
        PickerCategory::SystemKeys
    );
    assert_eq!(category_for_value("Redo"), PickerCategory::OtherKeys);
    assert_eq!(category_for_value("text:Ctrl+KeyV"), PickerCategory::Text);
    assert_eq!(
        category_for_value("kde:kwin/Window Close"),
        PickerCategory::DesktopShortcuts
    );
    assert_eq!(
        category_for_value("cmd:Ctrl+KeyV"),
        PickerCategory::Commands
    );
}
