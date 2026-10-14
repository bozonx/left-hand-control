//! `config.json`: `{ "version": 1, "settings": { … } }`.
//!
//! Reading is lenient: values
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

/// Reject unsupported configuration fields before loading settings.
pub fn validate(raw: Option<&Value>) -> Result<(), String> {
    let Some(raw) = raw.and_then(Value::as_object) else {
        return Ok(());
    };
    for field in ["layoutConditions", "commandTrust", "commandTimeoutSecs"] {
        if raw.contains_key(field) {
            return Err(format!("unsupported settings field: {field}"));
        }
    }
    if let Some(rules) = raw.get("autoRules").and_then(Value::as_array) {
        for rule in rules {
            if rule.get("apps").is_some() {
                return Err("unsupported auto-rule field: apps; use windows".into());
            }
        }
    }
    Ok(())
}

/// Normalised settings from the `settings` object of `config.json`.
pub fn from_value(raw: Option<&Value>) -> AppSettings {
    let base = AppSettings::default();
    let Some(raw) = raw.and_then(Value::as_object) else {
        return base;
    };
    let game_mode = raw.get("gameMode").and_then(Value::as_object);
    let mut settings = AppSettings {
        launch_on_startup: bool_or(raw, "launchOnStartup", base.launch_on_startup),
        start_mapper_on_launch: bool_or(raw, "startMapperOnLaunch", base.start_mapper_on_launch),
        appearance: match str_of(raw, "appearance") {
            Some("light") => Appearance::Light,
            Some("dark") => Appearance::Dark,
            Some("eink") => Appearance::EInk,
            _ => Appearance::System,
        },
        high_contrast: bool_or(raw, "highContrast", base.high_contrast),
        reduce_motion: bool_or(raw, "reduceMotion", base.reduce_motion),
        layout_osd: bool_or(raw, "layoutOsd", base.layout_osd),
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
        combo_timeout_ms: u64_or(raw, "comboTimeoutMs", base.combo_timeout_ms),
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
        settings.game_mode.use_steam = bool_or(game_mode, "useSteam", base.game_mode.use_steam);
        settings.game_mode.use_lutris = bool_or(game_mode, "useLutris", base.game_mode.use_lutris);
        settings.game_mode.use_gamescope =
            bool_or(game_mode, "useGamescope", base.game_mode.use_gamescope);
        settings.game_mode.use_app_category =
            bool_or(game_mode, "useAppCategory", base.game_mode.use_app_category);
        settings.game_mode.use_game_bar =
            bool_or(game_mode, "useGameBar", base.game_mode.use_game_bar);
        settings.game_mode.control = match str_of(game_mode, "control") {
            Some("on") => GameModeControl::On,
            Some("off") => GameModeControl::Off,
            _ => GameModeControl::Auto,
        };
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
    settings.auto_rules = raw
        .get("autoRules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(auto_rule)
        .collect();
    settings.auto_default_layout_id = str_of(raw, "autoDefaultLayoutId")
        .filter(|id| !id.is_empty())
        .map(str::to_owned);
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

fn condition_set(value: &Value) -> Option<LayoutConditionSet> {
    let value = value.as_object()?;
    let game_mode = str_of(value, "gameMode")
        .filter(|mode| matches!(*mode, "on" | "off"))
        .map(str::to_owned);
    let layouts: Vec<String> = string_list(value.get("layouts"))
        .into_iter()
        .filter(|layout| !layout.is_empty())
        .collect();
    let apps: Vec<String> = string_list(value.get("windows"))
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
            "gameMode": {"useGamemoded": false, "useLutris": false, "processMatchers": [{"name": " steam "}, 3]},
        })));
        assert_eq!(settings.appearance, Appearance::Dark);
        assert_eq!(settings.locale, LocalePreference::Auto);
        assert_eq!(settings.default_hold_timeout_ms, 200);
        assert_eq!(settings.tap_decision, "holdOnOtherKeyPress");
        assert_eq!(settings.linux_wayland_text_mode, None);
        assert!(!settings.game_mode.use_gamemoded);
        assert!(!settings.game_mode.use_lutris);
        assert!(settings.game_mode.use_steam);
        assert!(AppSettings::default().game_mode.use_lutris);
        assert_eq!(settings.game_mode.process_matchers.len(), 1);
        assert_eq!(settings.game_mode.process_matchers[0].name, "steam");
        assert_eq!(settings.game_mode.process_matchers[0].id, "process-0");
        assert!(settings.game_mode.process_matchers[0].only_active_window);
    }

    #[test]
    fn mapper_start_on_launch_is_off_by_default() {
        assert!(!AppSettings::default().start_mapper_on_launch);
        let settings = from_value(Some(&json!({"startMapperOnLaunch": true})));
        assert!(settings.start_mapper_on_launch);
    }

    #[test]
    fn manual_activation_is_independent_of_the_edited_layout() {
        let settings = from_value(Some(&json!({"currentLayoutId": "user:Main"})));
        assert_eq!(settings.current_layout_id.as_deref(), Some("user:Main"));
        assert_eq!(settings.manual_active_layout_id, None);
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
}
