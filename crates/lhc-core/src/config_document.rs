//! Editable configuration shared by every shell.
//!
//! The configuration lives in two files, exactly as the Tauri frontend
//! stores it: global settings in `config.json` and the current keyboard
//! layout in `current-layout.yaml`. Saved layouts live in the user
//! library (`layouts/<name>.yaml`, id `user:<name>`).
//!
//! Settings keep the raw JSON next to the typed view, so edits change only
//! the fields they own and settings a shell does not know survive a save.
//! Every save goes through [`TrackedFile`], which refuses to overwrite a
//! file another process changed since this document read it.

use crate::profile::actions::{self, Action, ActionIssue};
use crate::profile::auto_switch::{self, AutoSwitchContext};
use crate::profile::diagnostics::{self, RuleIssue};
use crate::profile::model::{
    AppConfig, AppSettings, Appearance, ExtraKey, Layer, LayerRule, LayoutMode, LayoutPreset, LocalePreference,
    USER_LAYOUT_PREFIX,
};
use crate::profile::{layout_file, settings};
use crate::storage::{StoragePaths, TrackedFile, WriteError};
use serde_json::{Value, json};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Reading or writing a file failed.
    Io(String),
    /// A file cannot be parsed.
    Parse(String),
    /// The configuration is not usable by the mapper.
    Invalid(String),
    /// An action cannot be assigned.
    InvalidAction(ActionIssue),
    /// Rules have blocking problems; the mapper cannot start.
    Rules(Vec<RuleIssue>),
    /// Another process changed a file after this document read it.
    ExternalChange,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "config I/O: {error}"),
            Self::Parse(error) => write!(f, "parse config: {error}"),
            Self::Invalid(error) => write!(f, "invalid config: {error}"),
            Self::InvalidAction(issue) => write!(f, "invalid action: {issue:?}"),
            Self::Rules(issues) => match issues.first() {
                Some(issue) => write!(f, "{issue}"),
                None => write!(f, "invalid rules"),
            },
            Self::ExternalChange => write!(f, "configuration was changed by another process"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<WriteError> for ConfigError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::ExternalChange => Self::ExternalChange,
            WriteError::Io(error) => Self::Io(error),
        }
    }
}

/// What a key does inside a layer keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAssignment {
    /// No entry: the base layout decides.
    Transparent,
    /// `null`: the key does nothing.
    Swallow,
    Action(String),
}

/// Mapper input computed from the document for the current system state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// JSON for [`crate::mapper::runtime::start`] / `update_config`.
    pub json: String,
    /// Layout the rules come from; `None` means native passthrough.
    pub layout_id: Option<String>,
}

pub struct ConfigDocument {
    paths: StoragePaths,
    settings_file: TrackedFile,
    settings_raw: Value,
    settings: AppSettings,
    layout_file: TrackedFile,
    layout: LayoutPreset,
}

impl ConfigDocument {
    pub fn load(paths: StoragePaths) -> Result<Self, ConfigError> {
        paths.ensure().map_err(ConfigError::Io)?;
        let (settings_file, settings_text) =
            TrackedFile::open(paths.config_path()).map_err(ConfigError::Io)?;
        let (layout_file, layout_text) =
            TrackedFile::open(paths.current_layout_path()).map_err(ConfigError::Io)?;
        let settings_raw = parse_settings(&settings_text)?;
        let layout = parse_layout(&layout_text)?;
        crate::gamemode::update_settings_from_config_json(&settings_text);
        Ok(Self {
            settings: settings::from_value(settings_raw.get("settings")),
            settings_raw,
            settings_file,
            layout,
            layout_file,
            paths,
        })
    }

    pub fn paths(&self) -> &StoragePaths {
        &self.paths
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn layout(&self) -> &LayoutPreset {
        &self.layout
    }

    /// Settings and the current layout combined.
    pub fn config(&self) -> AppConfig {
        AppConfig::from_parts(
            self.settings.clone(),
            self.layout.clone(),
            self.settings.current_layout_id.as_deref(),
        )
    }

    pub fn input_device(&self) -> Option<&str> {
        self.settings
            .input_device_path
            .as_deref()
            .filter(|path| !path.is_empty())
    }

    pub fn mouse_device(&self) -> Option<&str> {
        self.settings
            .input_mouse_device_path
            .as_deref()
            .filter(|path| !path.is_empty())
    }

    pub fn set_input_device(&mut self, path: &str) -> Result<(), ConfigError> {
        self.set_setting("inputDevicePath", json!(path))
    }

    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<(), ConfigError> {
        self.set_setting("appearance", json!(appearance.as_str()))
    }

    pub fn set_locale(&mut self, locale: LocalePreference) -> Result<(), ConfigError> {
        self.set_setting("locale", json!(locale.as_str()))
    }

    pub fn update_settings(
        &mut self,
        edit: impl FnOnce(&mut AppSettings),
    ) -> Result<(), ConfigError> {
        let mut updated = self.settings.clone();
        edit(&mut updated);
        let values = serde_json::to_value(&updated)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        let mut candidate = self.settings_raw.clone();
        let object = candidate
            .as_object_mut()
            .ok_or_else(|| ConfigError::Invalid("config.json must contain an object".into()))?;
        object.entry("version").or_insert(json!(1));
        let settings = object
            .entry("settings")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| ConfigError::Invalid("settings must be an object".into()))?;
        for (name, value) in values.as_object().into_iter().flatten() {
            settings.insert(name.clone(), value.clone());
        }
        let text = serde_json::to_string_pretty(&candidate)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        self.settings_file.write(&text)?;
        crate::gamemode::update_settings_from_config_json(&text);
        self.settings = updated;
        self.settings_raw = candidate;
        Ok(())
    }

    pub fn base_tap_action(&self, key: &str) -> Option<&str> {
        self.layout
            .rules
            .iter()
            .find(|rule| rule.key == key && rule.layer_id.is_empty() && rule.is_enabled())
            .and_then(|rule| rule.tap_action.as_deref())
    }

    pub fn set_base_tap_action(&mut self, key: &str, action: &str) -> Result<(), ConfigError> {
        if let Some(issue) = actions::validate(&Action::parse(Some(action)), &self.config()) {
            return Err(ConfigError::InvalidAction(issue));
        }
        let matching: Vec<_> = self
            .layout
            .rules
            .iter()
            .filter(|rule| rule.key == key && rule.is_enabled())
            .collect();
        if matching.iter().any(|rule| {
            !rule.layer_id.is_empty()
                || rule.condition_game_mode.is_some()
                || rule.condition_layouts.is_some()
                || rule.condition_apps_whitelist.is_some()
                || rule.condition_apps_blacklist.is_some()
                || rule
                    .hold_action
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                || !rule.double_tap_action.is_empty()
        }) || matching.len() > 1
        {
            return Err(ConfigError::Invalid(format!(
                "key {key} has a rule that requires the full editor"
            )));
        }
        self.update_layout(|layout| {
            if let Some(rule) = layout
                .rules
                .iter_mut()
                .find(|rule| rule.key == key && rule.is_enabled())
            {
                rule.tap_action = Some(action.into());
            } else {
                let mut index = 1;
                let id = loop {
                    let id = format!("slint-key-{index}");
                    if layout.rules.iter().all(|rule| rule.id != id) {
                        break id;
                    }
                    index += 1;
                };
                let mut rule = LayerRule::new(id, key);
                rule.tap_action = Some(action.into());
                layout.rules.push(rule);
            }
        })
    }

    pub fn create_layer(&mut self, name: &str, description: &str) -> Result<String, ConfigError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ConfigError::Invalid("layer name is required".into()));
        }
        let id = crate::profile::ids::generate("l_");
        self.update_layout(|layout| {
            layout.layers.push(Layer {
                id: id.clone(),
                name: name.into(),
                description: optional_text(description),
            });
            layout.layer_keymap_mut(&id);
        })?;
        Ok(id)
    }

    pub fn rename_layer(
        &mut self,
        id: &str,
        name: &str,
        description: &str,
    ) -> Result<(), ConfigError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ConfigError::Invalid("layer name is required".into()));
        }
        if !self.layout.layers.iter().any(|layer| layer.id == id) {
            return Err(ConfigError::Invalid("unknown layer".into()));
        }
        self.update_layout(|layout| {
            let layer = layout
                .layers
                .iter_mut()
                .find(|layer| layer.id == id)
                .unwrap();
            layer.name = name.into();
            layer.description = optional_text(description);
        })
    }

    pub fn clone_layer(
        &mut self,
        source_id: &str,
        name: &str,
        description: &str,
    ) -> Result<String, ConfigError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ConfigError::Invalid("layer name is required".into()));
        }
        if !self.layout.layers.iter().any(|layer| layer.id == source_id) {
            return Err(ConfigError::Invalid("unknown layer".into()));
        }
        let id = crate::profile::ids::generate("l_");
        self.update_layout(|layout| {
            let mut keymap = layout
                .layer_keymaps
                .get(source_id)
                .cloned()
                .unwrap_or_default();
            for extra in &mut keymap.extras {
                extra.id = crate::profile::ids::generate("e_");
            }
            layout.layer_keymaps.insert(id.clone(), keymap);
            layout.layers.push(Layer {
                id: id.clone(),
                name: name.into(),
                description: optional_text(description),
            });
        })?;
        Ok(id)
    }

    pub fn delete_layer(&mut self, id: &str) -> Result<(), ConfigError> {
        if !self.layout.layers.iter().any(|layer| layer.id == id) {
            return Err(ConfigError::Invalid("unknown layer".into()));
        }
        self.update_layout(|layout| {
            layout.layers.retain(|layer| layer.id != id);
            layout.layer_keymaps.remove(id);
            for rule in &mut layout.rules {
                if rule.layer_id == id {
                    rule.layer_id.clear();
                }
            }
        })
    }

    pub fn clear_layer_keys(&mut self, id: &str) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        self.update_layout(|layout| layout.layer_keymap_mut(id).keys.clear())
    }

    pub fn set_layer_extra(
        &mut self,
        layer_id: &str,
        index: Option<usize>,
        key: &str,
        action: Option<String>,
    ) -> Result<(), ConfigError> {
        self.require_layer(layer_id)?;
        if let Some(index) = index {
            self.require_extra(layer_id, index)?;
        }
        self.update_layout(|layout| {
            let extras = &mut layout.layer_keymap_mut(layer_id).extras;
            if let Some(index) = index {
                let extra = &mut extras[index];
                extra.key = key.into();
                extra.action = action;
            } else {
                extras.push(ExtraKey {
                    id: crate::profile::ids::generate("e_"),
                    key: key.into(),
                    action,
                });
            }
        })
    }

    pub fn move_layer_extra(
        &mut self,
        layer_id: &str,
        index: usize,
        next: usize,
    ) -> Result<(), ConfigError> {
        self.require_extra(layer_id, index)?;
        self.require_extra(layer_id, next)?;
        self.update_layout(|layout| layout.layer_keymap_mut(layer_id).extras.swap(index, next))
    }

    pub fn remove_layer_extra(&mut self, layer_id: &str, index: usize) -> Result<(), ConfigError> {
        self.require_extra(layer_id, index)?;
        self.update_layout(|layout| {
            layout.layer_keymap_mut(layer_id).extras.remove(index);
        })
    }

    pub fn clear_layer_extras(&mut self, id: &str) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        self.update_layout(|layout| layout.layer_keymap_mut(id).extras.clear())
    }

    fn require_layer(&self, id: &str) -> Result<(), ConfigError> {
        if self.layout.layers.iter().any(|layer| layer.id == id) {
            Ok(())
        } else {
            Err(ConfigError::Invalid("unknown layer".into()))
        }
    }

    fn require_extra(&self, id: &str, index: usize) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        if self
            .layout
            .layer_keymaps
            .get(id)
            .is_some_and(|map| index < map.extras.len())
        {
            Ok(())
        } else {
            Err(ConfigError::Invalid("unknown extra key".into()))
        }
    }

    /// Entry of `key` in the keymap of `layer_id`.
    pub fn layer_key(&self, layer_id: &str, key: &str) -> KeyAssignment {
        match self
            .layout
            .layer_keymaps
            .get(layer_id)
            .and_then(|keymap| keymap.keys.get(key))
        {
            None => KeyAssignment::Transparent,
            Some(None) => KeyAssignment::Swallow,
            Some(Some(action)) => KeyAssignment::Action(action.clone()),
        }
    }

    pub fn set_layer_key(
        &mut self,
        layer_id: &str,
        key: &str,
        assignment: KeyAssignment,
    ) -> Result<(), ConfigError> {
        if !self.layout.layers.iter().any(|layer| layer.id == layer_id) {
            return Err(ConfigError::Invalid(format!("unknown layer \"{layer_id}\"")));
        }
        if let KeyAssignment::Action(action) = &assignment {
            let parsed = Action::parse(Some(action));
            if matches!(parsed, Action::Native | Action::Swallow) {
                return Err(ConfigError::InvalidAction(ActionIssue::InvalidSyntax));
            }
            if let Some(issue) = actions::validate(&parsed, &self.config()) {
                return Err(ConfigError::InvalidAction(issue));
            }
        }
        self.update_layout(|layout| {
            let keys = &mut layout.layer_keymap_mut(layer_id).keys;
            match assignment {
                KeyAssignment::Transparent => {
                    keys.remove(key);
                }
                KeyAssignment::Swallow => {
                    keys.insert(key.into(), None);
                }
                KeyAssignment::Action(action) => {
                    keys.insert(key.into(), Some(action));
                }
            }
        })
    }

    /// Apply `edit` to a copy of the current layout and save it.
    pub fn update_layout(&mut self, edit: impl FnOnce(&mut LayoutPreset)) -> Result<(), ConfigError> {
        let mut candidate = self.layout.clone();
        edit(&mut candidate);
        self.layout_file.write(&layout_file::serialize(&candidate))?;
        self.layout = candidate;
        Ok(())
    }

    /// Re-read files other processes changed. Returns `true` when the
    /// document now reflects new contents.
    pub fn reload_if_changed(&mut self) -> Result<bool, ConfigError> {
        let settings_text = self.settings_file.changed().map_err(ConfigError::Io)?;
        let layout_text = self.layout_file.changed().map_err(ConfigError::Io)?;
        if settings_text.is_none() && layout_text.is_none() {
            return Ok(false);
        }
        if let Some(text) = settings_text {
            let raw = parse_settings(&text)?;
            crate::gamemode::update_settings_from_config_json(&text);
            self.settings = settings::from_value(raw.get("settings"));
            self.settings_raw = raw;
            self.settings_file.mark_read(text);
        }
        if let Some(text) = layout_text {
            self.layout = parse_layout(&text)?;
            self.layout_file.mark_read(text);
        }
        Ok(true)
    }

    /// Ids (`user:<name>`) of the layouts in the user library.
    pub fn layout_ids(&self) -> Result<Vec<String>, ConfigError> {
        Ok(self
            .paths
            .list_user_layouts()
            .map_err(ConfigError::Io)?
            .into_iter()
            .map(|name| format!("{USER_LAYOUT_PREFIX}{name}"))
            .collect())
    }

    pub fn load_layout(&self, id: &str) -> Result<LayoutPreset, ConfigError> {
        let name = id
            .strip_prefix(USER_LAYOUT_PREFIX)
            .ok_or_else(|| ConfigError::Invalid(format!("unknown layout id \"{id}\"")))?;
        let text = self.paths.load_user_layout(name).map_err(ConfigError::Io)?;
        parse_layout(&text)
    }

    /// Layout whose rules should run now: the manual choice, or in auto
    /// mode the first layout whose conditions match `ctx`.
    pub fn active_layout_id(&self, ctx: &AutoSwitchContext) -> Result<Option<String>, ConfigError> {
        Ok(match self.settings.layout_mode {
            LayoutMode::Manual => self.settings.manual_active_layout_id.clone(),
            LayoutMode::Auto => {
                auto_switch::pick_active_layout(&self.layout_ids()?, &self.settings, ctx)
            }
        })
    }

    /// Mapper configuration for the current system state, like
    /// `computeRuntimeConfig()` in the frontend: picks the active layout,
    /// refuses blocking rule problems and drops disabled or draft rules.
    pub fn runtime_config(&self, ctx: &AutoSwitchContext) -> Result<RuntimeConfig, ConfigError> {
        let layout_id = self.active_layout_id(ctx)?;
        // In auto mode "no match" means passthrough, never the current layout.
        let current = layout_id == self.settings.current_layout_id
            && (layout_id.is_some() || self.settings.layout_mode == LayoutMode::Manual);
        let mut config = if current {
            self.config()
        } else {
            let preset = match &layout_id {
                Some(id) => self.load_layout(id)?,
                None => LayoutPreset::default(),
            };
            AppConfig::from_parts(self.settings.clone(), preset, layout_id.as_deref())
        };
        let blocking: Vec<RuleIssue> = diagnostics::analyze_rules(&config)
            .into_iter()
            .filter(|issue| issue.code.is_error())
            .collect();
        if !blocking.is_empty() {
            return Err(ConfigError::Rules(blocking));
        }
        diagnostics::runtime_rules(&mut config);
        let json = config.to_json();
        validate_for_mapper(&json)?;
        Ok(RuntimeConfig { json, layout_id })
    }

    fn set_setting(&mut self, name: &str, value: Value) -> Result<(), ConfigError> {
        let mut candidate = self.settings_raw.clone();
        let object = candidate
            .as_object_mut()
            .ok_or_else(|| ConfigError::Invalid("config.json must contain an object".into()))?;
        object.entry("version").or_insert(json!(1));
        object
            .entry("settings")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| ConfigError::Invalid("settings must be an object".into()))?
            .insert(name.into(), value);
        let text = serde_json::to_string_pretty(&candidate)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        self.settings_file.write(&text)?;
        crate::gamemode::update_settings_from_config_json(&text);
        self.settings = settings::from_value(candidate.get("settings"));
        self.settings_raw = candidate;
        Ok(())
    }
}

fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn parse_settings(text: &str) -> Result<Value, ConfigError> {
    if text.trim().is_empty() {
        return Ok(json!({ "version": 1, "settings": {} }));
    }
    let value: Value =
        serde_json::from_str(text).map_err(|error| ConfigError::Parse(format!("config.json: {error}")))?;
    if !value.is_object() {
        return Err(ConfigError::Parse("config.json must contain an object".into()));
    }
    Ok(value)
}

fn parse_layout(text: &str) -> Result<LayoutPreset, ConfigError> {
    Ok(layout_file::parse(text)
        .map_err(ConfigError::Parse)?
        .unwrap_or_else(LayoutPreset::initial))
}

/// Run the mapper's own validation, which also checks key names.
fn validate_for_mapper(json: &str) -> Result<(), ConfigError> {
    let config: crate::mapper_config::AppConfig =
        serde_json::from_str(json).map_err(|error| ConfigError::Invalid(error.to_string()))?;
    #[cfg(target_os = "linux")]
    crate::mapper::validation::validate_config(&config).map_err(ConfigError::Invalid)?;
    let _ = config;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::diagnostics::RuleIssueCode;
    use std::fs;

    const LAYOUT: &str = "layers:\n  - id: nav\n    name: Navigation\n    keys:\n      KeyH: ArrowLeft\nrules:\n  - key: CapsLock\n    layer: nav\n    tap: Escape\n";

    fn document(settings: Value, layout: &str) -> (tempfile::TempDir, ConfigDocument) {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths.save_config(&serde_json::to_string_pretty(&settings).unwrap()).unwrap();
        paths.save_current_layout(layout).unwrap();
        (dir, ConfigDocument::load(paths).unwrap())
    }

    #[test]
    fn reads_settings_and_layout_from_their_own_files() {
        let (_dir, document) = document(
            json!({"version": 1, "settings": {"inputDevicePath": "/dev/input/event3", "appearance": "dark"}}),
            LAYOUT,
        );
        assert_eq!(document.input_device(), Some("/dev/input/event3"));
        assert_eq!(document.settings().appearance, Appearance::Dark);
        assert_eq!(document.layout().rules.len(), 1);
        let config = document.config();
        assert_eq!(config.layer_keymaps["nav"].keys["KeyH"].as_deref(), Some("ArrowLeft"));
    }

    #[test]
    fn settings_edit_preserves_unknown_fields_and_leaves_layout_alone() {
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"futureOption": 7}, "extra": true}),
            LAYOUT,
        );
        let layout_before = fs::read_to_string(document.paths.current_layout_path()).unwrap();
        document.set_input_device("/dev/input/event7").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap()).unwrap();
        assert_eq!(saved["settings"]["futureOption"], 7);
        assert_eq!(saved["extra"], true);
        assert_eq!(saved["settings"]["inputDevicePath"], "/dev/input/event7");
        assert_eq!(saved["settings"]["locale"], "en-US");
        assert!(saved.get("rules").is_none());
        assert_eq!(
            fs::read_to_string(document.paths.current_layout_path()).unwrap(),
            layout_before
        );
    }

    #[test]
    fn settings_batch_keeps_unknown_fields_and_refuses_external_changes() {
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"futureOption": 7}}),
            LAYOUT,
        );
        document.update_settings(|settings| {
            settings.default_hold_timeout_ms = 350;
            settings.game_mode.use_fullscreen = true;
        }).unwrap();
        let saved: Value = serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap()).unwrap();
        assert_eq!(saved["settings"]["futureOption"], 7);
        assert_eq!(saved["settings"]["defaultHoldTimeoutMs"], 350);
        assert_eq!(saved["settings"]["gameMode"]["useFullscreen"], true);
        fs::write(document.paths.config_path(), "{}").unwrap();
        assert_eq!(document.update_settings(|settings| settings.launch_on_startup = true), Err(ConfigError::ExternalChange));
    }

    #[test]
    fn base_tap_edit_preserves_other_rules_and_rejects_complex_rules() {
        let (_dir, mut document) = document(json!({"settings": {}}), LAYOUT);
        document.set_base_tap_action("KeyA", "Ctrl+KeyC").unwrap();
        assert_eq!(document.base_tap_action("KeyA"), Some("Ctrl+KeyC"));
        assert_eq!(document.layout().rules.len(), 2);
        document.set_base_tap_action("KeyA", "Ctrl+KeyV").unwrap();
        assert_eq!(document.base_tap_action("KeyA"), Some("Ctrl+KeyV"));
        assert_eq!(document.layout().rules.len(), 2);
        assert!(document.set_base_tap_action("CapsLock", "KeyB").is_err());
        assert_eq!(document.layout().rules.len(), 2);
        let reloaded = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(reloaded.base_tap_action("KeyA"), Some("Ctrl+KeyV"));
    }

    #[test]
    fn layer_keys_are_saved_to_the_layout_file() {
        let (_dir, mut document) = document(json!({"version": 1, "settings": {}}), LAYOUT);
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Action("ArrowDown".into()))
            .unwrap();
        document.set_layer_key("nav", "KeyH", KeyAssignment::Swallow).unwrap();
        assert_eq!(
            document.set_layer_key("nav", "KeyK", KeyAssignment::Action("macro:nope".into())),
            Err(ConfigError::InvalidAction(ActionIssue::UnknownMacro))
        );
        assert!(document.set_layer_key("missing", "KeyK", KeyAssignment::Swallow).is_err());
        let reloaded = ConfigDocument::load(document.paths.clone()).unwrap();
        assert_eq!(
            reloaded.layer_key("nav", "KeyJ"),
            KeyAssignment::Action("ArrowDown".into())
        );
        assert_eq!(reloaded.layer_key("nav", "KeyH"), KeyAssignment::Swallow);
        assert_eq!(reloaded.layer_key("nav", "KeyZ"), KeyAssignment::Transparent);
        assert_eq!(reloaded.layout().rules[0].tap_action.as_deref(), Some("Escape"));
        assert!(fs::read_to_string(document.paths.config_path()).unwrap().find("rules").is_none());
    }

    #[test]
    fn external_change_blocks_save_until_reload() {
        let (_dir, mut document) = document(json!({"version": 1, "settings": {}}), LAYOUT);
        document
            .paths
            .save_current_layout("layers:\n  - id: nav\n    name: Changed\n")
            .unwrap();
        assert_eq!(
            document.set_layer_key("nav", "KeyJ", KeyAssignment::Swallow),
            Err(ConfigError::ExternalChange)
        );
        assert!(document.reload_if_changed().unwrap());
        assert!(!document.reload_if_changed().unwrap());
        assert_eq!(document.layout().layers[0].name, "Changed");
        document.set_layer_key("nav", "KeyJ", KeyAssignment::Swallow).unwrap();
    }

    #[test]
    fn missing_files_load_as_a_new_installation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let document = ConfigDocument::load(paths).unwrap();
        assert_eq!(document.input_device(), None);
        assert_eq!(document.layout(), &LayoutPreset::initial());
    }

    #[test]
    fn runtime_config_uses_the_active_library_layout() {
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"currentLayoutId": "user:Main", "manualActiveLayoutId": "user:Other"}}),
            LAYOUT,
        );
        document
            .paths
            .save_user_layout("Other", "rules:\n  - key: KeyA\n    tap: KeyB\n  - key: KeyC\n    enabled: false\n    tap: KeyD\n", true)
            .unwrap();
        let runtime = document.runtime_config(&AutoSwitchContext::default()).unwrap();
        assert_eq!(runtime.layout_id.as_deref(), Some("user:Other"));
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert_eq!(value["rules"].as_array().unwrap().len(), 1);
        assert_eq!(value["rules"][0]["key"], "KeyA");
        assert_eq!(value["settings"]["currentLayoutId"], "user:Other");

        document.set_setting("manualActiveLayoutId", json!("user:Main")).unwrap();
        let runtime = document.runtime_config(&AutoSwitchContext::default()).unwrap();
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert_eq!(value["rules"][0]["key"], "CapsLock");
        assert_eq!(value["rules"][0]["tapAction"], "Escape");
        assert_eq!(value["rules"][0]["holdAction"], "");
    }

    #[test]
    fn auto_mode_without_a_match_is_passthrough() {
        let (_dir, document) = document(json!({"version": 1, "settings": {"layoutMode": "auto"}}), LAYOUT);
        let runtime = document.runtime_config(&AutoSwitchContext::default()).unwrap();
        assert_eq!(runtime.layout_id, None);
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert!(value["rules"].as_array().unwrap().is_empty());
    }

    #[test]
    fn bundled_layout_runs_in_the_mapper() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../public/ivank-layout.yaml");
        let (_dir, document) = document(json!({"version": 1, "settings": {}}), &fs::read_to_string(path).unwrap());
        assert!(document.layout().layers.len() > 1);
        let runtime = document.runtime_config(&AutoSwitchContext::default()).unwrap();
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert!(!value["rules"].as_array().unwrap().is_empty());
        let reparsed = layout_file::parse(&layout_file::serialize(document.layout())).unwrap().unwrap();
        assert_eq!(reparsed.layer_keymaps, document.layout().layer_keymaps);
        assert_eq!(reparsed.layers, document.layout().layers);
    }

    #[test]
    fn blocking_rule_issues_refuse_runtime_config() {
        let (_dir, document) = document(
            json!({"version": 1, "settings": {}}),
            "rules:\n  - key: KeyA\n    tap: KeyB\n  - key: KeyA\n    tap: KeyC\n",
        );
        match document.runtime_config(&AutoSwitchContext::default()) {
            Err(ConfigError::Rules(issues)) => {
                assert!(issues.iter().all(|issue| issue.code == RuleIssueCode::DuplicateTrigger))
            }
            other => panic!("unexpected {other:?}"),
        }
    }    #[test]
    fn layer_lifecycle_preserves_keymaps_and_detaches_rules() {
        let (_dir, mut document) = document(json!({"settings": {}}), LAYOUT);
        document.set_layer_extra("nav", None, "F13", Some("Escape".into())).unwrap();
        let copy = document.clone_layer("nav", "Navigation copy", "Copied").unwrap();
        assert_eq!(document.layout().layer_keymaps[&copy].keys["KeyH"].as_deref(), Some("ArrowLeft"));
        assert_eq!(document.layout().layer_keymaps[&copy].extras.len(), 1);
        assert_ne!(document.layout().layer_keymaps[&copy].extras[0].id, document.layout().layer_keymaps["nav"].extras[0].id);
        document.move_layer_extra("nav", 0, 0).unwrap();
        document.clear_layer_keys(&copy).unwrap();
        assert!(document.layout().layer_keymaps[&copy].keys.is_empty());
        document.delete_layer("nav").unwrap();
        assert!(document.layout().rules[0].layer_id.is_empty());
        assert!(!document.layout().layer_keymaps.contains_key("nav"));
        let loaded = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(loaded.layout().layers[0].name, "Navigation copy");
        assert!(loaded.layout().rules[0].layer_id.is_empty());
    }

}
