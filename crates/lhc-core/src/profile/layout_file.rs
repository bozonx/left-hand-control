//! Saved layout JSON files in the user library.
//!
//! Window conditions use `windows` and `excludeWindows` patterns.

use super::model::{
    Command, EmojiPage, LEFT_HAND_HOTKEYS, Layer, LayerKeymap, LayerRule, LayoutPreset, Macro,
    MacroStep, QuickAction, QuickActionPage,
};
use serde_json::{Map, Value};

/// Parse a layout file. Returns `Ok(None)` for an empty document.
pub fn parse(text: &str) -> Result<Option<LayoutPreset>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let doc: Value =
        serde_json::from_str(text).map_err(|error| format!("parse layout: {error}"))?;
    if let Some(object) = doc.as_object() {
        for rule in array(object, "rules") {
            for field in ["appsWhitelist", "appsBlacklist"] {
                if rule.contains_key(field) {
                    return Err(format!("unsupported layout rule field: {field}"));
                }
            }
        }
    }
    Ok(doc.as_object().map(from_object))
}

fn from_object(doc: &Map<String, Value>) -> LayoutPreset {
    // Items without an id get one numbered in file order, so parsing the
    // same text twice yields equal layouts (needed to detect real edits).
    let counter = std::cell::Cell::new(0u32);
    let next_id = |prefix: &str| {
        counter.set(counter.get() + 1);
        format!("{prefix}auto{}", counter.get())
    };
    let mut layers = Vec::new();
    let mut layer_keymaps = std::collections::BTreeMap::new();
    for layer in array(doc, "layers") {
        let Some(id) = non_empty(layer, "id") else {
            continue;
        };
        layers.push(Layer {
            id: id.into(),
            name: str_of(layer, "name").unwrap_or(id).into(),
            description: str_of(layer, "description").map(str::to_owned),
        });
        let keys = layer
            .get("keys")
            .and_then(Value::as_object)
            .map(|keys| {
                keys.iter()
                    .map(|(code, action)| (code.clone(), scalar(action).filter(|a| !a.is_empty())))
                    .collect()
            })
            .unwrap_or_default();
        layer_keymaps.insert(
            id.to_owned(),
            LayerKeymap {
                keys,
                extra_key_order: layer
                    .get("extraKeyOrder")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            },
        );
    }

    let rules = array(doc, "rules")
        .filter_map(|rule| {
            let key = non_empty(rule, "key")?;
            Some(LayerRule {
                id: str_of(rule, "id").map_or_else(|| next_id("r_"), str::to_owned),
                enabled: (rule.get("enabled") == Some(&Value::Bool(false))).then_some(false),
                condition_game_mode: str_of(rule, "gameMode")
                    .filter(|mode| matches!(*mode, "on" | "off"))
                    .map(str::to_owned),
                condition_layouts: string_list(rule, "layouts"),
                condition_apps_whitelist: string_list(rule, "windows"),
                condition_apps_blacklist: string_list(rule, "excludeWindows"),
                key: key.into(),
                layer_id: str_of(rule, "layer").unwrap_or("").into(),
                tap_action: three_state(rule, "tap"),
                hold_action: three_state(rule, "hold"),
                hold_behavior: rule
                    .get("onHold")
                    .and_then(|value| serde_json::from_value(value.clone()).ok()),
                long_hold_action: rule.get("longHold").and_then(scalar).unwrap_or_default(),
                long_hold_timeout_ms: rule.get("longHoldMs").and_then(Value::as_u64),
                isolate: key_list(rule.get("isolate")),
                hold_for: key_list(rule.get("holdFor")),
                double_tap_action: rule.get("dtap").and_then(scalar).unwrap_or_default(),
                hold_timeout_ms: rule.get("holdMs").and_then(Value::as_u64),
                tap_decision: rule
                    .get("tapDecision")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
                double_tap_timeout_ms: rule.get("dtapMs").and_then(Value::as_u64),
            })
        })
        .collect();

    let macros = array(doc, "macros")
        .filter_map(|item| {
            let id = non_empty(item, "id")?;
            let steps = item
                .get("steps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|step| match step {
                    Value::String(action) if !action.trim().is_empty() => Some(MacroStep {
                        id: next_id("s_"),
                        action: action.clone(),
                    }),
                    Value::Object(step) => {
                        let action = non_empty(step, "action")?;
                        Some(MacroStep {
                            id: str_of(step, "id").map_or_else(|| next_id("s_"), str::to_owned),
                            action: action.into(),
                        })
                    }
                    _ => None,
                })
                .collect();
            Some(Macro {
                id: id.into(),
                name: str_of(item, "name").unwrap_or(id).into(),
                steps,
                step_pause_ms: item.get("stepPauseMs").and_then(Value::as_u64),
                modifier_delay_ms: item.get("modifierDelayMs").and_then(Value::as_u64),
            })
        })
        .collect();

    let commands = array(doc, "commands")
        .filter_map(|item| {
            let id = non_empty(item, "id")?;
            Some(Command {
                id: id.into(),
                name: str_of(item, "name").unwrap_or(id).into(),
                linux: str_of(item, "linux").unwrap_or("").trim().into(),
                working_directory: non_empty(item, "workingDirectory")
                    .map(|value| value.trim().to_owned()),
            })
        })
        .collect::<Vec<_>>();

    let mut quick_action_pages: Vec<QuickActionPage> = array(doc, "quickActionPages")
        .map(|page| QuickActionPage {
            id: trimmed(page, "id").map_or_else(|| next_id("qap_"), str::to_owned),
            name: trimmed(page, "name").unwrap_or_default().into(),
            cells: page
                .get("cells")
                .and_then(Value::as_object)
                .map(|cells| {
                    cells
                        .iter()
                        .filter(|(key, _)| LEFT_HAND_HOTKEYS.contains(&key.as_str()))
                        .filter_map(|(key, value)| Some((key.clone(), quick_action(value)?)))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    if quick_action_pages.is_empty() {
        quick_action_pages.push(QuickActionPage::new(next_id("qap_"), String::new()));
    }

    let mut emoji_pages: Vec<EmojiPage> = array(doc, "emojiPages")
        .map(|page| {
            let id = trimmed(page, "id").map_or_else(|| next_id("emoji_"), str::to_owned);
            EmojiPage {
                name: trimmed(page, "name").unwrap_or_default().into(),
                cells: page
                    .get("cells")
                    .and_then(Value::as_object)
                    .map(|cells| {
                        cells
                            .iter()
                            .filter(|(key, _)| LEFT_HAND_HOTKEYS.contains(&key.as_str()))
                            .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.into())))
                            .collect()
                    })
                    .unwrap_or_default(),
                id,
            }
        })
        .collect();
    if emoji_pages.is_empty() {
        emoji_pages.push(EmojiPage::default_page());
    }

    LayoutPreset {
        description: trimmed(doc, "description").map(str::to_owned),
        layers,
        rules,
        layer_keymaps,
        macros,
        commands,
        quick_action_pages,
        emoji_pages,
    }
}

/// Serialise a preset as formatted JSON.
pub fn serialize(preset: &LayoutPreset) -> String {
    let mut doc = Map::new();
    if let Some(description) = &preset.description {
        put(&mut doc, "description", description.as_str());
    }
    doc.insert(
        "layers".into(),
        Value::Array(
            preset
                .layers
                .iter()
                .map(|layer| {
                    let mut out = Map::new();
                    put(&mut out, "id", layer.id.as_str());
                    put(&mut out, "name", layer.name.as_str());
                    if let Some(description) =
                        layer.description.as_deref().filter(|d| !d.is_empty())
                    {
                        put(&mut out, "description", description);
                    }
                    if let Some(keymap) = preset.layer_keymaps.get(&layer.id) {
                        if !keymap.extra_key_order.is_empty() {
                            out.insert(
                                "extraKeyOrder".into(),
                                Value::Array(
                                    keymap
                                        .extra_key_order
                                        .iter()
                                        .map(|key| Value::from(key.as_str()))
                                        .collect(),
                                ),
                            );
                        }
                        if !keymap.keys.is_empty() {
                            out.insert(
                                "keys".into(),
                                Value::Object(
                                    keymap
                                        .keys
                                        .iter()
                                        .map(|(code, action)| {
                                            (code.as_str().into(), optional(action))
                                        })
                                        .collect(),
                                ),
                            );
                        }
                    }
                    Value::Object(out)
                })
                .collect(),
        ),
    );
    doc.insert(
        "rules".into(),
        Value::Array(preset.rules.iter().map(rule_json).collect()),
    );
    doc.insert(
        "macros".into(),
        Value::Array(
            preset
                .macros
                .iter()
                .map(|item| {
                    let mut out = Map::new();
                    put(&mut out, "id", item.id.as_str());
                    put(&mut out, "name", item.name.as_str());
                    if let Some(pause) = item.step_pause_ms {
                        out.insert("stepPauseMs".into(), pause.into());
                    }
                    if let Some(delay) = item.modifier_delay_ms {
                        out.insert("modifierDelayMs".into(), delay.into());
                    }
                    out.insert(
                        "steps".into(),
                        Value::Array(
                            item.steps
                                .iter()
                                .map(|step| step.action.as_str().into())
                                .collect(),
                        ),
                    );
                    Value::Object(out)
                })
                .collect(),
        ),
    );
    doc.insert(
        "commands".into(),
        Value::Array(
            preset
                .commands
                .iter()
                .map(|command| {
                    let mut out = Map::new();
                    put(&mut out, "id", command.id.as_str());
                    put(&mut out, "name", command.name.as_str());
                    put(&mut out, "linux", command.linux.as_str());
                    if let Some(directory) = &command.working_directory {
                        put(&mut out, "workingDirectory", directory.as_str());
                    }
                    Value::Object(out)
                })
                .collect(),
        ),
    );
    doc.insert(
        "quickActionPages".into(),
        Value::Array(
            preset
                .quick_action_pages
                .iter()
                .map(|page| {
                    let mut out = Map::new();
                    put(&mut out, "id", page.id.as_str());
                    if !page.name.is_empty() {
                        put(&mut out, "name", page.name.as_str());
                    }
                    out.insert(
                        "cells".into(),
                        Value::Object(
                            LEFT_HAND_HOTKEYS
                                .iter()
                                .filter_map(|key| {
                                    let item = page
                                        .cells
                                        .get(*key)
                                        .filter(|item| !item.action.trim().is_empty())?;
                                    Some(((*key).into(), quick_action_json(item)))
                                })
                                .collect(),
                        ),
                    );
                    Value::Object(out)
                })
                .collect(),
        ),
    );
    doc.insert(
        "emojiPages".into(),
        Value::Array(
            preset
                .emoji_pages
                .iter()
                .map(|page| {
                    let mut out = Map::new();
                    put(&mut out, "id", page.id.as_str());
                    if !page.name.is_empty() {
                        put(&mut out, "name", page.name.as_str());
                    }
                    out.insert(
                        "cells".into(),
                        Value::Object(
                            LEFT_HAND_HOTKEYS
                                .iter()
                                .filter_map(|key| {
                                    let value = page.cells.get(*key).filter(|v| !v.is_empty())?;
                                    Some(((*key).into(), value.as_str().into()))
                                })
                                .collect(),
                        ),
                    );
                    Value::Object(out)
                })
                .collect(),
        ),
    );
    serde_json::to_string_pretty(&Value::Object(doc)).unwrap_or_default()
}

fn rule_json(rule: &LayerRule) -> Value {
    let mut out = Map::new();
    put(&mut out, "id", rule.id.as_str());
    put(&mut out, "key", rule.key.as_str());
    if rule.enabled == Some(false) {
        out.insert("enabled".into(), false.into());
    }
    if let Some(mode) = rule
        .condition_game_mode
        .as_deref()
        .filter(|mode| matches!(*mode, "on" | "off"))
    {
        put(&mut out, "gameMode", mode);
    }
    for (name, list) in [
        ("layouts", &rule.condition_layouts),
        ("windows", &rule.condition_apps_whitelist),
        ("excludeWindows", &rule.condition_apps_blacklist),
    ] {
        if let Some(list) = list.as_ref().filter(|list| !list.is_empty()) {
            out.insert(
                name.into(),
                Value::Array(list.iter().map(|item| item.as_str().into()).collect()),
            );
        }
    }
    if !rule.layer_id.is_empty() {
        put(&mut out, "layer", rule.layer_id.as_str());
    }
    if rule.tap_action.as_deref() != Some("") {
        out.insert("tap".into(), optional(&rule.tap_action));
    }
    if rule.hold_action.as_deref() != Some("") {
        out.insert("hold".into(), optional(&rule.hold_action));
    }
    for (name, list) in [("isolate", &rule.isolate), ("holdFor", &rule.hold_for)] {
        if let Some(list) = list
            .as_deref()
            .map(str::trim)
            .filter(|list| !list.is_empty())
        {
            put(&mut out, name, list);
        }
    }
    if let Some(behavior) = rule.hold_behavior {
        put(
            &mut out,
            "onHold",
            match behavior {
                super::model::HoldBehavior::None => "none",
                super::model::HoldBehavior::Layer => "layer",
                super::model::HoldBehavior::Action => "action",
            },
        );
    }
    if !rule.long_hold_action.is_empty() {
        put(&mut out, "longHold", rule.long_hold_action.as_str());
    }
    if let Some(ms) = rule.long_hold_timeout_ms {
        out.insert("longHoldMs".into(), ms.into());
    }
    if !rule.double_tap_action.is_empty() {
        put(&mut out, "dtap", rule.double_tap_action.as_str());
    }
    if let Some(mode) = rule.tap_decision {
        put(
            &mut out,
            "tapDecision",
            match mode {
                super::model::TapDecision::PermissiveHold => "permissiveHold",
                super::model::TapDecision::HoldOnOtherKeyPress => "holdOnOtherKeyPress",
            },
        );
    }
    if let Some(ms) = rule.hold_timeout_ms {
        out.insert("holdMs".into(), ms.into());
    }
    if let Some(ms) = rule.double_tap_timeout_ms {
        out.insert("dtapMs".into(), ms.into());
    }
    Value::Object(out)
}

/// A quick action cell: either the action itself or `{ name, action, icon }`.
/// Cells without an action are empty.
fn quick_action(value: &Value) -> Option<QuickAction> {
    let item = match value {
        Value::String(action) => QuickAction {
            action: action.clone(),
            ..QuickAction::default()
        },
        Value::Object(item) => QuickAction {
            name: trimmed(item, "name").unwrap_or_default().into(),
            action: str_of(item, "action").unwrap_or_default().into(),
            icon: trimmed(item, "icon").map(str::to_owned),
        },
        _ => return None,
    };
    (!item.action.trim().is_empty()).then_some(item)
}

fn quick_action_json(item: &QuickAction) -> Value {
    let icon = item.icon.as_deref().filter(|icon| !icon.is_empty());
    if item.name.is_empty() && icon.is_none() {
        return item.action.as_str().into();
    }
    let mut out = Map::new();
    if !item.name.is_empty() {
        put(&mut out, "name", item.name.as_str());
    }
    put(&mut out, "action", item.action.as_str());
    if let Some(icon) = icon {
        put(&mut out, "icon", icon);
    }
    Value::Object(out)
}

fn put(map: &mut Map<String, Value>, name: &str, value: &str) {
    map.insert(name.into(), value.into());
}

fn optional(value: &Option<String>) -> Value {
    value.as_deref().map_or(Value::Null, Into::into)
}

fn array<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> impl Iterator<Item = &'a Map<String, Value>> {
    object
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn str_of<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    object.get(name).and_then(Value::as_str)
}

fn non_empty<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    str_of(object, name).filter(|value| !value.is_empty())
}

fn trimmed<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    str_of(object, name)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Convert supported JSON scalars to their string representation.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// Absent → native (`""`), `null` → swallow (`None`), value → action.
fn three_state(object: &Map<String, Value>, name: &str) -> Option<String> {
    match object.get(name) {
        None => Some(String::new()),
        Some(Value::Null) => None,
        Some(value) => Some(scalar(value).unwrap_or_default()),
    }
}

fn string_list(object: &Map<String, Value>, name: &str) -> Option<Vec<String>> {
    let items: Vec<String> = super::settings::string_list(object.get(name))
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect();
    (!items.is_empty()).then_some(items)
}

fn key_list(value: Option<&Value>) -> Option<String> {
    let joined = match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    };
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
mod tests {
    #[test]
    fn per_rule_hold_detection_round_trips_through_json_and_mapper_json() {
        use super::super::model::TapDecision;
        for (value, expected) in [
            ("permissiveHold", TapDecision::PermissiveHold),
            ("holdOnOtherKeyPress", TapDecision::HoldOnOtherKeyPress),
        ] {
            let preset = parse(&format!(
                "{{\"rules\": [{{\"key\": \"Space\", \"tapDecision\": \"{value}\"}}]}}"
            ))
            .unwrap()
            .unwrap();
            assert_eq!(preset.rules[0].tap_decision, Some(expected));
            assert_eq!(parse(&serialize(&preset)).unwrap().unwrap(), preset);
            let mapper: crate::mapper::config::Rule =
                serde_json::from_value(serde_json::to_value(&preset.rules[0]).unwrap()).unwrap();
            assert_eq!(mapper.tap_decision, Some(expected));
        }
        let preset = parse(r#"{"rules": [{"key": "Space"}]}"#).unwrap().unwrap();
        assert_eq!(preset.rules[0].tap_decision, None);
        assert!(!serialize(&preset).contains("tapDecision"));
    }

    #[test]
    fn chord_and_long_hold_options_round_trip() {
        let preset = parse(r#"{"rules": [{"key": "ShiftLeft+ControlRight", "onHold": "action", "longHold": "text:hello", "longHoldMs": 1500}]}"#).unwrap().unwrap();
        let rule = &preset.rules[0];
        assert_eq!(
            rule.hold_behavior(),
            super::super::model::HoldBehavior::Action
        );
        assert_eq!(rule.long_hold_action, "text:hello");
        assert_eq!(rule.long_hold_timeout_ms, Some(1500));
        assert_eq!(parse(&serialize(&preset)).unwrap().unwrap(), preset);
    }

    #[test]
    fn parsing_is_deterministic_without_ids() {
        let text = r#"{"rules": [{"key": "CapsLock"}], "quickActionPages": [{"cells": {"KeyQ": "KeyA"}}]}"#;
        assert_eq!(super::parse(text), super::parse(text));
    }

    use super::*;

    const SAMPLE: &str = r#"
{
  "description": "Sample",
  "layers": [
    {
      "id": "nav",
      "name": "Navigation",
      "keys": {
        "KeyH": "ArrowLeft",
        "KeyQ": null,
        "Digit1": 5,
        "MouseForward": "Ctrl+KeyC"
      }
    }
  ],
  "rules": [
    {
      "key": "CapsLock",
      "layer": "nav",
      "tap": "Escape"
    },
    {
      "key": "Space",
      "hold": null,
      "layouts": [
        "us",
        ""
      ],
      "gameMode": "on",
      "isolate": [
        "KeyA",
        "KeyB"
      ],
      "holdMs": 250
    },
    {
      "key": "",
      "tap": "KeyA"
    }
  ],
  "macros": [
    {
      "id": "copyLine",
      "name": "Copy line",
      "steps": [
        "Home",
        "Shift+End",
        {
          "action": "Ctrl+KeyC"
        }
      ]
    }
  ],
  "commands": [
    {
      "id": "music",
      "linux": " playerctl play-pause "
    }
  ],
  "quickActionPages": [
    {
      "cells": {
        "KeyQ": {
          "name": "Lock",
          "action": "kde:ksmserver/Lock Session"
        },
        "KeyW": "Ctrl+KeyC",
        "KeyE": {
          "name": "Empty"
        },
        "Other": "KeyA"
      }
    }
  ],
  "emojiPages": [
    {
      "name": "Faces",
      "cells": {
        "KeyQ": "😀",
        "Other": "x"
      }
    }
  ]
}
"#;

    #[test]
    fn parses_layout_fields() {
        let preset = parse(SAMPLE).unwrap().unwrap();
        assert_eq!(preset.description.as_deref(), Some("Sample"));
        let nav = &preset.layer_keymaps["nav"];
        assert_eq!(nav.keys["KeyH"].as_deref(), Some("ArrowLeft"));
        assert_eq!(nav.keys["KeyQ"], None);
        assert_eq!(nav.keys["Digit1"].as_deref(), Some("5"));
        assert_eq!(nav.keys["MouseForward"].as_deref(), Some("Ctrl+KeyC"));
        assert_eq!(preset.rules.len(), 2);
        let caps = &preset.rules[0];
        assert_eq!(caps.layer_id, "nav");
        assert_eq!(caps.tap_action.as_deref(), Some("Escape"));
        assert_eq!(caps.hold_action.as_deref(), Some(""));
        let space = &preset.rules[1];
        assert_eq!(space.tap_action.as_deref(), Some(""));
        assert_eq!(space.hold_action, None);
        assert_eq!(space.condition_layouts, Some(vec!["us".to_string()]));
        assert_eq!(space.condition_game_mode.as_deref(), Some("on"));
        assert_eq!(space.isolate.as_deref(), Some("KeyA, KeyB"));
        assert_eq!(space.hold_timeout_ms, Some(250));
        assert_eq!(preset.macros[0].steps.len(), 3);
        assert_eq!(preset.commands[0].name, "music");
        assert_eq!(preset.commands[0].linux, "playerctl play-pause");
        assert_eq!(preset.quick_action_pages.len(), 1);
        let quick = &preset.quick_action_pages[0];
        assert!(quick.id.starts_with("qap_"));
        assert_eq!(quick.name, "");
        assert_eq!(quick.cells.len(), 2, "empty and unknown cells are dropped");
        assert_eq!(quick.cells["KeyQ"].name, "Lock");
        assert_eq!(quick.cells["KeyQ"].action, "kde:ksmserver/Lock Session");
        assert_eq!(quick.cells["KeyW"].name, "");
        assert_eq!(quick.cells["KeyW"].action, "Ctrl+KeyC");
        assert_eq!(preset.emoji_pages[0].cells.len(), 1);
    }

    /// Verify the layout parser against the recorded layout fixture.
    #[test]
    fn matches_the_recorded_layout_fixture() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let text = std::fs::read_to_string(root.join("layout-parity.json")).unwrap();
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("layout-parity.expected.json")).unwrap(),
        )
        .unwrap();
        let mut preset = parse(&text).unwrap().unwrap();
        for step in preset
            .macros
            .iter_mut()
            .flat_map(|item| item.steps.iter_mut())
        {
            if step.id != "s3" {
                step.id = "<generated>".into();
            }
        }
        assert_eq!(serde_json::to_value(&preset).unwrap(), expected);
    }

    #[test]
    fn unsupported_window_conditions_are_rejected() {
        for field in ["appsWhitelist", "appsBlacklist"] {
            let text = format!(
                "{{\"rules\": [{{\"key\": \"KeyA\", \"{field}\": [\"kate\"], \"windows\": [\"kate\"]}}]}}"
            );
            assert!(parse(&text).unwrap_err().contains(field));
        }
    }

    #[test]
    fn window_patterns_round_trip_without_substring_expansion() {
        let preset = parse(r#"{"rules": [{"key": "KeyA", "windows": ["org.kde.kate", "title:Doc"], "excludeWindows": ["firefox"]}]}"#).unwrap().unwrap();
        assert_eq!(
            preset.rules[0].condition_apps_whitelist,
            Some(vec!["org.kde.kate".into(), "title:Doc".into()])
        );
        assert_eq!(parse(&serialize(&preset)).unwrap().unwrap(), preset);
    }

    #[test]
    fn additional_key_order_round_trips() {
        let mut preset = parse(SAMPLE).unwrap().unwrap();
        assert!(preset.layer_keymaps["nav"].extra_key_order.is_empty());
        preset.layer_keymaps.get_mut("nav").unwrap().extra_key_order =
            vec!["F15".into(), "F13".into()];
        let again = parse(&serialize(&preset)).unwrap().unwrap();
        assert_eq!(again.layer_keymaps, preset.layer_keymaps);
    }

    #[test]
    fn serialisation_round_trips() {
        let preset = parse(SAMPLE).unwrap().unwrap();
        let text = serialize(&preset);
        let again = parse(&text).unwrap().unwrap();
        assert_eq!(again.layers, preset.layers);
        assert_eq!(
            again.layer_keymaps["nav"].keys,
            preset.layer_keymaps["nav"].keys
        );
        assert_eq!(again.rules.len(), preset.rules.len());
        for (a, b) in again.rules.iter().zip(&preset.rules) {
            assert_eq!(
                (&a.key, &a.tap_action, &a.hold_action),
                (&b.key, &b.tap_action, &b.hold_action)
            );
            assert_eq!(a.isolate, b.isolate);
            assert_eq!(a.id, b.id);
        }
        assert_eq!(again.macros[0].steps.len(), 3);
        assert_eq!(again.quick_action_pages, preset.quick_action_pages);
        assert_eq!(again.emoji_pages, preset.emoji_pages);
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap()["description"],
            "Sample"
        );
    }

    #[test]
    fn empty_or_scalar_documents_have_no_preset() {
        assert_eq!(parse("").unwrap(), None);
        assert_eq!(parse(r#""just text""#).unwrap(), None);
        assert!(parse("{\"layers\": [").is_err());
    }

    #[test]
    fn non_json_layouts_are_rejected() {
        assert!(parse("layers: []\nrules: []\n").is_err());
    }

    #[test]
    fn missing_emoji_pages_get_the_default_page() {
        let preset = parse(r#"{"layers": []}"#).unwrap().unwrap();
        assert_eq!(preset.emoji_pages, vec![EmojiPage::default_page()]);
    }
}
