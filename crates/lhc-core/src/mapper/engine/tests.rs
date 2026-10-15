//! Regression tests for mod.

use super::*;
use crate::mapper::config::{
    ActionSpec, AppConfig, Command, LayerKeymap, Macro, MacroStep, Rule, Settings, TapDecision,
};
use evdev::KeyCode;
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn config_with_rule(rule: serde_json::Value) -> AppConfig {
    serde_json::from_value(serde_json::json!({"rules": [rule]})).unwrap()
}

#[test]
fn unassigned_chord_ignores_double_tap_and_preserves_normal_keys() {
    for extra in ["", "text:double"] {
        let cfg = config_with_rule(
            serde_json::json!({"key": "ShiftLeft+ControlLeft", "doubleTapAction": extra}),
        );
        let engine = Engine::new(&cfg);
        assert!(engine.chord_rules.is_empty());
    }
}

#[test]
fn long_hold_runs_once_at_default_deadline_without_native_key() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyA", "longHoldAction": "text:done"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_A, true, now, &mut out);
    assert_eq!(engine.next_deadline(now), Some(Duration::from_secs(1)));
    engine.tick(now + Duration::from_millis(999), &mut out);
    assert!(out.is_empty());
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(matches!(out.as_slice(), [Out::Literal(text)] if text == "done"));
    out.clear();
    engine.handle(KeyCode::KEY_A, true, now + Duration::from_secs(2), &mut out);
    engine.tick(now + Duration::from_secs(3), &mut out);
    engine.handle(
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
        engine.handle(KeyCode::KEY_A, true, now, &mut out);
        engine.handle(
            KeyCode::KEY_B,
            true,
            now + Duration::from_millis(50),
            &mut out,
        );
        engine.handle(
            KeyCode::KEY_B,
            false,
            now + Duration::from_millis(100),
            &mut out,
        );
        assert!(matches!(
            out.as_slice(),
            [
                Out::KeyRaw {
                    key: KeyCode::KEY_B,
                    down: true
                },
                Out::KeyRaw {
                    key: KeyCode::KEY_B,
                    down: false
                }
            ]
        ));
        out.clear();
        engine.handle(
            KeyCode::KEY_A,
            false,
            now + Duration::from_millis(500),
            &mut out,
        );
        engine.tick(now + Duration::from_secs(2), &mut out);
        assert!(matches!(out.as_slice(), [Out::Stroke {ks, ..}] if ks.key == KeyCode::KEY_ESC));
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
    engine.handle(KeyCode::KEY_A, true, now, &mut out);
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(out.is_empty());
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(1500),
        &mut out,
    );
    assert!(matches!(out.as_slice(), [Out::Literal(text)] if text == "done"));
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
            assert_eq!(engine.active_layers, vec!["nav"]);
            engine.handle(release, false, now + Duration::from_millis(20), &mut out);
            assert!(engine.active_layers.is_empty());
            let other = if release == first { second } else { first };
            engine.handle(other, false, now + Duration::from_millis(30), &mut out);
            assert!(engine.emitted.is_empty());
            assert!(engine.mod_refs.values().all(|count| *count == 0));
            assert!(engine.chord_consumed.is_empty());
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
    assert!(engine.active_chords.is_empty());
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
fn explicit_hold_behavior_prevents_layer_and_delayed_action_from_running_together() {
    for mode in ["none", "layer", "action"] {
        let cfg = config_with_rule(
            serde_json::json!({"key": "KeyA", "holdBehavior": mode, "layerId": "nav", "tapAction": null, "longHoldAction": "text:done", "holdAction": "ControlLeft"}),
        );
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.handle(KeyCode::KEY_A, true, now, &mut out);
        engine.tick(now + Duration::from_secs(1), &mut out);
        assert_eq!(!engine.active_layers.is_empty(), mode == "layer");
        assert_eq!(
            out.iter().any(|event| matches!(event, Out::Literal(_))),
            mode == "action"
        );
        if mode == "action" {
            assert!(engine.emitted.is_empty());
        }
        engine.handle(
            KeyCode::KEY_A,
            false,
            now + Duration::from_secs(2),
            &mut out,
        );
        assert!(engine.emitted.is_empty());
    }
}

fn empty_cfg() -> AppConfig {
    AppConfig {
        rules: Vec::new(),
        layer_keymaps: HashMap::new(),
        macros: Vec::new(),
        commands: Vec::new(),
        settings: Settings::default(),
    }
}

#[test]
fn text_actions_preserve_whitespace_in_remote_rules_and_macros() {
    for text in [" ", "   ", "  a  b  ", "one\ntwo\n", "\t\n", "Привет 👋 "] {
        let action = format!("text:{text}");
        let cfg = config_with_rule(serde_json::json!({
            "key": "KeyA", "tapAction": action,
        }));
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        engine.execute_remote(&action, &mut out);
        assert!(matches!(out.as_slice(), [Out::Literal(value)] if value == text));
        out.clear();
        let now = Instant::now();
        engine.handle(KeyCode::KEY_A, true, now, &mut out);
        engine.handle(
            KeyCode::KEY_A,
            false,
            now + Duration::from_millis(1),
            &mut out,
        );
        engine.tick(now + Duration::from_secs(1), &mut out);
        assert!(
            out.iter()
                .any(|event| matches!(event, Out::Literal(value) if value == text))
        );

        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "text".into(),
            steps: vec![MacroStep { action }],
            step_pause_ms: Some(0),
            modifier_delay_ms: Some(0),
        });
        let mut engine = Engine::new(&cfg);
        out.clear();
        engine.execute_remote("macro:text", &mut out);
        engine.tick(Instant::now(), &mut out);
        assert!(matches!(out.as_slice(), [Out::Literal(value)] if value == text));
    }
}

#[test]
fn macro_waits_for_text_completion_before_next_step_and_pause() {
    for pause in [false, true] {
        let mut cfg = empty_cfg();
        let mut steps = vec![MacroStep {
            action: "text:hello\n ".into(),
        }];
        if pause {
            steps.push(MacroStep {
                action: "pause:100".into(),
            });
        }
        steps.push(MacroStep {
            action: "Enter".into(),
        });
        cfg.macros.push(Macro {
            id: "text".into(),
            steps,
            step_pause_ms: Some(10),
            modifier_delay_ms: Some(0),
        });
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        engine.execute_remote("macro:text", &mut out);
        let now = Instant::now();
        engine.tick(now, &mut out);
        assert!(matches!(out.as_slice(), [Out::Literal(_)]));
        out.clear();
        let completed = now + Duration::from_secs(5);
        engine.tick(completed, &mut out);
        assert!(out.is_empty());
        assert_eq!(engine.next_deadline(completed), None);
        engine.set_text_pending(false, completed);
        engine.tick(completed, &mut out);
        assert!(out.is_empty());
        engine.tick(completed + Duration::from_millis(9), &mut out);
        assert!(out.is_empty());
        engine.tick(
            completed + Duration::from_millis(if pause { 110 } else { 10 }),
            &mut out,
        );
        assert!(out.iter().any(|event| matches!(
            event,
            Out::KeyRaw {
                key: KeyCode::KEY_ENTER,
                down: true
            }
        )));
    }
}

#[test]
fn pending_text_does_not_keep_macro_modifiers_pressed() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "copy".into(),
        steps: vec![
            MacroStep {
                action: "Ctrl+KeyC".into(),
            },
            MacroStep {
                action: "Enter".into(),
            },
        ],
        step_pause_ms: Some(10),
        modifier_delay_ms: Some(10),
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    engine.execute_remote("macro:copy", &mut out);
    let now = Instant::now();
    engine.tick(now, &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: true
        }]
    ));
    out.clear();
    engine.set_text_pending(true, now);
    engine.tick(now + Duration::from_millis(10), &mut out);
    out.clear();
    engine.tick(now + Duration::from_millis(20), &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: false
        }]
    ));
    out.clear();
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(out.is_empty());
}

#[test]
fn unrelated_text_does_not_reset_an_existing_macro_pause() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "pause".into(),
        steps: vec![
            MacroStep {
                action: "pause:100".into(),
            },
            MacroStep {
                action: "Enter".into(),
            },
        ],
        step_pause_ms: Some(10),
        modifier_delay_ms: Some(0),
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    engine.execute_remote("macro:pause", &mut out);
    let now = Instant::now();
    engine.tick(now, &mut out);
    engine.set_text_pending(true, now + Duration::from_millis(20));
    engine.set_text_pending(false, now + Duration::from_millis(30));
    engine.tick(now + Duration::from_millis(99), &mut out);
    assert!(out.is_empty());
    engine.tick(now + Duration::from_millis(100), &mut out);
    assert!(out.iter().any(|event| matches!(
        event,
        Out::KeyRaw {
            key: KeyCode::KEY_ENTER,
            down: true
        }
    )));
}

#[test]
fn remote_execute_macro_uses_current_macro_table() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "hello".into(),
        steps: vec![MacroStep {
            action: "KeyH".into(),
        }],
        step_pause_ms: None,
        modifier_delay_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();

    engine.execute_remote("macro:hello", &mut out);
    assert!(out.is_empty());

    engine.tick(Instant::now() + Duration::from_millis(1), &mut out);
    assert!(matches!(
        out.as_slice(),
        [
            Out::KeyRaw {
                key: KeyCode::KEY_H,
                down: true
            },
            Out::KeyRaw {
                key: KeyCode::KEY_H,
                down: false
            },
        ]
    ));
}

#[test]
fn macro_pause_step_delays_following_steps() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "hello".into(),
        steps: vec![
            MacroStep {
                action: "KeyH".into(),
            },
            MacroStep {
                action: "pause:100".into(),
            },
            MacroStep {
                action: "KeyI".into(),
            },
        ],
        step_pause_ms: Some(1),
        modifier_delay_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.execute_remote("macro:hello", &mut out);
    engine.tick(now + Duration::from_millis(1), &mut out);
    assert!(matches!(
        out.as_slice(),
        [
            Out::KeyRaw {
                key: KeyCode::KEY_H,
                down: true
            },
            Out::KeyRaw {
                key: KeyCode::KEY_H,
                down: false
            },
        ]
    ));

    out.clear();
    engine.tick(now + Duration::from_millis(1), &mut out);
    assert!(out.is_empty());
    engine.tick(now + Duration::from_millis(100), &mut out);
    assert!(out.is_empty());
    engine.tick(now + Duration::from_millis(101), &mut out);
    assert!(matches!(
        out.as_slice(),
        [
            Out::KeyRaw {
                key: KeyCode::KEY_I,
                down: true
            },
            Out::KeyRaw {
                key: KeyCode::KEY_I,
                down: false
            },
        ]
    ));
}

#[test]
fn disabled_commands_are_skipped_in_macros_and_preserve_other_keys() {
    let mut cfg = empty_cfg();
    cfg.commands.push(Command {
        id: "blocked".into(),
        linux: "printf hello".into(),
        working_directory: None,
    });
    cfg.macros.push(Macro {
        id: "sequence".into(),
        steps: vec![
            MacroStep {
                action: "KeyA".into(),
            },
            MacroStep {
                action: "cmd:blocked".into(),
            },
        ],
        step_pause_ms: None,
        modifier_delay_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    engine.execute_remote("macro:sequence", &mut out);
    engine.execute_remote("cmd:blocked", &mut out);
    engine.tick(Instant::now() + Duration::from_millis(1), &mut out);
    assert!(out.iter().all(|item| !matches!(item, Out::RunCommand(_))));
    assert!(out.iter().any(|item| matches!(
        item,
        Out::KeyRaw {
            key: KeyCode::KEY_A,
            ..
        }
    )));
    out.clear();
    engine.execute_remote("cmd:blocked", &mut out);
    assert!(out.is_empty());
    engine.execute_remote("KeyB", &mut out);
    assert!(!out.is_empty());
}

#[test]
fn command_invocations_use_configured_directory_and_timeout() {
    let mut cfg = empty_cfg();
    cfg.commands.push(Command {
        id: "hello".into(),
        linux: "pwd".into(),
        working_directory: Some("~/Documents".into()),
    });
    cfg.settings.commands_enabled = true;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    engine.execute_remote("cmd:hello", &mut out);
    assert!(matches!(out.as_slice(), [Out::RunCommand(cmd)]
        if cmd.working_directory.as_deref() == Some("~/Documents")));
}

#[test]
fn remote_execute_command_uses_current_command_table() {
    let mut cfg = empty_cfg();
    cfg.commands.push(Command {
        id: "play".into(),
        linux: "playerctl play-pause".into(),
        working_directory: None,
    });
    cfg.settings.commands_enabled = true;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();

    engine.execute_remote("cmd:play", &mut out);

    assert!(matches!(
        out.as_slice(),
        [Out::RunCommand(cmd)]
            if cmd.program == "sh"
                && cmd.args == vec!["-lc".to_string(), "playerctl play-pause".to_string()]
    ));
}

#[test]
fn sel_layer_q_emits_ctrl_key_z() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Tab".into(),
        layer_id: "sel".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_TAB, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [Out::ChordPress { ks, .. }]
            if ks.mods == vec![KeyCode::KEY_LEFTCTRL] && ks.key == KeyCode::KEY_Z
    ));
}

#[test]
fn active_layer_bypasses_rule_for_same_key() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Space".into(),
        layer_id: "space".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Tab".into(),
        layer_id: "sel".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [Out::ChordPress { ks, .. }]
            if ks.mods.is_empty() && ks.key == KeyCode::KEY_ESC
    ));
}

#[test]
fn unmapped_key_in_active_layer_passthroughs() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Space".into(),
        layer_id: "space".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::KeyRaw {
                key: KeyCode::KEY_A,
                down: true
            },
            Out::KeyRaw {
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Space".into(),
        layer_id: "space".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("AltLeft".into()),
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks: alt, .. },
            Out::ChordPress { ks: tab, .. },
        ] if alt.mods.is_empty()
            && alt.key == KeyCode::KEY_LEFTALT
            && tab.mods.is_empty()
            && tab.key == KeyCode::KEY_TAB
    ));
}

/// Builds a `win` layer whose trigger uses a lazy hold: Alt is only
/// materialised for `Tab`. Every other layer key must run without Alt.
fn lazy_alt_win_cfg() -> AppConfig {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("AltLeft".into()),
        isolate: String::new(),
        hold_for: "Tab".into(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut win = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    win.keys.insert("Tab".into(), Some("Tab".into()));
    // A non-whitelisted key that emits a visible stroke.
    win.keys.insert("KeyC".into(), Some("Escape".into()));
    cfg.layer_keymaps.insert("win".into(), win);
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    cfg
}

#[test]
fn lazy_hold_does_not_materialize_alt_for_non_whitelist_key() {
    // Pressing a non-whitelisted layer key (KeyC) must never leak Alt to
    // the app — no lone Alt tap, so GTK/Firefox never opens its menu bar.
    let mut engine = Engine::new(&lazy_alt_win_cfg());
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_C,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_C,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTALT,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );

    let touches_alt = out.iter().any(|o| match o {
        Out::KeyRaw { key, .. } => *key == KeyCode::KEY_LEFTALT,
        Out::ChordPress { ks, .. } => {
            ks.key == KeyCode::KEY_LEFTALT || ks.mods.contains(&KeyCode::KEY_LEFTALT)
        }
        Out::ChordRelease { key, mods, .. } => {
            *key == KeyCode::KEY_LEFTALT || mods.contains(&KeyCode::KEY_LEFTALT)
        }
        _ => false,
    });
    assert!(!touches_alt, "Alt must never reach the app");
}

#[test]
fn lazy_hold_materializes_alt_for_whitelist_key() {
    // Tab is whitelisted → Alt materialises just in time (Alt+Tab) and is
    // released when the layer ends.
    let mut engine = Engine::new(&lazy_alt_win_cfg());
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks: alt, .. },
            Out::ChordPress { ks: tab, .. },
        ] if alt.mods.is_empty()
            && alt.key == KeyCode::KEY_LEFTALT
            && tab.mods.is_empty()
            && tab.key == KeyCode::KEY_TAB
    ));

    out.clear();
    engine.handle(
        KeyCode::KEY_TAB,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTALT,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );

    // Alt is released exactly once when the layer ends.
    let alt_releases = out
        .iter()
        .filter(|o| matches!(o, Out::KeyRaw { key, down: false } if *key == KeyCode::KEY_LEFTALT))
        .count();
    assert_eq!(alt_releases, 1, "Alt must be released once");
}

#[test]
fn layer_only_rule_is_not_treated_as_passthrough() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: "KeyW".into(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_TAB,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [Out::ChordPress { ks, .. }]
            if ks.mods.is_empty() && ks.key == KeyCode::KEY_ESC
    ));
}

// Serialise all tests that mutate the shared `active_window::CACHED`
// so they don't race when cargo runs the suite in parallel.
static APPS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn rule_with_apps(whitelist: Option<Vec<String>>, blacklist: Option<Vec<String>>) -> RuleEntry {
    RuleEntry {
        trigger_keys: vec![],
        tap: TapMode::Native,
        layer_id: None,
        hold: HoldMode::Swallow,
        isolate_keys: Vec::new(),
        whitelist_keys: Vec::new(),
        long_hold: None,
        long_hold_timeout: Duration::from_secs(1),
        double_tap: None,
        hold_timeout: Duration::from_millis(200),
        double_tap_window: Duration::from_millis(200),
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: whitelist,
        condition_apps_blacklist: blacklist,
    }
}

#[test]
fn apps_whitelist_blocks_when_no_active_window() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(None);
    let rule = rule_with_apps(Some(vec!["firefox".into()]), None);
    assert!(!rule_passes_apps(&rule));
}

#[test]
fn apps_whitelist_passes_when_title_matches() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(Some(crate::runtime_state::ActiveWindow {
        process_name: None,
        fullscreen: None,
        title: "Mozilla Firefox".into(),
        app_id: "navigator".into(),
    }));
    let rule = rule_with_apps(Some(vec!["title:firefox".into()]), None);
    assert!(rule_passes_apps(&rule));
    let rule = rule_with_apps(Some(vec!["firefox".into()]), None);
    assert!(!rule_passes_apps(&rule));
    crate::runtime_state::set_active_window(None);
}

#[test]
fn apps_whitelist_passes_when_app_id_matches_case_insensitive() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(Some(crate::runtime_state::ActiveWindow {
        process_name: None,
        fullscreen: None,
        title: "Library".into(),
        app_id: "Steam".into(),
    }));
    let rule = rule_with_apps(Some(vec!["steam".into()]), None);
    assert!(rule_passes_apps(&rule));
    crate::runtime_state::set_active_window(None);
}

#[test]
fn apps_blacklist_blocks_even_when_whitelist_passes() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(Some(crate::runtime_state::ActiveWindow {
        process_name: None,
        fullscreen: None,
        title: "Editor — secret".into(),
        app_id: "editor".into(),
    }));
    let rule = rule_with_apps(
        Some(vec!["editor".into()]),
        Some(vec!["title:secret".into()]),
    );
    assert!(!rule_passes_apps(&rule));
    crate::runtime_state::set_active_window(None);
}

#[test]
fn apps_no_lists_always_passes() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(None);
    let rule = rule_with_apps(None, None);
    assert!(rule_passes_apps(&rule));
}

#[test]
fn undetected_game_mode_counts_as_off() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_game_mode_active(false);
    let mut rule = RuleEntry {
        trigger_keys: vec![],
        tap: TapMode::Native,
        layer_id: None,
        hold: HoldMode::Swallow,
        isolate_keys: Vec::new(),
        whitelist_keys: Vec::new(),
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

    assert!(!rule_passes_conditions(&rule));
    rule.condition_game_mode = Some(GameModeCondition::Off);
    assert!(rule_passes_conditions(&rule));
}

#[test]
fn apps_blacklist_blocks_when_no_active_window() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(None);
    let rule = rule_with_apps(None, Some(vec!["secret".into()]));
    assert!(!rule_passes_apps(&rule));
}

#[test]
fn game_mode_condition_passes_when_cached_state_matches() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_game_mode_active(true);
    let rule = RuleEntry {
        trigger_keys: vec![],
        tap: TapMode::Native,
        layer_id: None,
        hold: HoldMode::Swallow,
        isolate_keys: Vec::new(),
        whitelist_keys: Vec::new(),
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

    assert!(rule_passes_conditions(&rule));
    crate::runtime_state::set_game_mode_active(false);
}

#[test]
fn duplicate_key_rules_use_first_matching_condition() {
    let _g = APPS_TEST_LOCK.lock().unwrap();
    crate::runtime_state::set_active_window(None);

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
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });

    let mut engine = Engine::new(&cfg);
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
    crate::runtime_state::set_active_window(None);
}

fn raw_keys(out: &[Out]) -> Vec<(KeyCode, bool)> {
    out.iter()
        .filter_map(|event| match event {
            Out::KeyRaw { key, down } => Some((*key, *down)),
            _ => None,
        })
        .collect()
}

fn fired_text(out: &[Out]) -> bool {
    out.iter().any(|event| matches!(event, Out::Literal(_)))
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
        assert!(engine.combo_buffer.is_empty());
        assert!(engine.chord_consumed.is_empty());
    }
}

#[test]
fn combo_window_expiry_types_the_member_and_blocks_the_chord() {
    let cfg = config_with_rule(serde_json::json!({"key": "KeyJ+KeyK", "tapAction": "text:esc"}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    assert_eq!(engine.next_deadline(now), Some(engine.combo_term));
    engine.tick(now + engine.combo_term, &mut out);
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
    assert_eq!(engine.active_layers, vec!["nav"]);
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
    assert!(engine.combo_buffer.is_empty());
}

#[test]
fn layer_presses_count_only_keys_pressed_inside_a_layer() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({"rules": [
        {"key": "Space", "layerId": "nav", "tapAction": "Space"},
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_A, true, now, &mut out);
    engine.handle(KeyCode::KEY_A, false, now, &mut out);
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    assert_eq!(engine.layer_presses(), 0);
    // The press that resolves the layer key as a hold is used inside it.
    engine.handle(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_J,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert_eq!(engine.top_layer(), Some("nav"));
    assert_eq!(engine.layer_presses(), 1);
    engine.handle(
        KeyCode::KEY_SPACE,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert_eq!(engine.top_layer(), None);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(40),
        &mut out,
    );
    assert_eq!(engine.layer_presses(), 1);
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
fn mouse_button_commits_pending_hold() {
    // All pending WaitingDecision holds commit on mouse button press,
    // regardless of hold mode (Keystroke, Native, or Swallow).
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "ShiftLeft".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("ControlLeft".into()),
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    // No hold committed yet — Shift is still in WaitingDecision.
    assert!(out.is_empty());

    // LMB click commits the pending hold (ControlLeft) so the physical
    // mouse event reaches the OS with the modifier active.
    engine.handle(
        KeyCode::BTN_LEFT,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::BTN_LEFT,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );

    // Releasing Shift releases the committed hold (ControlLeft).
    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks, .. },
            Out::KeyRaw { key: k1, down: true },
            Out::KeyRaw { key: k2, down: false },
            Out::KeyRaw { key: ctrl, down: false },
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "ShiftLeft".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("ControlLeft".into()),
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle(
        KeyCode::BTN_SIDE,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );

    // Any mouse button (not just primary) commits pending keystroke holds.
    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks, .. },
            Out::KeyRaw { key: k1, down: true },
            Out::KeyRaw { key: k2, down: false },
            Out::KeyRaw { key: ctrl, down: false },
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Space".into(),
        layer_id: "sp".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Swallow,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut sp = crate::mapper::config::LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    sp.keys
        .insert("MouseSide".into(), Some("BrowserBack".into()));
    cfg.layer_keymaps.insert("sp".into(), sp);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    assert!(out.is_empty()); // WaitingDecision, no output yet

    // BTN_SIDE pressed while Space is still in WaitingDecision.
    // commit_waiting_decisions commits Space (layer activates), then
    // should_handle_mouse_button sees the layer mapping and routes
    // BTN_SIDE through on_press → BrowserBack keystroke fires.
    engine.handle(
        KeyCode::BTN_SIDE,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::ChordPress { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_BACK
    ));

    out.clear();
    engine.handle(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_SPACE,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    // BTN_SIDE release emits the mapped key (KEY_BACK) release.
    // Space hold release is silent (Swallow hold mode).
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw { key: back, down: false }] if *back == KeyCode::KEY_BACK
    ));
}

#[test]
fn extra_mouse_button_can_be_rule_trigger() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "MouseSide".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("BrowserBack".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::BTN_SIDE, true, now, &mut out);
    engine.handle(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_BACK
    ));
}

#[test]
fn primary_mouse_button_can_be_layer_mapping() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Space".into(),
        layer_id: "mouse".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    engine.handle(
        KeyCode::BTN_LEFT,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    engine.handle(
        KeyCode::BTN_LEFT,
        false,
        now + Duration::from_millis(271),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks: press, .. },
            Out::KeyRaw {
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "Space".into(),
        layer_id: "mouse".into(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut mouse = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    mouse.keys.insert("MouseSide".into(), None);
    cfg.layer_keymaps.insert("mouse".into(), mouse);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    out.clear();
    engine.handle(
        KeyCode::BTN_SIDE,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    engine.handle(
        KeyCode::BTN_SIDE,
        false,
        now + Duration::from_millis(271),
        &mut out,
    );

    assert!(out.is_empty());
}

#[test]
fn isolate_suppresses_and_restores_layer_trigger_modifier() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("AltLeft".into()),
        isolate: "KeyW".into(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut win = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    win.keys.insert("KeyW".into(), Some("Ctrl+KeyA".into()));
    cfg.layer_keymaps.insert("win".into(), win);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_W,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_W,
        false,
        now + Duration::from_millis(11),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTALT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks: alt_hold, .. },
            Out::KeyRaw { key: release_alt_1, down: false },
            Out::ChordPress { ks: ctrl_a, .. },
            Out::KeyRaw { key: restore_alt, down: true },
            Out::ChordRelease { key: key_a, mods: ctrl_release, .. },
            Out::KeyRaw { key: release_alt_2, down: false },
        ] if alt_hold.mods.is_empty()
            && alt_hold.key == KeyCode::KEY_LEFTALT
            && *release_alt_1 == KeyCode::KEY_LEFTALT
            && ctrl_a.mods.as_slice() == [KeyCode::KEY_LEFTCTRL]
            && ctrl_a.key == KeyCode::KEY_A
            && *restore_alt == KeyCode::KEY_LEFTALT
            && *key_a == KeyCode::KEY_A
            && ctrl_release.as_slice() == [KeyCode::KEY_LEFTCTRL]
            && *release_alt_2 == KeyCode::KEY_LEFTALT
    ));
}

#[test]
fn tap_on_release_fires_tap_action() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "CapsLock".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: Some(200),
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_CAPSLOCK, true, now, &mut out);
    assert!(out.is_empty());

    engine.handle(
        KeyCode::KEY_CAPSLOCK,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_ESC
    ));
}

#[test]
fn hold_timeout_commits_hold_via_tick() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "AltLeft".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("ControlLeft".into()),
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: Some(50),
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    assert!(out.is_empty());

    engine.tick(now + Duration::from_millis(60), &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::ChordPress { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_LEFTCTRL
    ));
}

#[test]
fn double_tap_fires_double_tap_action() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: "KeyB".into(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    // First tap
    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty()); // WaitingSecond

    // Second tap within window
    engine.handle(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_B
    ));

    // Release of second tap must be silent
    out.clear();
    engine.handle(
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: "KeyB".into(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty());

    engine.tick(now + Duration::from_millis(250), &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_A
    ));
}

#[test]
fn double_tap_flush_on_other_key_press() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: "KeyB".into(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty());

    // Another key press flushes the deferred tap before processing
    engine.handle(
        KeyCode::KEY_W,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert_eq!(out.len(), 2);
    assert!(matches!(
        out[0],
        Out::Stroke { ref ks, .. } if ks.mods.is_empty() && ks.key == KeyCode::KEY_A
    ));
    assert!(matches!(
        out[1],
        Out::KeyRaw { key, down: true } if key == KeyCode::KEY_W
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("KeyA".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: "KeyB".into(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty()); // WaitingSecond, deadline = now + 210ms

    // Second press well past the 200 ms window.
    engine.handle(
        KeyCode::KEY_Q,
        true,
        now + Duration::from_millis(500),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_A
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

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw { key, down: true }] if *key == KeyCode::KEY_LEFTSHIFT
    ));
    out.clear();

    engine.handle(
        KeyCode::KEY_F,
        true,
        now + Duration::from_millis(5),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_J,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(15),
        &mut out,
    );
    // Everything deferred behind F's decision.
    assert!(out.is_empty());

    // F released first → F is a tap; the buffer replays in physical order.
    engine.handle(
        KeyCode::KEY_F,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [
                Out::Stroke { ks, .. },
                Out::KeyRaw { key: j, down: true },
                Out::KeyRaw { key: sh, down: false },
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
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    out.clear();

    engine.shutdown(&mut out);
    // Should emit: KeyC release, Ctrl release (ReleaseMods)
    assert!(
        out.iter()
            .any(|o| matches!(o, Out::KeyRaw { key, down: false } if *key == KeyCode::KEY_C))
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, Out::ReleaseMods(mods) if mods.contains(&KeyCode::KEY_LEFTCTRL)))
    );
    assert!(engine.active_layers.is_empty());
    assert!(engine.pending.is_empty());
}

#[test]
fn layer_missing_key_is_transparent() {
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
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(260), &mut out);
    out.clear();

    // KeyB without a mapping should passthrough
    engine.handle(
        KeyCode::KEY_B,
        true,
        now + Duration::from_millis(270),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw { key, down: true }] if *key == KeyCode::KEY_B
    ));
}

#[test]
fn mod_refs_shared_by_two_keys() {
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
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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

    // Release KeyA — Ctrl should NOT be released because KeyB still holds it
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(290),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, Out::ChordRelease { key, .. } if *key == KeyCode::KEY_C))
    );
    assert!(!out.iter().any(
        |o| matches!(o, Out::ChordRelease { mods, .. } if mods.contains(&KeyCode::KEY_LEFTCTRL))
    ));

    out.clear();
    // Release KeyB — now Ctrl should be released
    engine.handle(
        KeyCode::KEY_B,
        false,
        now + Duration::from_millis(300),
        &mut out,
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, Out::ChordRelease { key, .. } if *key == KeyCode::KEY_V))
    );
    assert!(out.iter().any(
        |o| matches!(o, Out::ChordRelease { mods, .. } if mods.contains(&KeyCode::KEY_LEFTCTRL))
    ));
}

#[test]
fn unknown_key_on_release_passes_through() {
    let mut engine = Engine::new(&empty_cfg());
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_A, false, now, &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw { key, down: false }] if *key == KeyCode::KEY_A
    ));
}

#[test]
fn commit_waiting_decisions_on_other_key_press() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "ShiftLeft".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("Escape".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("ControlLeft".into()),
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: Some(200),
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    // Asserts the interrupt-on-press semantics.
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    assert!(out.is_empty());

    // Another key press commits the pending hold
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(matches!(
        out.as_slice(),
        [
            Out::ChordPress { ks, .. },
            Out::KeyRaw { key: k1, down: true },
        ] if ks.mods.is_empty() && ks.key == KeyCode::KEY_LEFTCTRL && *k1 == KeyCode::KEY_A
    ));
}

/// A permissive-mode mod-tap rule with a 200 ms hold timeout.
fn tap_hold_rule(key: &str, tap: ActionSpec, hold: ActionSpec) -> Rule {
    Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: key.into(),
        layer_id: String::new(),
        tap_action: tap,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: hold,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: Some(200),
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    }
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

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    // Nothing emitted yet — A is buffered behind Shift's decision.
    assert!(out.is_empty());

    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [
                Out::Stroke { ks: esc, .. },
                Out::KeyRaw { key: a, down: true },
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

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );

    assert!(
        matches!(
            out.as_slice(),
            [
                Out::ChordPress { ks: ctrl, .. },
                Out::KeyRaw { key: a_down, down: true },
                Out::KeyRaw { key: a_up, down: false },
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

    engine.handle(KeyCode::KEY_CAPSLOCK, true, now, &mut out);
    engine.handle(
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
            [Out::ChordPress { ks, .. }] if ks.mods.is_empty() && ks.key == KeyCode::KEY_LEFT
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

    engine.handle(KeyCode::KEY_A, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(20),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );

    // No ControlLeft hold anywhere; Shift resolved as a tap (Escape).
    assert!(
        !out.iter().any(|o| matches!(o, Out::ChordPress { .. })),
        "hold must not commit"
    );
    assert!(
        out.iter()
            .any(|o| matches!(o, Out::Stroke { ks, .. } if ks.key == KeyCode::KEY_ESC)),
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

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_CAPSLOCK,
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
    // Shift tapped; Caps now deciding after replay.
    assert!(
        matches!(
            out.as_slice(),
            [Out::Stroke { ks, .. }] if ks.key == KeyCode::KEY_ESC
        ),
        "expected only Shift's Escape tap so far"
    );

    out.clear();
    engine.handle(
        KeyCode::KEY_CAPSLOCK,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [Out::Stroke { ks, .. }] if ks.key == KeyCode::KEY_TAB
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

    engine.handle(KeyCode::KEY_LEFTSHIFT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_A,
        true,
        now + Duration::from_millis(30),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_LEFTSHIFT,
        false,
        now + Duration::from_millis(60),
        &mut out,
    );

    assert!(
        matches!(
            out.as_slice(),
            [
                Out::Stroke { ks: copy, .. },
                Out::KeyRaw { key: a, down: true },
            ] if copy.mods == vec![KeyCode::KEY_LEFTCTRL]
                && copy.key == KeyCode::KEY_C
                && *a == KeyCode::KEY_A
        ),
        "expected Ctrl+C tap then replayed A"
    );
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
    engine.handle(KeyCode::KEY_CAPSLOCK, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_CAPSLOCK,
        false,
        now + Duration::from_millis(150),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [Out::Stroke { ks, .. }]
                if ks.mods == vec![KeyCode::KEY_LEFTCTRL] && ks.key == KeyCode::KEY_V
        ),
        "expected Ctrl+V tap"
    );

    // Nested layer use (J pressed and released while Caps held) → layer.
    out.clear();
    let t2 = now + Duration::from_millis(500);
    engine.handle(KeyCode::KEY_CAPSLOCK, true, t2, &mut out);
    engine.handle(
        KeyCode::KEY_J,
        true,
        t2 + Duration::from_millis(30),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_J,
        false,
        t2 + Duration::from_millis(60),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_CAPSLOCK,
        false,
        t2 + Duration::from_millis(90),
        &mut out,
    );
    assert!(
        matches!(
            out.as_slice(),
            [
                Out::ChordPress { ks: left, .. },
                Out::KeyRaw { key: up, down: false },
            ] if left.mods.is_empty()
                && left.key == KeyCode::KEY_LEFT
                && *up == KeyCode::KEY_LEFT
        ),
        "expected ArrowLeft press+release from the nav layer"
    );
}

#[test]
fn builder_skips_unknown_rule_key() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "UnknownKey".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Native,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let engine = Engine::new(&cfg);
    assert!(!engine.rules.contains_key(&KeyCode::KEY_A));
}

#[test]
fn builder_skips_primary_mouse_buttons_as_triggers() {
    let mut cfg = empty_cfg();
    for key in ["MouseLeft", "MouseRight", "MouseMiddle"] {
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            key: key.into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("Escape".into()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            isolate: String::new(),
            hold_for: String::new(),
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
    }
    let engine = Engine::new(&cfg);
    assert!(!engine.rules.contains_key(&KeyCode::BTN_LEFT));
    assert!(!engine.rules.contains_key(&KeyCode::BTN_RIGHT));
    assert!(!engine.rules.contains_key(&KeyCode::BTN_MIDDLE));
}

#[test]
fn builder_unknown_tap_action_falls_back_to_swallow() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("BadActionSyntax!!!".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Swallow,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(
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
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("macro:copyLine".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Native,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    // Tap fires the macro on release (no double-tap)
    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    assert!(out.is_empty());
    engine.handle(
        KeyCode::KEY_Q,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    // Macro should now be active
    assert!(engine.active_macro.is_some());
}

#[test]
fn starting_macro_mid_stroke_releases_pending_mods() {
    // A macro interrupted between its mod-press and mod-release phases
    // must release the pressed modifiers before the new macro starts.
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "a".into(),
        steps: vec![MacroStep {
            action: "Ctrl+KeyC".into(),
        }],
        step_pause_ms: Some(1),
        modifier_delay_ms: Some(5),
    });
    cfg.macros.push(Macro {
        id: "b".into(),
        steps: vec![MacroStep {
            action: "KeyH".into(),
        }],
        step_pause_ms: Some(1),
        modifier_delay_ms: Some(5),
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.execute_remote("macro:a", &mut out);
    // First tick presses Ctrl and enters the mod-delay phase.
    engine.tick(now + Duration::from_millis(1), &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: true
        }]
    ));
    out.clear();

    // Starting another macro mid-stroke must release Ctrl.
    engine.execute_remote("macro:b", &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: false
        }]
    ));
}

#[test]
fn shutdown_releases_mods_of_in_flight_macro() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "a".into(),
        steps: vec![MacroStep {
            action: "Ctrl+KeyC".into(),
        }],
        step_pause_ms: Some(1),
        modifier_delay_ms: Some(5),
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.execute_remote("macro:a", &mut out);
    engine.tick(now + Duration::from_millis(1), &mut out);
    out.clear();

    engine.shutdown(&mut out);
    assert!(out.iter().any(|o| matches!(
        o,
        Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: false
        }
    )));
    assert!(engine.active_macro.is_none());
}

#[test]
fn next_deadline_includes_macro_wake() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "a".into(),
        steps: vec![
            MacroStep {
                action: "KeyH".into(),
            },
            MacroStep {
                action: "KeyI".into(),
            },
        ],
        step_pause_ms: Some(20),
        modifier_delay_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.execute_remote("macro:a", &mut out);
    // First step fires; the next step is scheduled step_pause later —
    // the loop must be able to sleep exactly until then.
    engine.tick(now, &mut out);
    let deadline = engine.next_deadline(now).expect("macro wake deadline");
    assert!(deadline <= Duration::from_millis(20));
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
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
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
fn overlapping_isolate_keys_restore_only_after_last_release() {
    let mut cfg = empty_cfg();
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "AltLeft".into(),
        layer_id: "win".into(),
        tap_action: ActionSpec::Action("Enter".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Action("AltLeft".into()),
        isolate: "KeyW,KeyE".into(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut win = LayerKeymap {
        keys: HashMap::new(),
        ..Default::default()
    };
    win.keys.insert("KeyW".into(), Some("Ctrl+KeyA".into()));
    win.keys.insert("KeyE".into(), Some("Ctrl+KeyB".into()));
    cfg.layer_keymaps.insert("win".into(), win);
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_LEFTALT, true, now, &mut out);
    engine.handle(
        KeyCode::KEY_W,
        true,
        now + Duration::from_millis(10),
        &mut out,
    );
    engine.handle(
        KeyCode::KEY_E,
        true,
        now + Duration::from_millis(20),
        &mut out,
    );
    out.clear();

    // Releasing W must NOT restore Alt — E still isolates it.
    engine.handle(
        KeyCode::KEY_W,
        false,
        now + Duration::from_millis(30),
        &mut out,
    );
    assert!(!out.iter().any(|o| matches!(
        o,
        Out::KeyRaw {
            key: KeyCode::KEY_LEFTALT,
            down: true
        }
    )));

    out.clear();
    // Releasing E (last isolate key) restores Alt.
    engine.handle(
        KeyCode::KEY_E,
        false,
        now + Duration::from_millis(40),
        &mut out,
    );
    assert!(out.iter().any(|o| matches!(
        o,
        Out::KeyRaw {
            key: KeyCode::KEY_LEFTALT,
            down: true
        }
    )));
}

#[test]
fn empty_macro_does_not_crash() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "empty".into(),
        steps: vec![],
        step_pause_ms: None,
        modifier_delay_ms: None,
    });
    cfg.rules.push(Rule {
        enabled: true,
        condition_game_mode: None,
        condition_layouts: None,
        condition_apps_whitelist: None,
        condition_apps_blacklist: None,
        key: "KeyQ".into(),
        layer_id: String::new(),
        tap_action: ActionSpec::Action("macro:empty".into()),
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: ActionSpec::Swallow,
        isolate: String::new(),
        hold_for: String::new(),
        hold_timeout_ms: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    // Empty macro resolves to nothing → fast path Swallow+Swallow, no crash
    assert!(engine.active_macro.is_none());
}

#[test]
fn held_layer_snapshot_distinguishes_fast_represses_and_mapper_restarts() {
    let cfg =
        config_with_rule(serde_json::json!({"key": "Space", "layerId": "nav", "tapAction": null}));
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    let first = engine.held_layer_state();
    assert!(first.allows_hint("nav", first.hold_id));
    // Both events may arrive in the same fetch_key_events batch.
    engine.handle(KeyCode::KEY_SPACE, false, now, &mut out);
    assert!(engine.held_layer_state().layer.is_none());
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    let second = engine.held_layer_state();
    assert_eq!(second.layer, first.layer);
    assert!(!second.allows_hint("nav", first.hold_id));
    assert!(second.allows_hint("nav", second.hold_id));
    engine.handle(KeyCode::KEY_J, true, now, &mut out);
    engine.handle(KeyCode::KEY_J, false, now, &mut out);
    assert!(!engine.held_layer_state().allows_hint("nav", second.hold_id));
    engine.handle(KeyCode::KEY_SPACE, false, now, &mut out);
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    let third = engine.held_layer_state();
    assert!(third.allows_hint("nav", third.hold_id));
    let mut restarted = Engine::new(&cfg);
    restarted.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    assert_ne!(restarted.held_layer_state().hold_id, third.hold_id);
}

#[test]
fn nested_layers_keep_usage_suppressed_until_all_triggers_are_released() {
    let mut engine = Engine::new(&empty_cfg());
    let mut out = Vec::new();
    engine.push_layer("nav".into());
    let first = engine.held_layer_state();
    engine.handle(KeyCode::KEY_J, true, Instant::now(), &mut out);
    engine.push_layer("symbols".into());
    assert_eq!(engine.held_layer_state().hold_id, first.hold_id);
    assert!(engine.held_layer_state().used);
    engine.pop_layer("symbols");
    assert!(engine.held_layer_state().used);
    engine.pop_layer("nav");
    engine.push_layer("nav".into());
    assert!(!engine.held_layer_state().used);
    assert_ne!(engine.held_layer_state().hold_id, first.hold_id);
}

#[test]
fn failed_text_aborts_macro_before_enter() {
    let mut cfg = empty_cfg();
    cfg.macros.push(Macro {
        id: "text".into(),
        steps: vec![
            MacroStep {
                action: "text:hello".into(),
            },
            MacroStep {
                action: "Enter".into(),
            },
        ],
        step_pause_ms: Some(0),
        modifier_delay_ms: Some(0),
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    engine.execute_remote("macro:text", &mut out);
    let now = Instant::now();
    engine.tick(now, &mut out);
    assert!(matches!(out.as_slice(), [Out::Literal(_)]));
    out.clear();
    engine.text_input_failed(&mut out);
    engine.set_text_pending(false, now);
    engine.tick(now + Duration::from_secs(1), &mut out);
    assert!(out.is_empty());
}
