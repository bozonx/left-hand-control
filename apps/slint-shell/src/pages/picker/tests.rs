//! Regression tests for picker.

use super::*;

#[test]
fn capture_waits_for_all_releases_and_preserves_keys() {
    use slint::winit_030::winit::event::ElementState::{Pressed, Released};
    for trigger in [false, true] {
        for release_order in [
            ["ControlRight", "KeyK", "ShiftLeft"],
            ["ShiftLeft", "ControlRight", "KeyK"],
            ["KeyK", "ShiftLeft", "ControlRight"],
        ] {
            let mut capture = CaptureState::default();
            assert_eq!(capture.update("KeyZ", Released, trigger, false), None);
            for key in ["ShiftLeft", "ControlRight", "KeyK", "KeyK"] {
                assert_eq!(capture.update(key, Pressed, trigger, false), None);
            }
            assert_eq!(capture.held, ["ShiftLeft", "ControlRight", "KeyK"]);
            for key in &release_order[..2] {
                assert_eq!(capture.update(key, Released, trigger, false), None);
            }
            assert_eq!(
                capture
                    .update(release_order[2], Released, trigger, false)
                    .as_deref(),
                Some(if trigger {
                    "ShiftLeft+ControlRight+KeyK"
                } else {
                    "Ctrl+Shift+KeyK"
                })
            );
            assert!(capture.held.is_empty());
            assert!(capture.keys.is_empty());
        }
    }
}

#[test]
fn capture_keeps_released_keys_while_other_keys_are_held() {
    use slint::winit_030::winit::event::ElementState::{Pressed, Released};
    let mut capture = CaptureState::default();
    for (key, state) in [
        ("ControlLeft", Pressed),
        ("KeyA", Pressed),
        ("KeyA", Released),
        ("KeyB", Pressed),
        ("ControlLeft", Released),
    ] {
        assert_eq!(capture.update(key, state, false, false), None);
    }
    assert_eq!(capture.held, ["KeyB"]);
    assert_eq!(
        capture.update("KeyB", Released, false, false).as_deref(),
        Some("Ctrl+KeyA+KeyB")
    );
}

#[test]
fn captures_single_physical_keys_on_release() {
    use slint::winit_030::winit::event::ElementState::{Pressed, Released};
    for key_only in [false, true] {
        for key in ["ControlRight", "NumpadEnter", "AudioVolumeMute", "Escape"] {
            let mut capture = CaptureState::default();
            assert_eq!(capture.update(key, Pressed, false, key_only), None);
            assert_eq!(
                capture.update(key, Released, false, key_only).as_deref(),
                Some(key)
            );
        }
    }
}

#[test]
fn physical_key_names_match_config_codes() {
    use slint::winit_030::winit::keyboard::KeyCode as K;
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
    for value in ["", "Ctrl+KeyA", "builtin:copyLine", "text:hello", "NotAKey"] {
        assert!(!valid(value, true, false, "", None));
    }
    assert!(!valid("pause:100", false, false, "", None));
    assert!(valid("pause:100", false, true, "", None));
    assert!(!valid("pause:10001", false, true, "", None));
    assert!(!valid("pause:0", false, true, "", None));
    assert!(!valid("macro:self", false, true, "self", None));
    assert!(valid("builtin:copyLine", false, true, "copyLine", None));
}

#[test]
fn categories_follow_the_key_catalog() {
    assert_eq!(category_for_value("macro:copyLine"), PickerCategory::Macros);
    assert_eq!(
        category_for_value("builtin:copyLine"),
        PickerCategory::BuiltinMacros
    );
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
