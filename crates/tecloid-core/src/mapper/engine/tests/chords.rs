use super::*;

#[test]
fn unassigned_chord_ignores_double_tap_and_preserves_normal_keys() {
    for extra in ["", "text:double"] {
        let cfg = config_with_rule(
            serde_json::json!({"key": "ShiftLeft+ControlLeft", "doubleTapAction": extra}),
        );
        let engine = Engine::new(&cfg);
        assert!(engine.keymap.chord_rules.is_empty());
    }
}

#[test]
fn chord_layer_works_in_either_order_and_ends_on_any_release() {
    for (first, second) in [
        (KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_LEFTCTRL),
        (KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT),
    ] {
        for release in [first, second] {
            let cfg = config_with_rule(
                serde_json::json!({"key": "ShiftLeft+ControlLeft", "layerId": "nav", "tapAction": null}),
            );
            let mut engine = Engine::new(&cfg);
            let now = Instant::now();
            let mut out = Vec::new();
            engine.handle_key_event(first, true, now, &mut out);
            engine.handle_key_event(second, true, now + Duration::from_millis(10), &mut out);
            assert_eq!(engine.layers.active, vec!["nav"]);
            engine.handle_key_event(release, false, now + Duration::from_millis(20), &mut out);
            assert!(engine.layers.active.is_empty());
            let other = if release == first { second } else { first };
            engine.handle_key_event(other, false, now + Duration::from_millis(30), &mut out);
            assert!(engine.virt.emitted.is_empty());
            assert!(engine.virt.mod_refs.values().all(|count| *count == 0));
            assert!(engine.chords.consumed.is_empty());
        }
    }
}

#[test]
fn chord_long_hold_cancels_on_first_release_and_rearms_after_all_keys_are_up() {
    let cfg = config_with_rule(
        serde_json::json!({"key": "KeyA+KeyB", "tapAction": null, "longHoldAction": "text:done", "longHoldTimeoutMs": 500}),
    );
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_B, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_B,
        false,
        now + Duration::from_millis(400),
        &mut out,
    );
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(
        !out.iter()
            .any(|event| matches!(event, EngineOutput::Literal(_)))
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(1100),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(1200),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(1210),
        &mut out,
    );
    out.clear();
    engine.tick(now + Duration::from_millis(1800), &mut out);
    engine.tick(now + Duration::from_secs(3), &mut out);
    assert!(matches!(out.as_slice(), [EngineOutput::Literal(text)] if text == "done"));
    engine.shutdown(&mut out);
    assert!(engine.long_holds.is_empty());
    assert!(engine.chords.active.is_empty());
}

#[test]
fn chord_ignores_double_tap_without_delaying_tap() {
    let cfg = config_with_rule(
        serde_json::json!({"key": "ShiftLeft+ControlLeft", "tapAction": "text:tap", "doubleTapAction": "text:double"}),
    );
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    for (offset, first, second) in [
        (0, KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_LEFTCTRL),
        (40, KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT),
    ] {
        engine.handle_key_event(first, true, now + Duration::from_millis(offset), &mut out);
        engine.handle_key_event(
            second,
            true,
            now + Duration::from_millis(offset + 10),
            &mut out,
        );
        engine.handle_key_event(
            first,
            false,
            now + Duration::from_millis(offset + 20),
            &mut out,
        );
        assert!(
            out.iter()
                .any(|event| matches!(event, EngineOutput::Literal(text) if text == "tap"))
        );
        assert!(
            !out.iter()
                .any(|event| matches!(event, EngineOutput::Literal(text) if text == "double"))
        );
        engine.handle_key_event(
            second,
            false,
            now + Duration::from_millis(offset + 30),
            &mut out,
        );
        out.clear();
    }
}

#[test]
fn trigger_chord_members_never_reach_the_app() {
    for (first, second) in [
        (KeyCode::KEY_J, KeyCode::KEY_K),
        (KeyCode::KEY_K, KeyCode::KEY_J),
    ] {
        let cfg =
            config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(first, true, now, &mut out);
        assert!(out.is_empty());
        engine.handle_key_event(second, true, now + Duration::from_millis(20), &mut out);
        engine.handle_key_event(first, false, now + Duration::from_millis(60), &mut out);
        engine.handle_key_event(second, false, now + Duration::from_millis(70), &mut out);
        assert!(fired_text(&out));
        assert!(raw_keys(&out).is_empty());
        assert!(engine.chords.buffer.is_empty());
        assert!(engine.chords.consumed.is_empty());
    }
}

#[test]
fn trigger_chord_window_expiry_types_the_member_and_blocks_the_chord() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    assert_eq!(
        engine.next_deadline(now),
        Some(engine.keymap.trigger_chord_timeout)
    );
    engine.tick(now + engine.keymap.trigger_chord_timeout, &mut out);
    assert_eq!(raw_keys(&out), vec![(KeyCode::KEY_J, true)]);
    // `j` is already typed — a late K must not turn it into the chord.
    engine.handle_key_event(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(70),
        &mut out,
    );
    engine.tick(now + Duration::from_millis(200), &mut out);
    assert!(!fired_text(&out));
    assert_eq!(
        raw_keys(&out),
        vec![(KeyCode::KEY_J, true), (KeyCode::KEY_K, true)]
    );
}

#[test]
fn trigger_chord_window_comes_from_settings() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({
        "rules": [{"key": "KeyJ+KeyK", "tapAction": "text:esc"}],
        "settings": {"comboTimeoutMs": 120},
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    assert_eq!(engine.next_deadline(now), Some(Duration::from_millis(120)));
    engine.handle_key_event(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(100),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_K,
        false,
        now + Duration::from_millis(130),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_J,
        false,
        now + Duration::from_millis(140),
        &mut out,
    );
    assert!(fired_text(&out));
    assert!(raw_keys(&out).is_empty());
}

#[test]
fn trigger_chord_late_press_before_tick_does_not_extend_stale_window() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(80),
        &mut out,
    );
    engine.tick(now + Duration::from_millis(200), &mut out);
    assert!(!fired_text(&out));
    assert_eq!(
        raw_keys(&out),
        vec![(KeyCode::KEY_J, true), (KeyCode::KEY_K, true)]
    );
}

#[test]
fn trigger_chord_member_released_early_types_normally() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_J,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert_eq!(
        raw_keys(&out),
        vec![(KeyCode::KEY_J, true), (KeyCode::KEY_J, false)]
    );
    engine.handle_key_event(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_K,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert!(!fired_text(&out));
    assert_eq!(
        raw_keys(&out),
        vec![
            (KeyCode::KEY_J, true),
            (KeyCode::KEY_J, false),
            (KeyCode::KEY_K, true),
            (KeyCode::KEY_K, false),
        ]
    );
}

#[test]
fn trigger_chord_flush_keeps_physical_order() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    // A non-member press rules the chord out and replays J before itself.
    engine.handle_key_event(KeyCode::KEY_J, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_L,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert_eq!(
        raw_keys(&out),
        vec![(KeyCode::KEY_J, true), (KeyCode::KEY_L, true)]
    );
    engine.shutdown(&mut out);
    out.clear();

    // Shift released while J is buffered must stay behind J, or the
    // replayed `J` would lose its Shift.
    let now = now + Duration::from_secs(1);
    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.tick(now + Duration::from_millis(60), &mut out);
    assert_eq!(
        raw_keys(&out),
        vec![
            (KeyCode::KEY_LEFTSHIFT, true),
            (KeyCode::KEY_J, true),
            (KeyCode::KEY_LEFTSHIFT, false),
        ]
    );
}

#[test]
fn chords_are_transparent_through_a_layer_unless_it_maps_a_member() {
    let cfg = |nav: serde_json::Value| -> MapperConfig {
        serde_json::from_value(serde_json::json!({
            "rules": [
                {"key": "Space", "layerId": "nav", "tapAction": null},
                {"key": "KeyJ+KeyK", "tapAction": "text:esc"},
            ],
            "layerKeymaps": {"nav": {"keys": nav}}
        }))
        .unwrap()
    };
    let press = |cfg: &MapperConfig| {
        let mut engine = Engine::new(cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
        assert_eq!(engine.layers.active, vec!["nav"]);
        for (ms, key, down) in [
            (10, KeyCode::KEY_J, true),
            (20, KeyCode::KEY_K, true),
            (30, KeyCode::KEY_K, false),
            (40, KeyCode::KEY_J, false),
        ] {
            engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
        }
        out
    };
    // The layer leaves both members alone: the base chord works in it.
    let out = press(&cfg(serde_json::json!({"KeyL": "ArrowRight"})));
    assert!(fired_text(&out));
    assert!(raw_keys(&out).is_empty());
    // The layer maps J: no combo delay, J is the layer's key.
    let out = press(&cfg(serde_json::json!({"KeyJ": "ArrowLeft"})));
    assert!(!fired_text(&out));
    assert_eq!(transitions(&out)[0], (KeyCode::KEY_LEFT, true));
}

#[test]
fn longer_chord_wins_whatever_the_press_order() {
    let cfg = serde_json::json!({"rules": [
        {"key": "KeyA+KeyS", "tapAction": "KeyX"},
        {"key": "KeyA+KeyS+KeyD", "tapAction": "KeyY"},
    ]});
    for order in [
        [KeyCode::KEY_A, KeyCode::KEY_S, KeyCode::KEY_D],
        [KeyCode::KEY_A, KeyCode::KEY_D, KeyCode::KEY_S],
        [KeyCode::KEY_D, KeyCode::KEY_S, KeyCode::KEY_A],
    ] {
        let trace = timeline(
            cfg.clone(),
            &[
                (order[0], true, 0),
                (order[1], true, 5),
                (order[2], true, 10),
                (order[0], false, 60),
                (order[1], false, 61),
                (order[2], false, 62),
            ],
            &[0, 5, 10, 60, 61, 62],
        );
        assert_eq!(trace, [TraceEvent::Tap(KeyCode::KEY_Y, vec![])], "{order:?}");
    }
}

#[test]
fn shorter_chord_fires_when_the_longer_one_does_not_complete() {
    let cfg = serde_json::json!({"rules": [
        {"key": "KeyA+KeyS", "tapAction": "KeyX"},
        {"key": "KeyA+KeyS+KeyD", "tapAction": "KeyY"},
    ]});
    // Window expires while both are held.
    let held = timeline(
        cfg.clone(),
        &[
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_S, true, 5),
            (KeyCode::KEY_S, false, 100),
            (KeyCode::KEY_A, false, 110),
        ],
        &[0, 5, 30, 60, 100, 110],
    );
    assert_eq!(held, [TraceEvent::Tap(KeyCode::KEY_X, vec![])]);
    // A member released inside the window.
    let released = timeline(
        cfg.clone(),
        &[
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_S, true, 5),
            (KeyCode::KEY_S, false, 20),
            (KeyCode::KEY_A, false, 25),
        ],
        &[0, 5, 20, 25],
    );
    assert_eq!(released, [TraceEvent::Tap(KeyCode::KEY_X, vec![])]);
    // Another key rules the longer chord out; the shorter fires first.
    let other = timeline(
        cfg,
        &[
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_S, true, 5),
            (KeyCode::KEY_L, true, 15),
            (KeyCode::KEY_S, false, 20),
            (KeyCode::KEY_A, false, 21),
            (KeyCode::KEY_L, false, 30),
        ],
        &[0, 5, 15, 20, 21, 30],
    );
    assert_eq!(
        other,
        [
            TraceEvent::Tap(KeyCode::KEY_X, vec![]),
            TraceEvent::Raw(KeyCode::KEY_L, true),
            TraceEvent::Raw(KeyCode::KEY_L, false),
        ]
    );
}

#[test]
fn modifier_held_for_typing_does_not_start_a_modifier_chord() {
    let cfg = serde_json::json!({"rules": [
        {"key": "ShiftLeft+ControlRight", "longHoldAction": "Escape", "longHoldTimeoutMs": 1500}
    ]});
    let trace = timeline(
        cfg,
        &[
            (KeyCode::KEY_LEFTSHIFT, true, 0),
            (KeyCode::KEY_RIGHTCTRL, true, 100),
            (KeyCode::KEY_X, true, 150),
            (KeyCode::KEY_X, false, 180),
            (KeyCode::KEY_RIGHTCTRL, false, 250),
            (KeyCode::KEY_LEFTSHIFT, false, 300),
        ],
        &[0, 100, 150, 180, 250, 300],
    );
    assert_eq!(
        trace,
        [
            TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, true),
            TraceEvent::Raw(KeyCode::KEY_RIGHTCTRL, true),
            TraceEvent::Raw(KeyCode::KEY_X, true),
            TraceEvent::Raw(KeyCode::KEY_X, false),
            TraceEvent::Raw(KeyCode::KEY_RIGHTCTRL, false),
            TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, false),
        ]
    );
}

#[test]
fn modifier_chord_interrupted_by_a_key_becomes_the_shortcut() {
    let cfg = serde_json::json!({"rules": [
        {"key": "ShiftLeft+ControlRight", "longHoldAction": "Escape", "longHoldTimeoutMs": 1500}
    ]});
    // Pressed together, then X: Ctrl+Shift+X, not a chord tap and a bare x.
    let trace = timeline(
        cfg,
        &[
            (KeyCode::KEY_LEFTSHIFT, true, 0),
            (KeyCode::KEY_RIGHTCTRL, true, 10),
            (KeyCode::KEY_X, true, 150),
            (KeyCode::KEY_X, false, 180),
            (KeyCode::KEY_RIGHTCTRL, false, 250),
            (KeyCode::KEY_LEFTSHIFT, false, 300),
        ],
        &[0, 10, 150, 180, 250, 300],
    );
    let keys: Vec<(KeyCode, bool)> = trace
        .iter()
        .filter_map(|event| match event {
            TraceEvent::Raw(key, down) => Some((*key, *down)),
            TraceEvent::Press(key, mods) if mods.is_empty() => Some((*key, true)),
            _ => None,
        })
        .collect();
    let x = keys
        .iter()
        .position(|event| *event == (KeyCode::KEY_X, true))
        .unwrap();
    let held = |key: KeyCode| {
        keys[..x]
            .iter()
            .rfind(|(k, _)| *k == key)
            .is_some_and(|(_, down)| *down)
    };
    assert!(held(KeyCode::KEY_LEFTSHIFT) && held(KeyCode::KEY_RIGHTCTRL), "{trace:?}");
    assert!(!trace.iter().any(|event| matches!(event, TraceEvent::Tap(..))), "{trace:?}");
    // Everything pressed is released.
    for key in [KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTCTRL, KeyCode::KEY_X] {
        assert_eq!(keys.iter().rev().find(|(k, _)| *k == key), Some(&(key, false)));
    }
}

#[test]
fn duplicate_chords_use_first_matching_rule() {
    let mut cfg = empty_cfg();
    let mut first = tap_hold_rule(
        "ControlLeft+KeyQ",
        ActionSpec::Action("KeyA".into()),
        ActionSpec::Native,
    );
    first.condition_game_mode = Some(GameModeCondition::Ignore);
    let second = tap_hold_rule(
        "KeyQ+ControlLeft",
        ActionSpec::Action("KeyB".into()),
        ActionSpec::Native,
    );
    cfg.rules = vec![first, second];
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();
    engine.handle_key_event(KeyCode::KEY_LEFTCTRL, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(out.iter().any(|event| matches!(event,
        EngineOutput::Stroke { ks, .. } if ks.key == KeyCode::KEY_A)));
    assert!(!out.iter().any(|event| matches!(event,
        EngineOutput::Stroke { ks, .. } if ks.key == KeyCode::KEY_B)));
}

#[test]
fn chord_release_keeps_mod_held_as_main_key() {
    // KeyA holds ControlLeft as its *main* key; releasing the Ctrl+KeyC
    // chord on KeyB must not release Ctrl while KeyA is still down.
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
        retro_tap_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut sp = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    sp.keys.insert("KeyA".into(), Some("ControlLeft".into()));
    sp.keys.insert("KeyB".into(), Some("Ctrl+KeyC".into()));
    cfg.layer_keymaps.insert("sp".into(), sp);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(280),
        &mut out,
    );
    out.clear();

    // Release the chord: Ctrl must stay down (KeyA still holds it).
    engine.handle_key_event(
        KeyCode::KEY_B,
        false,
        now + Duration::from_millis(290),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::ChordRelease { key, mods, .. }]
            if *key == KeyCode::KEY_C && mods.is_empty()
    ));

    out.clear();
    // Now releasing KeyA releases Ctrl.
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(300),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: false
        }]
    ));
}

#[test]
fn chord_must_not_fire_single_key_double_tap() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key":"KeyA", "tapAction":"text:single", "doubleTapAction":"text:double"},
        {"key":"KeyA+KeyB", "tapAction":"text:chord"}
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = vec![];
    for (key, down, ms) in [
        (KeyCode::KEY_A, true, 0),
        (KeyCode::KEY_A, false, 10),
        (KeyCode::KEY_A, true, 20),
        (KeyCode::KEY_B, true, 25),
        (KeyCode::KEY_B, false, 30),
        (KeyCode::KEY_A, false, 35),
    ] {
        engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
        engine.tick(now + Duration::from_millis(ms), &mut out);
    }
    engine.tick(now + Duration::from_secs(1), &mut out);
    let texts: Vec<_> = out
        .iter()
        .filter_map(|o| match o {
            EngineOutput::Literal(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["single", "chord"]);
}

#[test]
fn chord_long_hold_fires_on_late_member_release_without_tick() {
    for released in [KeyCode::KEY_A, KeyCode::KEY_B] {
        let cfg = config_with_rule(serde_json::json!({
            "key":"KeyA+KeyB", "tapAction":"text:tap", "longHoldAction":"text:long", "longHoldTimeoutMs":100
        }));
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
        engine.handle_key_event(
            KeyCode::KEY_B,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );
        engine.handle_key_event(released, false, now + Duration::from_millis(110), &mut out);
        assert!(matches!(out.as_slice(), [EngineOutput::Literal(s)] if s == "long"));
        out.clear();
        engine.tick(now + Duration::from_secs(1), &mut out);
        assert!(out.is_empty());
    }
}

#[test]
fn space_audit_plain_and_layer_roll_preserve_order() {
    for rule in [
        None,
        Some(serde_json::json!({"key": "Space", "layerId": "nav"})),
        Some(serde_json::json!({"key": "Space", "layerId": "nav", "doubleTapAction": "Escape"})),
    ] {
        for gap in [1, 5, 20, 50] {
            assert_eq!(
                space_audit_presses(
                    rule.clone(),
                    &[
                        (KeyCode::KEY_A, true, 0),
                        (KeyCode::KEY_SPACE, true, gap),
                        (KeyCode::KEY_B, true, gap * 2),
                        (KeyCode::KEY_A, false, gap * 3),
                        (KeyCode::KEY_SPACE, false, gap * 3 + 1),
                        (KeyCode::KEY_B, false, gap * 3 + 2),
                    ]
                ),
                vec![KeyCode::KEY_A, KeyCode::KEY_SPACE, KeyCode::KEY_B]
            );
        }
    }
}

#[test]
fn space_audit_nested_letter_the_layer_ignores_keeps_the_space() {
    assert_eq!(
        space_audit_presses(
            Some(serde_json::json!({"key": "Space", "layerId": "nav"})),
            &[
                (KeyCode::KEY_SPACE, true, 0),
                (KeyCode::KEY_B, true, 10),
                (KeyCode::KEY_B, false, 20),
                (KeyCode::KEY_SPACE, false, 30),
            ]
        ),
        vec![KeyCode::KEY_SPACE, KeyCode::KEY_B]
    );
    // Typing that crosses the hold timeout, delivered live and as a batch.
    assert_eq!(
        space_audit_presses(
            Some(serde_json::json!({"key": "Space", "layerId": "nav"})),
            &[
                (KeyCode::KEY_SPACE, true, 0),
                (KeyCode::KEY_B, true, 180),
                (KeyCode::KEY_SPACE, false, 220),
                (KeyCode::KEY_B, false, 240),
            ]
        ),
        vec![KeyCode::KEY_SPACE, KeyCode::KEY_B]
    );
}

#[test]
fn space_audit_long_hold_action_taps_before_next_letter() {
    // Long-action rules do not buffer other keys; the next press resolves
    // the rule as a tap so the space stays ahead of the letter.
    assert_eq!(
        space_audit_presses(
            Some(serde_json::json!({"key": "Space", "longHoldAction": "Escape"})),
            &[
                (KeyCode::KEY_SPACE, true, 0),
                (KeyCode::KEY_B, true, 10),
                (KeyCode::KEY_SPACE, false, 20),
                (KeyCode::KEY_B, false, 30),
            ]
        ),
        vec![KeyCode::KEY_SPACE, KeyCode::KEY_B]
    );
    // A letter typed entirely while the space is still held stays behind it too.
    assert_eq!(
        space_audit_presses(
            Some(serde_json::json!({"key": "Space", "longHoldAction": "Escape"})),
            &[
                (KeyCode::KEY_SPACE, true, 0),
                (KeyCode::KEY_B, true, 10),
                (KeyCode::KEY_B, false, 20),
                (KeyCode::KEY_SPACE, false, 30),
            ]
        ),
        vec![KeyCode::KEY_SPACE, KeyCode::KEY_B]
    );
}

#[test]
fn long_hold_already_due_at_next_press_fires_before_letter() {
    // The press arrives after the long-hold timeout but before the tick:
    // the long hold wins and still comes before the letter.
    assert_eq!(
        space_audit_presses(
            Some(serde_json::json!({"key": "Space", "longHoldAction": "Escape", "longHoldTimeoutMs": 100})),
            &[
                (KeyCode::KEY_SPACE, true, 0),
                (KeyCode::KEY_B, true, 150),
                (KeyCode::KEY_B, false, 160),
                (KeyCode::KEY_SPACE, false, 170),
            ]
        ),
        vec![KeyCode::KEY_ESC, KeyCode::KEY_B]
    );
}

#[test]
fn unassigned_space_keeps_order_behind_another_tap_hold_key() {
    for double in ["", "Escape"] {
        for gap in [1, 10, 40] {
            assert_eq!(
                space_audit_presses(
                    Some(
                        serde_json::json!({"key":"KeyA", "holdAction":"ControlLeft", "doubleTapAction":double})
                    ),
                    &[
                        (KeyCode::KEY_A, true, 0),
                        (KeyCode::KEY_SPACE, true, gap),
                        (KeyCode::KEY_B, true, gap * 2),
                        (KeyCode::KEY_A, false, gap * 3),
                        (KeyCode::KEY_SPACE, false, gap * 3 + 1),
                        (KeyCode::KEY_B, false, gap * 3 + 2),
                    ],
                ),
                vec![KeyCode::KEY_A, KeyCode::KEY_SPACE, KeyCode::KEY_B],
            );
        }
    }
}

#[test]
fn unassigned_space_prevents_a_later_chord_from_overtaking_typing() {
    for tick_each in [false, true] {
        let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
            {"key":"KeyA", "holdAction":"ControlLeft"},
            {"key":"KeyB+KeyC", "tapAction":null, "holdAction":"KeyX"}
        ]}))
        .unwrap();
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        for (key, down, ms) in [
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_SPACE, true, 10),
            (KeyCode::KEY_B, true, 20),
            (KeyCode::KEY_C, true, 30),
            (KeyCode::KEY_A, false, 40),
            (KeyCode::KEY_SPACE, false, 50),
            (KeyCode::KEY_B, false, 60),
            (KeyCode::KEY_C, false, 70),
        ] {
            engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
            if tick_each {
                engine.tick(now + Duration::from_millis(ms), &mut out);
            }
        }
        let presses: Vec<_> = out
            .iter()
            .filter_map(|event| match event {
                EngineOutput::KeyRaw { key, down: true } => Some(*key),
                EngineOutput::Stroke { ks, .. } | EngineOutput::ChordPress { ks, .. } => {
                    Some(ks.key)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            presses,
            vec![
                KeyCode::KEY_A,
                KeyCode::KEY_SPACE,
                KeyCode::KEY_B,
                KeyCode::KEY_C
            ]
        );
        assert!(engine.virt.emitted.is_empty());
        assert!(engine.pending.is_empty());
    }
}

#[test]
fn unassigned_space_prevents_a_later_chord_from_changing_its_layer() {
    for tick_each in [false, true] {
        let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
            {"key":"KeyA", "holdAction":"ControlLeft"},
            {"key":"KeyB+KeyC", "tapAction":null, "layerId":"nav"}
        ], "layerKeymaps":{"nav":{"keys":{"Space":"Backspace"}}}}))
        .unwrap();
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        for (key, down, ms) in [
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_SPACE, true, 10),
            (KeyCode::KEY_B, true, 20),
            (KeyCode::KEY_C, true, 30),
            (KeyCode::KEY_A, false, 40),
            (KeyCode::KEY_SPACE, false, 50),
            (KeyCode::KEY_B, false, 60),
            (KeyCode::KEY_C, false, 70),
        ] {
            engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
            if tick_each {
                engine.tick(now + Duration::from_millis(ms), &mut out);
            }
        }
        let presses: Vec<_> = out
            .iter()
            .filter_map(|event| match event {
                EngineOutput::KeyRaw { key, down: true } => Some(*key),
                EngineOutput::Stroke { ks, .. } | EngineOutput::ChordPress { ks, .. } => {
                    Some(ks.key)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            presses,
            vec![
                KeyCode::KEY_A,
                KeyCode::KEY_SPACE,
                KeyCode::KEY_B,
                KeyCode::KEY_C
            ]
        );
        assert!(engine.virt.emitted.is_empty());
        assert!(engine.pending.is_empty());
    }
}

#[test]
fn chord_still_activates_after_earlier_deferred_typing_finishes() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key":"KeyA", "holdAction":"ControlLeft"},
        {"key":"KeyB+KeyC", "tapAction":null, "layerId":"nav"}
    ], "layerKeymaps":{"nav":{"keys":{"Space":"Backspace"}}}}))
    .unwrap();
    for second in [KeyCode::KEY_C, KeyCode::KEY_B] {
        let first = if second == KeyCode::KEY_C {
            KeyCode::KEY_B
        } else {
            KeyCode::KEY_C
        };
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        for (key, down, ms) in [
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_SPACE, true, 10),
            (KeyCode::KEY_A, false, 20),
            (KeyCode::KEY_SPACE, false, 30),
            (first, true, 40),
            (second, true, 50),
        ] {
            engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
        }
        assert_eq!(engine.layers.active, vec!["nav"]);
        assert!(out.iter().any(|event| matches!(
            event,
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_SPACE,
                down: true
            }
        )));
        engine.handle_key_event(first, false, now + Duration::from_millis(60), &mut out);
        engine.handle_key_event(second, false, now + Duration::from_millis(70), &mut out);
        assert!(engine.layers.active.is_empty());
        assert!(engine.pending.is_empty());
        assert!(engine.virt.emitted.is_empty());
        assert!(engine.chords.consumed.is_empty());
    }
}

#[test]
fn release_waiting_in_combo_buffer_decides_permissive_tap() {
    // D is released before its hold timeout, but the release sits behind a
    // buffered chord member when the timeout passes.
    let out = timeline(
        serde_json::json!({
            "rules": [
                {"key":"KeyD", "layerId":"nav", "holdTimeoutMs":200, "tapDecision":"permissiveHold"},
                {"key":"KeyX+KeyY", "tapAction":"text:chord"}
            ],
            "layerKeymaps": {"nav": {"keys": {"KeyX": "KeyQ"}}},
            "settings": {"comboTimeoutMs": 50}
        }),
        &[
            (KeyCode::KEY_D, true, 0),
            (KeyCode::KEY_X, true, 180),
            (KeyCode::KEY_D, false, 190),
            (KeyCode::KEY_X, false, 400),
        ],
        &[180, 190, 200, 210, 230, 240, 400],
    );
    assert_eq!(
        out,
        [
            TraceEvent::Tap(KeyCode::KEY_D, vec![]),
            TraceEvent::Raw(KeyCode::KEY_X, true),
            TraceEvent::Raw(KeyCode::KEY_X, false)
        ]
    );
}

#[test]
fn second_press_waiting_in_combo_buffer_is_a_double_tap() {
    let out = timeline(
        serde_json::json!({
            "rules": [
                {"key":"KeyA", "tapAction":"text:single", "doubleTapAction":"text:double", "doubleTapTimeoutMs":250},
                {"key":"KeyA+KeyB", "tapAction":"text:chord"}
            ],
            "settings": {"comboTimeoutMs": 50}
        }),
        &[
            (KeyCode::KEY_A, true, 0),
            (KeyCode::KEY_A, false, 10),
            (KeyCode::KEY_A, true, 240),
            (KeyCode::KEY_A, false, 300),
        ],
        &[10, 240, 260, 265, 290, 300],
    );
    assert_eq!(out, [TraceEvent::Text("double".into())]);
}

#[test]
fn chord_with_an_undecided_member_needs_the_combo_window() {
    let cfg = serde_json::json!({
        "rules": [
            {"key":"Space", "layerId":"nav", "holdTimeoutMs":200},
            {"key":"Space+KeyJ", "tapAction":"text:chord"}
        ],
        "layerKeymaps": {"nav": {"keys": {"KeyJ": "ArrowLeft"}}},
        "settings": {"comboTimeoutMs": 50}
    });
    let late = timeline(
        cfg.clone(),
        &[
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_J, true, 120),
            (KeyCode::KEY_J, false, 150),
            (KeyCode::KEY_SPACE, false, 400),
        ],
        &[0, 50, 60, 120, 150, 200, 400],
    );
    assert_eq!(
        late,
        [
            TraceEvent::Press(KeyCode::KEY_LEFT, vec![]),
            TraceEvent::Raw(KeyCode::KEY_LEFT, false)
        ]
    );
    let together = timeline(
        cfg,
        &[
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_J, true, 30),
            (KeyCode::KEY_J, false, 80),
            (KeyCode::KEY_SPACE, false, 90),
        ],
        &[0, 30, 80, 90],
    );
    assert_eq!(together, [TraceEvent::Text("chord".into())]);
}

#[test]
fn chord_nested_in_undecided_tap_hold_key_does_not_overtake_it() {
    let out = timeline(
        serde_json::json!({
            "rules": [
                {"key":"Space", "layerId":"nav"},
                {"key":"KeyJ+KeyK", "tapAction":"text:chord"}
            ],
            "layerKeymaps": {"nav": {"keys": {"KeyJ": "ArrowLeft", "KeyK": "ArrowRight"}}},
            "settings": {"comboTimeoutMs": 50}
        }),
        &[
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_J, true, 20),
            (KeyCode::KEY_K, true, 30),
            (KeyCode::KEY_J, false, 80),
            (KeyCode::KEY_K, false, 90),
            (KeyCode::KEY_SPACE, false, 120),
        ],
        &[0, 20, 30, 80, 90, 120],
    );
    assert_eq!(
        out,
        [
            TraceEvent::Press(KeyCode::KEY_LEFT, vec![]),
            TraceEvent::Press(KeyCode::KEY_RIGHT, vec![]),
            TraceEvent::Raw(KeyCode::KEY_LEFT, false),
            TraceEvent::Raw(KeyCode::KEY_RIGHT, false)
        ]
    );
}

#[test]
fn chord_rolled_under_undecided_tap_hold_key_types_its_keys_in_order() {
    // Members deferred behind the decision replay as typing, like other
    // keys buffered behind a tap-hold key: the chord never overtakes it.
    let out = timeline(
        serde_json::json!({
            "rules": [
                {"key":"Space", "layerId":"nav"},
                {"key":"KeyJ+KeyK", "tapAction":"text:chord"}
            ],
            "layerKeymaps": {"nav": {"keys": {"KeyJ": "ArrowLeft"}}},
            "settings": {"comboTimeoutMs": 50}
        }),
        &[
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_J, true, 20),
            (KeyCode::KEY_K, true, 30),
            (KeyCode::KEY_SPACE, false, 60),
            (KeyCode::KEY_J, false, 80),
            (KeyCode::KEY_K, false, 90),
        ],
        &[0, 20, 30, 60, 80, 90],
    );
    assert_eq!(
        out,
        [
            TraceEvent::Tap(KeyCode::KEY_SPACE, vec![]),
            TraceEvent::Raw(KeyCode::KEY_J, true),
            TraceEvent::Raw(KeyCode::KEY_K, true),
            TraceEvent::Raw(KeyCode::KEY_J, false),
            TraceEvent::Raw(KeyCode::KEY_K, false)
        ]
    );
}

#[test]
fn shortcut_chord_fires_with_a_modifier_held_beforehand() {
    let cfg = serde_json::json!({"rules": [{"key": "ControlLeft+KeyC", "tapAction": "KeyX"}]});
    let trace = timeline(
        cfg,
        &[
            (KeyCode::KEY_LEFTCTRL, true, 0),
            (KeyCode::KEY_C, true, 200),
            (KeyCode::KEY_C, false, 230),
            (KeyCode::KEY_LEFTCTRL, false, 260),
        ],
        &[0, 200, 230, 260, 400],
    );
    assert!(trace.contains(&TraceEvent::Tap(KeyCode::KEY_X, vec![])), "{trace:?}");
    assert!(!trace.contains(&TraceEvent::Raw(KeyCode::KEY_C, true)), "{trace:?}");
}

#[test]
fn shortcut_chord_main_key_types_without_delay() {
    let cfg = serde_json::json!({"rules": [{"key": "ControlLeft+KeyC", "tapAction": "KeyX"}]});
    let mut engine = Engine::new(&serde_json::from_value::<MapperConfig>(cfg).unwrap());
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_C, true, Instant::now(), &mut out);
    assert!(
        out.iter().any(|event| matches!(event, EngineOutput::KeyRaw { key: KeyCode::KEY_C, down: true }))
    );
}

#[test]
fn shortcut_chord_needs_exactly_its_modifiers() {
    let cfg = serde_json::json!({"rules": [{"key": "ControlLeft+KeyC", "tapAction": "KeyX"}]});
    let trace = timeline(
        cfg,
        &[
            (KeyCode::KEY_LEFTCTRL, true, 0),
            (KeyCode::KEY_LEFTSHIFT, true, 100),
            (KeyCode::KEY_C, true, 200),
            (KeyCode::KEY_C, false, 230),
            (KeyCode::KEY_LEFTSHIFT, false, 250),
            (KeyCode::KEY_LEFTCTRL, false, 260),
        ],
        &[0, 100, 200, 230, 250, 260, 400],
    );
    assert!(trace.contains(&TraceEvent::Raw(KeyCode::KEY_C, true)), "{trace:?}");
    assert!(!trace.iter().any(|event| matches!(event, TraceEvent::Tap(..))), "{trace:?}");
}

#[test]
fn shorter_shortcut_chord_does_not_wait_for_a_longer_one() {
    let cfg = serde_json::json!({"rules": [
        {"key": "ControlLeft+KeyC", "tapAction": "KeyX"},
        {"key": "ControlLeft+ShiftLeft+KeyC", "tapAction": "KeyZ"}
    ]});
    // Shift after the main key does not turn Ctrl+C into Ctrl+Shift+C.
    let trace = timeline(
        cfg,
        &[
            (KeyCode::KEY_LEFTCTRL, true, 0),
            (KeyCode::KEY_C, true, 200),
            (KeyCode::KEY_LEFTSHIFT, true, 210),
            (KeyCode::KEY_C, false, 230),
            (KeyCode::KEY_LEFTSHIFT, false, 250),
            (KeyCode::KEY_LEFTCTRL, false, 260),
        ],
        &[0, 200, 210, 230, 250, 260, 400],
    );
    assert!(trace.contains(&TraceEvent::Tap(KeyCode::KEY_X, vec![])), "{trace:?}");
    assert!(!trace.contains(&TraceEvent::Tap(KeyCode::KEY_Z, vec![])), "{trace:?}");
}

#[test]
fn generic_modifiers_expand_to_both_sides_and_preserve_releases() {
    for shift in [KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT] {
        for ctrl in [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL] {
            let cfg = config_with_rule(serde_json::json!({"key": "Shift+Ctrl", "layerId": "nav", "tapAction": null}));
            let mut engine = Engine::new(&cfg);
            let now = Instant::now();
            let mut out = Vec::new();
            engine.handle_key_event(shift, true, now, &mut out);
            engine.handle_key_event(ctrl, true, now + Duration::from_millis(10), &mut out);
            assert_eq!(engine.layers.active, vec!["nav"]);
            engine.handle_key_event(ctrl, false, now + Duration::from_millis(20), &mut out);
            engine.handle_key_event(shift, false, now + Duration::from_millis(30), &mut out);
            assert!(engine.layers.active.is_empty());
            assert!(engine.virt.emitted.is_empty());
        }
    }
}

#[test]
fn physical_trigger_takes_priority_over_generic_in_either_rule_order() {
    for reverse in [false, true] {
        let mut cfg = config_with_rule(serde_json::json!({"key": "Ctrl+KeyC", "layerId": "nav", "tapAction": null}));
        let mut specific = cfg.rules[0].clone();
        specific.key = "ControlRight+KeyC".into();
        specific.layer_id = "specific".into();
        cfg.rules.push(specific);
        if reverse { cfg.rules.reverse(); }
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_RIGHTCTRL, true, now, &mut out);
        engine.handle_key_event(KeyCode::KEY_C, true, now + Duration::from_millis(10), &mut out);
        assert_eq!(engine.layers.active, vec!["specific"]);
    }
}
