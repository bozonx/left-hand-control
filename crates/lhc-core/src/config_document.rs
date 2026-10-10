//! Editable configuration shared by every shell.
//!
//! Global settings live in `config.json`; the current keyboard layout
//! stays in memory. Saved layouts live in the user
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
    AppConfig, AppSettings, Appearance, Layer, LayerRule, LayoutMode, LayoutPreset,
    LocalePreference,
};
use crate::profile::{app_match, layout_file, settings};
use crate::storage::{StoragePaths, TrackedFile, WriteError};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt};

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
    Macro(crate::profile::macros::MacroIssue),
    Menu(crate::profile::menus::MenuIssue),
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
            Self::Menu(issue) => write!(f, "invalid menu: {issue:?}"),
            Self::Macro(issue) => write!(f, "invalid macro: {issue:?}"),
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
    /// Whether the rules or the auto-mode choice look at window titles,
    /// see [`crate::active_window::set_titles_needed`].
    pub uses_titles: bool,
}

pub struct ConfigDocument {
    paths: StoragePaths,
    settings_file: TrackedFile,
    settings_raw: Value,
    settings: AppSettings,
    layout: LayoutPreset,
    saved_layout: LayoutPreset,
    library_files: BTreeMap<String, TrackedFile>,
}

impl ConfigDocument {
    pub fn load(paths: StoragePaths) -> Result<Self, ConfigError> {
        paths.ensure().map_err(ConfigError::Io)?;
        let (settings_file, settings_text) =
            TrackedFile::open(paths.config_path()).map_err(ConfigError::Io)?;
        let settings_raw = parse_settings(&settings_text)?;
        let settings = settings::from_value(settings_raw.get("settings"));
        let layout = match settings.current_layout_id.as_deref() {
            Some(id) => {
                let name = crate::profile::model::user_layout_name(id)
                    .ok_or_else(|| ConfigError::Invalid(format!("unknown layout id \"{id}\"")))?;
                parse_layout(&paths.load_user_layout(name).map_err(ConfigError::Io)?)?
            }
            None => LayoutPreset::initial(),
        };
        crate::gamemode::set_settings(&settings.game_mode);
        let library_files = paths
            .list_user_layouts()
            .map_err(ConfigError::Io)?
            .into_iter()
            .map(|name| {
                TrackedFile::open(paths.layouts_dir().join(format!("{name}.yaml")))
                    .map(|(file, _)| (name, file))
            })
            .collect::<Result<_, _>>()
            .map_err(ConfigError::Io)?;
        let mut settings = settings::from_value(settings_raw.get("settings"));
        if settings_raw.pointer("/settings/commandsEnabled").is_none() {
            settings.commands_enabled = !layout.commands.is_empty();
        }
        Ok(Self {
            library_files,
            settings,
            settings_raw,
            settings_file,
            saved_layout: layout.clone(),
            layout,
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
        settings.remove("commandTrust");
        settings.remove("commandTimeoutSecs");
        let previous = serde_json::to_value(&self.settings)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        for name in previous
            .as_object()
            .into_iter()
            .flat_map(|values| values.keys())
        {
            settings.remove(name);
        }
        for (name, value) in values.as_object().into_iter().flatten() {
            settings.insert(name.clone(), value.clone());
        }
        let text = serde_json::to_string_pretty(&candidate)
            .map_err(|error| ConfigError::Parse(error.to_string()))?;
        let autostart_changed = cfg!(target_os = "linux")
            && updated.launch_on_startup != self.settings.launch_on_startup;
        if autostart_changed {
            if self
                .settings_file
                .changed()
                .map_err(ConfigError::Io)?
                .is_some()
            {
                return Err(ConfigError::ExternalChange);
            }
            crate::autostart::set_enabled(&self.paths, updated.launch_on_startup)
                .map_err(ConfigError::Io)?;
        }
        if let Err(error) = self.settings_file.write(&text) {
            if autostart_changed
                && let Err(rollback) =
                    crate::autostart::set_enabled(&self.paths, self.settings.launch_on_startup)
            {
                log::error!("restore autostart registration: {rollback}");
            }
            return Err(error.into());
        }
        crate::gamemode::set_settings(&updated.game_mode);
        self.settings = updated;
        self.settings_raw = candidate;
        Ok(())
    }

    pub fn base_tap_action(&self, key: &str) -> Option<&str> {
        self.layout.base_tap_action(key)
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
                || !rule.long_hold_action.is_empty()
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
            let keymap = layout
                .layer_keymaps
                .get(source_id)
                .cloned()
                .unwrap_or_default();
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
        self.update_layout(|layout| {
            let map = layout.layer_keymap_mut(id);
            map.keys.clear();
            map.extra_key_order.clear();
        })
    }

    /// Save the displayed additional-key order after a drag operation.
    pub fn reorder_extra_keys(
        &mut self,
        id: &str,
        mut keys: Vec<String>,
        from: usize,
        to: usize,
    ) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        if from >= keys.len() || to >= keys.len() || from == to {
            return Ok(());
        }
        let key = keys.remove(from);
        keys.insert(to, key);
        self.update_layout(|layout| layout.layer_keymap_mut(id).extra_key_order = keys)
    }

    fn require_layer(&self, id: &str) -> Result<(), ConfigError> {
        if self.layout.layers.iter().any(|layer| layer.id == id) {
            Ok(())
        } else {
            Err(ConfigError::Invalid("unknown layer".into()))
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
            return Err(ConfigError::Invalid(format!(
                "unknown layer \"{layer_id}\""
            )));
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
            let map = layout.layer_keymap_mut(layer_id);
            let keys = &mut map.keys;
            match assignment {
                KeyAssignment::Transparent => {
                    keys.remove(key);
                    map.extra_key_order.retain(|code| code != key);
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

    pub(crate) fn ensure_files_unchanged(&mut self) -> Result<(), ConfigError> {
        if self
            .settings_file
            .changed()
            .map_err(ConfigError::Io)?
            .is_some()
        {
            return Err(ConfigError::ExternalChange);
        }
        Ok(())
    }

    /// Apply `edit` to a copy of the current layout.
    pub fn update_layout(
        &mut self,
        edit: impl FnOnce(&mut LayoutPreset),
    ) -> Result<(), ConfigError> {
        self.ensure_files_unchanged()?;
        let mut candidate = self.layout.clone();
        edit(&mut candidate);

        self.layout = candidate;
        Ok(())
    }

    /// Re-read files other processes changed. Returns `true` when the
    /// document now reflects new contents.
    pub fn reload_if_changed(&mut self) -> Result<bool, ConfigError> {
        let settings_text = self.settings_file.changed().map_err(ConfigError::Io)?;

        let settings_raw = settings_text.as_deref().map(parse_settings).transpose()?;

        let next_settings = settings_raw
            .as_ref()
            .map(|raw| settings::from_value(raw.get("settings")))
            .unwrap_or_else(|| self.settings.clone());
        let next_layout = match next_settings.current_layout_id.as_deref() {
            Some(id) => self.load_layout(id)?,
            None => LayoutPreset::initial(),
        };
        let replace_layout = next_settings.current_layout_id != self.settings.current_layout_id
            || layout_file::serialize(&self.layout) == layout_file::serialize(&self.saved_layout);
        let names = self.paths.list_user_layouts().map_err(ConfigError::Io)?;
        let mut added = BTreeMap::new();
        let mut updates = Vec::new();
        for name in &names {
            if let Some(file) = self.library_files.get_mut(name) {
                if let Some(text) = file.changed().map_err(ConfigError::Io)? {
                    parse_layout(&text)?;
                    updates.push((name.clone(), text));
                }
            } else {
                let (file, text) =
                    TrackedFile::open(self.paths.layouts_dir().join(format!("{name}.yaml")))
                        .map_err(ConfigError::Io)?;
                parse_layout(&text)?;
                added.insert(name.clone(), file);
            }
        }
        let changed = settings_text.is_some()
            || !added.is_empty()
            || !updates.is_empty()
            || self.library_files.keys().any(|name| !names.contains(name));
        if let (Some(text), Some(raw)) = (settings_text, settings_raw) {
            self.settings = settings::from_value(raw.get("settings"));
            crate::gamemode::set_settings(&self.settings.game_mode);
            self.settings_raw = raw;
            self.settings_file.mark_read(text);
        }
        if replace_layout {
            self.layout = next_layout.clone();
        }
        self.saved_layout = next_layout;
        if self
            .settings_raw
            .pointer("/settings/commandsEnabled")
            .is_none()
        {
            self.settings.commands_enabled = !self.layout.commands.is_empty();
        }
        self.library_files.retain(|name, _| names.contains(name));
        self.library_files.extend(added);
        for (name, text) in updates {
            self.library_files.get_mut(&name).unwrap().mark_read(text);
        }
        Ok(changed)
    }

    /// Ids (`user:<name>`) of the layouts in the user library.
    pub fn layout_ids(&self) -> Result<Vec<String>, ConfigError> {
        Ok(self
            .paths
            .list_user_layouts()
            .map_err(ConfigError::Io)?
            .into_iter()
            .map(|name| crate::profile::model::user_layout_id(&name))
            .collect())
    }

    pub fn load_layout(&self, id: &str) -> Result<LayoutPreset, ConfigError> {
        let name = crate::profile::model::user_layout_name(id)
            .ok_or_else(|| ConfigError::Invalid(format!("unknown layout id \"{id}\"")))?;
        let text = self.paths.load_user_layout(name).map_err(ConfigError::Io)?;
        parse_layout(&text)
    }

    /// Layout whose rules should run now: the manual choice, or in auto
    /// mode the first matching rule's layout or the default.
    pub fn active_layout_id(&self, ctx: &AutoSwitchContext) -> Result<Option<String>, ConfigError> {
        Ok(match self.settings.layout_mode {
            LayoutMode::Manual => self.settings.manual_active_layout_id.clone(),
            LayoutMode::Auto => self.auto_choice(ctx)?.layout_id,
        })
    }

    /// Which automatic rule applies under `ctx`, whatever the current mode.
    pub fn auto_choice(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<auto_switch::AutoChoice, ConfigError> {
        Ok(auto_switch::choose(
            &self.layout_ids()?,
            &self.settings,
            ctx,
        ))
    }

    /// Mapper configuration for the current system state, like
    /// `computeRuntimeConfig()` in the frontend: picks the active layout,
    /// refuses blocking rule problems and drops disabled or draft rules.
    pub fn runtime_config(&self, ctx: &AutoSwitchContext) -> Result<RuntimeConfig, ConfigError> {
        let layout_id = self.active_layout_id(ctx)?;
        // In auto mode "no match" means passthrough, never the current layout.
        let mut config = AppConfig::from_parts(
            self.settings.clone(),
            self.layout_for_activation(layout_id.as_deref())?,
            layout_id.as_deref(),
        );
        let blocking: Vec<RuleIssue> = diagnostics::analyze_rules(&config)
            .into_iter()
            .filter(|issue| issue.code.is_error())
            .collect();
        if !blocking.is_empty() {
            return Err(ConfigError::Rules(blocking));
        }
        diagnostics::runtime_rules(&mut config);
        let rule_patterns = config.rules.iter().flat_map(|rule| {
            [
                &rule.condition_apps_whitelist,
                &rule.condition_apps_blacklist,
            ]
            .into_iter()
            .flatten()
            .flatten()
        });
        let auto_patterns = self
            .settings
            .auto_rules
            .iter()
            .filter(|_| self.settings.layout_mode == LayoutMode::Auto)
            .flat_map(|rule| &rule.conditions.apps);
        let uses_titles = app_match::uses_title(rule_patterns.chain(auto_patterns));
        let json = config.to_json();
        validate_for_mapper(&json)?;
        Ok(RuntimeConfig {
            json,
            layout_id,
            uses_titles,
        })
    }

    pub fn active_layout(&self, ctx: &AutoSwitchContext) -> Result<LayoutPreset, ConfigError> {
        self.layout_for_activation(self.active_layout_id(ctx)?.as_deref())
    }

    pub fn layout_for_activation(&self, id: Option<&str>) -> Result<LayoutPreset, ConfigError> {
        if id == self.settings.current_layout_id.as_deref()
            && (id.is_some() || self.settings.layout_mode == LayoutMode::Manual)
        {
            return Ok(self.layout.clone());
        }
        match id {
            Some(id) => self.load_layout(id),
            None => Ok(LayoutPreset::default()),
        }
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
        self.settings = settings::from_value(candidate.get("settings"));
        crate::gamemode::set_settings(&self.settings.game_mode);
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
    let value: Value = serde_json::from_str(text)
        .map_err(|error| ConfigError::Parse(format!("config.json: {error}")))?;
    if !value.is_object() {
        return Err(ConfigError::Parse(
            "config.json must contain an object".into(),
        ));
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

    fn document(mut settings: Value, layout: &str) -> (tempfile::TempDir, ConfigDocument) {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        if settings["settings"]["currentLayoutId"].is_null() {
            settings["settings"]["currentLayoutId"] = json!("user:Test");
        }
        let name = settings["settings"]["currentLayoutId"]
            .as_str()
            .unwrap()
            .strip_prefix("user:")
            .unwrap();
        paths
            .save_config(&serde_json::to_string_pretty(&settings).unwrap())
            .unwrap();
        paths.save_user_layout(name, layout, true).unwrap();
        (dir, ConfigDocument::load(paths).unwrap())
    }

    #[test]
    fn reload_detects_library_additions_edits_and_deletions() {
        let (_dir, mut doc) = document(json!({"settings": {}}), LAYOUT);
        assert!(!doc.reload_if_changed().unwrap());
        doc.paths.save_user_layout("Nav", LAYOUT, false).unwrap();
        assert!(doc.reload_if_changed().unwrap());
        assert!(!doc.reload_if_changed().unwrap());
        doc.paths
            .save_user_layout("Nav", &LAYOUT.replace("Escape", "Enter"), true)
            .unwrap();
        assert!(doc.reload_if_changed().unwrap());
        assert!(!doc.reload_if_changed().unwrap());
        doc.paths.delete_user_layout("Nav").unwrap();
        assert!(doc.reload_if_changed().unwrap());
        assert!(!doc.reload_if_changed().unwrap());
    }

    #[test]
    fn failed_reload_does_not_accept_only_part_of_the_changes() {
        let (_dir, mut doc) = document(json!({"settings": {"appearance": "dark"}}), LAYOUT);
        doc.paths
            .save_config(r#"{"settings":{"appearance":"light","currentLayoutId":"user:Test"}}"#)
            .unwrap();
        doc.paths
            .save_user_layout("Test", "layers: [", true)
            .unwrap();
        assert!(doc.reload_if_changed().is_err());
        assert_eq!(doc.settings().appearance, Appearance::Dark);
        doc.paths
            .save_user_layout("Test", &LAYOUT.replace("Escape", "Enter"), true)
            .unwrap();
        assert!(doc.reload_if_changed().unwrap());
        assert_eq!(doc.settings().appearance, Appearance::Light);
        assert_eq!(doc.layout().rules[0].tap_action.as_deref(), Some("Enter"));
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
        assert_eq!(
            config.layer_keymaps["nav"].keys["KeyH"].as_deref(),
            Some("ArrowLeft")
        );
    }

    #[test]
    fn settings_edit_preserves_unknown_fields_and_leaves_layout_alone() {
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"futureOption": 7}, "extra": true}),
            LAYOUT,
        );
        let layout_before =
            fs::read_to_string(document.paths.layouts_dir().join("Test.yaml")).unwrap();
        document.set_input_device("/dev/input/event7").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap())
                .unwrap();
        assert_eq!(saved["settings"]["futureOption"], 7);
        assert_eq!(saved["extra"], true);
        assert_eq!(saved["settings"]["inputDevicePath"], "/dev/input/event7");
        assert_eq!(saved["settings"]["locale"], "en-US");
        assert!(saved.get("rules").is_none());
        assert_eq!(
            fs::read_to_string(document.paths.layouts_dir().join("Test.yaml")).unwrap(),
            layout_before
        );
    }

    #[test]
    fn migrated_auto_rules_are_saved_next_to_legacy_conditions() {
        let legacy = json!({"user:Test": {"enabledInAuto": true, "whitelist": {"apps": ["kate"]}}});
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"layoutConditions": legacy}}),
            LAYOUT,
        );
        assert_eq!(document.settings().auto_rules.len(), 1);
        document
            .update_settings(|settings| settings.auto_default_layout_id = Some("user:Test".into()))
            .unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap())
                .unwrap();
        // The legacy shell still reads `layoutConditions`.
        assert_eq!(saved["settings"]["layoutConditions"], legacy);
        assert_eq!(saved["settings"]["autoRules"][0]["layoutId"], "user:Test");
        assert_eq!(
            saved["settings"]["autoRules"][0]["windows"],
            json!(["*kate*", "title:kate"])
        );
        assert!(saved["settings"]["autoRules"][0].get("apps").is_none());
        assert_eq!(saved["settings"]["autoDefaultLayoutId"], "user:Test");
    }

    #[test]
    fn settings_batch_keeps_unknown_fields_and_refuses_external_changes() {
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"futureOption": 7}}),
            LAYOUT,
        );
        document
            .update_settings(|settings| {
                settings.default_hold_timeout_ms = 350;
                settings.game_mode.use_fullscreen = true;
            })
            .unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap())
                .unwrap();
        assert_eq!(saved["settings"]["futureOption"], 7);
        assert_eq!(saved["settings"]["defaultHoldTimeoutMs"], 350);
        assert_eq!(saved["settings"]["gameMode"]["useFullscreen"], true);
        fs::write(document.paths.config_path(), "{}").unwrap();
        assert_eq!(
            document.update_settings(|settings| settings.launch_on_startup = true),
            Err(ConfigError::ExternalChange)
        );
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
        assert_eq!(reloaded.base_tap_action("KeyA"), None);
    }

    #[test]
    fn additional_key_order_survives_saved_layout_reload() {
        let (_dir, mut document) = document(json!({"version": 1, "settings": {}}), LAYOUT);
        for key in ["F13", "F14", "F15"] {
            document
                .set_layer_key("nav", key, KeyAssignment::Swallow)
                .unwrap();
        }
        document
            .reorder_extra_keys("nav", vec!["F13".into(), "F14".into(), "F15".into()], 0, 2)
            .unwrap();
        document.save_current_layout_as("Ordered").unwrap();
        let restarted = ConfigDocument::load(document.paths.clone()).unwrap();
        assert_eq!(
            restarted.layout().layer_keymaps["nav"].extra_key_order,
            ["F14", "F15", "F13"]
        );
        document
            .set_layer_key("nav", "F15", KeyAssignment::Transparent)
            .unwrap();
        assert_eq!(
            document.layout().layer_keymaps["nav"].extra_key_order,
            ["F14", "F13"]
        );
        let copy = document.clone_layer("nav", "Copy", "").unwrap();
        assert_eq!(
            document.layout().layer_keymaps[&copy].extra_key_order,
            ["F14", "F13"]
        );
        document.clear_layer_keys("nav").unwrap();
        assert!(
            document.layout().layer_keymaps["nav"]
                .extra_key_order
                .is_empty()
        );
    }

    #[test]
    fn layer_keys_remain_in_memory_until_explicitly_saved() {
        let (_dir, mut document) = document(json!({"version": 1, "settings": {}}), LAYOUT);
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Action("ArrowDown".into()))
            .unwrap();
        document
            .set_layer_key("nav", "KeyH", KeyAssignment::Swallow)
            .unwrap();
        assert_eq!(
            document.set_layer_key("nav", "KeyK", KeyAssignment::Action("macro:nope".into())),
            Err(ConfigError::InvalidAction(ActionIssue::UnknownMacro))
        );
        assert!(
            document
                .set_layer_key("missing", "KeyK", KeyAssignment::Swallow)
                .is_err()
        );
        let restarted = ConfigDocument::load(document.paths.clone()).unwrap();
        assert_eq!(
            restarted.layer_key("nav", "KeyJ"),
            KeyAssignment::Transparent
        );
        assert_eq!(
            restarted.layer_key("nav", "KeyH"),
            KeyAssignment::Action("ArrowLeft".into())
        );
        let reloaded = &document;
        assert_eq!(
            reloaded.layer_key("nav", "KeyJ"),
            KeyAssignment::Action("ArrowDown".into())
        );
        assert_eq!(reloaded.layer_key("nav", "KeyH"), KeyAssignment::Swallow);
        assert_eq!(
            reloaded.layer_key("nav", "KeyZ"),
            KeyAssignment::Transparent
        );
        assert_eq!(
            reloaded.layout().rules[0].tap_action.as_deref(),
            Some("Escape")
        );
        assert!(
            fs::read_to_string(document.paths.config_path())
                .unwrap()
                .find("rules")
                .is_none()
        );
    }

    #[test]
    fn external_change_blocks_save_until_reload() {
        let (_dir, mut document) = document(json!({"version": 1, "settings": {}}), LAYOUT);
        document
            .paths
            .save_config(r#"{"settings":{"currentLayoutId":"user:Test","locale":"ru-RU"}}"#)
            .unwrap();
        document
            .paths
            .save_user_layout("Test", "layers:\n  - id: nav\n    name: Changed\n", true)
            .unwrap();
        assert_eq!(
            document.set_layer_key("nav", "KeyJ", KeyAssignment::Swallow),
            Err(ConfigError::ExternalChange)
        );
        assert!(document.reload_if_changed().unwrap());
        assert!(!document.reload_if_changed().unwrap());
        assert_eq!(document.layout().layers[0].name, "Changed");
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Swallow)
            .unwrap();
    }

    #[test]
    fn legacy_drafts_are_ignored_and_session_edits_do_not_touch_disk() {
        let (_dir, mut document) = document(json!({"settings": {}}), LAYOUT);
        let paths = document.paths().clone();
        fs::write(paths.data_dir().join("current-layout.yaml"), "{broken").unwrap();
        fs::write(paths.settings_dir().join("ui-state.json"), "{broken").unwrap();
        let saved = paths.load_user_layout("Test").unwrap();
        document.set_base_tap_action("KeyA", "Enter").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        assert_eq!(paths.load_user_layout("Test").unwrap(), saved);
        let restarted = ConfigDocument::load(paths.clone()).unwrap();
        assert_eq!(restarted.base_tap_action("KeyA"), None);
        assert_eq!(restarted.settings().locale, LocalePreference::English);
        assert_eq!(
            fs::read_to_string(paths.data_dir().join("current-layout.yaml")).unwrap(),
            "{broken"
        );
        document.save_current_layout_as("Saved").unwrap();
        document.set_base_tap_action("KeyA", "Tab").unwrap();
        let restarted = ConfigDocument::load(paths).unwrap();
        assert_eq!(restarted.base_tap_action("KeyA"), Some("Enter"));
    }

    #[test]
    fn external_settings_keep_drafts_and_discard_uses_latest_saved_layout() {
        let (_dir, mut document) = document(json!({"settings": {}}), LAYOUT);
        document.set_base_tap_action("KeyA", "Enter").unwrap();
        document
            .paths
            .save_config(r#"{"settings":{"currentLayoutId":"user:Test","locale":"ru-RU"}}"#)
            .unwrap();
        document
            .paths
            .save_user_layout("Test", &LAYOUT.replace("Escape", "Tab"), true)
            .unwrap();
        assert!(document.reload_if_changed().unwrap());
        assert_eq!(document.base_tap_action("KeyA"), Some("Enter"));
        assert_eq!(document.settings().locale, LocalePreference::Russian);
        let saved = document.load_layout("user:Test").unwrap();
        document.update_layout(|layout| *layout = saved).unwrap();
        assert_eq!(document.base_tap_action("KeyA"), None);
        assert_eq!(
            document.layout().rules[0].tap_action.as_deref(),
            Some("Tab")
        );
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
    fn runtime_config_reports_whether_titles_are_needed() {
        let (_dir, mut document) = document(
            json!({"version": 1, "settings": {"currentLayoutId": "user:Main", "manualActiveLayoutId": "user:Other"}}),
            LAYOUT,
        );
        let save = |document: &ConfigDocument, rules: &str| {
            document
                .paths
                .save_user_layout("Other", &format!("rules:\n{rules}"), true)
                .unwrap();
        };
        let uses_titles = |document: &ConfigDocument| {
            document
                .runtime_config(&AutoSwitchContext::default())
                .unwrap()
                .uses_titles
        };
        save(
            &document,
            "  - key: KeyA\n    tap: KeyB\n    windows: [kate]\n",
        );
        assert!(!uses_titles(&document));
        save(
            &document,
            "  - key: KeyA\n    tap: KeyB\n    excludeWindows: ['title:Secret']\n",
        );
        assert!(uses_titles(&document));
        // Disabled rules never run, so they need no titles.
        save(
            &document,
            "  - key: KeyA\n    enabled: false\n    tap: KeyB\n    windows: ['title:x']\n",
        );
        assert!(!uses_titles(&document));
        // Auto rules count only in auto mode.
        document
            .update_settings(|settings| {
                settings.auto_rules = vec![crate::profile::model::AutoRule {
                    id: "r".into(),
                    enabled: true,
                    layout_id: None,
                    conditions: crate::profile::model::LayoutConditionSet {
                        apps: vec!["title:YouTube".into()],
                        ..Default::default()
                    },
                }];
            })
            .unwrap();
        assert!(!uses_titles(&document));
        document
            .update_settings(|settings| settings.layout_mode = LayoutMode::Auto)
            .unwrap();
        assert!(uses_titles(&document));
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
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        assert_eq!(runtime.layout_id.as_deref(), Some("user:Other"));
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert_eq!(value["rules"].as_array().unwrap().len(), 1);
        assert_eq!(value["rules"][0]["key"], "KeyA");
        assert_eq!(value["settings"]["currentLayoutId"], "user:Other");

        document
            .set_setting("manualActiveLayoutId", json!("user:Main"))
            .unwrap();
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert_eq!(value["rules"][0]["key"], "CapsLock");
        assert_eq!(value["rules"][0]["tapAction"], "Escape");
        assert_eq!(value["rules"][0]["holdAction"], "");
    }

    #[test]
    fn auto_mode_without_a_match_is_passthrough() {
        let (_dir, document) = document(
            json!({"version": 1, "settings": {"layoutMode": "auto"}}),
            LAYOUT,
        );
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        assert_eq!(runtime.layout_id, None);
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert!(value["rules"].as_array().unwrap().is_empty());
    }

    #[test]
    fn bundled_layout_runs_in_the_mapper() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../public/ivank-layout.yaml");
        let (_dir, document) = document(
            json!({"version": 1, "settings": {}}),
            &fs::read_to_string(path).unwrap(),
        );
        assert!(document.layout().layers.len() > 1);
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert!(!value["rules"].as_array().unwrap().is_empty());
        let reparsed = layout_file::parse(&layout_file::serialize(document.layout()))
            .unwrap()
            .unwrap();
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
                assert!(
                    issues
                        .iter()
                        .all(|issue| issue.code == RuleIssueCode::DuplicateTrigger)
                )
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn layer_lifecycle_preserves_keymaps_and_detaches_rules() {
        let (_dir, mut document) = document(json!({"settings": {}}), LAYOUT);
        document
            .set_layer_key("nav", "F13", KeyAssignment::Action("Escape".into()))
            .unwrap();
        let copy = document
            .clone_layer("nav", "Navigation copy", "Copied")
            .unwrap();
        assert_eq!(
            document.layout().layer_keymaps[&copy].keys["KeyH"].as_deref(),
            Some("ArrowLeft")
        );
        assert_eq!(
            document.layout().layer_keymaps[&copy].keys["F13"].as_deref(),
            Some("Escape")
        );
        document.clear_layer_keys(&copy).unwrap();
        assert!(document.layout().layer_keymaps[&copy].keys.is_empty());
        document.delete_layer("nav").unwrap();
        assert!(document.layout().rules[0].layer_id.is_empty());
        assert!(!document.layout().layer_keymaps.contains_key("nav"));
        let loaded = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(loaded.layout().layers[0].name, "Navigation");
        document.save_current_layout_as("Saved").unwrap();
        let saved = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(saved.layout().layers[0].name, "Navigation copy");
        assert!(saved.layout().rules[0].layer_id.is_empty());
    }
}
