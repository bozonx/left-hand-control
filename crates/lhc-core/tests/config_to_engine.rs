//! Integration: saved config and layout files → `ConfigDocument` → runtime
//! config → `Engine` → emitted outputs. Real parsing, validation and layout
//! activation; key events and time are synthetic and no evdev/uinput is used,
//! so the end of the chain is `EngineOutput`, not device events (see `mapper_e2e`).
#![cfg(any(target_os = "linux", target_os = "windows"))]

use lhc_core::{
    StoragePaths,
    config_document::ConfigDocument,
    mapper::{
        engine::{Engine, EngineOutput},
        key_code::KeyCode,
    },
    profile::auto_switch::AutoSwitchContext,
};
use std::time::{Duration, Instant};

const LAYOUT: &str = r#"{
  "version": 1,
  "layers": [{ "id": "nav", "keys": { "KeyH": "ArrowLeft", "KeyJ": "macro:copyLine" } }],
  "rules": [
    { "key": "CapsLock", "tap": "Escape", "hold": { "layer": "nav", "timeoutMs": 200 } },
    { "key": "KeyQ", "tap": "Ctrl+KeyZ", "when": { "keyboardLayouts": ["us"] } },
    { "key": "KeyW", "enabled": false, "tap": "KeyX" }
  ],
  "macros": [{ "id": "copyLine", "name": "Copy line", "steps": ["Home", "Shift+End", "Ctrl+KeyC"] }]
}"#;

fn document(config: &str) -> (tempfile::TempDir, ConfigDocument) {
    let directory = tempfile::tempdir().unwrap();
    let paths = StoragePaths::new(
        directory.path().join("settings"),
        directory.path().join("data"),
    );
    paths.save_user_layout("Main", LAYOUT, true).unwrap();
    paths.save_config(config).unwrap();
    let document = ConfigDocument::load(paths).unwrap();
    (directory, document)
}

const MANUAL: &str = r#"{"version":1,"settings":{"layouts":{"manual":"user:Main"}}}"#;

fn manual_engine() -> (tempfile::TempDir, Engine) {
    engine_for(MANUAL)
}

fn engine_for(config: &str) -> (tempfile::TempDir, Engine) {
    let (directory, document) = document(config);
    let runtime = document
        .runtime_config(&AutoSwitchContext::default())
        .unwrap();
    assert_eq!(runtime.layout_id.as_deref(), Some("user:Main"));
    (directory, Engine::new(&runtime.mapper))
}

/// Feed `events` (key, down, ms since start), then run timers until idle.
fn run(engine: &mut Engine, events: &[(KeyCode, bool, u64)]) -> Vec<EngineOutput> {
    let start = Instant::now();
    let mut out = Vec::new();
    let mut now = start;
    for &(key, down, at) in events {
        let when = start + Duration::from_millis(at);
        while let Some(wait) = engine.next_deadline(now) {
            if now + wait > when {
                break;
            }
            now += wait;
            engine.tick(now, &mut out);
        }
        now = when;
        engine.handle_key_event(key, down, now, &mut out);
    }
    for _ in 0..100 {
        let Some(wait) = engine.next_deadline(now) else {
            break;
        };
        now += wait;
        engine.tick(now, &mut out);
    }
    out
}

/// Keys pressed by strokes and raw presses, in order (macros press modifiers raw).
fn pressed(out: &[EngineOutput]) -> Vec<KeyCode> {
    out.iter()
        .filter_map(|output| match output {
            EngineOutput::Stroke { ks, .. } | EngineOutput::ChordPress { ks, .. } => Some(ks.key),
            EngineOutput::KeyRaw { key, down: true } => Some(*key),
            _ => None,
        })
        .collect()
}

#[test]
fn tap_fires_the_configured_action() {
    let (_dir, mut engine) = manual_engine();
    let out = run(
        &mut engine,
        &[
            (KeyCode::KEY_CAPSLOCK, true, 0),
            (KeyCode::KEY_CAPSLOCK, false, 50),
        ],
    );
    assert_eq!(pressed(&out), [KeyCode::KEY_ESC]);
}

#[test]
fn hold_activates_the_layer_and_its_keys() {
    let (_dir, mut engine) = manual_engine();
    let out = run(
        &mut engine,
        &[
            (KeyCode::KEY_CAPSLOCK, true, 0),
            (KeyCode::KEY_H, true, 300),
            (KeyCode::KEY_H, false, 320),
            (KeyCode::KEY_CAPSLOCK, false, 400),
        ],
    );
    assert_eq!(pressed(&out), [KeyCode::KEY_LEFT]);
    assert_eq!(engine.top_layer(), None);
}

#[test]
fn layer_key_runs_the_macro_steps_in_order() {
    let press_j_in_layer = [
        (KeyCode::KEY_CAPSLOCK, true, 0),
        (KeyCode::KEY_J, true, 300),
        (KeyCode::KEY_J, false, 320),
        (KeyCode::KEY_CAPSLOCK, false, 2000),
    ];
    // User macros are an in-development feature: off by default.
    let (_dir, mut engine) = manual_engine();
    assert_eq!(pressed(&run(&mut engine, &press_j_in_layer)), []);

    let (_dir, mut engine) = engine_for(
        r#"{"version":1,"settings":{"experimental":true,"layouts":{"manual":"user:Main"}}}"#,
    );
    let out = run(&mut engine, &press_j_in_layer);
    assert_eq!(
        pressed(&out),
        [
            KeyCode::KEY_HOME,
            KeyCode::KEY_LEFTSHIFT,
            KeyCode::KEY_END,
            KeyCode::KEY_LEFTCTRL,
            KeyCode::KEY_C
        ]
    );
}

#[test]
fn disabled_rules_pass_the_key_through() {
    let (_dir, mut engine) = manual_engine();
    let out = run(
        &mut engine,
        &[(KeyCode::KEY_W, true, 0), (KeyCode::KEY_W, false, 20)],
    );
    assert_eq!(pressed(&out), [KeyCode::KEY_W]);
}

#[test]
fn rule_conditions_follow_the_system_context() {
    let (_dir, mut engine) = manual_engine();
    let tap = [(KeyCode::KEY_Q, true, 0), (KeyCode::KEY_Q, false, 20)];
    engine.set_context_source(Box::new(|| AutoSwitchContext {
        system_layout: Some("ru".into()),
        ..Default::default()
    }));
    assert_eq!(pressed(&run(&mut engine, &tap)), [KeyCode::KEY_Q]);
    engine.set_context_source(Box::new(|| AutoSwitchContext {
        system_layout: Some("us".into()),
        ..Default::default()
    }));
    assert_eq!(pressed(&run(&mut engine, &tap)), [KeyCode::KEY_Z]);
}

#[test]
fn auto_mode_without_a_match_passes_keys_through() {
    let (_dir, document) = document(r#"{"version":1,"settings":{"layouts":{"mode":"auto"}}}"#);
    let runtime = document
        .runtime_config(&AutoSwitchContext::default())
        .unwrap();
    assert_eq!(runtime.layout_id, None);
    let mut engine = Engine::new(&runtime.mapper);
    let out = run(
        &mut engine,
        &[
            (KeyCode::KEY_CAPSLOCK, true, 0),
            (KeyCode::KEY_CAPSLOCK, false, 50),
        ],
    );
    assert_eq!(pressed(&out), [KeyCode::KEY_CAPSLOCK]);
}
