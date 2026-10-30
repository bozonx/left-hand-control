//! Regression tests for picker.

use super::*;
use crate::ui::PickerCategory;

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
    let keys = Filter {
        key_only: true,
        ..Filter::default()
    };
    assert!(keys.accepts("ControlRight", None));
    for value in [
        "",
        "Ctrl+KeyA",
        "builtin:copyLineText",
        "text:hello",
        "NotAKey",
    ] {
        assert!(!keys.accepts(value, None));
    }
    assert!(!Filter::default().accepts("pause:100", None));
    let step = Filter {
        macro_step: true,
        excluded_macro: "self".into(),
        ..Filter::default()
    };
    assert!(step.accepts("pause:100", None));
    assert!(!step.accepts("pause:10001", None));
    assert!(!step.accepts("pause:0", None));
    assert!(!step.accepts("pause:+100", None));
    assert!(step.accepts("pause: 100", None));
    assert!(!step.accepts("macro:self", None));
    assert!(step.accepts("builtin:copyLineText", None));
}

#[test]
fn picker_restrictions_accept_valid_values_and_reject_invalid_ones() {
    let trigger = Filter {
        key_only: true,
        trigger: true,
        ..Filter::default()
    };
    for (value, expected) in [("ShiftLeft+KeyA", true), ("NotAKey", false), ("", false)] {
        assert_eq!(trigger.admits(value), expected, "listed trigger: {value}");
        assert_eq!(
            trigger.accepts(value, None),
            expected,
            "accepted trigger: {value}"
        );
    }
    let extra = Filter {
        unavailable_keys: vec!["KeyA".into()],
        key_only: true,
        ..Filter::default()
    };
    for (value, expected) in [("KeyA", false), ("KeyB", true)] {
        assert_eq!(extra.admits(value), expected, "listed extra key: {value}");
        assert_eq!(
            extra.accepts(value, None),
            expected,
            "accepted extra key: {value}"
        );
    }
    let held = Filter {
        held_key: true,
        ..Filter::default()
    };
    for (value, expected) in [("text:hi", false), ("ControlLeft", true)] {
        assert_eq!(held.admits(value), expected, "listed held key: {value}");
        assert_eq!(
            held.accepts(value, None),
            expected,
            "accepted held key: {value}"
        );
    }
}

#[test]
fn requests_choose_the_initial_behavior() {
    let tap = PickerRequest {
        allow_default: true,
        allow_swallow: true,
        swallowed: true,
        ..PickerRequest::default()
    };
    assert_eq!(tap.behavior(false), PickerBehavior::Swallow);
    let default = PickerRequest {
        allow_default: true,
        ..PickerRequest::default()
    };
    assert_eq!(default.behavior(true), PickerBehavior::Native);
    assert_eq!(default.behavior(false), PickerBehavior::Action);
}

#[test]
fn unassigned_behavior_keeps_each_context_label() {
    use crate::ui::DefaultAssignmentBehavior;

    for (behavior, label) in [
        (DefaultAssignmentBehavior::Passthrough, Msg::PickerNative),
        (DefaultAssignmentBehavior::Inherit, Msg::PickerInherit),
        (DefaultAssignmentBehavior::NoAction, Msg::PickerNoAction),
        (DefaultAssignmentBehavior::EmptyCell, Msg::PickerEmptyCell),
        (DefaultAssignmentBehavior::LayerOnly, Msg::PickerLayerOnly),
    ] {
        let request = PickerRequest {
            default_behavior: behavior,
            ..PickerRequest::default()
        };
        assert_eq!(request.native_label(), label);
    }
    assert_eq!(
        PickerRequest::plain(PickerTarget::QuickAction, false).default_behavior,
        DefaultAssignmentBehavior::EmptyCell
    );
}

#[test]
fn categories_follow_the_key_catalog() {
    assert_eq!(category_for_value("macro:copyLine"), PickerCategory::Macros);
    assert_eq!(
        category_for_value("builtin:copyLineText"),
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

#[test]
fn applying_actions_validates_references_against_the_document() {
    use lhc_core::profile::model::{Command, Macro};

    let (_dir, document) = super::super::test_document();
    document
        .edit(crate::document::View::Macros, |config| {
            config.update_layout(|layout| {
                layout.macros.push(Macro {
                    id: "pickerMacro".into(),
                    name: "Picker macro".into(),
                    steps: vec![],
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                });
                layout.commands.push(Command {
                    id: "pickerCommand".into(),
                    name: "Picker command".into(),
                    posix_script: "true".into(),
                    powershell_script: "exit 0".into(),
                    working_directory: None,
                });
            })
        })
        .unwrap();
    let filter = Filter::default();
    let config = document.read();
    for (value, expected) in [
        ("macro:pickerMacro", true),
        ("macro:missingMacro", false),
        ("cmd:pickerCommand", true),
        ("cmd:missingCommand", false),
    ] {
        assert_eq!(filter.accepts(value, Some(&config)), expected, "{value}");
    }
    let recursive = Filter {
        macro_step: true,
        excluded_macro: "pickerMacro".into(),
        ..Filter::default()
    };
    assert!(!recursive.accepts("macro:pickerMacro", Some(&config)));
}
