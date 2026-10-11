//! Rule checks run before the mapper starts.

use super::actions::{self, Action};
use super::model::{AppConfig, LayerRule};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleIssueCode {
    MissingTrigger,
    InvalidTrigger,
    DuplicateTrigger,
    ShadowedTrigger,
    UnknownLayer,
    InvalidTapAction,
    InvalidHoldAction,
    InvalidDoubleTapAction,
    UnknownListedKey,
}

impl RuleIssueCode {
    /// Drafts and trigger conflicts do not block the mapper.
    pub fn is_error(self) -> bool {
        !matches!(
            self,
            Self::MissingTrigger | Self::DuplicateTrigger | Self::ShadowedTrigger
        )
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
                    "Rules for trigger \"{trigger}\" may overlap; the first matching rule wins."
                )
            }
            RuleIssueCode::ShadowedTrigger => {
                write!(
                    f,
                    "Rule for trigger \"{trigger}\" is covered by an earlier rule and will not run."
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
            RuleIssueCode::UnknownListedKey => {
                write!(f, "Rule \"{trigger}\" lists an unknown key.")
            }
        }
    }
}

pub fn analyze_rules(config: &AppConfig) -> Vec<RuleIssue> {
    let layer_ids: HashSet<&str> = config
        .layers
        .iter()
        .map(|layer| layer.id.as_str())
        .collect();
    let mut by_trigger: BTreeMap<Vec<&str>, Vec<&LayerRule>> = BTreeMap::new();
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
        if !super::key_catalog::valid_trigger(trigger) {
            issues.push(issue(RuleIssueCode::InvalidTrigger, rule, Some(trigger)));
        }
        if let Some(mut keys) = super::key_catalog::trigger_keys(trigger) {
            keys.sort_unstable();
            by_trigger.entry(keys).or_default().push(rule);
        }
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
        if actions::validate(&Action::parse(Some(&rule.long_hold_action)), config).is_some() {
            issues.push(issue(RuleIssueCode::InvalidHoldAction, rule, Some(trigger)));
        }
        if actions::validate(&Action::parse(Some(&rule.double_tap_action)), config).is_some() {
            issues.push(issue(
                RuleIssueCode::InvalidDoubleTapAction,
                rule,
                Some(trigger),
            ));
        }
        if [&rule.isolate, &rule.hold_for]
            .into_iter()
            .flatten()
            .any(|list| super::key_catalog::unknown_keys(list).next().is_some())
        {
            issues.push(issue(RuleIssueCode::UnknownListedKey, rule, Some(trigger)));
        }
    }
    for (trigger, rules) in by_trigger.into_iter().filter(|(_, rules)| rules.len() > 1) {
        for (index, rule) in rules.iter().enumerate().skip(1) {
            let earlier = &rules[..index];
            let code = if earlier
                .iter()
                .any(|previous| conditions_cover(previous, rule))
            {
                RuleIssueCode::ShadowedTrigger
            } else if earlier
                .iter()
                .any(|previous| conditions_overlap(previous, rule))
            {
                RuleIssueCode::DuplicateTrigger
            } else {
                continue;
            };
            issues.push(issue(code, rule, Some(&trigger.join("+"))));
        }
    }

    issues
}

fn list(value: &Option<Vec<String>>) -> &[String] {
    value.as_deref().unwrap_or_default()
}

fn game(rule: &LayerRule) -> Option<&str> {
    rule.condition_game_mode
        .as_deref()
        .filter(|mode| matches!(*mode, "on" | "off"))
}

// App ids, process names and titles can match different patterns on the same
// window. Only proven whitelist/blacklist exclusion makes them disjoint.
fn apps_exclude(whitelist: &[String], blacklist: &[String]) -> bool {
    !whitelist.is_empty()
        && whitelist.iter().all(|pattern| {
            blacklist
                .iter()
                .any(|excluded| super::app_match::covers(excluded, pattern))
        })
}

fn conditions_overlap(a: &LayerRule, b: &LayerRule) -> bool {
    if game(a).is_some() && game(b).is_some() && game(a) != game(b) {
        return false;
    }
    let (a_layouts, b_layouts) = (list(&a.condition_layouts), list(&b.condition_layouts));
    if !a_layouts.is_empty()
        && !b_layouts.is_empty()
        && !a_layouts.iter().any(|layout| b_layouts.contains(layout))
    {
        return false;
    }
    !apps_exclude(
        list(&a.condition_apps_whitelist),
        list(&b.condition_apps_blacklist),
    ) && !apps_exclude(
        list(&b.condition_apps_whitelist),
        list(&a.condition_apps_blacklist),
    )
}

fn conditions_cover(earlier: &LayerRule, later: &LayerRule) -> bool {
    let layouts = list(&earlier.condition_layouts);
    let later_layouts = list(&later.condition_layouts);
    let apps = list(&earlier.condition_apps_whitelist);
    let later_apps = list(&later.condition_apps_whitelist);
    let excluded = list(&earlier.condition_apps_blacklist);
    let later_excluded = list(&later.condition_apps_blacklist);
    conditions_overlap(earlier, later)
        && (game(earlier).is_none() || game(earlier) == game(later))
        && (layouts.is_empty()
            || (!later_layouts.is_empty()
                && later_layouts.iter().all(|layout| layouts.contains(layout))))
        && (apps.is_empty()
            || (!later_apps.is_empty()
                && later_apps.iter().all(|pattern| {
                    apps.iter()
                        .any(|outer| super::app_match::covers(outer, pattern))
                })))
        && excluded.iter().all(|pattern| {
            later_excluded
                .iter()
                .any(|outer| super::app_match::covers(outer, pattern))
        })
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
        assert!(!codes.iter().any(|(id, _)| *id == "a"));
        assert!(codes.contains(&("b", RuleIssueCode::ShadowedTrigger)));
        assert!(codes.contains(&("l", RuleIssueCode::UnknownLayer)));
        assert!(codes.contains(&("h", RuleIssueCode::InvalidHoldAction)));
        assert!(codes.contains(&("m", RuleIssueCode::InvalidTrigger)));
        assert!(!codes.iter().any(|(id, _)| *id == "d"));
        assert!(!RuleIssueCode::MissingTrigger.is_error());
    }

    #[test]
    fn conflicts_are_warnings_and_equivalent_chords_are_compared() {
        let issues = analyze_rules(&config(vec![
            rule("a", "Ctrl+ShiftLeft"),
            rule("b", "ShiftLeft+ControlLeft"),
        ]));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "b");
        assert_eq!(issues[0].code, RuleIssueCode::ShadowedTrigger);
        assert!(!issues[0].code.is_error());
        assert!(!RuleIssueCode::DuplicateTrigger.is_error());
        assert!(RuleIssueCode::InvalidTrigger.is_error());
    }

    #[test]
    fn disjoint_game_layout_and_app_exclusions_allow_repeated_triggers() {
        let mut a = rule("a", "KeyQ");
        let mut b = rule("b", "KeyQ");
        a.condition_game_mode = Some("on".into());
        b.condition_game_mode = Some("off".into());
        assert!(analyze_rules(&config(vec![a.clone(), b.clone()])).is_empty());
        a.condition_game_mode = None;
        b.condition_game_mode = Some("ignore".into());
        a.condition_layouts = Some(vec!["en".into()]);
        b.condition_layouts = Some(vec!["ru".into()]);
        assert!(analyze_rules(&config(vec![a.clone(), b.clone()])).is_empty());
        a.condition_layouts = None;
        b.condition_layouts = Some(vec![]);
        a.condition_apps_whitelist = Some(vec!["Firefox".into()]);
        b.condition_apps_blacklist = Some(vec!["fire*".into()]);
        assert!(analyze_rules(&config(vec![a.clone(), b.clone()])).is_empty());
        assert!(analyze_rules(&config(vec![b, a])).is_empty());
    }

    #[test]
    fn conditional_rule_before_fallback_overlaps_but_reverse_shadows() {
        let mut conditional = rule("app", "KeyQ");
        conditional.condition_apps_whitelist = Some(vec!["firefox".into()]);
        let fallback = rule("fallback", "KeyQ");
        let issues = analyze_rules(&config(vec![conditional.clone(), fallback.clone()]));
        assert_eq!(issues[0].code, RuleIssueCode::DuplicateTrigger);
        assert_eq!(issues[0].rule_id, "fallback");
        let issues = analyze_rules(&config(vec![fallback, conditional]));
        assert_eq!(issues[0].code, RuleIssueCode::ShadowedTrigger);
        assert_eq!(issues[0].rule_id, "app");
    }

    #[test]
    fn partial_layout_overlap_and_title_patterns_are_not_declared_disjoint() {
        let mut a = rule("a", "KeyQ");
        let mut b = rule("b", "KeyQ");
        a.condition_layouts = Some(vec!["en".into(), "ru".into()]);
        b.condition_layouts = Some(vec!["ru".into(), "de".into()]);
        a.condition_apps_whitelist = Some(vec!["firefox".into()]);
        b.condition_apps_whitelist = Some(vec!["title:YouTube".into()]);
        let issues = analyze_rules(&config(vec![a, b]));
        assert_eq!(issues[0].code, RuleIssueCode::DuplicateTrigger);
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
