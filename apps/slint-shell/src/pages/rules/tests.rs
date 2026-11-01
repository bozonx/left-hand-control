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
    let (_dir, document) = super::super::test_document();
    document
        .edit(View::Rules, |config| {
            config.update_layout(|layout| {
                layout.layers = vec![Layer {
                    id: "nav".into(),
                    name: "Navigation".into(),
                    description: None,
                }];
                layout.rules = vec![
                    KeyRule::new("first".into(), "KeyA"),
                    KeyRule::new("second".into(), "KeyB"),
                ];
                layout.rules[0].long_hold_timeout_ms = Some(1000);
                layout.rules[1].long_hold_timeout_ms = Some(2000);
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
    let layer = document.read().layout().layers[0].id.clone();
    let original = document.read().layout().rules[1].clone();
    editor.set_selected(-1);
    editor.invoke_choose(layer.clone().into());
    assert_ne!(editor.get_status().id, "");
    assert_eq!(editor.get_dialog(), RuleDialog::Layer);
    assert_eq!(editor.get_selected(), -1);
    assert_eq!(document.read().layout().rules[1], original);
    editor.invoke_open_dialog(1, RuleDialog::Layer);
    editor.invoke_choose(layer.clone().into());
    assert_eq!(editor.get_dialog(), RuleDialog::None);
    assert_eq!(document.read().layout().rules[1].layer_id, layer);
}
