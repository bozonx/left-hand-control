//! `config.json`: `{ "version": 1, "settings": { … } }`.
//!
//! Reading is lenient like `normalizeSettings()` in the frontend: values
//! of the wrong type fall back to defaults instead of failing the load.

use super::model::{
    AppSettings, Appearance, AutoRule, GameModeControl, GameModeProcessMatcher, LayoutConditionSet,
    LayoutMode, LocalePreference,
};
use serde_json::{Map, Value};

const TEXT_MODES: [&str; 6] = [
    "libei",
    "libei-pure",
    "keycode",
    "clipboard",
    "ydotool",
    "xdotool",
];

/// Normalised settings from the `settings` object of `config.json`.
pub fn from_value(raw: Option<&Value>) -> AppSettings {
    let base = AppSettings::default();
    let Some(raw) = raw.and_then(Value::as_object) else {
        return base;
    };
    let game_mode = raw.get("gameMode").and_then(Value::as_object);
    let mut settings = AppSettings {
        launch_on_startup: bool_or(raw, "launchOnStartup", base.launch_on_startup),
        appearance: match str_of(raw, "appearance") {
            Some("light") => Appearance::Light,
            Some("dark") => Appearance::Dark,
            Some("eink") => Appearance::EInk,
            _ => Appearance::System,
        },
        high_contrast: bool_or(raw, "highContrast", base.high_contrast),
        reduce_motion: bool_or(raw, "reduceMotion", base.reduce_motion),
        locale: match str_of(raw, "locale") {
            Some("en-US") => LocalePreference::English,
            Some("ru-RU") => LocalePreference::Russian,
            _ => LocalePreference::Auto,
        },
        default_long_hold_timeout_ms: u64_or(
            raw,
            "defaultLongHoldTimeoutMs",
            base.default_long_hold_timeout_ms,
        ),
        default_hold_timeout_ms: u64_or(raw, "defaultHoldTimeoutMs", base.default_hold_timeout_ms),
        tap_decision: match str_of(raw, "tapDecision") {
            Some(value @ ("permissiveHold" | "holdOnOtherKeyPress")) => value.into(),
            _ => base.tap_decision,
        },
        default_double_tap_timeout_ms: u64_or(
            raw,
            "defaultDoubleTapTimeoutMs",
            base.default_double_tap_timeout_ms,
        ),
        default_macro_step_pause_ms: u64_or(
            raw,
            "defaultMacroStepPauseMs",
            base.default_macro_step_pause_ms,
        ),
        default_macro_modifier_delay_ms: u64_or(
            raw,
            "defaultMacroModifierDelayMs",
            base.default_macro_modifier_delay_ms,
        ),
        input_device_path: match raw.get("inputDevicePath") {
            Some(Value::String(path)) => Some(path.clone()),
            _ => base.input_device_path,
        },
        input_mouse_device_path: str_of(raw, "inputMouseDevicePath").map(str::to_owned),
        current_layout_id: str_of(raw, "currentLayoutId").map(str::to_owned),
        layout_mode: if str_of(raw, "layoutMode") == Some("auto") {
            LayoutMode::Auto
        } else {
            LayoutMode::Manual
        },
        manual_active_layout_id: str_of(raw, "manualActiveLayoutId")
            .filter(|id| !id.is_empty())
            .map(str::to_owned),
        layout_order: string_list(raw.get("layoutOrder")),
        auto_rules: Vec::new(),
        auto_default_layout_id: None,
        commands_enabled: bool_or(raw, "commandsEnabled", base.commands_enabled),
        game_mode: base.game_mode.clone(),
        linux_wayland_text_mode: str_of(raw, "linuxWaylandTextMode")
            .filter(|mode| TEXT_MODES.contains(mode))
            .map(str::to_owned),
        linux_ydotool_path: str_of(raw, "linuxYdotoolPath").unwrap_or("").into(),
        linux_xdotool_path: str_of(raw, "linuxXdotoolPath").unwrap_or("").into(),
    };
    if let Some(game_mode) = game_mode {
        settings.game_mode.use_gamemoded =
            bool_or(game_mode, "useGamemoded", base.game_mode.use_gamemoded);
        settings.game_mode.use_fullscreen =
            bool_or(game_mode, "useFullscreen", base.game_mode.use_fullscreen);
        settings.game_mode.remember_control = bool_or(
            game_mode,
            "rememberControl",
            base.game_mode.remember_control,
        );
        settings.game_mode.control = match str_of(game_mode, "control") {
            Some("on") => GameModeControl::On,
            Some("off") => GameModeControl::Off,
            _ => GameModeControl::Auto,
        };
        settings.game_mode.block_popups =
            bool_or(game_mode, "blockPopups", base.game_mode.block_popups);
        if let Some(matchers) = game_mode.get("processMatchers").and_then(Value::as_array) {
            settings.game_mode.process_matchers = matchers
                .iter()
                .filter_map(Value::as_object)
                .enumerate()
                .map(|(index, item)| GameModeProcessMatcher {
                    id: str_of(item, "id")
                        .filter(|id| !id.is_empty())
                        .map_or_else(|| format!("process-{index}"), str::to_owned),
                    name: str_of(item, "name").unwrap_or("").trim().into(),
                    only_active_window: item.get("onlyActiveWindow") != Some(&Value::Bool(false)),
                    is_blacklist: item.get("isBlacklist") == Some(&Value::Bool(true)),
                })
                .collect();
        }
    }
    match raw.get("autoRules").and_then(Value::as_array) {
        Some(rules) => {
            settings.auto_rules = rules.iter().enumerate().filter_map(auto_rule).collect();
            settings.auto_default_layout_id = str_of(raw, "autoDefaultLayoutId")
                .filter(|id| !id.is_empty())
                .map(str::to_owned);
        }
        None => {
            (settings.auto_rules, settings.auto_default_layout_id) =
                migrate_layout_conditions(raw, &settings.layout_order);
        }
    }
    if settings.layout_mode == LayoutMode::Manual && settings.manual_active_layout_id.is_none() {
        settings.manual_active_layout_id = settings
            .current_layout_id
            .clone()
            .filter(|id| !id.is_empty());
    }
    settings
}

fn auto_rule((index, value): (usize, &Value)) -> Option<AutoRule> {
    let value = value.as_object()?;
    Some(AutoRule {
        id: str_of(value, "id")
            .filter(|id| !id.is_empty())
            .map_or_else(|| format!("rule-{index}"), str::to_owned),
        layout_id: str_of(value, "layoutId")
            .filter(|id| !id.is_empty())
            .map(str::to_owned),
        enabled: value.get("enabled") != Some(&Value::Bool(false)),
        conditions: condition_set(&Value::Object(value.clone())).unwrap_or_default(),
    })
}

/// Rules equivalent to the legacy per-layout `layoutConditions`: layouts
/// included in auto mode become rules in library order, the first one
/// without a whitelist becomes the default (later ones never applied).
/// A blacklist turns into an "off" rule placed before the layout's rule;
/// that is exact unless a later layout would have matched instead.
fn migrate_layout_conditions(
    raw: &Map<String, Value>,
    order: &[String],
) -> (Vec<AutoRule>, Option<String>) {
    let Some(conditions) = raw.get("layoutConditions").and_then(Value::as_object) else {
        return (Vec::new(), None);
    };
    let mut ids: Vec<&String> = order
        .iter()
        .filter(|id| conditions.contains_key(*id))
        .collect();
    ids.extend(conditions.keys().filter(|id| !order.contains(id)));
    let mut rules = Vec::new();
    for id in ids {
        let Some(rule) = conditions[id].as_object() else {
            continue;
        };
        if rule.get("enabledInAuto") != Some(&Value::Bool(true)) {
            continue;
        }
        if let Some(blacklist) = rule.get("blacklist").and_then(condition_set) {
            rules.push(AutoRule {
                id: format!("rule-{}", rules.len()),
                enabled: true,
                layout_id: None,
                conditions: blacklist,
            });
        }
        match rule.get("whitelist").and_then(condition_set) {
            Some(whitelist) => rules.push(AutoRule {
                id: format!("rule-{}", rules.len()),
                enabled: true,
                layout_id: Some(id.clone()),
                conditions: whitelist,
            }),
            None => return (rules, Some(id.clone())),
        }
    }
    (rules, None)
}

fn condition_set(value: &Value) -> Option<LayoutConditionSet> {
    let value = value.as_object()?;
    let game_mode = str_of(value, "gameMode")
        .filter(|mode| matches!(*mode, "on" | "off"))
        .map(str::to_owned);
    let layouts: Vec<String> = string_list(value.get("layouts"))
        .into_iter()
        .filter(|layout| !layout.is_empty())
        .collect();
    // `apps` held substrings of the title or app id before patterns.
    let apps: Vec<String> = match value.get("windows") {
        Some(windows) => string_list(Some(windows)),
        None => super::app_match::list_from_legacy(&string_list(value.get("apps"))),
    }
    .into_iter()
    .filter(|app| !app.trim().is_empty())
    .collect();
    if game_mode.is_none() && layouts.is_empty() && apps.is_empty() {
        return None;
    }
    Some(LayoutConditionSet {
        game_mode,
        layouts,
        apps,
    })
}

fn str_of<'a>(object: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    object.get(name).and_then(Value::as_str)
}

fn bool_or(object: &Map<String, Value>, name: &str, default: bool) -> bool {
    object.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn u64_or(object: &Map<String, Value>, name: &str, default: u64) -> u64 {
    object.get(name).and_then(Value::as_u64).unwrap_or(default)
}

pub(crate) fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_settings_use_defaults() {
        assert_eq!(from_value(None), AppSettings::default());
        assert_eq!(from_value(Some(&json!("x"))), AppSettings::default());
    }

    #[test]
    fn invalid_values_fall_back_per_field() {
        let settings = from_value(Some(&json!({
            "appearance": "dark",
            "locale": "de-DE",
            "defaultHoldTimeoutMs": "fast",
            "tapDecision": "holdOnOtherKeyPress",
            "linuxWaylandTextMode": "magic",
            "gameMode": {"useGamemoded": false, "processMatchers": [{"name": " steam "}, 3]},
        })));
        assert_eq!(settings.appearance, Appearance::Dark);
        assert_eq!(settings.locale, LocalePreference::Auto);
        assert_eq!(settings.default_hold_timeout_ms, 200);
        assert_eq!(settings.tap_decision, "holdOnOtherKeyPress");
        assert_eq!(settings.linux_wayland_text_mode, None);
        assert!(!settings.game_mode.use_gamemoded);
        assert_eq!(settings.game_mode.process_matchers.len(), 1);
        assert_eq!(settings.game_mode.process_matchers[0].name, "steam");
        assert_eq!(settings.game_mode.process_matchers[0].id, "process-0");
        assert!(settings.game_mode.process_matchers[0].only_active_window);
    }

    #[test]
    fn manual_layout_falls_back_to_current_layout() {
        let settings = from_value(Some(&json!({"currentLayoutId": "user:Main"})));
        assert_eq!(
            settings.manual_active_layout_id.as_deref(),
            Some("user:Main")
        );
    }

    #[test]
    fn auto_rules_are_read_leniently() {
        let settings = from_value(Some(&json!({
            "autoRules": [
                {"id": "a", "layoutId": "user:A", "windows": ["blender"], "layouts": [""]},
                {"layoutId": null, "gameMode": "maybe", "enabled": false},
                3,
            ],
            "autoDefaultLayoutId": "user:B",
            "layoutConditions": {"user:C": {"enabledInAuto": true}},
        })));
        assert_eq!(settings.auto_rules.len(), 2);
        assert_eq!(settings.auto_rules[0].conditions.apps, ["blender"]);
        assert!(settings.auto_rules[0].conditions.layouts.is_empty());
        assert_eq!(settings.auto_rules[1].id, "rule-1");
        assert_eq!(settings.auto_rules[1].layout_id, None);
        assert!(settings.auto_rules[0].enabled);
        assert!(!settings.auto_rules[1].enabled);
        assert!(settings.auto_rules[1].conditions.is_empty());
        assert_eq!(settings.auto_default_layout_id.as_deref(), Some("user:B"));
    }

    #[test]
    fn legacy_layout_conditions_become_rules() {
        let settings = from_value(Some(&json!({
            "layoutOrder": ["user:B", "user:A"],
            "layoutConditions": {
                "user:A": {"enabledInAuto": true, "blacklist": {"apps": ["blender"]}},
                "user:B": {"enabledInAuto": true, "whitelist": {"layouts": ["ru"]}},
                "user:C": {"enabledInAuto": true},
                "user:D": {"whitelist": {"layouts": ["us"]}},
            },
        })));
        let targets: Vec<_> = settings
            .auto_rules
            .iter()
            .map(|rule| rule.layout_id.as_deref())
            .collect();
        assert_eq!(targets, [Some("user:B"), None]);
        assert_eq!(settings.auto_rules[0].conditions.layouts, ["ru"]);
        assert_eq!(
            settings.auto_rules[1].conditions.apps,
            ["*blender*", "title:blender"]
        );
        assert_eq!(settings.auto_default_layout_id.as_deref(), Some("user:A"));
        let none = from_value(Some(&json!({"layoutConditions": {
            "user:A": {"enabledInAuto": true, "whitelist": {"apps": ["kate"]}},
        }})));
        assert_eq!(none.auto_rules.len(), 1);
        assert_eq!(none.auto_default_layout_id, None);
    }
}
