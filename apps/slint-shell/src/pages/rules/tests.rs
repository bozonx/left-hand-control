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
