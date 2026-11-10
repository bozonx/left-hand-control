//! Regressions for delayed input batches and typing across layer deadlines.

use super::*;

fn nav_config() -> MapperConfig {
    serde_json::from_value(serde_json::json!({
        "rules": [{"key":"Space", "layerId":"nav"}],
        "layerKeymaps": {"nav":{"keys":{"KeyJ":"ArrowDown"}}}
    }))
    .unwrap()
}

#[test]
fn replayed_release_precedes_the_next_press_of_the_same_key() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({
        "rules": [
            {"key":"Space", "layerId":"nav"},
            {"key":"AltLeft", "layerId":"other", "tapDecision":"permissiveHold"}
        ],
        "settings":{"tapDecision":"holdOnOtherKeyPress"}
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let out = run(
        &mut engine,
        &[
            (KeyCode::KEY_B, true),
            (KeyCode::KEY_SPACE, true),
            (KeyCode::KEY_LEFTALT, true),
            (KeyCode::KEY_B, false),
            (KeyCode::KEY_B, true),
            (KeyCode::KEY_B, false),
            (KeyCode::KEY_LEFTALT, false),
            (KeyCode::KEY_SPACE, false),
        ],
    );
    assert_eq!(
        raw_keys(&out),
        vec![
            (KeyCode::KEY_B, true),
            (KeyCode::KEY_B, false),
            (KeyCode::KEY_B, true),
            (KeyCode::KEY_B, false),
        ]
    );
    assert!(engine.pending.is_empty());
    assert!(engine.decision_buffer.is_empty());
    assert!(engine.virt.emitted.is_empty());
}

#[test]
fn first_ordinary_key_decides_both_nested_release_and_timeout() {
    for release_j in [90, 230] {
        let cfg = nav_config();
        let value = serde_json::to_value(cfg).unwrap();
        let events = [
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_A, true, 30),
            (KeyCode::KEY_J, true, 60),
            (KeyCode::KEY_J, false, release_j),
            (KeyCode::KEY_A, false, 250),
            (KeyCode::KEY_SPACE, false, 270),
        ];
        for ticks in [vec![], vec![200]] {
            let trace = timeline(value.clone(), &events, &ticks);
            assert_eq!(
                trace,
                vec![
                    TraceEvent::Tap(KeyCode::KEY_SPACE, vec![]),
                    TraceEvent::Raw(KeyCode::KEY_A, true),
                    TraceEvent::Raw(KeyCode::KEY_J, true),
                    TraceEvent::Raw(KeyCode::KEY_J, false),
                    TraceEvent::Raw(KeyCode::KEY_A, false),
                ]
            );
        }
    }
}

#[test]
fn mapped_first_key_keeps_the_hold_when_a_later_unmapped_key_is_released() {
    let mut engine = Engine::new(&nav_config());
    run(
        &mut engine,
        &[
            (KeyCode::KEY_SPACE, true),
            (KeyCode::KEY_J, true),
            (KeyCode::KEY_A, true),
            (KeyCode::KEY_A, false),
        ],
    );
    assert_eq!(engine.top_layer(), Some("nav"));
}

#[test]
fn shortcuts_do_not_start_flow_tap_after_the_modifier_is_released() {
    let mut engine = Engine::new(&nav_config());
    run(
        &mut engine,
        &[
            (KeyCode::KEY_LEFTCTRL, true),
            (KeyCode::KEY_A, true),
            (KeyCode::KEY_A, false),
            (KeyCode::KEY_LEFTCTRL, false),
            (KeyCode::KEY_SPACE, true),
            (KeyCode::KEY_J, true),
            (KeyCode::KEY_J, false),
        ],
    );
    assert_eq!(engine.top_layer(), Some("nav"));
}

#[test]
fn flow_tap_only_applies_to_recent_typing_in_permissive_mode() {
    for mode in [
        TapDecision::PermissiveHold,
        TapDecision::HoldOnOtherKeyPress,
    ] {
        for gap in [134, 135, 200] {
            let mut cfg = nav_config();
            cfg.settings.tap_decision = mode;
            let mut engine = Engine::new(&cfg);
            let start = Instant::now();
            let mut out = vec![];
            engine.handle_key_event(KeyCode::KEY_A, true, start, &mut out);
            engine.handle_key_event(KeyCode::KEY_A, false, start, &mut out);
            out.clear();
            let at = start + Duration::from_millis(gap);
            engine.handle_key_event(KeyCode::KEY_SPACE, true, at, &mut out);
            engine.handle_key_event(KeyCode::KEY_J, true, at, &mut out);
            engine.handle_key_event(KeyCode::KEY_J, false, at, &mut out);
            let flow = gap < 135 && mode == TapDecision::PermissiveHold;
            assert_eq!(engine.top_layer().is_none(), flow);
            engine.handle_key_event(KeyCode::KEY_SPACE, false, at, &mut out);
            assert!(engine.pending.is_empty());
        }
    }
}

#[test]
fn inactive_rules_and_chords_do_not_make_a_layer_significant() {
    for trigger in [serde_json::json!("KeyA"), serde_json::json!("KeyA+KeyB")] {
        let mut cfg = nav_config();
        let mut rule = tap_hold_rule(
            "KeyA",
            ActionSpec::Native,
            ActionSpec::Action("ShiftLeft".into()),
        );
        rule.key = serde_json::from_value(trigger).unwrap();
        rule.condition_game_mode = Some(GameModeCondition::On);
        cfg.rules.push(rule);
        let mut engine = Engine::new(&cfg);
        engine.set_context_source(Box::new(AutoSwitchContext::default));
        let out = run(
            &mut engine,
            &[
                (KeyCode::KEY_SPACE, true),
                (KeyCode::KEY_A, true),
                (KeyCode::KEY_A, false),
                (KeyCode::KEY_SPACE, false),
            ],
        );
        assert!(
            out.iter().any(
                |o| matches!(o, EngineOutput::Stroke {ks, ..} if ks.key == KeyCode::KEY_SPACE)
            )
        );
    }
}

#[test]
fn double_tap_second_press_counts_as_layer_use() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({
        "rules":[{"key":"Space","layerId":"nav","tapAction":null},
                 {"key":"CapsLock","tapAction":"KeyA","doubleTapAction":"KeyB"}]
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    run(
        &mut engine,
        &[
            (KeyCode::KEY_SPACE, true),
            (KeyCode::KEY_CAPSLOCK, true),
            (KeyCode::KEY_CAPSLOCK, false),
            (KeyCode::KEY_CAPSLOCK, true),
        ],
    );
    assert_eq!(engine.layer_presses(), 2);
}

#[test]
fn seeded_batches_leave_no_pending_keys_modifiers_or_layers() {
    let mut cfg = switcher_cfg();
    cfg.rules.push(tap_hold_rule(
        "Space",
        ActionSpec::Native,
        ActionSpec::Action("ShiftLeft".into()),
    ));
    cfg.rules.push(serde_json::from_value(serde_json::json!({
        "key":"CapsLock", "tapAction":"KeyA", "doubleTapAction":"KeyB", "holdAction":"ControlLeft"
    })).unwrap());
    cfg.rules.push(
        serde_json::from_value(serde_json::json!({
            "key":"KeyD", "tapAction":"KeyD", "longHoldAction":"KeyE"
        }))
        .unwrap(),
    );
    let keys = [
        KeyCode::KEY_SPACE,
        KeyCode::KEY_LEFTALT,
        KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_TAB,
        KeyCode::KEY_CAPSLOCK,
        KeyCode::KEY_B,
        KeyCode::KEY_D,
        KeyCode::BTN_LEFT,
    ];
    let mut seed = 0x127a_35bc_9876_4321u64;
    let mut random = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for mode in [
        TapDecision::PermissiveHold,
        TapDecision::HoldOnOtherKeyPress,
    ] {
        cfg.settings.tap_decision = mode;
        for case in 0..20_000 {
            let mut engine = Engine::new(&cfg);
            let mut out = vec![];
            let mut at = Instant::now();
            let mut down = HashSet::new();
            for _ in 0..32 {
                at += Duration::from_millis(random() % 80);
                let key = keys[random() as usize % keys.len()];
                let press = if down.remove(&key) {
                    false
                } else {
                    down.insert(key);
                    true
                };
                engine.handle_key_event(key, press, at, &mut out);
                if random().is_multiple_of(7) {
                    engine.tick(at, &mut out);
                }
            }
            // Release in a fixed order, independently of HashSet iteration.
            for key in keys {
                if down.contains(&key) {
                    at += Duration::from_millis(1);
                    engine.handle_key_event(key, false, at, &mut out);
                }
            }
            engine.tick(at + Duration::from_secs(5), &mut out);
            let mut virtual_down = HashSet::new();
            for event in &out {
                let edges = if let EngineOutput::Stroke { ks, .. } = event {
                    ks.mods
                        .iter()
                        .copied()
                        .map(|key| (key, true))
                        .chain([(ks.key, true), (ks.key, false)])
                        .chain(ks.mods.iter().rev().copied().map(|key| (key, false)))
                        .collect()
                } else {
                    transitions(std::slice::from_ref(event))
                };
                for (key, down) in edges {
                    if down {
                        virtual_down.insert(key);
                    } else {
                        assert!(
                            virtual_down.remove(&key),
                            "orphan release {key:?}, mode {mode:?}, case {case}"
                        );
                    }
                }
            }
            assert!(
                virtual_down.is_empty(),
                "stuck output, mode {mode:?}, case {case}"
            );
            assert!(
                engine.pending.is_empty()
                    && engine.virt.emitted.is_empty()
                    && engine.decision_buffer.is_empty()
                    && engine.layers.active.is_empty()
                    && engine.virt.mod_refs.values().all(|count| *count == 0),
                "mode {mode:?}, case {case}"
            );
        }
    }
}
