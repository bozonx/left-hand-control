use super::*;
use crate::profile::auto_switch::AutoSwitchContext;

fn with_window(title: &str, app_id: &str) -> AutoSwitchContext {
    AutoSwitchContext {
        window: Some(crate::runtime_state::ActiveWindow {
            title: title.into(),
            app_id: app_id.into(),
            process_name: None,
            fullscreen: None,
        }),
        ..Default::default()
    }
}

#[test]
fn apps_whitelist_blocks_when_no_active_window() {
    let ctx = AutoSwitchContext::default();
    let rule = rule_with_apps(Some(vec!["firefox".into()]), None);
    assert!(!rule_passes_apps(&rule, &ctx));
}

#[test]
fn apps_whitelist_passes_when_title_matches() {
    let ctx = with_window("Mozilla Firefox", "navigator");
    let rule = rule_with_apps(Some(vec!["title:firefox".into()]), None);
    assert!(rule_passes_apps(&rule, &ctx));
    let rule = rule_with_apps(Some(vec!["firefox".into()]), None);
    assert!(!rule_passes_apps(&rule, &ctx));
}

#[test]
fn apps_whitelist_passes_when_app_id_matches_case_insensitive() {
    let ctx = with_window("Library", "Steam");
    let rule = rule_with_apps(Some(vec!["steam".into()]), None);
    assert!(rule_passes_apps(&rule, &ctx));
}

#[test]
fn apps_blacklist_blocks_even_when_whitelist_passes() {
    let ctx = with_window("Editor — secret", "editor");
    let rule = rule_with_apps(
        Some(vec!["editor".into()]),
        Some(vec!["title:secret".into()]),
    );
    assert!(!rule_passes_apps(&rule, &ctx));
}

#[test]
fn apps_no_lists_always_passes() {
    let ctx = AutoSwitchContext::default();
    let rule = rule_with_apps(None, None);
    assert!(rule_passes_apps(&rule, &ctx));
}

#[test]
fn undetected_game_mode_counts_as_off() {
    let ctx = AutoSwitchContext::default();
    let mut rule = RuleEntry {
        tap_decision: TapDecision::PermissiveHold,
        trigger_keys: vec![],
        tap: TapMode::Native,
        layer_id: None,
        hold: HoldMode::Swallow,
        long_hold: None,
        long_hold_timeout: Duration::from_secs(1),
        double_tap: None,
        hold_timeout: Duration::from_millis(200),
        double_tap_window: Duration::from_millis(200),
        condition_game_mode: Some(GameModeCondition::On),
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
    };

    assert!(!rule_passes_conditions(&rule, &ctx));
    rule.condition_game_mode = Some(GameModeCondition::Off);
    assert!(rule_passes_conditions(&rule, &ctx));
}

#[test]
fn apps_blacklist_blocks_when_no_active_window() {
    let ctx = AutoSwitchContext::default();
    let rule = rule_with_apps(None, Some(vec!["secret".into()]));
    assert!(!rule_passes_apps(&rule, &ctx));
}

#[test]
fn game_mode_condition_passes_when_cached_state_matches() {
    let ctx = AutoSwitchContext {
        game_mode_active: true,
        ..Default::default()
    };
    let rule = RuleEntry {
        tap_decision: TapDecision::PermissiveHold,
        trigger_keys: vec![],
        tap: TapMode::Native,
        layer_id: None,
        hold: HoldMode::Swallow,
        long_hold: None,
        long_hold_timeout: Duration::from_secs(1),
        double_tap: None,
        hold_timeout: Duration::from_millis(200),
        double_tap_window: Duration::from_millis(200),
        condition_game_mode: Some(GameModeCondition::On),
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
    };

    assert!(rule_passes_conditions(&rule, &ctx));
}

#[test]
fn duplicate_key_rules_use_first_matching_condition() {

    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: Some(vec!["firefox".into()]),
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyB".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });

    let mut engine = Engine::new(&cfg);
    engine.set_context_source(Box::new(AutoSwitchContext::default));
    let mut out = Vec::new();
    let now = Instant::now();
    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_B
    ));
}
