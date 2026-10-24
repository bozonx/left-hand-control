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
    InvalidLayerKey,
    InvalidLayerAction,
    InvalidMacro,
    InvalidMacroStep,
    MacroCycle,
    InvalidMenuAction,
    InvalidTiming,
    InvalidCondition,
    InvalidRuleConfiguration,
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
            RuleIssueCode::InvalidLayerKey => write!(f, "Unknown layer key at {trigger}."),
            RuleIssueCode::InvalidLayerAction => write!(f, "Invalid layer action at {trigger}."),
            RuleIssueCode::InvalidMacroStep => write!(f, "Invalid macro step at {trigger}."),
            RuleIssueCode::MacroCycle => write!(f, "Macro call cycle at {trigger}."),
            RuleIssueCode::InvalidMacro => write!(f, "Invalid macro at {trigger}."),
            RuleIssueCode::InvalidMenuAction => write!(f, "Invalid menu action at {trigger}."),
            RuleIssueCode::InvalidTiming => write!(f, "Invalid timing at {trigger}."),
            RuleIssueCode::InvalidCondition => write!(f, "Invalid window condition at {trigger}."),
            RuleIssueCode::InvalidRuleConfiguration => {
                write!(f, "Unsupported rule combination at {trigger}.")
            }
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

/// Check the complete active configuration, including actions outside rules.
/// `trigger` carries the document path for issues outside a rule.
pub fn analyze(config: &AppConfig) -> Vec<RuleIssue> {
    let mut issues = analyze_rules(config);
    let mut add = |code, path: String| {
        issues.push(RuleIssue {
            code,
            rule_id: String::new(),
            trigger: Some(path),
        })
    };
    if super::timing::validate_settings(&config.settings).is_err() {
        add(RuleIssueCode::InvalidTiming, "settings.timing".into());
    }
    let invalid_pattern = |pattern: &String| super::app_match::AppMatcher::parse(pattern).is_none();
    for (index, rule) in config
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.is_enabled() && rule.has_trigger())
    {
        for (field, value, range) in [
            (
                "hold.timeoutMs",
                rule.hold_timeout_ms,
                super::timing::TAP_HOLD,
            ),
            (
                "doubleTap.timeoutMs",
                rule.double_tap_timeout_ms,
                super::timing::TAP_HOLD,
            ),
            (
                "longHold.timeoutMs",
                rule.long_hold_timeout_ms,
                super::timing::LONG_HOLD,
            ),
        ] {
            if value.is_some_and(|value| !range.contains(value)) {
                add(
                    RuleIssueCode::InvalidTiming,
                    format!("rules[{index}].{field}"),
                );
            }
        }
        if list(&rule.condition_apps_whitelist)
            .iter()
            .chain(list(&rule.condition_apps_blacklist))
            .any(invalid_pattern)
        {
            add(
                RuleIssueCode::InvalidCondition,
                format!("rules[{index}].when"),
            );
        }
        if rule.key.contains('+') && !rule.double_tap_action.is_empty() {
            add(
                RuleIssueCode::InvalidRuleConfiguration,
                format!("rules[{index}].doubleTap"),
            );
        }
    }
    for (layer, map) in &config.layer_keymaps {
        for (key, action) in &map.keys {
            let path = format!("layers[{layer}].keys[{key}]");
            if !super::key_catalog::valid_key(key) {
                add(RuleIssueCode::InvalidLayerKey, path.clone());
            }
            if actions::validate(&Action::parse(action.as_deref()), config).is_some() {
                add(RuleIssueCode::InvalidLayerAction, path);
            }
        }
        for key in &map.extra_key_order {
            if !super::key_catalog::valid_key(key) {
                add(
                    RuleIssueCode::InvalidLayerKey,
                    format!("layers[{layer}].extraKeyOrder[{key}]"),
                );
            }
        }
    }
    for item in &config.macros {
        for (index, step) in item.steps.iter().enumerate() {
            if super::macros::validate_step(config, &step.action).is_err() {
                add(
                    RuleIssueCode::InvalidMacroStep,
                    format!("macros[{}].steps[{index}]", item.id),
                );
            }
        }
        match super::macros::validate_loaded(config, item) {
            Err(crate::config_document::ConfigError::Macro(super::macros::MacroIssue::Cycle)) => {
                add(RuleIssueCode::MacroCycle, format!("macros[{}]", item.id))
            }
            Err(crate::config_document::ConfigError::Macro(
                super::macros::MacroIssue::InvalidId | super::macros::MacroIssue::DuplicateId,
            )) => add(
                RuleIssueCode::InvalidMacro,
                format!("macros[{}].id", item.id),
            ),
            Err(crate::config_document::ConfigError::Macro(
                super::macros::MacroIssue::DelayRange,
            )) => add(RuleIssueCode::InvalidTiming, format!("macros[{}]", item.id)),
            _ => {}
        }
    }
    for page in &config.quick_action_pages {
        for (key, cell) in &page.cells {
            if !super::model::LEFT_HAND_HOTKEYS.contains(&key.as_str())
                || actions::validate(&Action::parse(Some(&cell.action)), config).is_some()
            {
                add(
                    RuleIssueCode::InvalidMenuAction,
                    format!("quickActionPages[{}].cells[{key}]", page.id),
                );
            }
        }
    }
    for (index, rule) in config
        .settings
        .auto_rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.enabled)
    {
        if rule.conditions.apps.iter().any(invalid_pattern) {
            add(
                RuleIssueCode::InvalidCondition,
                format!("settings.autoRules[{index}].when.windows"),
            );
        }
        if rule.conditions.excluded_apps.iter().any(invalid_pattern) {
            add(
                RuleIssueCode::InvalidCondition,
                format!("settings.autoRules[{index}].when.excludeWindows"),
            );
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
    fn incomplete_rule_drafts_do_not_block_runtime() {
        let mut draft = rule("draft", "");
        draft.condition_apps_whitelist = Some(vec!["title:".into()]);
        let issues = analyze(&config(vec![draft]));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, RuleIssueCode::MissingTrigger);
        assert!(!issues[0].code.is_error());
    }

    #[test]
    fn invalid_conditions_and_ignored_rule_combinations_are_visible() {
        let layout = super::super::layout_file::parse(
            r#"{"schemaVersion":1,"rules":[
            {"id":"bad-condition","key":"KeyA","when":{"windows":["title:"]}},
            {"id":"bad-double","key":"KeyC+KeyD","doubleTap":{"action":"KeyE"}}
        ]}"#,
        )
        .unwrap()
        .unwrap();
        let issues = analyze(&AppConfig::from_parts(AppSettings::default(), layout, None));
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == RuleIssueCode::InvalidCondition)
                .count(),
            1
        );
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == RuleIssueCode::InvalidRuleConfiguration)
                .count(),
            1
        );
    }

    #[test]
    fn complete_diagnostics_cover_non_rule_actions_and_cycles() {
        let layout = super::super::layout_file::parse(
            r#"{"schemaVersion":1,
            "layers":[{"id":"nav","keys":{"NoSuchKey":"KeyA","KeyA":"macro:missing"}}],
            "macros":[{"id":"a","steps":["macro:b"]},{"id":"b","steps":["macro:a"]}],
            "quickActionPages":[{"id":"q","cells":{"KeyQ":"cmd:missing"}}]}"#,
        )
        .unwrap()
        .unwrap();
        let config = AppConfig::from_parts(AppSettings::default(), layout, None);
        let issues = analyze(&config);
        for code in [
            RuleIssueCode::InvalidLayerKey,
            RuleIssueCode::InvalidLayerAction,
            RuleIssueCode::MacroCycle,
            RuleIssueCode::InvalidMenuAction,
        ] {
            assert!(issues.iter().any(|issue| issue.code == code), "{issues:?}");
        }
        assert!(
            issues
                .iter()
                .all(|issue| issue.code.is_error() && issue.trigger.is_some())
        );
    }

    #[test]
    fn unknown_macro_keys_pauses_and_references_are_diagnosed() {
        for step in [
            "NotAKey",
            "macro:missing",
            "cmd:missing",
            "pause:0",
            "pause:10001",
            "pause:18446744073709551615",
        ] {
            let text = serde_json::json!({"schemaVersion":1,"macros":[{"id":"m","steps":[step]}]})
                .to_string();
            let layout = super::super::layout_file::parse(&text).unwrap().unwrap();
            let config = AppConfig::from_parts(AppSettings::default(), layout, None);
            assert!(
                analyze(&config)
                    .iter()
                    .any(|issue| issue.code == RuleIssueCode::InvalidMacroStep),
                "{step}"
            );
        }
        for step in ["pause:1", "pause:10000"] {
            let text = serde_json::json!({"schemaVersion":1,"macros":[{"id":"m","steps":[step]}]})
                .to_string();
            let layout = super::super::layout_file::parse(&text).unwrap().unwrap();
            assert!(
                analyze(&AppConfig::from_parts(AppSettings::default(), layout, None)).is_empty()
            );
        }
    }

    #[test]
    fn reports_blocking_and_draft_issues() {
        let mut disabled = rule("d", "KeyA");
        disabled.enabled = Some(false);
        let mut layer = rule("l", "KeyB");
        layer.layer_id = "missing".into();
        let mut hold = rule("h", "KeyC");
        hold.hold_action = Some("builtin:copyLine".into());
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
