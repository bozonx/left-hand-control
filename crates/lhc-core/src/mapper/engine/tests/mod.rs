//! Engine tests, one module per topic; shared configs and helpers live here.

use super::*;
use crate::mapper::config::{
    ActionSpec, Command, LayerKeymap, Macro, MacroStep, MapperConfig, Rule, Settings, TapDecision,
};
use crate::mapper::key_code::KeyCode;
use std::collections::HashMap;
use std::time::{Duration, Instant};

mod builder;
mod chords;
mod conditions;
mod layers;
mod macros;
mod mouse;
mod tap_hold;

fn config_with_rule(rule: serde_json::Value) -> MapperConfig {
    serde_json::from_value(serde_json::json!({"rules": [rule]})).unwrap()
}

fn empty_cfg() -> MapperConfig {
    MapperConfig {
        rules: Vec::new(),
        layer_keymaps: HashMap::new(),
        macros: Vec::new(),
        commands: Vec::new(),
        settings: Settings::default(),
    }
}

/// A `win` layer held by AltLeft (tap = Enter) whose Tab and CapsLock keep
/// their modifiers until the layer ends, plus an ordinary KeyC.
fn switcher_cfg() -> MapperConfig {
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
    let mut win = LayerKeymap::default();
    win.keys.insert("Tab".into(), Some("Alt+Tab".into()));
    win.keys
        .insert("CapsLock".into(), Some("Alt+Shift+Tab".into()));
    win.keys.insert("KeyC".into(), Some("Alt+KeyC".into()));
    win.hold_modifiers = vec!["Tab".into(), "CapsLock".into()];
    cfg.layer_keymaps.insert("win".into(), win);
    cfg.settings.tap_decision = TapDecision::HoldOnOtherKeyPress;
    cfg
}

/// Feed `events` (key, down) 10 ms apart and return the output.
fn run(engine: &mut Engine, events: &[(KeyCode, bool)]) -> Vec<EngineOutput> {
    let now = Instant::now();
    let mut out = Vec::new();
    for (i, (key, down)) in events.iter().enumerate() {
        engine.handle_key_event(
            *key,
            *down,
            now + Duration::from_millis(10 * i as u64),
            &mut out,
        );
    }
    out
}

/// Virtual key transitions of `out`, in order, as (key, down).
fn transitions(out: &[EngineOutput]) -> Vec<(KeyCode, bool)> {
    let mut keys = Vec::new();
    for o in out {
        match o {
            EngineOutput::KeyRaw { key, down } => keys.push((*key, *down)),
            EngineOutput::ChordPress { ks, .. } => {
                keys.extend(ks.mods.iter().map(|m| (*m, true)));
                keys.push((ks.key, true));
            }
            EngineOutput::ChordRelease { key, mods, .. } => {
                keys.push((*key, false));
                keys.extend(mods.iter().rev().map(|m| (*m, false)));
            }
            EngineOutput::ReleaseMods(mods) => keys.extend(mods.iter().map(|m| (*m, false))),
            _ => {}
        }
    }
    keys
}

fn rule_with_apps(whitelist: Option<Vec<String>>, blacklist: Option<Vec<String>>) -> RuleEntry {
    RuleEntry {
        tap_decision: TapDecision::PermissiveHold,
        tap: TapMode::Native,
        layer_id: None,
        hold: HoldOutputMode::Swallow,
        long_hold: None,
        long_hold_timeout: Duration::from_secs(1),
        double_tap: None,
        hold_timeout: Duration::from_millis(200),
        double_tap_window: Duration::from_millis(200),
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: whitelist,
        condition_excluded_window_patterns: blacklist,
    }
}

fn raw_keys(out: &[EngineOutput]) -> Vec<(KeyCode, bool)> {
    out.iter()
        .filter_map(|event| match event {
            EngineOutput::KeyRaw { key, down } => Some((*key, *down)),
            _ => None,
        })
        .collect()
}

fn fired_text(out: &[EngineOutput]) -> bool {
    out.iter()
        .any(|event| matches!(event, EngineOutput::Literal(_)))
}

/// A permissive-mode mod-tap rule with a 200 ms hold timeout.
fn tap_hold_rule(key: &str, tap: ActionSpec, hold: ActionSpec) -> Rule {
    Rule {
        enabled: true,
        condition_game_mode: None,
        condition_keyboard_layouts: None,
        condition_window_patterns: None,
        condition_excluded_window_patterns: None,
        key: key.into(),
        layer_id: String::new(),
        tap_action: tap,
        hold_behavior: None,
        long_hold_action: String::new(),
        long_hold_timeout_ms: None,
        hold_action: hold,
        hold_timeout_ms: Some(200),
        tap_decision: None,
        double_tap_action: String::new(),
        double_tap_timeout_ms: None,
    }
}

/// A held `nav` layer on Space whose KeyJ runs macro `m`.
fn layer_macro_cfg(steps: serde_json::Value) -> MapperConfig {
    serde_json::from_value(serde_json::json!({
        "rules": [{"key": "Space", "layerId": "nav", "holdAction": null}],
        "layerKeymaps": {"nav": {"keys": {"KeyJ": "macro:m"}}},
        "macros": [{"id": "m", "steps": steps, "stepPauseMs": 10}],
        "settings": {"defaultHoldTimeoutMs": 50, "macroTimeoutMs": 1000}
    }))
    .unwrap()
}

/// Run the engine clock from `from` to `to` in 5 ms ticks.
fn run_ticks(engine: &mut Engine, from: Instant, to: Instant, out: &mut Vec<EngineOutput>) {
    let mut at = from;
    while at <= to {
        engine.tick(at, out);
        at += Duration::from_millis(5);
    }
}

/// Exercise both live delivery and a batch drained before the next timer tick.
fn space_audit_presses(
    rule: Option<serde_json::Value>,
    events: &[(KeyCode, bool, u64)],
) -> Vec<KeyCode> {
    let cfg = rule.map(config_with_rule).unwrap_or_else(empty_cfg);
    let mut results = Vec::new();
    for tick_each in [false, true] {
        let mut engine = Engine::new(&cfg);
        let now = Instant::now();
        let mut out = Vec::new();
        for &(key, down, ms) in events {
            let at = now + Duration::from_millis(ms);
            engine.handle_key_event(key, down, at, &mut out);
            if tick_each {
                engine.tick(at, &mut out);
            }
        }
        engine.tick(now + Duration::from_secs(2), &mut out);
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
        assert!(engine.pending.is_empty());
        assert!(engine.virt.emitted.is_empty());
        results.push(presses);
    }
    assert_eq!(results[0], results[1], "batch delivery changed typing");
    results.remove(0)
}

/// Feed timed events and ticks in time order (events before a tick at the
/// same millisecond) and describe the output compactly.
fn timeline(cfg: serde_json::Value, events: &[(KeyCode, bool, u64)], ticks: &[u64]) -> Vec<String> {
    let cfg: MapperConfig = serde_json::from_value(cfg).unwrap();
    let mut engine = Engine::new(&cfg);
    let start = Instant::now();
    let mut out = Vec::new();
    let mut steps: Vec<(u64, Option<(KeyCode, bool)>)> = events
        .iter()
        .map(|&(key, down, ms)| (ms, Some((key, down))))
        .chain(ticks.iter().map(|&ms| (ms, None)))
        .collect();
    steps.sort_by_key(|(ms, event)| (*ms, event.is_none()));
    for (ms, event) in steps {
        let at = start + Duration::from_millis(ms);
        match event {
            Some((key, down)) => engine.handle_key_event(key, down, at, &mut out),
            None => engine.tick(at, &mut out),
        }
    }
    engine.tick(start + Duration::from_secs(5), &mut out);
    assert!(engine.pending.is_empty() && engine.virt.emitted.is_empty());
    out.iter()
        .map(|event| match event {
            EngineOutput::KeyRaw { key, down } => {
                format!("{key:?}{}", if *down { "+" } else { "-" })
            }
            EngineOutput::Stroke { ks, .. } => format!("tap {:?}", ks.key),
            EngineOutput::ChordPress { ks, .. } => format!("press {:?}", ks.key),
            EngineOutput::ChordRelease { key, .. } => format!("release {key:?}"),
            EngineOutput::ReleaseMods(mods) => format!("release {mods:?}"),
            EngineOutput::Literal(text) => format!("text {text}"),
            _ => "other".into(),
        })
        .collect()
}
