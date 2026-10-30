use super::*;

#[test]
fn long_hold_runs_once_at_default_deadline_without_native_key() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyA", "longHoldAction": "text:done"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    assert_eq!(engine.next_deadline(now), Some(Duration::from_secs(1)));
    engine.tick(now + Duration::from_millis(999), &mut out);
    assert!(out.is_empty());
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(matches!(out.as_slice(), [EngineOutput::Literal(text)] if text == "done"));
    out.clear();
    engine.handle_key_event(KeyCode::KEY_A, true, now + Duration::from_secs(2), &mut out);
    engine.tick(now + Duration::from_secs(3), &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_secs(4),
        &mut out,
    );
    assert!(out.is_empty());
    assert_eq!(engine.next_deadline(now), None);
}

#[test]
fn long_hold_early_release_taps_and_other_keys_do_not_commit_it() {
    for mode in ["permissiveHold", "holdOnOtherKeyPress"] {
        let mut cfg = config_with_rule(
            serde_json::json!({"key": "KeyA", "tapAction": "Escape", "longHoldAction": "text:done", "longHoldTimeoutMs": 600}),
        );
        cfg.settings.tap_decision = serde_json::from_value(serde_json::json!(mode)).unwrap();
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
        engine.handle_key_event(
            KeyCode::KEY_B,
            true,
            now + Duration::from_millis(50),
            &mut out,
        );
        engine.handle_key_event(
            KeyCode::KEY_B,
            false,
            now + Duration::from_millis(100),
            &mut out,
        );
        assert!(matches!(
            out.as_slice(),
            [
                EngineOutput::KeyRaw {
                    key: KeyCode::KEY_B,
                    down: true
                },
                EngineOutput::KeyRaw {
                    key: KeyCode::KEY_B,
                    down: false
                }
            ]
        ));
        out.clear();
        engine.handle_key_event(
            KeyCode::KEY_A,
            false,
            now + Duration::from_millis(500),
            &mut out,
        );
        engine.tick(now + Duration::from_secs(2), &mut out);
        assert!(
            matches!(out.as_slice(), [EngineOutput::Stroke {ks, ..}] if ks.key == KeyCode::KEY_ESC)
        );
    }
}

#[test]
fn long_hold_override_fires_even_when_release_arrives_before_tick() {
    let cfg = config_with_rule(
        serde_json::json!({"key": "KeyA", "longHoldAction": "text:done", "longHoldTimeoutMs": 1500}),
    );
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(out.is_empty());
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(1500),
        &mut out,
    );
    assert!(matches!(out.as_slice(), [EngineOutput::Literal(text)] if text == "done"));
}

#[test]
fn tap_on_release_fires_tap_action() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "CapsLock".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: Some(200),
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_CAPSLOCK, true, now, &mut out);
    assert!(out.is_empty());

    engine.handle_key_event(
        KeyCode::KEY_CAPSLOCK,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_ESC
    ));
}

#[test]
fn hold_timeout_commits_hold_via_tick() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "AltLeft".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("ControlLeft".into()),
        hold_timeout_ms: Some(50),
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTALT, true, now, &mut out);
    assert!(out.is_empty());

    engine.tick(now + Duration::from_millis(60), &mut out);
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::ChordPress { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_LEFTCTRL
    ));
}

#[test]
fn double_tap_fires_double_tap_action() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: "KeyB".into(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    // First tap
    engine.handle_key_event(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty()); // WaitingSecond

    // Second tap within window
    engine.handle_key_event(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_B
    ));

    // Release of second tap must be silent
    out.clear();
    engine.handle_key_event(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert!(out.is_empty());
}

#[test]
fn double_tap_window_expired_fires_tap() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: "KeyB".into(),
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
    assert!(out.is_empty());

    engine.tick(now + Duration::from_millis(250), &mut out);
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_A
    ));
}

#[test]
fn double_tap_flush_on_other_key_press() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: "KeyB".into(),
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
    assert!(out.is_empty());

    // Another key press flushes the deferred tap before processing
    engine.handle_key_event(
        KeyCode::KEY_W,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert_eq!(out.len(), 2);
    assert!(matches!(
        out[0],
        EngineOutput::Stroke { ref ks, .. } if ks.mods.is_empty() && ks.key == KeyCode::KEY_A
    ));
    assert!(matches!(
        out[1],
        EngineOutput::KeyRaw { key, down: true } if key == KeyCode::KEY_W
    ));
}

#[test]
fn double_tap_late_second_press_is_tap_not_double_tap() {
    // The second press lands after the double-tap window has closed but
    // before any `tick` fires the deferred tap (`handle` runs before
    // `tick` in the loop). It must resolve the first tap and be treated
    // as a fresh press, never as a double-tap.
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: "KeyB".into(),
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
    assert!(out.is_empty()); // WaitingSecond, deadline = now + 210ms

    // Second press well past the 200 ms window.
    engine.handle_key_event(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(500),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [EngineOutput::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_A
        ),
        "late second press must fire the first tap (KeyA), not the double-tap"
    );
}

#[test]
fn permissive_prior_key_release_replays_after_buffer() {
    // Shift↓ (held, passthrough), F↓ (rule, deciding), J↓ (buffered),
    // Shift↑. The Shift release must not overtake the buffered J: when the
    // buffer drains, J is pressed before Shift is released, preserving the
    // modifier state J was physically pressed under.
    let mut cfg = empty_cfg();
    cfg.rules.push(tap_hold_rule(
        "KeyF",
        ActionSpec::Action("Escape".into()),
        ActionSpec::Action("ControlLeft".into()),
    ));
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::KeyRaw { key, down: true }] if *key == KeyCode::KEY_LEFTSHIFT
    ));
    out.clear();

    engine.handle_key_event(
        KeyCode::KEY_F,
        true,
        now + Duration::from_millis(5),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(15),
        &mut out,
    );
    // Everything deferred behind F's decision.
    assert!(out.is_empty());

    // F released first → F is a tap; the buffer replays in physical order.
    engine.handle_key_event(
        KeyCode::KEY_F,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [
                EngineOutput::Stroke { ks, .. },
                EngineOutput::KeyRaw { key: j, down: true },
                EngineOutput::KeyRaw { key: sh, down: false },
            ] if ks.key == KeyCode::KEY_ESC
                && *j == KeyCode::KEY_J
                && *sh == KeyCode::KEY_LEFTSHIFT
        ),
        "expected Esc tap, then J down, then Shift up ordered last"
    );
}

#[test]
fn shutdown_releases_all_emitted_keys_and_mods() {
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
    sp.keys.insert("KeyA".into(), Some("Ctrl+KeyC".into()));
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
    out.clear();

    engine.shutdown(&mut out);
    // Should emit: KeyC release, Ctrl release (ReleaseMods)
    assert!(
        out.iter().any(
            |o| matches!(o, EngineOutput::KeyRaw { key, down: false } if *key == KeyCode::KEY_C)
        )
    );
    assert!(out.iter().any(
        |o| matches!(o, EngineOutput::ReleaseMods(mods) if mods.contains(&KeyCode::KEY_LEFTCTRL))
    ));
    assert!(engine.layers.active.is_empty());
    assert!(engine.pending.is_empty());
}

#[test]
fn mod_refs_shared_by_two_keys() {
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
    sp.keys.insert("KeyA".into(), Some("Ctrl+KeyC".into()));
    sp.keys.insert("KeyB".into(), Some("Ctrl+KeyV".into()));
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

    // Release KeyA — Ctrl should NOT be released because KeyB still holds it
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(290),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, EngineOutput::ChordRelease { key, .. } if *key == KeyCode::KEY_C))
    );
    assert!(!out.iter().any(
        |o| matches!(o, EngineOutput::ChordRelease { mods, .. } if mods.contains(&KeyCode::KEY_LEFTCTRL))
    ));

    out.clear();
    // Release KeyB — now Ctrl should be released
    engine.handle_key_event(
        KeyCode::KEY_B,
        false,
        now + Duration::from_millis(300),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, EngineOutput::ChordRelease { key, .. } if *key == KeyCode::KEY_V))
    );
    assert!(out.iter().any(
        |o| matches!(o, EngineOutput::ChordRelease { mods, .. } if mods.contains(&KeyCode::KEY_LEFTCTRL))
    ));
}

#[test]
fn unknown_key_on_release_passes_through() {
    let mut engine = Engine::new(&empty_cfg());
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_A, false, now, &mut out);
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::KeyRaw { key, down: false }] if *key == KeyCode::KEY_A
    ));
}

#[test]
fn commit_waiting_decisions_on_other_key_press() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "ShiftLeft".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("ControlLeft".into()),
        hold_timeout_ms: Some(200),
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    // Asserts the interrupt-on-press semantics.
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    assert!(out.is_empty());

    // Another key press commits the pending hold
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::ChordPress { ks, .. },
            EngineOutput::KeyRaw { key: k1, down: true },
        ] if ks.mods.is_empty() && ks.key == KeyCode::KEY_LEFTCTRL && *k1 == KeyCode::KEY_A
    ));
}

#[test]
fn permissive_roll_fires_tap_not_hold() {
    // Shift↓, A↓, Shift↑ (A still held): rolling overlap while the
    // deciding key is released first → Shift is a tap, then A replays.
    let mut cfg = empty_cfg();
    cfg.rules.push(tap_hold_rule(
        "ShiftLeft",
        ActionSpec::Action("Escape".into()),
        ActionSpec::Action("ControlLeft".into()),
    ));
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    // Nothing emitted yet — A is buffered behind Shift's decision.
    assert!(out.is_empty());

    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [
                EngineOutput::Stroke { ks: esc, .. },
                EngineOutput::KeyRaw { key: a, down: true },
            ] if esc.mods.is_empty()
                && esc.key == KeyCode::KEY_ESC
                && *a == KeyCode::KEY_A
        ),
        "expected tap Escape then replayed A"
    );
}

#[test]
fn permissive_nested_release_commits_hold() {
    // Shift↓, A↓, A↑ (A pressed and released fully inside the decision
    // window) → Shift is a hold; A is typed under the held modifier.
    let mut cfg = empty_cfg();
    cfg.rules.push(tap_hold_rule(
        "ShiftLeft",
        ActionSpec::Action("Escape".into()),
        ActionSpec::Action("ControlLeft".into()),
    ));
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );

    assert!(
        matches!(
            out.as_slice(),
            [
                EngineOutput::ChordPress { ks: ctrl, .. },
                EngineOutput::KeyRaw { key: a_down, down: true },
                EngineOutput::KeyRaw { key: a_up, down: false },
            ] if ctrl.mods.is_empty()
                && ctrl.key == KeyCode::KEY_LEFTCTRL
                && *a_down == KeyCode::KEY_A
                && *a_up == KeyCode::KEY_A
        ),
        "expected Ctrl hold then A press+release"
    );
}

#[test]
fn permissive_timeout_commits_hold_with_buffer() {
    // Caps↓ (layer rule), J↓ buffered, then the hold timeout elapses via
    // tick → layer commits and the buffered J replays in the layer.
    let mut cfg = empty_cfg();
    let mut caps = tap_hold_rule("CapsLock", ActionSpec::Native, ActionSpec::Native);
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

    engine.handle_key_event(KeyCode::KEY_CAPSLOCK, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty());

    engine.tick(now + Duration::from_millis(250), &mut out);
    assert!(
        matches!(
            out.as_slice(),
            [EngineOutput::ChordPress { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_LEFT
        ),
        "expected buffered J to resolve as Left in the nav layer"
    );
}

#[test]
fn permissive_release_of_prior_key_does_not_commit() {
    // A is pressed *before* Shift starts deciding. A's release must not
    // commit Shift's hold; releasing Shift first still fires its tap.
    let mut cfg = empty_cfg();
    cfg.rules.push(tap_hold_rule(
        "ShiftLeft",
        ActionSpec::Action("Escape".into()),
        ActionSpec::Action("ControlLeft".into()),
    ));
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );

    // No ControlLeft hold anywhere; Shift resolved as a tap (Escape).
    assert!(
        !out.iter()
            .any(|o| matches!(o, EngineOutput::ChordPress { .. })),
        "hold must not commit"
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, EngineOutput::Stroke { ks, .. } if ks.key == KeyCode::KEY_ESC)),
        "Shift tap (Escape) must fire"
    );
}

#[test]
fn permissive_chain_of_two_rule_keys_both_tap() {
    // Shift↓, Caps↓ (buffered behind Shift), Shift↑ → Shift taps and Caps
    // replays into its own decision; Caps↑ → Caps taps.
    let mut cfg = empty_cfg();
    cfg.rules.push(tap_hold_rule(
        "ShiftLeft",
        ActionSpec::Action("Escape".into()),
        ActionSpec::Action("ControlLeft".into()),
    ));
    cfg.rules.push(tap_hold_rule(
        "CapsLock",
        ActionSpec::Action("Tab".into()),
        ActionSpec::Action("AltLeft".into()),
    ));
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_CAPSLOCK,
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
    // Shift tapped; Caps now deciding after replay.
    assert!(
        matches!(
            out.as_slice(),
            [EngineOutput::Stroke { ks, .. }] if ks.key == KeyCode::KEY_ESC
        ),
        "expected only Shift's Escape tap so far"
    );

    out.clear();
    engine.handle_key_event(
        KeyCode::KEY_CAPSLOCK,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [EngineOutput::Stroke { ks, .. }] if ks.key == KeyCode::KEY_TAB
        ),
        "expected Caps's Tab tap"
    );
}

#[test]
fn permissive_native_hold_shift_roll_still_taps() {
    // Mirrors the user's real rule: ShiftLeft tap=Ctrl+KeyC with an
    // implicit native hold. A rolling overlap with a letter must fire
    // the Ctrl+C tap, not commit the native Shift hold.
    let mut cfg = empty_cfg();
    cfg.rules.push(tap_hold_rule(
        "ShiftLeft",
        ActionSpec::Action("Ctrl+KeyC".into()),
        ActionSpec::Native,
    ));
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(30),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(60),
        &mut out,
    );

    assert!(
        matches!(
            out.as_slice(),
            [
                EngineOutput::Stroke { ks: copy, .. },
                EngineOutput::KeyRaw { key: a, down: true },
            ] if copy.mods == vec![KeyCode::KEY_LEFTCTRL]
                && copy.key == KeyCode::KEY_C
                && *a == KeyCode::KEY_A
        ),
        "expected Ctrl+C tap then replayed A"
    );
}

#[test]
fn rule_hold_detection_overrides_default_and_can_mix_in_one_layout() {
    for default in [
        TapDecision::PermissiveHold,
        TapDecision::HoldOnOtherKeyPress,
    ] {
        let mut cfg = empty_cfg();
        cfg.settings.tap_decision = default;
        for (key, decision) in [
            ("KeyF", Some(TapDecision::PermissiveHold)),
            ("Space", Some(TapDecision::HoldOnOtherKeyPress)),
            ("KeyD", None),
        ] {
            let mut rule = tap_hold_rule(
                key,
                ActionSpec::Native,
                ActionSpec::Action("ControlLeft".into()),
            );
            rule.tap_decision = decision;
            cfg.rules.push(rule);
        }
        let mut engine = Engine::new(&cfg);
        for (key, press_hold) in [
            (KeyCode::KEY_F, false),
            (KeyCode::KEY_SPACE, true),
            (KeyCode::KEY_D, default == TapDecision::HoldOnOtherKeyPress),
        ] {
            let mut out = Vec::new();
            let now = Instant::now();
            engine.handle_key_event(key, true, now, &mut out);
            engine.handle_key_event(
                KeyCode::KEY_C,
                true,
                now + Duration::from_millis(10),
                &mut out,
            );
            if press_hold {
                assert!(
                    matches!(out.as_slice(), [EngineOutput::ChordPress { ks, .. }, EngineOutput::KeyRaw { key: KeyCode::KEY_C, down: true }] if ks.key == KeyCode::KEY_LEFTCTRL)
                );
            } else {
                assert!(
                    out.is_empty(),
                    "release detection must buffer the second key"
                );
            }
            engine.handle_key_event(key, false, now + Duration::from_millis(20), &mut out);
            if !press_hold {
                assert!(
                    matches!(out.as_slice(), [EngineOutput::Stroke { ks, .. }, EngineOutput::KeyRaw { key: KeyCode::KEY_C, down: true }] if ks.key == key)
                );
            }
            engine.handle_key_event(
                KeyCode::KEY_C,
                false,
                now + Duration::from_millis(30),
                &mut out,
            );
        }
    }
}

#[test]
fn buffered_rule_keeps_its_own_hold_detection_after_replay() {
    let mut cfg = empty_cfg();
    let mut letter = tap_hold_rule(
        "KeyF",
        ActionSpec::Native,
        ActionSpec::Action("ShiftLeft".into()),
    );
    letter.tap_decision = Some(TapDecision::PermissiveHold);
    let mut layer_key = tap_hold_rule(
        "Space",
        ActionSpec::Native,
        ActionSpec::Action("ControlLeft".into()),
    );
    layer_key.tap_decision = Some(TapDecision::HoldOnOtherKeyPress);
    cfg.rules = vec![letter, layer_key];
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();
    engine.handle_key_event(KeyCode::KEY_F, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_SPACE,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty());
    engine.handle_key_event(
        KeyCode::KEY_F,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(
        matches!(out.as_slice(), [EngineOutput::Stroke { ks, .. }] if ks.key == KeyCode::KEY_F)
    );
    out.clear();
    engine.handle_key_event(
        KeyCode::KEY_C,
        true,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert!(
        matches!(out.as_slice(), [EngineOutput::ChordPress { ks, .. }, EngineOutput::KeyRaw { key: KeyCode::KEY_C, down: true }] if ks.key == KeyCode::KEY_LEFTCTRL)
    );
}

#[test]
fn shared_main_key_is_released_only_by_the_last_owner() {
    for target in ["ControlLeft", "KeyC"] {
        let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
            {"key": "KeyA", "tapAction": null, "holdAction": target},
            {"key": "KeyB", "tapAction": null, "holdAction": target}
        ]}))
        .unwrap();
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
        engine.handle_key_event(KeyCode::KEY_B, true, now, &mut out);
        out.clear();
        engine.handle_key_event(KeyCode::KEY_A, false, now, &mut out);
        assert!(out.is_empty());
        engine.handle_key_event(KeyCode::KEY_B, false, now, &mut out);
        assert!(matches!(
            out.as_slice(),
            [EngineOutput::KeyRaw { down: false, .. }]
        ));
    }
}

#[test]
fn release_after_hold_deadline_commits_hold_before_release() {
    let cfg = config_with_rule(serde_json::json!({
        "key": "KeyA", "tapAction": "Escape", "holdAction": "ControlLeft", "holdTimeoutMs": 10
    }));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(
        matches!(out.as_slice(), [EngineOutput::ChordPress { ks, .. }, EngineOutput::KeyRaw { key: KeyCode::KEY_LEFTCTRL, down: false }] if ks.key == KeyCode::KEY_LEFTCTRL)
    );
}

#[test]
fn late_release_replays_buffered_keys_before_releasing_the_hold() {
    let cfg = config_with_rule(serde_json::json!({
        "key": "KeyA", "tapAction": "Escape", "holdAction": "ControlLeft", "holdTimeoutMs": 10
    }));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(5),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(matches!(out.as_slice(), [
        EngineOutput::ChordPress { ks, .. },
        EngineOutput::KeyRaw { key: KeyCode::KEY_B, down: true },
        EngineOutput::KeyRaw { key: KeyCode::KEY_LEFTCTRL, down: false }
    ] if ks.key == KeyCode::KEY_LEFTCTRL));
}

#[test]
fn nested_release_must_not_leave_key_pressed() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key":"KeyA", "tapAction":"Escape", "holdAction":"ControlLeft", "holdTimeoutMs":200},
        {"key":"KeyB", "tapAction":"Tab", "holdAction":"ShiftLeft", "holdTimeoutMs":200}
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = vec![];
    for (key, down, ms) in [
        (KeyCode::KEY_A, true, 0),
        (KeyCode::KEY_B, true, 10),
        (KeyCode::KEY_C, true, 20),
        (KeyCode::KEY_C, false, 30),
        (KeyCode::KEY_B, false, 40),
        (KeyCode::KEY_A, false, 50),
    ] {
        engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
        engine.tick(now + Duration::from_millis(ms), &mut out);
    }
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(
        engine.virt.emitted.is_empty(),
        "all physical keys were released"
    );
    let c_events: Vec<_> = out
        .iter()
        .filter_map(|event| match event {
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_C,
                down,
            } => Some(*down),
            _ => None,
        })
        .collect();
    assert_eq!(c_events, vec![true, false]);
}

#[test]
fn buffered_long_hold_must_fire() {
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key":"KeyA", "tapAction":"Escape", "holdAction":"ControlLeft", "holdTimeoutMs":200},
        {"key":"KeyB", "tapAction":"Tab", "longHoldAction":"text:long", "longHoldTimeoutMs":100}
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = vec![];
    for (key, down, ms) in [
        (KeyCode::KEY_A, true, 0),
        (KeyCode::KEY_B, true, 10),
        (KeyCode::KEY_B, false, 150),
        (KeyCode::KEY_A, false, 160),
    ] {
        engine.handle_key_event(key, down, now + Duration::from_millis(ms), &mut out);
        engine.tick(now + Duration::from_millis(ms), &mut out);
    }
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, EngineOutput::Literal(s) if s == "long"))
            .count(),
        1
    );
    assert!(
        !out.iter()
            .any(|o| matches!(o, EngineOutput::Stroke { ks, .. } if ks.key == KeyCode::KEY_TAB))
    );
}

#[test]
fn actions_unavailable_here_are_skipped_with_a_notice() {
    let (record, seen) = std::sync::mpsc::channel();
    let _events = crate::events::bus().subscribe(move |event| {
        if let crate::events::CoreEvent::ActionUnavailable(action) = event
            && action.contains("foreignOnly")
        {
            record.send(action.clone()).unwrap();
        }
    }).unwrap();
    let mut cfg =
        config_with_rule(serde_json::json!({"key": "KeyQ", "tapAction": "cmd:foreignOnly"}));
    // A script only for the other OS: this one cannot run the command.
    cfg.commands.push(Command {
        id: "foreignOnly".into(),
        posix_script: if cfg!(target_os = "windows") {
            "ls"
        } else {
            ""
        }
        .into(),
        powershell_script: if cfg!(target_os = "windows") {
            ""
        } else {
            "dir"
        }
        .into(),
        working_directory: None,
    });
    cfg.macros.push(Macro {
        id: "mixed".into(),
        steps: vec![
            MacroStep {
                action: "KeyA".into(),
            },
            MacroStep {
                action: "cmd:foreignOnly".into(),
            },
        ],
        step_pause_ms: None,
        modifier_delay_ms: None,
    });
    cfg.settings.commands_enabled = true;
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle_key_event(KeyCode::KEY_Q, false, now, &mut out);
    engine.execute_remote("macro:mixed", &mut out);
    engine.tick(now + Duration::from_millis(1), &mut out);
    // Neither the trigger key nor a half-run macro reaches the OS.
    assert!(out.is_empty());
    assert_eq!(
        (0..2).map(|_| seen.recv_timeout(Duration::from_secs(1)).unwrap()).collect::<Vec<_>>(),
        ["cmd:foreignOnly", "cmd:foreignOnly"]
    );
}

#[test]
fn repeats_reemit_only_keys_that_hold_a_virtual_key() {
    let cfg = config_with_rule(
        serde_json::json!({"key": "Space", "layerId": "nav", "tapAction": "Space"}),
    );
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    out.clear();
    engine.repeat(KeyCode::KEY_A, &mut out);
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::KeyRaw {
            key: KeyCode::KEY_A,
            down: true
        }]
    ));
    out.clear();
    // A released key and an undecided tap/hold key do not repeat.
    engine.handle_key_event(KeyCode::KEY_A, false, now, &mut out);
    out.clear();
    engine.repeat(KeyCode::KEY_A, &mut out);
    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.repeat(KeyCode::KEY_SPACE, &mut out);
    assert!(out.is_empty());
}

#[test]
fn confirmed_hold_repeats_the_mapped_key_until_release() {
    let cfg =
        config_with_rule(serde_json::json!({"key":"KeyA","tapAction":"KeyB","holdAction":"KeyB"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle_key_event(KeyCode::KEY_A, true, now, &mut out);
    engine.repeat(KeyCode::KEY_A, &mut out);
    assert!(out.is_empty());
    engine.tick(now + Duration::from_secs(1), &mut out);
    out.clear();
    engine.repeat(KeyCode::KEY_A, &mut out);
    engine.repeat(KeyCode::KEY_A, &mut out);
    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_B,
                down: true
            },
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_B,
                down: true
            }
        ]
    ));
    engine.handle_key_event(
        KeyCode::KEY_A,
        false,
        now + Duration::from_secs(2),
        &mut out,
    );
    out.clear();
    engine.repeat(KeyCode::KEY_A, &mut out);
    assert!(out.is_empty());
}

#[test]
fn left_and_right_modifier_taps_remain_distinct() {
    let names = [
        "ShiftLeft",
        "ShiftRight",
        "ControlLeft",
        "ControlRight",
        "AltLeft",
        "AltRight",
    ];
    let inputs = [
        KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_RIGHTSHIFT,
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_RIGHTCTRL,
        KeyCode::KEY_LEFTALT,
        KeyCode::KEY_RIGHTALT,
    ];
    let actions = ["KeyL", "KeyR", "KeyC", "KeyD", "KeyE", "KeyF"];
    let outputs = [
        KeyCode::KEY_L,
        KeyCode::KEY_R,
        KeyCode::KEY_C,
        KeyCode::KEY_D,
        KeyCode::KEY_E,
        KeyCode::KEY_F,
    ];
    let rules: Vec<_> = names
        .iter()
        .zip(actions)
        .map(|(key, action)| serde_json::json!({"key":key,"tapAction":action}))
        .collect();
    let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules":rules})).unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    for (index, (input, expected)) in inputs.into_iter().zip(outputs).enumerate() {
        let at = now + Duration::from_millis(index as u64 * 150);
        let mut out = Vec::new();
        engine.handle_key_event(input, true, at, &mut out);
        engine.handle_key_event(input, false, at + Duration::from_millis(20), &mut out);
        assert!(
            matches!(out.as_slice(), [EngineOutput::Stroke {ks, ..}] if ks.key == expected),
            "{}",
            names[index]
        );
    }
}

#[test]
fn release_waiting_in_decision_buffer_decides_long_hold_tap() {
    let out = timeline(
        serde_json::json!({"rules": [
            {"key":"KeyL", "tapAction":"text:tap", "longHoldAction":"text:long", "longHoldTimeoutMs":100},
            {"key":"KeyD", "holdAction":"ControlLeft", "holdTimeoutMs":300, "tapDecision":"permissiveHold"}
        ]}),
        &[
            (KeyCode::KEY_L, true, 0),
            (KeyCode::KEY_D, true, 10),
            (KeyCode::KEY_X, true, 20),
            (KeyCode::KEY_L, false, 50),
            (KeyCode::KEY_D, false, 200),
            (KeyCode::KEY_X, false, 210),
        ],
        &[0, 10, 20, 50, 100, 150, 200, 210],
    );
    assert_eq!(
        out,
        [
            TraceEvent::Tap(KeyCode::KEY_D, vec![]),
            TraceEvent::Raw(KeyCode::KEY_X, true),
            TraceEvent::Text("tap".into()),
            TraceEvent::Raw(KeyCode::KEY_X, false)
        ]
    );
}

#[test]
fn deferred_double_tap_tap_keeps_a_modifier_released_in_the_window() {
    let out = timeline(
        serde_json::json!({"rules": [
            {"key":"KeyA", "doubleTapAction":"text:double", "doubleTapTimeoutMs":250}
        ]}),
        &[
            (KeyCode::KEY_LEFTCTRL, true, 0),
            (KeyCode::KEY_A, true, 20),
            (KeyCode::KEY_A, false, 60),
            (KeyCode::KEY_LEFTCTRL, false, 80),
        ],
        &[20, 60, 80, 400],
    );
    assert_eq!(
        out,
        [
            TraceEvent::Raw(KeyCode::KEY_LEFTCTRL, true),
            TraceEvent::Tap(KeyCode::KEY_A, vec![]),
            TraceEvent::Raw(KeyCode::KEY_LEFTCTRL, false)
        ]
    );
}

#[test]
fn tap_hold_tap_keeps_a_modifier_released_before_the_key() {
    for mode in ["permissiveHold", "holdOnOtherKeyPress"] {
        let rule = serde_json::json!({"rules": [
            {"key":"KeyA", "holdAction":"ControlLeft", "holdTimeoutMs":200, "tapDecision":mode}
        ]});
        // Tap: Shift is released only after the tap that it shifted.
        let out = timeline(
            rule.clone(),
            &[
                (KeyCode::KEY_LEFTSHIFT, true, 0),
                (KeyCode::KEY_A, true, 20),
                (KeyCode::KEY_LEFTSHIFT, false, 60),
                (KeyCode::KEY_A, false, 80),
            ],
            &[20, 60, 80],
        );
        assert_eq!(
            out,
            [
                TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, true),
                TraceEvent::Tap(KeyCode::KEY_A, vec![]),
                TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, false)
            ],
            "{mode}"
        );
        // Hold by timeout: the deferred release follows the committed hold.
        let out = timeline(
            rule.clone(),
            &[
                (KeyCode::KEY_LEFTSHIFT, true, 0),
                (KeyCode::KEY_A, true, 20),
                (KeyCode::KEY_LEFTSHIFT, false, 60),
                (KeyCode::KEY_A, false, 400),
            ],
            &[20, 60, 220, 400],
        );
        assert_eq!(
            out,
            [
                TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, true),
                TraceEvent::Press(KeyCode::KEY_LEFTCTRL, vec![]),
                TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, false),
                TraceEvent::Raw(KeyCode::KEY_LEFTCTRL, false)
            ],
            "{mode}"
        );
    }
    // Hold on other key press: the release replays before the interrupting
    // press that commits the hold.
    let out = timeline(
        serde_json::json!({"rules": [
            {"key":"KeyA", "holdAction":"ControlLeft", "tapDecision":"holdOnOtherKeyPress"}
        ]}),
        &[
            (KeyCode::KEY_LEFTSHIFT, true, 0),
            (KeyCode::KEY_A, true, 20),
            (KeyCode::KEY_LEFTSHIFT, false, 40),
            (KeyCode::KEY_X, true, 60),
            (KeyCode::KEY_X, false, 70),
            (KeyCode::KEY_A, false, 80),
        ],
        &[],
    );
    assert_eq!(
        out,
        [
            TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, true),
            TraceEvent::Press(KeyCode::KEY_LEFTCTRL, vec![]),
            TraceEvent::Raw(KeyCode::KEY_LEFTSHIFT, false),
            TraceEvent::Raw(KeyCode::KEY_X, true),
            TraceEvent::Raw(KeyCode::KEY_X, false),
            TraceEvent::Raw(KeyCode::KEY_LEFTCTRL, false)
        ]
    );
}

#[test]
fn tap_then_hold_activates_the_layer_and_quick_second_tap_is_double_tap() {
    let cfg = serde_json::json!({
        "rules": [{"key":"Space", "layerId":"nav", "doubleTapAction":"Escape"}],
        "layerKeymaps": {"nav": {"keys": {"KeyJ": "ArrowLeft"}}}
    });
    let held = timeline(
        cfg.clone(),
        &[
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_SPACE, false, 30),
            (KeyCode::KEY_SPACE, true, 100),
            (KeyCode::KEY_J, true, 600),
            (KeyCode::KEY_J, false, 650),
            (KeyCode::KEY_SPACE, false, 700),
        ],
        &[0, 30, 100, 400, 600, 650, 700],
    );
    assert_eq!(
        held,
        [
            TraceEvent::Press(KeyCode::KEY_LEFT, vec![]),
            TraceEvent::Raw(KeyCode::KEY_LEFT, false)
        ]
    );
    let tapped = timeline(
        cfg,
        &[
            (KeyCode::KEY_SPACE, true, 0),
            (KeyCode::KEY_SPACE, false, 30),
            (KeyCode::KEY_SPACE, true, 100),
            (KeyCode::KEY_SPACE, false, 130),
        ],
        &[0, 30, 100, 130],
    );
    assert_eq!(tapped, [TraceEvent::Tap(KeyCode::KEY_ESC, vec![])]);
}

#[test]
fn hold_deadline_boundary_is_identical_for_ticks_and_release_events() {
    for mode in ["permissiveHold", "holdOnOtherKeyPress"] {
        for elapsed in [199, 200, 201] {
            for tick_first in [false, true] {
                let cfg = serde_json::json!({"rules": [{
                    "key": "KeyA", "tapAction": "Escape", "holdAction": "ControlLeft",
                    "holdTimeoutMs": 200, "tapDecision": mode
                }]});
                let cfg: MapperConfig = serde_json::from_value(cfg).unwrap();
                let mut engine = Engine::new(&cfg);
                let start = Instant::now();
                let mut out = vec![];
                engine.handle_key_event(KeyCode::KEY_A, true, start, &mut out);
                engine.tick(start + Duration::from_millis(199.min(elapsed)), &mut out);
                assert!(out.is_empty(), "hold fired before deadline");
                if tick_first {
                    engine.tick(start + Duration::from_millis(elapsed), &mut out);
                    assert_eq!(out.len(), usize::from(elapsed >= 200));
                }
                engine.handle_key_event(
                    KeyCode::KEY_A,
                    false,
                    start + Duration::from_millis(elapsed),
                    &mut out,
                );
                if elapsed < 200 {
                    assert!(matches!(out.as_slice(), [EngineOutput::Stroke { ks, .. }]
                        if ks.key == KeyCode::KEY_ESC && ks.mods.is_empty()));
                } else {
                    assert!(
                        matches!(out.as_slice(), [
                        EngineOutput::ChordPress { ks, .. },
                        EngineOutput::KeyRaw { key: KeyCode::KEY_LEFTCTRL, down: false }
                    ] if ks.key == KeyCode::KEY_LEFTCTRL && ks.mods.is_empty()),
                        "{mode}, {elapsed}, tick_first={tick_first}"
                    );
                }
                let count = out.len();
                engine.tick(start + Duration::from_secs(2), &mut out);
                assert_eq!(out.len(), count, "action fired again after release");
                assert!(engine.pending.is_empty() && engine.virt.emitted.is_empty());
            }
        }
    }
}

#[test]
fn double_tap_deadline_uses_event_time_before_or_after_timer_delivery() {
    for second_press in [259, 260, 261] {
        for tick_before_press in [false, true] {
            let cfg = config_with_rule(serde_json::json!({
                "key": "KeyA", "tapAction": "Escape", "doubleTapAction": "Tab",
                "doubleTapTimeoutMs": 250
            }));
            let mut engine = Engine::new(&cfg);
            let start = Instant::now();
            let mut out = vec![];
            engine.handle_key_event(KeyCode::KEY_A, true, start, &mut out);
            engine.handle_key_event(
                KeyCode::KEY_A,
                false,
                start + Duration::from_millis(10),
                &mut out,
            );
            engine.tick(start + Duration::from_millis(259), &mut out);
            assert!(out.is_empty(), "single tap fired before deadline");
            if tick_before_press {
                engine.tick(start + Duration::from_millis(second_press), &mut out);
                assert_eq!(out.len(), usize::from(second_press >= 260));
            }
            engine.handle_key_event(
                KeyCode::KEY_A,
                true,
                start + Duration::from_millis(second_press),
                &mut out,
            );
            engine.handle_key_event(
                KeyCode::KEY_A,
                false,
                start + Duration::from_millis(second_press + 10),
                &mut out,
            );
            engine.tick(start + Duration::from_secs(2), &mut out);
            // A timer delivered at the exact deadline expires the first tap;
            // an event delivered first at that instant is still in the window.
            let double = second_press < 260 || (second_press == 260 && !tick_before_press);
            let expected = if double {
                vec![KeyCode::KEY_TAB]
            } else {
                vec![KeyCode::KEY_ESC, KeyCode::KEY_ESC]
            };
            let keys: Vec<_> = out
                .iter()
                .map(|event| match event {
                    EngineOutput::Stroke { ks, .. } if ks.mods.is_empty() => ks.key,
                    _ => panic!("unexpected non-tap output"),
                })
                .collect();
            assert_eq!(
                keys, expected,
                "second_press={second_press}, tick_before_press={tick_before_press}"
            );
            assert!(engine.pending.is_empty() && engine.virt.emitted.is_empty());
        }
    }
}

#[test]
fn mapped_tap_and_hold_keep_their_own_modifiers() {
    let cfg = serde_json::json!({"rules": [{
        "key": "KeyA", "tapAction": "Ctrl+KeyC", "holdAction": "Alt+Tab",
        "holdTimeoutMs": 200
    }]});
    let tapped = timeline(
        cfg.clone(),
        &[(KeyCode::KEY_A, true, 0), (KeyCode::KEY_A, false, 50)],
        &[],
    );
    assert_eq!(
        tapped,
        [TraceEvent::Tap(KeyCode::KEY_C, vec![KeyCode::KEY_LEFTCTRL])]
    );
    let held = timeline(
        cfg,
        &[(KeyCode::KEY_A, true, 0), (KeyCode::KEY_A, false, 250)],
        &[200],
    );
    assert_eq!(
        held,
        [
            TraceEvent::Press(KeyCode::KEY_TAB, vec![KeyCode::KEY_LEFTALT]),
            TraceEvent::Release(KeyCode::KEY_TAB, vec![KeyCode::KEY_LEFTALT]),
        ]
    );
}
