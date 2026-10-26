use super::*;

#[test]
fn mouse_button_commits_pending_hold() {
    // All pending WaitingDecision holds commit on mouse button press,
    // regardless of hold mode (Keystroke, Native, or Swallow).
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
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    // No hold committed yet — Shift is still in WaitingDecision.
    assert!(out.is_empty());

    // LMB click commits the pending hold (ControlLeft) so the physical
    // mouse event reaches the OS with the modifier active.
    engine.handle_key_event(
        KeyCode::BTN_LEFT,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::BTN_LEFT,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );

    // Releasing Shift releases the committed hold (ControlLeft).
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::ChordPress { ks, .. },
            EngineOutput::KeyRaw { key: k1, down: true },
            EngineOutput::KeyRaw { key: k2, down: false },
            EngineOutput::KeyRaw { key: ctrl, down: false },
        ] if ks.mods.is_empty()
            && ks.key == KeyCode::KEY_LEFTCTRL
            && *k1 == KeyCode::BTN_LEFT
            && *k2 == KeyCode::BTN_LEFT
            && *ctrl == KeyCode::KEY_LEFTCTRL
    ));
}

#[test]
fn extra_mouse_button_commits_pending_keystroke_hold() {
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
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );

    // Any mouse button (not just primary) commits pending keystroke holds.
    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::ChordPress { ks, .. },
            EngineOutput::KeyRaw { key: k1, down: true },
            EngineOutput::KeyRaw { key: k2, down: false },
            EngineOutput::KeyRaw { key: ctrl, down: false },
        ] if ks.mods.is_empty()
            && ks.key == KeyCode::KEY_LEFTCTRL
            && *k1 == KeyCode::BTN_SIDE
            && *k2 == KeyCode::BTN_SIDE
            && *ctrl == KeyCode::KEY_LEFTCTRL
    ));
}

#[test]
fn mouse_button_commits_layer_trigger_enabling_layer_mapping() {
    // Layer trigger still in WaitingDecision when the mouse button is
    // pressed. commit_waiting_decisions fires first; should_handle_mouse_button
    // then finds the mapping in the now-active layer and routes the button
    // through on_press rather than the fast path.
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
        hold_action: ActionSpec::Swallow,
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut sp = crate::mapper::config::LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    sp.keys
        .insert("MouseBack".into(), Some("BrowserBack".into()));
    cfg.layer_keymaps.insert("sp".into(), sp);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    assert!(out.is_empty()); // WaitingDecision, no output yet

    // BTN_SIDE pressed while Space is still in WaitingDecision.
    // commit_waiting_decisions commits Space (layer activates), then
    // should_handle_mouse_button sees the layer mapping and routes
    // BTN_SIDE through on_press → BrowserBack keystroke fires.
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::ChordPress { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_BACK
    ));

    out.clear();
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::KEY_SPACE,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    // BTN_SIDE release emits the mapped key (KEY_BACK) release.
    // Space hold release is silent (Swallow hold mode).
    assert!(matches!(
        out.as_slice(),
        [EngineOutput::KeyRaw { key: back, down: false }] if *back == KeyCode::KEY_BACK
    ));
}

#[test]
fn extra_mouse_button_can_be_rule_trigger() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "MouseBack".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("BrowserBack".into()),
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

    engine.handle_key_event(KeyCode::BTN_SIDE, true, now, &mut out);
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [EngineOutput::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_BACK
    ));
}

#[test]
fn primary_mouse_button_can_be_layer_mapping() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Space".into(),
        layer_id: "mouse".into(),
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
    let mut mouse = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    mouse.keys.insert("MouseLeft".into(), Some("Escape".into()));
    cfg.layer_keymaps.insert("mouse".into(), mouse);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    engine.handle_key_event(
        KeyCode::BTN_LEFT,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::BTN_LEFT,
        false,
        now + Duration::from_millis(271),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            EngineOutput::ChordPress { ks: press, .. },
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_ESC,
                down: false,
            }
        ] if press.mods.is_empty() && press.key == KeyCode::KEY_ESC
    ));
}

#[test]
fn explicit_null_mouse_key_in_layer_keymap_swallows_key() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: "Space".into(),
        layer_id: "mouse".into(),
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
    let mut mouse = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    mouse.keys.insert("MouseBack".into(), None);
    cfg.layer_keymaps.insert("mouse".into(), mouse);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle_key_event(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    out.clear();
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    engine.handle_key_event(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(271),
        &mut out,
    );

    assert!(out.is_empty());
}
