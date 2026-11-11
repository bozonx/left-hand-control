//! Regression tests for rules.

use super::*;

#[test]
fn chip_text_shows_three_items_and_counts_the_rest() {
    let list = |items: &[&str]| Some(items.iter().map(|item| item.to_string()).collect());
    assert_eq!(chip_text(&None), "");
    assert_eq!(chip_text(&list(&["a", "b", "c"])), "a, b, c");
    assert_eq!(chip_text(&list(&["a", "b", "c", "d", "e"])), "a, b, c +2");
}

#[test]
fn long_hold_accepts_milliseconds_in_range_and_rejects_the_rest() {
    for value in [100, 1500, 60000] {
        assert_eq!(parse_long_hold(&format!(" {value} ")), Ok(value));
        assert_eq!(optional_long_hold(&value.to_string()), Ok(Some(value)));
    }
    assert_eq!(optional_long_hold(""), Ok(None));
    for value in ["0", "99", "-1", "1.5", "60001", "invalid"] {
        assert_eq!(parse_long_hold(value), Err(Msg::LongHoldInvalid));
    }
}

#[test]
fn hold_modes_clear_incompatible_options_and_preserve_layer_assignments() {
    let mut rule = KeyRule::new("r".into(), "ShiftLeft+ControlLeft");
    rule.layer_id = "nav".into();
    rule.hold_action = Some("AltLeft".into());
    assert_eq!(hold_mode(&rule), UiHoldBehavior::Layer);
    set_hold_behavior(&mut rule, HoldBehavior::Action);
    assert_eq!(rule.hold_behavior(), HoldBehavior::Action);
    assert!(rule.layer_id.is_empty());
    assert_eq!(rule.hold_action.as_deref(), Some(""));
    rule.long_hold_action = "text:hello".into();
    set_hold_behavior(&mut rule, HoldBehavior::Layer);
    assert!(rule.long_hold_action.is_empty());
    assert_eq!(rule.hold_behavior(), HoldBehavior::Layer);
}

#[test]
fn choosing_a_layer_or_long_hold_action_switches_the_hold_mode() {
    let mut rule = KeyRule::new("r".into(), "Tab");
    rule.long_hold_action = "text:hello".into();
    set_property(&mut rule, RuleDialog::Layer, "nav").unwrap();
    assert_eq!(rule.hold_behavior(), HoldBehavior::Layer);
    assert!(rule.long_hold_action.is_empty());
    set_property(&mut rule, RuleDialog::LongHold, "MediaStop").unwrap();
    assert_eq!(rule.hold_behavior(), HoldBehavior::Action);
    assert!(rule.layer_id.is_empty());
    set_property(&mut rule, RuleDialog::LongHold, "").unwrap();
    assert_eq!(rule.hold_behavior(), HoldBehavior::Action);
}

#[test]
fn choosing_nothing_clears_all_hold_assignments() {
    let mut rule = KeyRule::new("r".into(), "Tab");
    rule.layer_id = "nav".into();
    rule.long_hold_action = "text:hello".into();
    rule.hold_action = Some("AltLeft".into());
    set_hold_behavior(&mut rule, HoldBehavior::None);
    assert_eq!(rule.hold_behavior(), HoldBehavior::None);
    assert!(rule.layer_id.is_empty());
    assert!(rule.long_hold_action.is_empty());
    assert_eq!(rule.hold_action.as_deref(), Some(""));
}

#[test]
fn properties_validate_and_apply() {
    let mut rule = KeyRule::new("r".into(), "");
    set_property(&mut rule, RuleDialog::Key, "CapsLock").unwrap();
    set_property(&mut rule, RuleDialog::HoldTimeout, " 250 ").unwrap();
    assert_eq!(
        (rule.key.as_str(), rule.hold_timeout_ms),
        ("CapsLock", Some(250))
    );
    assert_eq!(
        set_property(&mut rule, RuleDialog::DoubleTimeout, "-1"),
        Err(Msg::TimeoutInvalid)
    );
    set_property(&mut rule, RuleDialog::HoldTimeout, "").unwrap();
    assert_eq!(rule.hold_timeout_ms, None);
}

#[test]
fn rule_ids_are_unique() {
    let rules = vec![KeyRule::new("slint-rule-1".into(), "")];
    assert_eq!(unique_rule_id(&rules), "slint-rule-2");
}

#[test]
fn tap_hold_timeouts_share_the_file_contract() {
    assert_eq!(parse_timeout(""), Ok(None));
    for value in [50, 1000] {
        assert_eq!(parse_timeout(&value.to_string()), Ok(Some(value)));
    }
    for value in ["0", "49", "1001", "18446744073709551615", "1.5"] {
        assert_eq!(parse_timeout(value), Err(Msg::TimeoutInvalid));
    }
}

#[test]
fn incompatible_trigger_change_preserves_the_entire_rule() {
    let mut rule = KeyRule::new("r".into(), "KeyA");
    rule.double_tap_action = "KeyB".into();
    rule.double_tap_timeout_ms = Some(300);
    let original = rule.clone();
    assert!(matches!(
        set_property(&mut rule, RuleDialog::Key, "KeyA+KeyC"),
        Err(Msg::Rule(_))
    ));
    assert_eq!(rule, original);
    set_property(&mut rule, RuleDialog::Key, "KeyD").unwrap();
    assert_eq!(rule.double_tap_action, "KeyB");
    set_property(&mut rule, RuleDialog::DoubleTap, "").unwrap();
    set_property(&mut rule, RuleDialog::Key, "KeyA+KeyC").unwrap();
    let chord = rule.clone();
    assert!(set_property(&mut rule, RuleDialog::DoubleTap, "KeyB").is_err());
    assert_eq!(rule, chord);
    rule.double_tap_action = "KeyB".into();
    set_property(&mut rule, RuleDialog::DoubleTap, "").unwrap();
    assert!(rule.double_tap_action.is_empty());
}

#[test]
fn advanced_draft_validates_every_field_before_changing_the_rule() {
    let mut rule = KeyRule::new("r".into(), "KeyA");
    let original = rule.clone();
    let mut draft = AdvancedDraft {
        hold_timeout: "250".into(),
        double_timeout: "300".into(),
        long_hold_timeout: "invalid".into(),
        hold: Some("Ctrl+KeyC".into()),
        detection: HoldDetection::Press,
    };
    assert_eq!(draft.apply(&mut rule), Err(Msg::LongHoldInvalid));
    assert_eq!(rule, original);
    draft.long_hold_timeout = "1500".into();
    draft.hold = Some("cmd:invalid-held-action".into());
    assert_eq!(draft.apply(&mut rule), Err(Msg::ActionRequired));
    assert_eq!(rule, original);
    draft.hold = None;
    draft.apply(&mut rule).unwrap();
    assert_eq!(rule.hold_action, None);
    assert_eq!(rule.hold_timeout_ms, Some(250));
    assert_eq!(rule.double_tap_timeout_ms, Some(300));
    assert_eq!(rule.long_hold_timeout_ms, Some(1500));
    assert_eq!(rule.tap_decision, Some(TapDecision::HoldOnOtherKeyPress));
}

#[test]
fn layer_dialog_loads_its_rule_and_reports_a_vanished_selection() {
    init_platform();
    let (_dir, document) = super::super::test_document();
    document
        .edit(View::Rules, |config| {
            config.update_profile(|profile| {
                profile.layers = vec![Layer {
                    id: "nav".into(),
                    name: "Navigation".into(),
                    description: None,
                }];
                profile.rules = vec![
                    KeyRule::new("first".into(), "KeyA"),
                    KeyRule::new("second".into(), "KeyB"),
                ];
                profile.rules[0].long_hold_timeout_ms = Some(1000);
                profile.rules[1].long_hold_timeout_ms = Some(2000);
            })
        })
        .unwrap();
    let ui = SettingsWindow::new().unwrap();
    bind(&ui, &document);
    let editor = ui.global::<RulesEditor>();
    editor.invoke_select(0);
    editor.invoke_open_dialog(1, RuleDialog::Layer);
    assert_eq!(editor.get_key(), "KeyB");
    assert_eq!(editor.get_long_hold_timeout(), "2000");
    let layer = document.read().profile().layers[0].id.clone();
    let original = document.read().profile().rules[1].clone();
    editor.set_selected(-1);
    editor.invoke_choose(layer.clone().into());
    assert_ne!(editor.get_status().id, "");
    assert_eq!(editor.get_dialog(), RuleDialog::Layer);
    assert_eq!(editor.get_selected(), -1);
    assert_eq!(document.read().profile().rules[1], original);
    editor.invoke_open_dialog(1, RuleDialog::Layer);
    editor.invoke_choose(layer.clone().into());
    assert_eq!(editor.get_dialog(), RuleDialog::None);
    assert_eq!(document.read().profile().rules[1].layer_id, layer);
}

fn init_platform() {
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
    struct Platform;
    impl slint::platform::Platform for Platform {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
        }
    }
    slint::platform::set_platform(Box::new(Platform)).unwrap();
}

fn editor_fixture() -> (tempfile::TempDir, Rc<Document>, SettingsWindow) {
    init_platform();
    let (dir, document) = super::super::test_document();
    document
        .edit(View::Rules, |config| {
            config.update_profile(|profile| {
                profile.rules = vec![
                    KeyRule::new("first".into(), "KeyA"),
                    KeyRule::new("second".into(), "KeyB"),
                ];
            })
        })
        .unwrap();
    let ui = SettingsWindow::new().unwrap();
    bind(&ui, &document);
    (dir, document, ui)
}

#[test]
fn background_updates_preserve_drafts_and_reject_all_stale_operations() {
    let (_dir, document, ui) = editor_fixture();
    let editor = ui.global::<RulesEditor>();
    editor.invoke_open_dialog(0, RuleDialog::Advanced);
    editor.set_hold_timeout("333".into());
    editor.set_apps_include("firefox".into());
    editor.set_enabled(false);
    document
        .edit(View::Library, |config| {
            config.update_profile(|profile| {
                profile.rules.swap(0, 1);
            })
        })
        .unwrap();
    let original = document.read().profile().clone();
    assert_eq!(editor.get_hold_timeout(), "333");
    assert_eq!(editor.get_apps_include(), "firefox");
    assert!(!editor.get_enabled());
    editor.invoke_apply_advanced();
    editor.invoke_apply_conditions();
    editor.invoke_remove();
    editor.invoke_set_hold_mode(UiHoldBehavior::Layer);
    editor.invoke_create_layer("Stale layer".into());
    assert_eq!(*document.read().profile(), original);
    assert_eq!(editor.get_status().id, "config-edit-conflict");
    assert_eq!(editor.get_dialog(), RuleDialog::Advanced);
    editor.set_field(RuleDialog::Tap);
    assert_eq!(
        choose(&ui, &document, "KeyC", None),
        Err(Msg::from(
            &tecloid_core::config_document::ConfigError::EditConflict
        ))
    );
    // Reopening starts a new edit of the current rule.
    editor.invoke_open_dialog(0, RuleDialog::Advanced);
    editor.set_hold_timeout("333".into());
    editor.invoke_apply_advanced();
    assert_eq!(
        document.read().profile().rules[0].hold_timeout_ms,
        Some(333)
    );
}

#[test]
fn vanished_picker_target_cannot_turn_into_a_new_rule() {
    let (_dir, document, ui) = editor_fixture();
    let editor = ui.global::<RulesEditor>();
    editor.invoke_open_dialog(1, RuleDialog::Key);
    ui.global::<ActionPicker>().set_opened(true);
    document
        .edit(View::Library, |config| {
            config.update_profile(|profile| {
                profile.rules.pop();
            })
        })
        .unwrap();
    assert_eq!(editor.get_selected(), -1);
    assert!(choose(&ui, &document, "KeyC", None).is_err());
    assert_eq!(document.read().profile().rules.len(), 1);
    ui.global::<ActionPicker>().set_opened(false);
    editor.invoke_add();
    choose(&ui, &document, "KeyC", None).unwrap();
    assert_eq!(document.read().profile().rules[0].key, "KeyC");
}

#[test]
fn failed_edit_keeps_draft_and_allows_retry() {
    let (_dir, document, ui) = editor_fixture();
    let editor = ui.global::<RulesEditor>();
    editor.invoke_open_dialog(0, RuleDialog::Advanced);
    editor.set_hold_timeout("333".into());
    editor.set_double_timeout("444".into());
    editor.set_hold("Ctrl+KeyC".into());
    editor.set_enabled(false);
    let before = document.read().profile().clone();
    let revision = document.revision();
    assert!(
        change(&ui, &document, true, |profile| {
            profile.rules[0].hold_timeout_ms = Some(1);
        })
        .is_err()
    );
    assert_eq!(*document.read().profile(), before);
    assert_eq!(editor.get_hold_timeout(), "333");
    assert_eq!(editor.get_double_timeout(), "444");
    assert_eq!(editor.get_hold(), "Ctrl+KeyC");
    assert!(!editor.get_enabled());
    assert_eq!(document.revision(), revision);
    assert_eq!(editor.get_dialog(), RuleDialog::Advanced);
    editor.invoke_apply_advanced();
    assert_eq!(
        document.read().profile().rules[0].hold_timeout_ms,
        Some(333)
    );
    assert!(!document.read().profile().rules[0].is_enabled());
}

#[test]
fn diagnostics_refresh_when_macro_and_command_dependencies_change() {
    use tecloid_core::config::model::{Command, Macro, MacroStep};
    let (_dir, document, ui) = editor_fixture();
    document
        .edit(View::Library, |config| {
            config.update_profile(|profile| {
                profile.rules[0].tap_action = Some("macro:m".into());
                profile.rules[1].tap_action = Some("cmd:c".into());
            })
        })
        .unwrap();
    let editor = ui.global::<RulesEditor>();
    let issues = || {
        editor
            .get_rows()
            .iter()
            .map(|row| row.diagnostic.id.to_string())
            .collect::<Vec<_>>()
    };
    assert!(issues().iter().all(|id| !id.is_empty()));
    document
        .edit(View::Macros, |config| {
            config.update_profile(|profile| {
                profile.macros.push(Macro {
                    id: "m".into(),
                    name: "Macro".into(),
                    steps: vec![MacroStep {
                        id: "step".into(),
                        action: "KeyC".into(),
                    }],
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                });
            })
        })
        .unwrap();
    assert!(issues()[0].is_empty());
    document
        .edit(View::Menus, |config| {
            config.update_profile(|profile| {
                profile.commands.push(Command {
                    id: "c".into(),
                    name: "Command".into(),
                    posix_script: "true".into(),
                    powershell_script: "exit 0".into(),
                    working_directory: None,
                });
            })
        })
        .unwrap();
    assert!(issues().iter().all(String::is_empty));
    document
        .edit(View::Macros, |config| {
            config.update_profile(|profile| {
                profile.macros.clear();
            })
        })
        .unwrap();
    assert!(!issues()[0].is_empty());
    document
        .edit(View::Menus, |config| {
            config.update_profile(|profile| {
                profile.commands.clear();
            })
        })
        .unwrap();
    assert!(!issues()[1].is_empty());
}

#[test]
fn empty_trigger_draft_can_be_disabled_and_deleted() {
    let (_dir, document, ui) = editor_fixture();
    document
        .edit(View::Library, |config| {
            config.update_profile(|profile| {
                profile.rules[0].key.clear();
            })
        })
        .unwrap();
    let editor = ui.global::<RulesEditor>();
    editor.invoke_open_dialog(0, RuleDialog::Advanced);
    editor.set_enabled(false);
    editor.invoke_apply_advanced();
    assert!(!document.read().profile().rules[0].is_enabled());
    editor.invoke_open_dialog(0, RuleDialog::Advanced);
    editor.set_dialog(RuleDialog::Remove);
    editor.invoke_remove();
    assert_eq!(document.read().profile().rules.len(), 1);
    assert_eq!(document.read().profile().rules[0].id, "second");
}

#[test]
fn refresh_and_external_edits_clear_stale_conflict_status() {
    let (_dir, document, ui) = editor_fixture();
    let editor = ui.global::<RulesEditor>();
    editor.invoke_open_dialog(0, RuleDialog::Conditions);
    document
        .edit(View::Layers, |config| {
            config.create_layer("Extra", "").map(|_| ())
        })
        .unwrap();
    assert_eq!(editor.get_status().id, "config-edit-conflict");
    editor.set_dialog(RuleDialog::None);
    editor.invoke_refresh();
    assert_eq!(editor.get_status().id, "");

    editor.invoke_open_dialog(0, RuleDialog::Tap);
    let picker = ui.global::<ActionPicker>();
    picker.set_opened(true);
    document
        .edit(View::Layers, |config| {
            config.create_layer("Another", "").map(|_| ())
        })
        .unwrap();
    assert_eq!(
        choose(&ui, &document, "KeyC", None),
        Err(Msg::from(
            &tecloid_core::config_document::ConfigError::EditConflict
        ))
    );
    picker.set_opened(false);
    editor.set_field(RuleDialog::None);
    editor.invoke_open_dialog(0, RuleDialog::Tap);
    choose(&ui, &document, "KeyC", None).unwrap();
    assert_eq!(
        document.read().profile().rules[0].tap_action.as_deref(),
        Some("KeyC")
    );
}
