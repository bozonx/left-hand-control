use crate::app_storage;
use lhc_core::mapper_config::AppConfig;
use serde_json::{Value, json};

pub const KEY_CODES: [&str; 80] = [
    "Escape",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "PrintScreen",
    "ScrollLock",
    "Pause",
    "Backquote",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Digit0",
    "Minus",
    "Equal",
    "Backspace",
    "Insert",
    "Home",
    "Tab",
    "KeyQ",
    "KeyW",
    "KeyE",
    "KeyR",
    "KeyT",
    "KeyY",
    "KeyU",
    "KeyI",
    "KeyO",
    "KeyP",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Delete",
    "End",
    "CapsLock",
    "KeyA",
    "KeyS",
    "KeyD",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyJ",
    "KeyK",
    "KeyL",
    "Semicolon",
    "Quote",
    "Enter",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ShiftLeft",
    "KeyZ",
    "KeyX",
    "KeyC",
    "KeyV",
    "KeyB",
    "KeyN",
    "KeyM",
    "Comma",
    "Period",
    "Slash",
    "AltLeft",
    "Space",
    "ArrowLeft",
    "ArrowDown",
    "ArrowRight",
];

pub struct ConfigState {
    value: Value,
    config: AppConfig,
}

fn base_rule(rule: &Value, key: &str) -> bool {
    rule.get("key").and_then(Value::as_str) == Some(key)
        && rule
            .get("layerId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
        && [
            "conditionGameMode",
            "conditionLayouts",
            "conditionAppsWhitelist",
            "conditionAppsBlacklist",
        ]
        .iter()
        .all(|name| rule.get(*name).is_none_or(Value::is_null))
}

impl ConfigState {
    pub fn load() -> Result<Self, String> {
        let (value, config) = app_storage::load_config()?;
        Ok(Self { value, config })
    }

    pub fn raw(&self) -> Result<String, String> {
        serde_json::to_string(&self.value).map_err(|error| error.to_string())
    }

    pub fn input_device(&self) -> Option<&str> {
        self.value
            .pointer("/settings/inputDevicePath")?
            .as_str()
            .filter(|value| !value.is_empty())
    }

    pub fn mouse_device(&self) -> Option<&str> {
        self.value
            .pointer("/settings/inputMouseDevicePath")?
            .as_str()
            .filter(|value| !value.is_empty())
    }

    pub fn rule_count(&self) -> usize {
        self.config.rules.len()
    }

    pub fn action(&self, key: &str) -> Option<String> {
        self.value
            .get("rules")?
            .as_array()?
            .iter()
            .find(|rule| base_rule(rule, key))?
            .get("tapAction")?
            .as_str()
            .map(str::to_owned)
    }

    pub fn save_action(&mut self, key: &str, action: &str) -> Result<(), String> {
        self.ensure_unchanged()?;
        let candidate = self.updated_action(key, action)?;
        let config = app_storage::parse_config(&candidate)?;
        app_storage::save_config(&candidate)?;
        self.value = candidate;
        self.config = config;
        Ok(())
    }

    pub fn save_device(&mut self, path: &str) -> Result<(), String> {
        self.ensure_unchanged()?;
        let mut candidate = self.value.clone();
        let root = candidate
            .as_object_mut()
            .ok_or("config.json must contain an object")?;
        let settings = root
            .entry("settings")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("settings must be an object")?;
        settings.insert("inputDevicePath".into(), json!(path));
        let config = app_storage::parse_config(&candidate)?;
        app_storage::save_config(&candidate)?;
        self.value = candidate;
        self.config = config;
        Ok(())
    }

    fn ensure_unchanged(&self) -> Result<(), String> {
        let (current, _) = app_storage::load_config()?;
        if current != self.value {
            return Err(
                "Конфигурация изменена другим приложением; перезапустите Slint перед сохранением"
                    .into(),
            );
        }
        Ok(())
    }

    fn updated_action(&self, key: &str, action: &str) -> Result<Value, String> {
        let mut candidate = self.value.clone();
        let root = candidate
            .as_object_mut()
            .ok_or("config.json must contain an object")?;
        let rules = root
            .entry("rules")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or("rules must be an array")?;
        if let Some(rule) = rules.iter_mut().find(|rule| base_rule(rule, key)) {
            rule.as_object_mut()
                .ok_or("rule must be an object")?
                .insert("tapAction".into(), json!(action));
        } else if rules
            .iter()
            .any(|rule| rule.get("key").and_then(Value::as_str) == Some(key))
        {
            return Err(format!(
                "Клавиша {key} имеет только условные или послойные правила; отредактируйте её в полном редакторе"
            ));
        } else {
            rules.push(json!({ "enabled": true, "key": key, "tapAction": action }));
        }
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_codes_match_linux_mapper() {
        assert_eq!(KEY_CODES.len(), 80);
        #[cfg(target_os = "linux")]
        for code in KEY_CODES {
            assert!(
                lhc_core::mapper::keys::code_to_key(code).is_some(),
                "{code}"
            );
        }
    }

    #[test]
    fn updating_rule_preserves_other_config_fields() {
        let value = json!({"version": 1, "settings": {"inputDevicePath": "/dev/input/event3", "appearance": "dark"}, "rules": [{"key": "KeyQ", "tapAction": "Escape", "holdAction": "ControlLeft", "enabled": false}]});
        let state = ConfigState {
            config: app_storage::parse_config(&value).unwrap(),
            value: value.clone(),
        };
        let changed = state.updated_action("KeyQ", "text:hello").unwrap();
        assert_eq!(changed["rules"][0]["tapAction"], "text:hello");
        assert_eq!(changed["rules"][0]["holdAction"], "ControlLeft");
        assert_eq!(changed["rules"][0]["enabled"], false);
        assert_eq!(changed["settings"], value["settings"]);
        assert!(app_storage::parse_config(&changed).is_ok());
    }

    #[test]
    fn conditional_rule_is_not_overwritten() {
        let value =
            json!({"rules": [{"key": "KeyQ", "tapAction": "Escape", "conditionLayouts": ["us"]}]});
        let state = ConfigState {
            config: app_storage::parse_config(&value).unwrap(),
            value,
        };
        assert!(state.updated_action("KeyQ", "text:hello").is_err());
    }

    #[test]
    fn base_rule_can_change_beside_layer_rule() {
        let value = json!({"rules": [
            {"key": "KeyQ", "tapAction": "Escape"},
            {"key": "KeyQ", "layerId": "layer-a", "tapAction": "Tab"}
        ], "layerKeymaps": {"layer-a": {"keys": {}}}});
        let state = ConfigState {
            config: app_storage::parse_config(&value).unwrap(),
            value,
        };
        let changed = state.updated_action("KeyQ", "text:hello").unwrap();
        assert_eq!(changed["rules"][0]["tapAction"], "text:hello");
        assert_eq!(changed["rules"][1]["tapAction"], "Tab");
    }
}
