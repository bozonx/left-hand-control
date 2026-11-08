//! Validated rule options, independent of shell widgets and persistence.

use super::{
    actions,
    model::{HoldBehavior, KeyRule, TapDecision},
    timing,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOptionsError {
    Timeout,
    LongHold,
    HeldKey,
}

/// Parsed advanced options. Validation completes before any field is changed.
pub struct RuleOptions {
    pub hold_timeout_ms: Option<u64>,
    pub double_tap_timeout_ms: Option<u64>,
    pub long_hold_timeout_ms: Option<u64>,
    pub hold_action: Option<String>,
    pub tap_decision: Option<TapDecision>,
}

impl RuleOptions {
    pub fn apply(self, rule: &mut KeyRule) -> Result<(), RuleOptionsError> {
        if [self.hold_timeout_ms, self.double_tap_timeout_ms]
            .into_iter()
            .flatten()
            .any(|ms| !timing::TAP_HOLD.contains(ms))
        {
            return Err(RuleOptionsError::Timeout);
        }
        if self
            .long_hold_timeout_ms
            .is_some_and(|ms| !timing::LONG_HOLD.contains(ms))
        {
            return Err(RuleOptionsError::LongHold);
        }
        if self
            .hold_action
            .as_deref()
            .is_some_and(|key| !key.is_empty() && !actions::valid_held_key(key))
        {
            return Err(RuleOptionsError::HeldKey);
        }
        rule.hold_timeout_ms = self.hold_timeout_ms;
        rule.double_tap_timeout_ms = self.double_tap_timeout_ms;
        rule.long_hold_timeout_ms = self.long_hold_timeout_ms;
        rule.tap_decision = self.tap_decision;
        if rule.hold_behavior() != HoldBehavior::Action {
            rule.hold_action = self.hold_action;
        }
        Ok(())
    }
}

impl KeyRule {
    /// Switch hold modes, dropping assignments that the new mode does not use.
    pub fn set_hold_behavior(&mut self, behavior: HoldBehavior) {
        self.hold_behavior = Some(behavior);
        if behavior != HoldBehavior::Layer {
            self.layer_id.clear();
            self.hold_action = Some(String::new());
        }
        if behavior != HoldBehavior::Action {
            self.long_hold_action.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> RuleOptions {
        RuleOptions {
            hold_timeout_ms: Some(250),
            double_tap_timeout_ms: Some(300),
            long_hold_timeout_ms: Some(1500),
            hold_action: Some("Ctrl+KeyC".into()),
            tap_decision: Some(TapDecision::HoldOnOtherKeyPress),
        }
    }

    #[test]
    fn invalid_options_leave_the_whole_rule_unchanged() {
        let original = KeyRule::new("r".into(), "KeyA");
        for (draft, error) in [
            (
                RuleOptions {
                    hold_timeout_ms: Some(49),
                    ..options()
                },
                RuleOptionsError::Timeout,
            ),
            (
                RuleOptions {
                    double_tap_timeout_ms: Some(1001),
                    ..options()
                },
                RuleOptionsError::Timeout,
            ),
            (
                RuleOptions {
                    long_hold_timeout_ms: Some(99),
                    ..options()
                },
                RuleOptionsError::LongHold,
            ),
            (
                RuleOptions {
                    hold_action: Some("cmd:bad".into()),
                    ..options()
                },
                RuleOptionsError::HeldKey,
            ),
        ] {
            let mut rule = original.clone();
            assert_eq!(draft.apply(&mut rule), Err(error));
            assert_eq!(rule, original);
        }
    }

    #[test]
    fn options_preserve_long_hold_output_and_allow_inherited_timings() {
        let mut rule = KeyRule::new("r".into(), "KeyA");
        rule.set_hold_behavior(HoldBehavior::Action);
        rule.long_hold_action = "text:hello".into();
        options().apply(&mut rule).unwrap();
        assert_eq!(rule.hold_action.as_deref(), Some(""));
        assert_eq!(rule.long_hold_action, "text:hello");
        rule.set_hold_behavior(HoldBehavior::Layer);
        rule.layer_id = "nav".into();
        RuleOptions {
            hold_timeout_ms: None,
            double_tap_timeout_ms: None,
            long_hold_timeout_ms: None,
            hold_action: None,
            tap_decision: None,
        }
        .apply(&mut rule)
        .unwrap();
        assert_eq!(rule.hold_action, None);
        assert_eq!(rule.hold_timeout_ms, None);
        assert_eq!(rule.layer_id, "nav");
        assert!(rule.long_hold_action.is_empty());
    }
}
