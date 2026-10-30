use super::*;

#[test]
fn builder_skips_unknown_rule_key() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "UnknownKey".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let engine = Engine::new(&cfg);
    assert!(!engine.layout.rules.contains_key(&KeyCode::KEY_A));
}

#[test]
fn builder_skips_primary_mouse_buttons_as_triggers() {
    let mut cfg = empty_cfg();
    for key in ["MouseLeft", "MouseRight", "MouseMiddle"] {
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: key.into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("Escape".into()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
    }
    let engine = Engine::new(&cfg);
    assert!(!engine.layout.rules.contains_key(&KeyCode::BTN_LEFT));
    assert!(!engine.layout.rules.contains_key(&KeyCode::BTN_RIGHT));
    assert!(!engine.layout.rules.contains_key(&KeyCode::BTN_MIDDLE));
}

#[test]
fn builder_unknown_tap_action_falls_back_to_swallow() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("BadActionSyntax!!!".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Swallow,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    // Unknown tap action resolves to Swallow, hold is Swallow → fast path, nothing emitted
    assert!(out.is_empty());
}

#[test]
fn builder_resolves_builtin_macro() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("builtin:copyLineText".into()),
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
    let mut out = Vec::new();
    let now = Instant::now();

    // Tap fires the macro on release (no double-tap)
    engine.handle_key_event(KeyCode::KEY_Q, true, now, &mut out);
    assert!(out.is_empty());
    engine.handle_key_event(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    // Macro should now be active
    assert!(engine.runner.active.is_some());
}

#[test]
fn trigger_chord_buffered_long_hold_resolves_at_release_timestamp() {
    for release_ms in [19, 20, 21] {
        let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
            {"key":"KeyA", "tapAction":"text:tap", "longHoldAction":"text:long", "longHoldTimeoutMs":20},
            {"key":"KeyA+KeyB", "tapAction":"text:chord"}
        ]})).unwrap();
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
        engine.handle_key_event(
            KeyCode::KEY_A,
            false,
            now + Duration::from_millis(release_ms),
            &mut out,
        );
        engine.tick(now + Duration::from_secs(1), &mut out);
        let expected = if release_ms < 20 { "tap" } else { "long" };
        assert!(matches!(out.as_slice(), [EngineOutput::Literal(s)] if s == expected));
    }
}

#[test]
fn remote_and_bound_actions_resolve_the_same_way() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({
        "macros": [{"id": "m", "steps": [{"action": "KeyA"}]}],
        "rules": [{"key": "KeyQ", "tapAction": "  text: hi "}]
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_Q, false, now, &mut out);
    engine.execute_remote("  text: hi ", &mut out);
    let texts: Vec<_> = out
        .iter()
        .filter_map(|item| match item {
            EngineOutput::Literal(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, [" hi ", " hi "]);
    out.clear();
    // Things that do nothing outside a macro stay silent everywhere.
    for action in ["", "pause:100", "macro:missing", "NoSuchKey"] {
        engine.execute_remote(action, &mut out);
    }
    assert!(out.is_empty());
}

#[test]
fn builtin_and_user_macros_resolve_independently_at_every_depth() {
    use crate::mapper::action::MacroStepItem;

    let mut cfg = empty_cfg();
    cfg.macros = vec![
        Macro {
            id: "copyLine".into(),
            steps: vec![MacroStep {
                action: "text:user".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        },
        Macro {
            id: "outer".into(),
            steps: vec![
                MacroStep {
                    action: "macro:copyLine".into(),
                },
                MacroStep {
                    action: "builtin:copyLineText".into(),
                },
            ],
            step_pause_ms: None,
            modifier_delay_ms: None,
        },
    ];
    crate::mapper::validation::validate_config(&cfg).unwrap();
    let engine = Engine::new(&cfg);
    let user = &engine.layout.macros["macro:copyLine"];
    let builtin = &engine.layout.macros["builtin:copyLineText"];
    let outer = &engine.layout.macros["macro:outer"];
    assert!(matches!(&user.steps[..], [MacroStepItem::Literal(text)] if text == "user"));
    assert!(
        builtin
            .steps
            .iter()
            .all(|step| matches!(step, MacroStepItem::Stroke(_)))
    );
    assert_eq!(outer.steps.len(), 1 + builtin.steps.len());
    assert!(matches!(&outer.steps[0], MacroStepItem::Literal(text) if text == "user"));
    assert!(
        outer.steps[1..]
            .iter()
            .all(|step| matches!(step, MacroStepItem::Stroke(_)))
    );
    for reference in ["macro:copyLine", "builtin:copyLineText"] {
        assert!(matches!(
            crate::mapper::engine::builder::resolve_action(
                reference,
                &engine.layout.macros,
                &HashMap::new(),
                "test"
            ),
            Some(model::CompiledAction::Macro(_))
        ));
    }
    let engine = Engine::new(&empty_cfg());
    assert!(
        crate::mapper::engine::builder::resolve_action(
            "macro:copyLine",
            &engine.layout.macros,
            &HashMap::new(),
            "test"
        )
        .is_none()
    );
}

#[test]
fn development_gate_disables_user_macros_popups_and_long_hold() {
    for enabled in [false, true] {
        let mut cfg: MapperConfig = serde_json::from_value(serde_json::json!({
            "settings": {"inDevelopment": enabled},
            "macros": [{"id": "user", "steps": [{"action": "text:user"}]}],
            "rules": [
                {"key": "KeyA", "longHoldAction": "text:hold"},
                {"key": "KeyB", "tapAction": "macro:user"},
                {"key": "KeyC", "tapAction": "app:showEmojiMenuLast"},
                {"key": "KeyD", "tapAction": "app:showQuickMenu1"}
            ]
        }))
        .unwrap();
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
        engine.tick(now + Duration::from_secs(2), &mut out);
        assert_eq!(
            out.iter()
                .any(|item| matches!(item, EngineOutput::Literal(text) if text == "hold")),
            enabled
        );
        assert_eq!(engine.layout.macros.contains_key("macro:user"), enabled);
        for rule in &mut cfg.rules {
            rule.long_hold_action.clear();
        }
        let engine = Engine::new(&cfg);
        for key in [KeyCode::KEY_B, KeyCode::KEY_C, KeyCode::KEY_D] {
            assert_eq!(
                matches!(
                    engine.layout.rules[&key][0].tap,
                    crate::mapper::engine::model::TapMode::Action(_)
                ),
                enabled
            );
        }
        assert!(
            !engine.layout.macros.is_empty(),
            "built-in macros stay available"
        );
    }
}

#[test]
fn swallowed_tap_with_native_hold_emits_nothing() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Enter".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Swallow,
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
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_ENTER, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_ENTER,
        false,
        now + Duration::from_millis(500),
        &mut out,
    );
    assert!(out.is_empty());
}
