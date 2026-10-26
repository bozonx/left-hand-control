use super::*;


#[test]
fn unassigned_chord_ignores_double_tap_and_preserves_normal_keys() {
    for extra in ["", "text:double"] {
        let cfg = config_with_rule(
            serde_json::json!({"key": "ShiftLeft+ControlLeft", "doubleTapAction": extra}),
        );
        let engine = Engine::new(&cfg);
        assert!(engine.layout.chord_rules.is_empty());
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
            engine.handle(first, true, now, &mut out);
            engine.handle(second, true, now + Duration::from_millis(10), &mut out);
            assert_eq!(engine.layers.active, vec!["nav"]);
            engine.handle(release, false, now + Duration::from_millis(20), &mut out);
            assert!(engine.layers.active.is_empty());
            let other = if release == first { second } else { first };
            engine.handle(other, false, now + Duration::from_millis(30), &mut out);
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
    engine.handle(KeyCode::KEY_B, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_B,
        false,
        now + Duration::from_millis(400),
        &mut out,
    );
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(!out.iter().any(|event| matches!(event, Out::Literal(_))));
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(1100),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(1200),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(1210),
        &mut out,
    );
    out.clear();
    engine.tick(now + Duration::from_millis(1800), &mut out);
    engine.tick(now + Duration::from_secs(3), &mut out);
    assert!(matches!(out.as_slice(), [Out::Literal(text)] if text == "done"));
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
        engine.handle(first, true, now + Duration::from_millis(offset), &mut out);
        engine.handle(
            second,
            true,
            now + Duration::from_millis(offset + 10),
            &mut out,
        );
        engine.handle(
            first,
            false,
            now + Duration::from_millis(offset + 20),
            &mut out,
        );
        assert!(
            out.iter()
                .any(|event| matches!(event, Out::Literal(text) if text == "tap"))
        );
        assert!(
            !out.iter()
                .any(|event| matches!(event, Out::Literal(text) if text == "double"))
        );
        engine.handle(
            second,
            false,
            now + Duration::from_millis(offset + 30),
            &mut out,
        );
        out.clear();
    }
}

#[test]
fn combo_members_never_reach_the_app() {
    for (first, second) in [
        (KeyCode::KEY_J, KeyCode::KEY_K),
        (KeyCode::KEY_K, KeyCode::KEY_J),
    ] {
        let cfg =
            config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle(first, true, now, &mut out);
        assert!(out.is_empty());
        engine.handle(second, true, now + Duration::from_millis(20), &mut out);
        engine.handle(first, false, now + Duration::from_millis(60), &mut out);
        engine.handle(second, false, now + Duration::from_millis(70), &mut out);
        assert!(fired_text(&out));
        assert!(raw_keys(&out).is_empty());
        assert!(engine.chords.buffer.is_empty());
        assert!(engine.chords.consumed.is_empty());
    }
}

#[test]
fn combo_window_expiry_types_the_member_and_blocks_the_chord() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    assert_eq!(engine.next_deadline(now), Some(engine.layout.combo_term));
    engine.tick(now + engine.layout.combo_term, &mut out);
    assert_eq!(raw_keys(&out), vec![(KeyCode::KEY_J, true)]);
    // `j` is already typed — a late K must not turn it into the chord.
    engine.handle(
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
fn combo_window_comes_from_settings() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({
        "rules": [{"key": "KeyJ+KeyK", "tapAction": "text:esc"}],
        "settings": {"comboTimeoutMs": 120},
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    assert_eq!(engine.next_deadline(now), Some(Duration::from_millis(120)));
    engine.handle(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(100),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_K,
        false,
        now + Duration::from_millis(130),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_J,
        false,
        now + Duration::from_millis(140),
        &mut out,
    );
    assert!(fired_text(&out));
    assert!(raw_keys(&out).is_empty());
}

#[test]
fn combo_late_press_before_tick_does_not_extend_stale_window() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    engine.handle(
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
fn combo_member_released_early_types_normally() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_J,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert_eq!(
        raw_keys(&out),
        vec![(KeyCode::KEY_J, true), (KeyCode::KEY_J, false)]
    );
    engine.handle(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.handle(
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
fn combo_flush_keeps_physical_order() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    // A non-member press rules the chord out and replays J before itself.
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    engine.handle(
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
    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
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
fn chords_are_inactive_inside_a_layer() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key": "Space", "layerId": "nav", "tapAction": null},
        {"key": "KeyJ+KeyK", "tapAction": "text:esc"},
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    assert_eq!(engine.layers.active, vec!["nav"]);
    engine.handle(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    // No combo delay inside the layer: J flows through at once.
    assert_eq!(raw_keys(&out), vec![(KeyCode::KEY_J, true)]);
    engine.handle(
        KeyCode::KEY_K,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(!fired_text(&out));
    assert!(engine.chords.buffer.is_empty());
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
    engine.handle(KeyCode::KEY_LEFTCTRL, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(out.iter().any(|event| matches!(event,
        Out::Stroke { ks, .. } if ks.key == KeyCode::KEY_A)));
    assert!(!out.iter().any(|event| matches!(event,
        Out::Stroke { ks, .. } if ks.key == KeyCode::KEY_B)));
}

#[test]
fn chord_release_keeps_mod_held_as_main_key() {
    // KeyA holds ControlLeft as its *main* key; releasing the Ctrl+KeyC
    // chord on KeyB must not release Ctrl while KeyA is still down.
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
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
    sp.keys.insert("KeyA".into(), Some("ControlLeft".into()));
    sp.keys.insert("KeyB".into(), Some("Ctrl+KeyC".into()));
    cfg.layer_keymaps.insert("sp".into(), sp);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(280),
        &mut out,
    );
    out.clear();

    // Release the chord: Ctrl must stay down (KeyA still holds it).
    engine.handle(
        KeyCode::KEY_B,
        false,
        now + Duration::from_millis(290),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::ChordRelease { key, mods, .. }]
            if *key == KeyCode::KEY_C && mods.is_empty()
    ));

    out.clear();
    // Now releasing KeyA releases Ctrl.
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(300),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: false
        }]
    ));
}

#[test]
fn chord_must_not_fire_single_key_double_tap() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({"rules": [
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
        engine.handle(key, down, now + Duration::from_millis(ms), &mut out);
        engine.tick(now + Duration::from_millis(ms), &mut out);
    }
    engine.tick(now + Duration::from_secs(1), &mut out);
    let texts: Vec<_> = out
        .iter()
        .filter_map(|o| match o {
            Out::Literal(s) => Some(s.as_str()),
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
        engine.handle(KeyCode::KEY_A, true, now, &mut out);
        engine.handle(
            KeyCode::KEY_B,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );
        engine.handle(released, false, now + Duration::from_millis(110), &mut out);
        assert!(matches!(out.as_slice(), [Out::Literal(s)] if s == "long"));
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
fn space_audit_nested_letter_selects_layer_without_space() {
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
        vec![KeyCode::KEY_B]
    );
}

#[test]
fn space_audit_long_hold_action_allows_letter_to_overtake_space() {
    // Long-action rules deliberately do not buffer other keys. A native
    // tap therefore appears at release, after a later physical key press.
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
        vec![KeyCode::KEY_B, KeyCode::KEY_SPACE]
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
        let cfg: AppConfig = serde_json::from_value(serde_json::json!({"rules": [
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
            engine.handle(key, down, now + Duration::from_millis(ms), &mut out);
            if tick_each {
                engine.tick(now + Duration::from_millis(ms), &mut out);
            }
        }
        let presses: Vec<_> = out
            .iter()
            .filter_map(|event| match event {
                Out::KeyRaw { key, down: true } => Some(*key),
                Out::Stroke { ks, .. } | Out::ChordPress { ks, .. } => Some(ks.key),
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
        let cfg: AppConfig = serde_json::from_value(serde_json::json!({"rules": [
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
            engine.handle(key, down, now + Duration::from_millis(ms), &mut out);
            if tick_each {
                engine.tick(now + Duration::from_millis(ms), &mut out);
            }
        }
        let presses: Vec<_> = out
            .iter()
            .filter_map(|event| match event {
                Out::KeyRaw { key, down: true } => Some(*key),
                Out::Stroke { ks, .. } | Out::ChordPress { ks, .. } => Some(ks.key),
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
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({"rules": [
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
            engine.handle(key, down, now + Duration::from_millis(ms), &mut out);
        }
        assert_eq!(engine.layers.active, vec!["nav"]);
        assert!(out.iter().any(|event| matches!(
            event,
            Out::KeyRaw {
                key: KeyCode::KEY_SPACE,
                down: true
            }
        )));
        engine.handle(first, false, now + Duration::from_millis(60), &mut out);
        engine.handle(second, false, now + Duration::from_millis(70), &mut out);
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
    assert_eq!(out, ["tap KEY_D", "KEY_X+", "KEY_X-"]);
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
    assert_eq!(out, ["text double"]);
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
    assert_eq!(late, ["press KEY_LEFT", "KEY_LEFT-"]);
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
    assert_eq!(together, ["text chord"]);
}
