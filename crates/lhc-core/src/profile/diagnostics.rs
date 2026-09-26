//! Rule checks run before the mapper starts (`utils/ruleDiagnostics.ts`).

use super::actions::{self, Action};
use super::model::{AppConfig, LayerRule};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleIssueCode {
    MissingTrigger,
    InvalidTrigger,
    DuplicateTrigger,
    UnknownLayer,
    InvalidTapAction,
    InvalidHoldAction,
    InvalidDoubleTapAction,
}

impl RuleIssueCode {
    /// Warnings do not block the mapper; the rule is skipped.
    pub fn is_error(self) -> bool {
        self != Self::MissingTrigger
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleIssue {
    pub code: RuleIssueCode,
    pub rule_id: String,
    pub trigger: Option<String>,
}

impl std::fmt::Display for RuleIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let trigger = self.trigger.as_deref().unwrap_or("");
        match self.code {
            RuleIssueCode::MissingTrigger => {
                write!(
                    f,
                    "Rule without a trigger is saved as a draft and will be ignored."
                )
            }
            RuleIssueCode::InvalidTrigger => {
                write!(f, "Trigger \"{trigger}\" cannot be used for a rule.")
            }
            RuleIssueCode::DuplicateTrigger => {
                write!(
                    f,
                    "Trigger \"{trigger}\" is used by more than one active rule."
                )
            }
            RuleIssueCode::UnknownLayer => {
                write!(
                    f,
                    "Rule \"{trigger}\" points to a layer that no longer exists."
                )
            }
            RuleIssueCode::InvalidTapAction => {
                write!(f, "Rule \"{trigger}\" has an invalid tap action.")
            }
            RuleIssueCode::InvalidHoldAction => {
                write!(f, "Rule \"{trigger}\" has an invalid hold action.")
            }
            RuleIssueCode::InvalidDoubleTapAction => {
                write!(f, "Rule \"{trigger}\" has an invalid double-tap action.")
            }
        }
    }
}

const MOUSE_TRIGGERS: [&str; 3] = ["MouseLeft", "MouseRight", "MouseMiddle"];

pub fn analyze_rules(config: &AppConfig) -> Vec<RuleIssue> {
    let layer_ids: HashSet<&str> = config
        .layers
        .iter()
        .map(|layer| layer.id.as_str())
        .collect();
    let mut by_trigger: BTreeMap<&str, Vec<&LayerRule>> = BTreeMap::new();
    let mut issues = Vec::new();
    let issue = |code, rule: &LayerRule, trigger: Option<&str>| RuleIssue {
        code,
        rule_id: rule.id.clone(),
        trigger: trigger.map(str::to_owned),
    };
    for rule in config.rules.iter().filter(|rule| rule.is_enabled()) {
        if !rule.has_trigger() {
            issues.push(issue(RuleIssueCode::MissingTrigger, rule, None));
            continue;
        }
        let trigger = rule.key.trim();
        if MOUSE_TRIGGERS.contains(&trigger)
            || matches!(Action::parse(Some(trigger)), Action::Keys(ref k) if k.contains('+'))
        {
            issues.push(issue(RuleIssueCode::InvalidTrigger, rule, Some(trigger)));
        }
        by_trigger.entry(trigger).or_default().push(rule);
        if !rule.layer_id.is_empty() && !layer_ids.contains(rule.layer_id.as_str()) {
            issues.push(issue(RuleIssueCode::UnknownLayer, rule, Some(trigger)));
        }
        if actions::validate(&Action::parse(rule.tap_action.as_deref()), config).is_some() {
            issues.push(issue(RuleIssueCode::InvalidTapAction, rule, Some(trigger)));
        }
        let hold = Action::parse(rule.hold_action.as_deref());
        if !matches!(hold, Action::Native | Action::Swallow | Action::Keys(_))
            || actions::validate(&hold, config).is_some()
        {
            issues.push(issue(RuleIssueCode::InvalidHoldAction, rule, Some(trigger)));
        }
        if actions::validate(&Action::parse(Some(&rule.double_tap_action)), config).is_some() {
            issues.push(issue(
                RuleIssueCode::InvalidDoubleTapAction,
                rule,
                Some(trigger),
            ));
        }
    }
    for (trigger, rules) in by_trigger.into_iter().filter(|(_, rules)| rules.len() > 1) {
        for rule in rules {
            issues.push(issue(RuleIssueCode::DuplicateTrigger, rule, Some(trigger)));
        }
    }
    issues
}

/// Drop disabled rules and drafts without a trigger.
pub fn runtime_rules(config: &mut AppConfig) {
    config
        .rules
        .retain(|rule| rule.is_enabled() && rule.has_trigger());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::model::{AppSettings, Layer, LayoutPreset};

    fn config(rules: Vec<LayerRule>) -> AppConfig {
        let preset = LayoutPreset {
            layers: vec![Layer {
                id: "nav".into(),
                name: "Nav".into(),
                description: None,
            }],
            rules,
            ..LayoutPreset::initial()
        };
        AppConfig::from_parts(AppSettings::default(), preset, None)
    }

    fn rule(id: &str, key: &str) -> LayerRule {
        LayerRule::new(id.into(), key)
    }

    #[test]
    fn reports_blocking_and_draft_issues() {
        let mut disabled = rule("d", "KeyA");
        disabled.enabled = Some(false);
        let mut layer = rule("l", "KeyB");
        layer.layer_id = "missing".into();
        let mut hold = rule("h", "KeyC");
        hold.hold_action = Some("macro:copyLine".into());
        let issues = analyze_rules(&config(vec![
            rule("a", "KeyA"),
            rule("b", "KeyA"),
            disabled,
            rule("draft", " "),
            layer,
            hold,
            rule("m", "MouseLeft"),
        ]));
        let codes: Vec<_> = issues
            .iter()
            .map(|i| (i.rule_id.as_str(), i.code))
            .collect();
        assert!(codes.contains(&("draft", RuleIssueCode::MissingTrigger)));
        assert!(codes.contains(&("a", RuleIssueCode::DuplicateTrigger)));
        assert!(codes.contains(&("b", RuleIssueCode::DuplicateTrigger)));
        assert!(codes.contains(&("l", RuleIssueCode::UnknownLayer)));
        assert!(codes.contains(&("h", RuleIssueCode::InvalidHoldAction)));
        assert!(codes.contains(&("m", RuleIssueCode::InvalidTrigger)));
        assert!(!codes.iter().any(|(id, _)| *id == "d"));
        assert!(!RuleIssueCode::MissingTrigger.is_error());
    }

    #[test]
    fn runtime_rules_skip_disabled_and_drafts() {
        let mut disabled = rule("d", "KeyA");
        disabled.enabled = Some(false);
        let mut config = config(vec![disabled, rule("draft", ""), rule("ok", "KeyB")]);
        runtime_rules(&mut config);
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].id, "ok");
    }
}
