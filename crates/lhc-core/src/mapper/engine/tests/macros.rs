use super::*;


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
        // Only the run's time limit is left to wake the loop.
        assert!(
            engine
                .next_deadline(completed)
                .is_some_and(|wait| wait > Duration::from_secs(4))
        );
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
    engine.execute_remote("macro:hello", &mut out);
    let now = Instant::now();
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
        windows: "printf hello".into(),
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
        windows: "pwd".into(),
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
        windows: "playerctl play-pause".into(),
        working_directory: None,
    });
    cfg.settings.commands_enabled = true;
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();

    engine.execute_remote("cmd:play", &mut out);

    let expected = crate::mapper::commands::shell_command(&cfg.commands[0], None).expect("script");
    assert!(matches!(
        out.as_slice(),
        [Out::RunCommand(cmd)] if cmd.program == expected.program && cmd.args == expected.args
    ));
}

#[test]
fn starting_macro_mid_stroke_queues_it_after_the_stroke() {
    // A macro started while another is between its mod-press and
    // mod-release phases waits: the running stroke completes first.
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
    out.clear();

    engine.execute_remote("macro:b", &mut out);
    assert!(out.is_empty());
    for ms in [10, 20, 30] {
        engine.tick(now + Duration::from_millis(ms), &mut out);
    }
    assert_eq!(
        raw_keys(&out),
        [
            (KeyCode::KEY_C, true),
            (KeyCode::KEY_C, false),
            (KeyCode::KEY_LEFTCTRL, false),
            (KeyCode::KEY_H, true),
            (KeyCode::KEY_H, false),
        ]
    );
}

#[test]
fn macro_repeats_inside_a_held_layer_and_queues_at_most_three_runs() {
    let mut engine = Engine::new(&layer_macro_cfg(serde_json::json!([
        {"action": "KeyH"}, {"action": "KeyI"}
    ])));
    let now = Instant::now();
    let at = |ms| now + Duration::from_millis(ms);
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(at(60), &mut out);
    assert_eq!(engine.top_layer(), Some("nav"));

    // Five quick presses: one run, three queued, one dropped.
    for i in 0..5 {
        engine.handle(KeyCode::KEY_J, true, at(61 + 2 * i), &mut out);
        engine.handle(KeyCode::KEY_J, false, at(62 + 2 * i), &mut out);
    }
    run_ticks(&mut engine, at(71), at(500), &mut out);
    let presses = raw_keys(&out)
        .into_iter()
        .filter(|(key, down)| *down && [KeyCode::KEY_H, KeyCode::KEY_I].contains(key))
        .map(|(key, _)| key)
        .collect::<Vec<_>>();
    assert_eq!(presses, [KeyCode::KEY_H, KeyCode::KEY_I].repeat(4));
    assert_eq!(engine.top_layer(), Some("nav"));

    // Finished runs leave room: the layer is still held, so it runs again.
    out.clear();
    engine.handle(KeyCode::KEY_J, true, at(600), &mut out);
    engine.handle(KeyCode::KEY_J, false, at(601), &mut out);
    run_ticks(&mut engine, at(600), at(700), &mut out);
    assert_eq!(raw_keys(&out).len(), 4);
}

#[test]
fn macro_over_the_time_limit_is_aborted_and_the_queue_continues() {
    let mut engine = Engine::new(&layer_macro_cfg(serde_json::json!([
        {"action": "KeyH"}, {"action": "pause:5000"}, {"action": "KeyI"}
    ])));
    let now = Instant::now();
    let at = |ms| now + Duration::from_millis(ms);
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(at(60), &mut out);
    engine.handle(KeyCode::KEY_J, true, at(100), &mut out);
    engine.handle(KeyCode::KEY_J, false, at(101), &mut out);
    engine.handle(KeyCode::KEY_J, true, at(102), &mut out);
    engine.handle(KeyCode::KEY_J, false, at(103), &mut out);
    engine.tick(at(100), &mut out);
    engine.tick(at(110), &mut out);
    assert_eq!(
        engine.next_deadline(at(120)),
        Some(Duration::from_millis(980))
    );

    // The first run times out before KeyI; the queued run starts at once.
    engine.tick(at(1100), &mut out);
    engine.tick(at(1100), &mut out);
    assert_eq!(
        raw_keys(&out),
        [
            (KeyCode::KEY_H, true),
            (KeyCode::KEY_H, false),
            (KeyCode::KEY_H, true),
            (KeyCode::KEY_H, false),
        ]
    );
    assert!(engine.runner.active.is_some());
    assert!(engine.runner.queue.is_empty());
}

#[test]
fn shutdown_and_reconfigure_drop_queued_macro_runs() {
    let cfg = layer_macro_cfg(serde_json::json!([{"action": "KeyH"}, {"action": "KeyI"}]));
    for reconfigure in [false, true] {
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        engine.execute_remote("macro:m", &mut out);
        engine.execute_remote("macro:m", &mut out);
        assert_eq!(engine.runner.queue.len(), 1);
        if reconfigure {
            engine.reconfigure(&cfg, &mut out);
        } else {
            engine.shutdown(&mut out);
        }
        assert!(engine.runner.active.is_none());
        assert!(engine.runner.queue.is_empty());
        run_ticks(&mut engine, now, now + Duration::from_millis(100), &mut out);
        assert!(raw_keys(&out).is_empty());
    }
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
    assert!(engine.runner.active.is_none());
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
        hold_timeout_ms: None,
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    });
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    let now = Instant::now();

    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    // Empty macro resolves to nothing → fast path Swallow+Swallow, no crash
    assert!(engine.runner.active.is_none());
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

#[test]
fn macro_and_physical_modifier_ownership_survives_either_release_order() {
    for physical_first in [false, true] {
        let cfg: AppConfig = serde_json::from_value(serde_json::json!({"macros": [
            {"id": "m", "steps": [{"action": "Ctrl+KeyC"}], "modifierDelayMs": 20}
        ]}))
        .unwrap();
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();
        if physical_first {
            engine.handle(KeyCode::KEY_LEFTCTRL, true, now, &mut out);
        }
        engine.execute_remote("macro:m", &mut out);
        let now = Instant::now();
        engine.tick(now, &mut out);
        if physical_first {
            engine.handle(
                KeyCode::KEY_LEFTCTRL,
                false,
                now + Duration::from_millis(1),
                &mut out,
            );
            assert!(!out.iter().any(|o| matches!(
                o,
                Out::KeyRaw {
                    key: KeyCode::KEY_LEFTCTRL,
                    down: false
                }
            )));
        } else {
            engine.handle(
                KeyCode::KEY_LEFTCTRL,
                true,
                now + Duration::from_millis(1),
                &mut out,
            );
        }
        engine.tick(now + Duration::from_millis(25), &mut out);
        out.clear();
        engine.tick(now + Duration::from_millis(50), &mut out);
        assert_eq!(
            out.iter().any(|o| matches!(
                o,
                Out::KeyRaw {
                    key: KeyCode::KEY_LEFTCTRL,
                    down: false
                }
            )),
            physical_first
        );
        if !physical_first {
            engine.handle(
                KeyCode::KEY_LEFTCTRL,
                false,
                now + Duration::from_millis(60),
                &mut out,
            );
            assert!(matches!(
                out.last(),
                Some(Out::KeyRaw {
                    key: KeyCode::KEY_LEFTCTRL,
                    down: false
                })
            ));
        }
    }
}

#[test]
fn unchecked_exponential_macro_expansion_is_bounded() {
    let mut cfg = AppConfig::default();
    for n in 0..11 {
        cfg.macros.push(Macro {
            id: format!("m{n}"),
            steps: (0..10)
                .map(|_| MacroStep {
                    action: if n == 0 {
                        "KeyA".into()
                    } else {
                        format!("macro:m{}", n - 1)
                    },
                })
                .collect(),
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
    }
    assert!(
        super::super::validation::validate_config(&cfg)
            .unwrap_err()
            .contains("expansion exceeds")
    );
    let engine = Engine::new(&cfg);
    assert!(engine.layout.macros.values().all(|m| m.steps.len() <= 1000));
}

#[test]
fn cancelling_a_macro_keeps_a_new_physical_modifier_owner() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({"macros": [
        {"id": "m", "steps": [{"action": "Ctrl+KeyC"}], "modifierDelayMs": 20}
    ]}))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let mut out = Vec::new();
    engine.execute_remote("macro:m", &mut out);
    let now = Instant::now();
    engine.tick(now, &mut out);
    engine.handle(KeyCode::KEY_LEFTCTRL, true, now, &mut out);
    out.clear();
    engine.reconfigure(&AppConfig::default(), &mut out);
    assert!(out.is_empty());
    engine.handle(KeyCode::KEY_LEFTCTRL, false, now, &mut out);
    assert!(matches!(
        out.as_slice(),
        [Out::KeyRaw {
            key: KeyCode::KEY_LEFTCTRL,
            down: false
        }]
    ));
}

#[test]
fn disabling_commands_revokes_a_pending_tap_without_releasing_other_keys() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({
        "settings": {"commandsEnabled": true},
        "commands": [{"id": "test", "linux": "true"}],
        "rules": [{"key": "KeyA", "tapAction": "cmd:test"}]
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_A, true, now, &mut out);
    engine.reconfigure(&AppConfig::default(), &mut out);
    engine.handle(
        KeyCode::KEY_A,
        false,
        now + Duration::from_millis(10),
        &mut out,
    );
    assert!(out.is_empty());
}

#[test]
fn layer_key_bound_to_an_empty_macro_swallows_the_key() {
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({
        "macros": [{"id": "empty", "steps": []}],
        "rules": [{"key": "Space", "layerId": "nav", "tapAction": "Space"}],
        "layerKeymaps": {"nav": {"keys": {"KeyQ": "macro:empty", "KeyW": ""}}}
    }))
    .unwrap();
    crate::mapper::validation::validate_config(&cfg).unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_SPACE, true, now, &mut out);
    engine.tick(now + Duration::from_millis(300), &mut out);
    out.clear();
    let t = now + Duration::from_millis(310);
    engine.handle(KeyCode::KEY_Q, true, t, &mut out);
    engine.handle(KeyCode::KEY_Q, false, t, &mut out);
    // Like a tap bound to the same macro: the key does nothing.
    assert!(raw_keys(&out).is_empty());
    // An empty entry still leaves the key to the base layout.
    engine.handle(KeyCode::KEY_W, true, t, &mut out);
    assert_eq!(raw_keys(&out), [(KeyCode::KEY_W, true)]);
}

#[test]
fn bindings_to_commands_that_cannot_run_report_instead_of_doing_nothing() {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let record = seen.clone();
    crate::events::bus().subscribe(move |event| {
        if matches!(event, crate::events::CoreEvent::ActionUnavailable(action)
            if action == "cmd:disabledHere" || action == "cmd:")
        {
            *record.lock().unwrap() += 1;
        }
    });
    let cfg: AppConfig = serde_json::from_value(serde_json::json!({
        "settings": {"commandsEnabled": false},
        "commands": [{"id": "disabledHere", "linux": "true", "windows": "dir"}],
        "rules": [{"key": "KeyQ", "tapAction": "cmd:disabledHere"}]
    }))
    .unwrap();
    let mut engine = Engine::new(&cfg);
    let now = Instant::now();
    let mut out = Vec::new();
    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.handle(KeyCode::KEY_Q, false, now, &mut out);
    engine.execute_remote("cmd:disabledHere", &mut out);
    assert!(out.is_empty());
    assert_eq!(*seen.lock().unwrap(), 2);

    // A tap still waiting for its decision when commands are switched off
    // reports at release instead of silently doing nothing.
    let enabled: AppConfig = serde_json::from_value(serde_json::json!({
        "settings": {"commandsEnabled": true},
        "commands": [{"id": "disabledHere", "linux": "true", "windows": "dir"}],
        "rules": [{"key": "KeyQ", "tapAction": "cmd:disabledHere"}]
    }))
    .unwrap();
    let mut engine = Engine::new(&enabled);
    engine.handle(KeyCode::KEY_Q, true, now, &mut out);
    engine.reconfigure(&cfg, &mut out);
    engine.handle(KeyCode::KEY_Q, false, now, &mut out);
    assert!(out.is_empty());
    assert_eq!(*seen.lock().unwrap(), 3);
}
