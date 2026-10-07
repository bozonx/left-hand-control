//! `config.json`: `{ "version": 1, "settings": { … } }`.
//!
//! Reading is lenient like `normalizeSettings()` in the frontend: values
//! of the wrong type fall back to defaults instead of failing the load.

use super::model::{
    AppSettings, Appearance, CommandTrustEntry, GameModeProcessMatcher, LayoutConditionRule,
    LayoutConditionSet, LayoutMode, LocalePreference,
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
        locale: match str_of(raw, "locale") {
            Some("en-US") => LocalePreference::English,
            Some("ru-RU") => LocalePreference::Russian,
            _ => LocalePreference::Auto,
        },
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
        layout_conditions: raw
            .get("layoutConditions")
            .and_then(Value::as_object)
            .map(|conditions| {
                conditions
                    .iter()
                    .filter_map(|(id, value)| Some((id.clone(), condition_rule(value)?)))
                    .collect()
            })
            .unwrap_or_default(),
        command_trust: raw
            .get("commandTrust")
            .and_then(Value::as_object)
            .map(|trust| {
                trust
                    .iter()
                    .filter(|(id, _)| !id.is_empty())
                    .filter_map(|(id, value)| {
                        let value = value.as_object()?;
                        let fingerprint = str_of(value, "fingerprint").filter(|f| !f.is_empty())?;
                        Some((
                            id.clone(),
                            CommandTrustEntry {
                                fingerprint: fingerprint.into(),
                                trusted_at: str_of(value, "trustedAt").unwrap_or("").into(),
                            },
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        command_timeout_secs: u64_or(raw, "commandTimeoutSecs", base.command_timeout_secs)
            .clamp(1, i32::MAX as u64),
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
    if settings.layout_mode == LayoutMode::Manual && settings.manual_active_layout_id.is_none() {
        settings.manual_active_layout_id = settings
            .current_layout_id
            .clone()
            .filter(|id| !id.is_empty());
    }
    settings
}

fn condition_rule(value: &Value) -> Option<LayoutConditionRule> {
    let value = value.as_object()?;
    let rule = LayoutConditionRule {
        enabled_in_auto: value.get("enabledInAuto") == Some(&Value::Bool(true)),
        whitelist: value.get("whitelist").and_then(condition_set),
        blacklist: value.get("blacklist").and_then(condition_set),
    };
    (rule.enabled_in_auto || rule.whitelist.is_some() || rule.blacklist.is_some()).then_some(rule)
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
    let apps: Vec<String> = string_list(value.get("apps"))
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
    fn empty_condition_sets_are_dropped() {
        let settings = from_value(Some(&json!({"layoutConditions": {
            "user:A": {"whitelist": {"layouts": []}},
            "user:B": {"enabledInAuto": true, "blacklist": {"gameMode": "on", "layouts": ["", "us"]}},
        }})));
        assert!(!settings.layout_conditions.contains_key("user:A"));
        let rule = &settings.layout_conditions["user:B"];
        assert!(rule.enabled_in_auto);
        assert_eq!(rule.blacklist.as_ref().unwrap().layouts, vec!["us"]);
    }
}
