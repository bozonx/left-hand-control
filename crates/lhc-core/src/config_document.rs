//! Editable `config.json` shared by every shell.
//!
//! The document keeps the raw JSON value next to the typed [`AppConfig`]:
//! edits change only the fields they own, so settings and sections that a
//! shell does not understand yet survive a save unchanged. Every commit is
//! parsed and validated first, refuses to overwrite a file another process
//! changed since it was read, and refreshes the game-mode watcher settings.
//!
//! Editing operations for the rest of the product (layers, macros, commands,
//! quick actions, …) belong here as well, so UI code only presents them.

use crate::mapper_config::AppConfig;
use crate::storage::StoragePaths;
use serde_json::{Value, json};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Reading or writing the file failed.
    Io(String),
    /// The JSON cannot be parsed into the config model.
    Parse(String),
    /// The config parsed but failed validation.
    Invalid(String),
    /// Another process changed the file after this document read it.
    ExternalChange,
    /// The key only has conditional or per-layer rules, which the simple
    /// editor must not overwrite.
    ConditionalRules { key: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "config I/O: {error}"),
            Self::Parse(error) => write!(f, "parse config.json: {error}"),
            Self::Invalid(error) => write!(f, "invalid config: {error}"),
            Self::ExternalChange => write!(f, "config.json was changed by another process"),
            Self::ConditionalRules { key } => write!(
                f,
                "key {key} only has conditional or layer rules; edit it in the full editor"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

pub struct ConfigDocument {
    paths: StoragePaths,
    value: Value,
    config: AppConfig,
    /// File contents this document is based on, for external-change checks.
    disk: String,
}

impl ConfigDocument {
    pub fn load(paths: StoragePaths) -> Result<Self, ConfigError> {
        let disk = paths.load_config().map_err(ConfigError::Io)?;
        let value = parse_raw(&disk)?;
        let config = parse(&value)?;
        crate::gamemode::update_settings_from_config_json(&disk);
        Ok(Self {
            paths,
            value,
            config,
            disk,
        })
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Compact JSON for the mapper runtime.
    pub fn raw(&self) -> String {
        self.value.to_string()
    }

    pub fn rule_count(&self) -> usize {
        self.config.rules.len()
    }

    pub fn input_device(&self) -> Option<&str> {
        self.setting_str("inputDevicePath")
    }

    pub fn mouse_device(&self) -> Option<&str> {
        self.setting_str("inputMouseDevicePath")
    }

    /// Tap action of the unconditional base-layer rule for `key`.
    pub fn base_tap_action(&self, key: &str) -> Option<String> {
        self.value
            .get("rules")?
            .as_array()?
            .iter()
            .find(|rule| is_base_rule(rule, key))?
            .get("tapAction")?
            .as_str()
            .map(str::to_owned)
    }

    pub fn set_base_tap_action(&mut self, key: &str, action: &str) -> Result<(), ConfigError> {
        let candidate = with_base_tap_action(&self.value, key, action)?;
        self.commit(candidate)
    }

    pub fn set_input_device(&mut self, path: &str) -> Result<(), ConfigError> {
        let mut candidate = self.value.clone();
        let settings = object_entry(&mut candidate, "settings")?;
        settings.insert("inputDevicePath".into(), json!(path));
        self.commit(candidate)
    }

    /// Re-read the file if another process changed it. Returns `true` when
    /// the document now reflects new contents.
    pub fn reload_if_changed(&mut self) -> Result<bool, ConfigError> {
        let disk = self.paths.load_config().map_err(ConfigError::Io)?;
        if disk == self.disk {
            return Ok(false);
        }
        let value = parse_raw(&disk)?;
        let config = parse(&value)?;
        crate::gamemode::update_settings_from_config_json(&disk);
        self.value = value;
        self.config = config;
        self.disk = disk;
        Ok(true)
    }

    fn setting_str(&self, name: &str) -> Option<&str> {
        self.value
            .get("settings")?
            .get(name)?
            .as_str()
            .filter(|value| !value.is_empty())
    }

    fn commit(&mut self, candidate: Value) -> Result<(), ConfigError> {
        let config = parse(&candidate)?;
        let current = self.paths.load_config().map_err(ConfigError::Io)?;
        if current != self.disk {
            return Err(ConfigError::ExternalChange);
        }
        let raw = serde_json::to_string_pretty(&candidate)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        self.paths.save_config(&raw).map_err(ConfigError::Io)?;
        crate::gamemode::update_settings_from_config_json(&raw);
        self.value = candidate;
        self.config = config;
        self.disk = raw;
        Ok(())
    }
}

fn parse_raw(raw: &str) -> Result<Value, ConfigError> {
    if raw.trim().is_empty() {
        return Ok(json!({"version": 1, "rules": [], "settings": {}}));
    }
    serde_json::from_str(raw).map_err(|error| ConfigError::Parse(error.to_string()))
}

/// Parse and validate a config value.
pub fn parse(value: &Value) -> Result<AppConfig, ConfigError> {
    let config: AppConfig = serde_json::from_value(value.clone())
        .map_err(|error| ConfigError::Parse(error.to_string()))?;
    #[cfg(target_os = "linux")]
    crate::mapper::validation::validate_config(&config).map_err(ConfigError::Invalid)?;
    Ok(config)
}

fn is_base_rule(rule: &Value, key: &str) -> bool {
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

fn object_entry<'a>(
    value: &'a mut Value,
    name: &str,
) -> Result<&'a mut serde_json::Map<String, Value>, ConfigError> {
    value
        .as_object_mut()
        .ok_or_else(|| ConfigError::Invalid("config.json must contain an object".into()))?
        .entry(name)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| ConfigError::Invalid(format!("{name} must be an object")))
}

fn with_base_tap_action(value: &Value, key: &str, action: &str) -> Result<Value, ConfigError> {
    let mut candidate = value.clone();
    let rules = candidate
        .as_object_mut()
        .ok_or_else(|| ConfigError::Invalid("config.json must contain an object".into()))?
        .entry("rules")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| ConfigError::Invalid("rules must be an array".into()))?;
    if let Some(rule) = rules.iter_mut().find(|rule| is_base_rule(rule, key)) {
        rule.as_object_mut()
            .ok_or_else(|| ConfigError::Invalid("rule must be an object".into()))?
            .insert("tapAction".into(), json!(action));
    } else if rules
        .iter()
        .any(|rule| rule.get("key").and_then(Value::as_str) == Some(key))
    {
        return Err(ConfigError::ConditionalRules { key: key.into() });
    } else {
        rules.push(json!({ "enabled": true, "key": key, "tapAction": action }));
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(value: Value) -> (tempfile::TempDir, ConfigDocument) {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_config(&serde_json::to_string_pretty(&value).unwrap())
            .unwrap();
        let document = ConfigDocument::load(paths).unwrap();
        (dir, document)
    }

    #[test]
    fn updating_rule_preserves_other_config_fields() {
        let value = json!({"version": 1, "settings": {"inputDevicePath": "/dev/input/event3", "appearance": "dark"}, "rules": [{"key": "KeyQ", "tapAction": "Escape", "holdAction": "ControlLeft", "enabled": false}]});
        let changed = with_base_tap_action(&value, "KeyQ", "text:hello").unwrap();
        assert_eq!(changed["rules"][0]["tapAction"], "text:hello");
        assert_eq!(changed["rules"][0]["holdAction"], "ControlLeft");
        assert_eq!(changed["rules"][0]["enabled"], false);
        assert_eq!(changed["settings"], value["settings"]);
        assert!(parse(&changed).is_ok());
    }

    #[test]
    fn conditional_rule_is_not_overwritten() {
        let value =
            json!({"rules": [{"key": "KeyQ", "tapAction": "Escape", "conditionLayouts": ["us"]}]});
        assert_eq!(
            with_base_tap_action(&value, "KeyQ", "text:hello"),
            Err(ConfigError::ConditionalRules { key: "KeyQ".into() })
        );
    }

    #[test]
    fn base_rule_can_change_beside_layer_rule() {
        let value = json!({"rules": [
            {"key": "KeyQ", "tapAction": "Escape"},
            {"key": "KeyQ", "layerId": "layer-a", "tapAction": "Tab"}
        ], "layerKeymaps": {"layer-a": {"keys": {}}}});
        let changed = with_base_tap_action(&value, "KeyQ", "text:hello").unwrap();
        assert_eq!(changed["rules"][0]["tapAction"], "text:hello");
        assert_eq!(changed["rules"][1]["tapAction"], "Tab");
    }

    #[test]
    fn commit_persists_and_reads_back() {
        let (_dir, mut document) = document(json!({"rules": [], "settings": {}}));
        document.set_base_tap_action("KeyA", "text:hi").unwrap();
        document.set_input_device("/dev/input/event7").unwrap();
        let reloaded = ConfigDocument::load(document.paths.clone()).unwrap();
        assert_eq!(reloaded.base_tap_action("KeyA").as_deref(), Some("text:hi"));
        assert_eq!(reloaded.input_device(), Some("/dev/input/event7"));
        assert_eq!(reloaded.rule_count(), 1);
    }

    #[test]
    fn external_change_blocks_commit_until_reload() {
        let (_dir, mut document) = document(json!({"rules": []}));
        document
            .paths
            .save_config(r#"{"rules": [{"key": "KeyB", "tapAction": "Tab"}]}"#)
            .unwrap();
        assert_eq!(
            document.set_base_tap_action("KeyA", "text:hi"),
            Err(ConfigError::ExternalChange)
        );
        assert!(document.reload_if_changed().unwrap());
        assert!(!document.reload_if_changed().unwrap());
        assert_eq!(document.base_tap_action("KeyB").as_deref(), Some("Tab"));
        document.set_base_tap_action("KeyA", "text:hi").unwrap();
        assert_eq!(document.rule_count(), 2);
    }

    #[test]
    fn empty_file_loads_as_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let document = ConfigDocument::load(paths).unwrap();
        assert_eq!(document.rule_count(), 0);
        assert_eq!(document.input_device(), None);
    }
}
