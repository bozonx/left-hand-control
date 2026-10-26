use super::*;

#[test]
fn explicit_hold_behavior_prevents_layer_and_delayed_action_from_running_together() {
    for mode in ["none", "layer", "action"] {
        let cfg = config_with_rule(
            serde_json::json!({"key": "KeyA", "holdBehavior": mode, "layerId": "nav", "tapAction": null, "longHoldAction": "text:done", "holdAction": "ControlLeft"}),
        );
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
        engine.tick(now + Duration::from_secs(1), &mut out);
        assert_eq!(!engine.layers.active.is_empty(), mode == "layer");
        assert_eq!(
            out.iter()
                .any(|event| matches!(event, EngineOutput::Literal(_))),
            mode == "action"
        );
        if mode == "action" {
            assert!(engine.virt.emitted.is_empty());
        }
        engine.handle_key_event(
            KeyCode::KEY_A,
            false,
            now + Duration::from_secs(2),
            &mut out,
        );
        assert!(engine.virt.emitted.is_empty());
    }
}

#[test]
fn sel_layer_q_emits_ctrl_key_z() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Tab".into(),
        layer_id: "sel".into(),
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

    let mut sel = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    sel.keys.insert("KeyQ".into(), Some("Ctrl+KeyZ".into()));
    cfg.layer_keymaps.insert("sel".into(), sel);
    // Asserts the interrupt-on-press semantics.
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_TAB, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [EngineOutput::ChordPress { ks, .. }]
            if ks.mods == vec![KeyCode::KEY_LEFTCTRL] && ks.key == KeyCode::KEY_Z
    ));
}

#[test]
fn active_layer_bypasses_rule_for_same_key() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Space".into(),
        layer_id: "space".into(),
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
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Tab".into(),
        layer_id: "sel".into(),
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

    let mut space = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    space.keys.insert("Tab".into(), Some("Escape".into()));
    cfg.layer_keymaps.insert("space".into(), space);
    // Asserts the interrupt-on-press semantics.
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [EngineOutput::ChordPress { ks, .. }]
            if ks.mods.is_empty() && ks.key == KeyCode::KEY_ESC
    ));
}

#[test]
fn unmapped_key_in_active_layer_passthroughs() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Space".into(),
        layer_id: "space".into(),
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
    cfg.layer_keymaps.insert(
        "space".into(),
        LayerKeymap {
            keys: HashMap::new(),
            ..Default::default()
        },
    );
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_A,
                down: true
            },
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_A,
                down: false
            }
        ]
    ));
}

#[test]
fn explicit_null_in_layer_keymap_swallows_key() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Space".into(),
        layer_id: "space".into(),
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
    let mut space = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    space.keys.insert("KeyA".into(), None);
    cfg.layer_keymaps.insert("space".into(), space);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );

    assert!(out.is_empty());
}

#[test]
fn hold_can_activate_layer_and_modifier_together() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("AltLeft".into()),
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });

    let mut win = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    win.keys.insert("Tab".into(), Some("Tab".into()));
    cfg.layer_keymaps.insert("win".into(), win);
    // Asserts the interrupt-on-press semantics.
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::ChordPress { ks: alt, .. },
            EngineOutput::ChordPress { ks: tab, .. },
        ] if alt.mods.is_empty()
            && alt.key == KeyCode::KEY_LEFTALT
            && tab.mods.is_empty()
            && tab.key == KeyCode::KEY_TAB
    ));
}

#[test]
fn hold_modifiers_keep_alt_down_across_tab_presses() {
    const ALT: KeyCode = KeyCode::KEY_LEFTALT;
    const TAB: KeyCode = KeyCode::KEY_TAB;
    let mut engine = Engine::new(&switcher_cfg());
    let out = run(
        &mut engine,
        &[
            (ALT, true),
            (TAB, true),
            (TAB, false),
            (TAB, true),
            (TAB, false),
            (ALT, false),
        ],
    );
    assert_eq!(
        transitions(&out),
        [
            (ALT, true),
            (TAB, true),
            (TAB, false),
            (TAB, true),
            (TAB, false),
            (ALT, false),
        ]
    );
}

#[test]
fn hold_modifiers_switch_direction_without_releasing_alt() {
    const CAPS: KeyCode = KeyCode::KEY_CAPSLOCK;
    const ALT: KeyCode = KeyCode::KEY_LEFTALT;
    const SHIFT: KeyCode = KeyCode::KEY_LEFTSHIFT;
    const TAB: KeyCode = KeyCode::KEY_TAB;
    let mut engine = Engine::new(&switcher_cfg());
    let out = run(
        &mut engine,
        &[
            (ALT, true),
            (CAPS, true),
            (CAPS, false),
            (TAB, true),
            (TAB, false),
            (ALT, false),
        ],
    );
    assert_eq!(
        transitions(&out),
        [
            (ALT, true),
            (SHIFT, true),
            (TAB, true),
            (TAB, false),
            (SHIFT, false),
            (TAB, true),
            (TAB, false),
            (ALT, false),
        ]
    );
}

#[test]
fn ordinary_layer_keys_release_their_modifiers_with_the_key() {
    const KEY_C: KeyCode = KeyCode::KEY_C;
    const ALT: KeyCode = KeyCode::KEY_LEFTALT;
    let mut engine = Engine::new(&switcher_cfg());
    let out = run(
        &mut engine,
        &[(ALT, true), (KEY_C, true), (KEY_C, false), (ALT, false)],
    );
    assert_eq!(
        transitions(&out),
        [(ALT, true), (KEY_C, true), (KEY_C, false), (ALT, false)]
    );
}

#[test]
fn held_modifiers_outlive_a_main_key_still_down_at_layer_end() {
    const ALT: KeyCode = KeyCode::KEY_LEFTALT;
    const TAB: KeyCode = KeyCode::KEY_TAB;
    let mut engine = Engine::new(&switcher_cfg());
    let out = run(
        &mut engine,
        &[(ALT, true), (TAB, true), (ALT, false), (TAB, false)],
    );
    assert_eq!(
        transitions(&out),
        [(ALT, true), (TAB, true), (ALT, false), (TAB, false)]
    );
    let mut out = Vec::new();
    engine.shutdown(&mut out);
    assert!(transitions(&out).is_empty());
}

#[test]
fn layer_only_rule_is_not_treated_as_passthrough() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });

    let mut win = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    win.keys.insert("Tab".into(), Some("Escape".into()));
    cfg.layer_keymaps.insert("win".into(), win);
    // Asserts the interrupt-on-press semantics.
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [EngineOutput::ChordPress { ks, .. }]
            if ks.mods.is_empty() && ks.key == KeyCode::KEY_ESC
    ));
}

#[test]
fn layer_presses_count_only_keys_pressed_inside_a_layer() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key": "Space", "layerId": "nav", "tapAction": "Space"},
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_A, false, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    assert_eq!(engine.layer_presses(), 0);
    // The press that resolves the layer key as a hold is used inside it.
    engine.handle_key_event(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_J,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert_eq!(engine.top_layer(), Some("nav"));
    assert_eq!(engine.layer_presses(), 1);
    engine.handle_key_event(
        KeyCode::KEY_SPACE,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert_eq!(engine.top_layer(), None);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(40),
        &mut out,
    );
    assert_eq!(engine.layer_presses(), 1);
}

#[test]
fn layer_missing_key_is_transparent() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Space".into(),
        layer_id: "sp".into(),
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
    let mut sp = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    sp.keys.insert("KeyA".into(), Some("Escape".into()));

    cfg.layer_keymaps.insert("sp".into(), sp);

    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    out.clear();

    // KeyB without a mapping should passthrough
    engine.handle_key_event(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::KeyRaw { key, down: true }] if *key == KeyCode::KEY_B
    ));
}

#[test]
fn permissive_layer_rule_quick_tap_fires_paste() {
    // Mirrors the user's real rule: CapsLock layer=nav tap=Ctrl+KeyV.
    // A quick solo tap fires the paste; a nested layer use holds.
    let mut cfg = empty_cfg();
    let mut caps = tap_hold_rule(
        "CapsLock",
        ActionSpec::Action("Ctrl+KeyV".into()),
        ActionSpec::Native,
    );
    caps.layer_id = "nav".into();
    cfg.rules.push(caps);
    let mut nav = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    nav.keys.insert("KeyJ".into(), Some("ArrowLeft".into()));
    cfg.layer_keymaps.insert("nav".into(), nav);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    // Solo tap within the timeout → paste.
    engine.handle_key_event(KeyCode::KEY_CAPSLOCK, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_CAPSLOCK,
        false,
        now + Duration::from_millis(150),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [EngineOutput::Stroke { ks, .. }]
                if ks.mods == vec![KeyCode::KEY_LEFTCTRL] && ks.key == KeyCode::KEY_V
        ),
        "expected Ctrl+V tap"
    );

    // Nested layer use (J pressed and released while Caps held) → layer.
    out.clear();
    let t2 = now + Duration::from_millis(500);
    engine.handle_key_event(KeyCode::KEY_CAPSLOCK, true, t2, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_J,
        true,
        t2 + Duration::from_millis(30),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_J,
        false,
        t2 + Duration::from_millis(60),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_CAPSLOCK,
        false,
        t2 + Duration::from_millis(90),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [
                EngineOutput::ChordPress { ks: left, .. },
                EngineOutput::KeyRaw { key: up, down: false },
            ] if left.mods.is_empty()
                && left.key == KeyCode::KEY_LEFT
                && *up == KeyCode::KEY_LEFT
        ),
        "expected ArrowLeft press+release from the nav layer"
    );
}

#[test]
fn held_layer_snapshot_distinguishes_fast_represses_and_mapper_restarts() {
    let cfg =
        config_with_rule(serde_json::json!({"key": "Space", "layerId": "nav", "tapAction": null}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    let first = engine.held_layer_state();
    assert!(first.allows_hint("nav", first.hold_id));
    // Both events may arrive in the same fetch_key_events batch.
    engine.handle_key_event(KeyCode::KEY_SPACE, false, now, &mut out);
    assert!(engine.held_layer_state().layer.is_none());
    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    let second = engine.held_layer_state();
    assert_eq!(second.layer, first.layer);
    assert!(!second.allows_hint("nav", first.hold_id));
    assert!(second.allows_hint("nav", second.hold_id));
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_J, false, now, &mut out);
    assert!(!engine.held_layer_state().allows_hint("nav", second.hold_id));
    engine.handle_key_event(KeyCode::KEY_SPACE, false, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    let third = engine.held_layer_state();
    assert!(third.allows_hint("nav", third.hold_id));
    let mut restarted = Engine::new(&cfg);
    restarted.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    assert_ne!(restarted.held_layer_state().hold_id, third.hold_id);
}

#[test]
fn nested_layers_keep_usage_suppressed_until_all_triggers_are_released() {
    let mut engine = Engine::new(&empty_cfg());
    let mut out = Vec::new();
    engine.layers.push("nav".into());
    let first = engine.held_layer_state();
    engine.handle_key_event(KeyCode::KEY_J, true, Instant::now(), &mut out);
    engine.layers.push("symbols".into());
    assert_eq!(engine.held_layer_state().hold_id, first.hold_id);
    assert!(engine.held_layer_state().used);
    engine.layers.pop("symbols");
    assert!(engine.held_layer_state().used);
    engine.layers.pop("nav");
    engine.layers.push("nav".into());
    assert!(!engine.held_layer_state().used);
    assert_ne!(engine.held_layer_state().hold_id, first.hold_id);
}

#[test]
fn reconfigure_preserves_held_keys_layers_and_original_release_mapping() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({
        "rules": [{"key": "Space", "tapAction": null, "layerId": "nav"}],
        "layerKeymaps": {"nav": {"keys": {"KeyJ": "ArrowLeft"}}}
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    out.clear();
    engine.reconfigure(&MapperConfig::default(), &mut out);
    assert!(out.is_empty());
    assert_eq!(engine.top_layer(), Some("nav"));
    engine.handle_key_event(KeyCode::KEY_J, false, now, &mut out);
    assert!(matches!(
        out.last(),
        Some(EngineOutput::KeyRaw {
            key: KeyCode::KEY_LEFT,
            down: false
        })
    ));
    engine.handle_key_event(KeyCode::KEY_SPACE, false, now, &mut out);
    assert_eq!(engine.top_layer(), None);
    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, false, now, &mut out);
    assert!(matches!(
        out.last(),
        Some(EngineOutput::KeyRaw {
            key: KeyCode::KEY_LEFTSHIFT,
            down: false
        })
    ));
}
