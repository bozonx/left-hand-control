//! Editable application configuration.
//!
//! Global settings live in `config.json`; the current keyboard layout
//! stays in memory. Saved layouts live in the user
//! library (`layouts/<name>.json`, id `user:<name>`).
//!
//! Both formats are described in [`crate::profile::settings`] and
//! [`crate::profile::layout_file`].

use crate::profile::actions::{self, Action, ActionIssue};
use crate::profile::auto_switch::{self, AutoSwitchContext};
use crate::profile::diagnostics::{self, RuleIssue};
use crate::profile::model::{
    AppConfig, AppSettings, Appearance, Layer, LayerRule, LayoutMode, LayoutPreset,
    LocalePreference,
};
use crate::profile::names::{self, NameIssue, NameKind};
use crate::profile::{app_match, key_catalog, layout_file, settings};
use crate::storage::StoragePaths;
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
    Macro(crate::profile::macros::MacroIssue),
    Menu(crate::profile::menus::MenuIssue),
    /// A layer or layout name cannot be used.
    Name(NameKind, NameIssue),
    /// Rules have blocking problems; the mapper cannot start.
    Rules(Vec<RuleIssue>),
    /// The edited data no longer matches the editor snapshot.
    EditConflict,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "config I/O: {error}"),
            Self::Parse(error) => write!(f, "parse config: {error}"),
            Self::Invalid(error) => write!(f, "invalid config: {error}"),
            Self::Menu(issue) => write!(f, "invalid menu: {issue:?}"),
            Self::Name(kind, issue) => write!(f, "invalid {kind:?} name: {issue:?}"),
            Self::Macro(issue) => write!(f, "invalid macro: {issue:?}"),
            Self::InvalidAction(issue) => write!(f, "invalid action: {issue:?}"),
            Self::Rules(issues) => match issues.first() {
                Some(issue) => write!(f, "{issue}"),
                None => write!(f, "invalid rules"),
            },
            Self::EditConflict => write!(f, "configuration changed since editing started"),
        }
    }
}

impl std::error::Error for ConfigError {}

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
    settings: AppSettings,
    layout: LayoutPreset,
}

impl ConfigDocument {
    pub fn load(paths: StoragePaths) -> Result<Self, ConfigError> {
        paths.ensure().map_err(ConfigError::Io)?;
        let settings_text = paths.load_config().map_err(ConfigError::Io)?;
        let settings = settings::parse(&settings_text).map_err(ConfigError::Parse)?;
        let layout = match settings.current_layout_id.as_deref() {
            Some(id) => {
                let name = crate::profile::model::user_layout_name(id)
                    .ok_or_else(|| ConfigError::Invalid(format!("unknown layout id \"{id}\"")))?;
                parse_layout(&paths.load_user_layout(name).map_err(ConfigError::Io)?)?
            }
            None => LayoutPreset::initial(),
        };
        crate::gamemode::set_settings(&settings.game_mode);
        Ok(Self {
            settings,
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
        self.update_settings(|settings| settings.input_device_path = Some(path.to_owned()))
    }

    pub fn set_appearance(&mut self, appearance: Appearance) -> Result<(), ConfigError> {
        self.update_settings(|settings| settings.appearance = appearance)
    }

    pub fn set_locale(&mut self, locale: LocalePreference) -> Result<(), ConfigError> {
        self.update_settings(|settings| settings.locale = locale)
    }

    pub fn update_settings(
        &mut self,
        edit: impl FnOnce(&mut AppSettings),
    ) -> Result<(), ConfigError> {
        let mut updated = self.settings.clone();
        edit(&mut updated);
        let text = settings::serialize(&updated);
        let autostart_changed = crate::autostart::SUPPORTED
            && updated.launch_on_startup != self.settings.launch_on_startup;
        if autostart_changed {
            crate::autostart::set_enabled(&self.paths, updated.launch_on_startup)
                .map_err(ConfigError::Io)?;
        }
        if let Err(error) = self.paths.save_config(&text) {
            if autostart_changed
                && let Err(rollback) =
                    crate::autostart::set_enabled(&self.paths, self.settings.launch_on_startup)
            {
                log::error!("restore autostart registration: {rollback}");
            }
            return Err(ConfigError::Io(error));
        }
        crate::gamemode::set_settings(&updated.game_mode);
        self.settings = updated;
        Ok(())
    }

    /// Align launch on startup with the system at application start: the
    /// user may have removed or disabled it in the desktop or Windows
    /// settings. An active registration is refreshed so it points to this
    /// executable after a move or upgrade; a missing or disabled one is left
    /// untouched. Returns whether the setting changed.
    pub fn sync_autostart(&mut self) -> Result<bool, ConfigError> {
        if !crate::autostart::SUPPORTED {
            return Ok(false);
        }
        let registered = crate::autostart::is_enabled(&self.paths).map_err(ConfigError::Io)?;
        if registered {
            crate::autostart::set_enabled(&self.paths, true).map_err(ConfigError::Io)?;
        }
        if registered == self.settings.launch_on_startup {
            return Ok(false);
        }
        let mut updated = self.settings.clone();
        updated.launch_on_startup = registered;
        self.paths
            .save_config(&settings::serialize(&updated))
            .map_err(ConfigError::Io)?;
        self.settings = updated;
        Ok(true)
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

    /// Trimmed `name` for a new layer or for renaming layer `except`.
    pub fn layer_name(&self, name: &str, except: Option<&str>) -> Result<String, ConfigError> {
        names::layer_name(&self.layout, name, except)
            .map_err(|issue| ConfigError::Name(NameKind::Layer, issue))
    }

    pub fn create_layer(&mut self, name: &str, description: &str) -> Result<String, ConfigError> {
        let name = self.layer_name(name, None)?;
        let id = crate::profile::ids::generate("l_");
        self.update_layout(|layout| {
            layout.layers.push(Layer {
                id: id.clone(),
                name,
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
        let name = self.layer_name(name, Some(id))?;
        if !self.layout.layers.iter().any(|layer| layer.id == id) {
            return Err(ConfigError::Invalid("unknown layer".into()));
        }
        self.update_layout(|layout| {
            let layer = layout
                .layers
                .iter_mut()
                .find(|layer| layer.id == id)
                .unwrap();
            layer.name = name;
            layer.description = optional_text(description);
        })
    }

    pub fn clone_layer(
        &mut self,
        source_id: &str,
        name: &str,
        description: &str,
    ) -> Result<String, ConfigError> {
        let name = self.layer_name(name, None)?;
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
                name,
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
        self.check_assignment(&assignment)?;
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

    /// Set an additional key of `layer_id`, replacing the row of `previous`
    /// when it is given. The row keeps its display position.
    pub fn set_extra_key(
        &mut self,
        layer_id: &str,
        previous: Option<&str>,
        key: &str,
        assignment: KeyAssignment,
    ) -> Result<(), ConfigError> {
        self.require_layer(layer_id)?;
        if !key_catalog::valid_key(key) {
            return Err(ConfigError::Invalid("invalid additional key".into()));
        }
        if previous != Some(key)
            && self
                .layout
                .layer_keymaps
                .get(layer_id)
                .is_some_and(|map| map.keys.contains_key(key))
        {
            return Err(ConfigError::Invalid("key already assigned".into()));
        }
        self.check_assignment(&assignment)?;
        self.update_layout(|layout| {
            let map = layout.layer_keymap_mut(layer_id);
            if let Some(previous) = previous {
                map.keys.remove(previous);
                for code in &mut map.extra_key_order {
                    if code == previous {
                        *code = key.to_owned();
                    }
                }
            }
            match assignment {
                KeyAssignment::Transparent => {
                    map.keys.remove(key);
                    map.extra_key_order.retain(|code| code != key);
                }
                KeyAssignment::Swallow => {
                    map.keys.insert(key.into(), None);
                }
                KeyAssignment::Action(action) => {
                    map.keys.insert(key.into(), Some(action));
                }
            }
        })
    }

    fn check_assignment(&self, assignment: &KeyAssignment) -> Result<(), ConfigError> {
        if let KeyAssignment::Action(action) = assignment {
            let parsed = Action::parse(Some(action));
            if matches!(parsed, Action::Native | Action::Swallow) {
                return Err(ConfigError::InvalidAction(ActionIssue::InvalidSyntax));
            }
            if let Some(issue) = actions::validate(&parsed, &self.config()) {
                return Err(ConfigError::InvalidAction(issue));
            }
        }
        Ok(())
    }

    /// Exchange the entries of keys `a` and `b` in the keymap of `layer_id`.
    pub fn swap_layer_keys(&mut self, layer_id: &str, a: &str, b: &str) -> Result<(), ConfigError> {
        self.require_layer(layer_id)?;
        if a == b {
            return Ok(());
        }
        self.update_layout(|layout| {
            let keys = &mut layout.layer_keymap_mut(layer_id).keys;
            let first = keys.remove(a);
            let second = keys.remove(b);
            if let Some(value) = first {
                keys.insert(b.into(), value);
            }
            if let Some(value) = second {
                keys.insert(a.into(), value);
            }
        })
    }

    /// Apply `edit` to a copy of the current layout.
    pub fn update_layout(
        &mut self,
        edit: impl FnOnce(&mut LayoutPreset),
    ) -> Result<(), ConfigError> {
        let mut candidate = self.layout.clone();
        edit(&mut candidate);

        self.layout = candidate;
        Ok(())
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

    /// Mapper configuration for the current system state: picks the active layout,
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
}

fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn parse_layout(text: &str) -> Result<LayoutPreset, ConfigError> {
    Ok(layout_file::parse(text)
        .map_err(ConfigError::Parse)?
        .unwrap_or_else(LayoutPreset::initial))
}

/// Run the mapper's own validation, which also checks key names.
fn validate_for_mapper(json: &str) -> Result<(), ConfigError> {
    let config: crate::mapper::config::AppConfig =
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
    use serde_json::{Value, json};
    use std::fs;

    const LAYOUT: &str = r#"{"schemaVersion": 1, "layers": [{"id": "nav", "name": "Navigation", "keys": {"KeyH": "ArrowLeft"}}], "rules": [{"key": "CapsLock", "hold": {"layer": "nav"}, "tap": "Escape"}]}"#;

    /// A document whose edited and active layout is `layout`, saved as
    /// `Test` unless `state.editedLayoutId` names another one.
    fn document(mut settings: Value, layout: &str) -> (tempfile::TempDir, ConfigDocument) {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        settings["schemaVersion"] = json!(1);
        if settings["state"]["editedLayoutId"].is_null() {
            settings["state"]["editedLayoutId"] = json!("user:Test");
        }
        if settings["settings"]["activeLayoutId"].is_null() {
            settings["settings"]["activeLayoutId"] = settings["state"]["editedLayoutId"].clone();
        }
        let name = settings["state"]["editedLayoutId"]
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
    fn reads_settings_and_layout_from_their_own_files() {
        let (_dir, document) = document(
            json!({"settings": {"keyboardDevice": "/dev/input/by-id/kbd", "appearance": "dark"}}),
            LAYOUT,
        );
        assert_eq!(document.input_device(), Some("/dev/input/by-id/kbd"));
        assert_eq!(document.settings().appearance, Appearance::Dark);
        assert_eq!(document.layout().rules.len(), 1);
        let config = document.config();
        assert_eq!(
            config.layer_keymaps["nav"].keys["KeyH"].as_deref(),
            Some("ArrowLeft")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn launch_on_startup_follows_the_system_registration() {
        let (dir, mut document) = document(json!({}), LAYOUT);
        let entry = dir
            .path()
            .join("autostart/dev.bozonx.left-hand-control.desktop");
        document
            .update_settings(|settings| settings.launch_on_startup = true)
            .unwrap();
        assert!(entry.exists());
        assert!(!document.sync_autostart().unwrap());

        // Disabled in the desktop's autostart settings: the setting follows.
        fs::write(&entry, "[Desktop Entry]\nExec=old\nHidden=true\n").unwrap();
        assert!(document.sync_autostart().unwrap());
        assert!(!document.settings().launch_on_startup);
        assert!(entry.exists(), "a disabled entry stays for the desktop");
        let saved = fs::read_to_string(document.paths.config_path()).unwrap();
        assert!(saved.contains("\"launchOnStartup\": false"));

        // Added outside the application: adopted and pointed at this binary.
        fs::write(&entry, "[Desktop Entry]\nExec=old\n").unwrap();
        assert!(document.sync_autostart().unwrap());
        assert!(document.settings().launch_on_startup);
        assert!(!fs::read_to_string(&entry).unwrap().contains("Exec=old"));
    }

    #[test]
    fn settings_edit_leaves_layout_alone() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
        let layout_before =
            fs::read_to_string(document.paths.layouts_dir().join("Test.json")).unwrap();
        document.set_input_device("/dev/input/by-id/other").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap())
                .unwrap();
        assert_eq!(saved["schemaVersion"], 1);
        assert_eq!(
            saved["settings"]["keyboardDevice"],
            "/dev/input/by-id/other"
        );
        assert_eq!(saved["settings"]["locale"], "en-US");
        assert!(saved.get("rules").is_none());
        assert_eq!(
            fs::read_to_string(document.paths.layouts_dir().join("Test.json")).unwrap(),
            layout_before
        );
    }

    #[test]
    fn invalid_settings_are_rejected_without_rewriting_the_file() {
        for text in [
            json!({"schemaVersion": 1, "settings": {"futureOption": 7}}).to_string(),
            json!({"schemaVersion": 1, "settings": {"appearance": "blue"}}).to_string(),
            json!({"schemaVersion": 2, "settings": {}}).to_string(),
            json!({"settings": {}}).to_string(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
            paths.save_config(&text).unwrap();
            assert!(matches!(
                ConfigDocument::load(paths.clone()),
                Err(ConfigError::Parse(_))
            ));
            assert_eq!(paths.load_config().unwrap(), text);
        }
    }

    #[test]
    fn commands_require_explicit_permission_on_load() {
        let (_dir, mut document) = document(
            json!({}),
            r#"{"schemaVersion": 1, "commands": [{"id": "hello", "unix": "printf hello"}]}"#,
        );
        assert!(!document.settings().commands_enabled);
        document
            .paths
            .save_config(
                &json!({"schemaVersion": 1, "settings": {"appearance": "dark"},
                    "state": {"editedLayoutId": "user:Test"}})
                .to_string(),
            )
            .unwrap();
        document = ConfigDocument::load(document.paths().clone()).unwrap();
        assert!(!document.settings().commands_enabled);
    }

    #[test]
    fn settings_batch_overwrites_external_changes() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
        document
            .update_settings(|settings| {
                settings.default_hold_timeout_ms = 350;
                settings.game_mode.use_fullscreen = true;
            })
            .unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap())
                .unwrap();
        assert_eq!(saved["settings"]["timing"]["holdMs"], 350);
        assert_eq!(
            saved["settings"]["gameMode"]["detectors"]["fullscreen"],
            true
        );
        fs::write(document.paths.config_path(), r#"{"schemaVersion": 1}"#).unwrap();
        document
            .update_settings(|settings| settings.default_hold_timeout_ms = 400)
            .unwrap();
        assert_eq!(document.settings().default_hold_timeout_ms, 400);
        let saved: Value = serde_json::from_str(&document.paths.load_config().unwrap()).unwrap();
        assert_eq!(saved["settings"]["timing"]["holdMs"], 400);
        assert_eq!(
            saved["settings"]["gameMode"]["detectors"]["fullscreen"],
            true
        );
    }

    #[test]
    fn base_tap_edit_preserves_other_rules_and_rejects_complex_rules() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
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
        let (_dir, mut document) = document(json!({}), LAYOUT);
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
    fn extra_keys_are_validated_and_keep_their_position() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
        for key in ["F13", "F14"] {
            document
                .set_extra_key("nav", None, key, KeyAssignment::Swallow)
                .unwrap();
        }
        document
            .reorder_extra_keys("nav", vec!["F13".into(), "F14".into()], 0, 1)
            .unwrap();
        assert!(
            document
                .set_extra_key("nav", None, "F13", KeyAssignment::Swallow)
                .is_err()
        );
        assert!(
            document
                .set_extra_key("nav", None, "NoSuchKey", KeyAssignment::Swallow)
                .is_err()
        );
        assert!(
            document
                .set_extra_key(
                    "nav",
                    None,
                    "F15",
                    KeyAssignment::Action("macro:none".into())
                )
                .is_err()
        );
        document
            .set_extra_key(
                "nav",
                Some("F13"),
                "F16",
                KeyAssignment::Action("F1".into()),
            )
            .unwrap();
        let map = &document.layout().layer_keymaps["nav"];
        assert_eq!(map.extra_key_order, ["F14", "F16"]);
        assert_eq!(map.keys["F16"].as_deref(), Some("F1"));
        assert!(!map.keys.contains_key("F13"));
        document
            .set_extra_key("nav", Some("F16"), "F16", KeyAssignment::Transparent)
            .unwrap();
        let map = &document.layout().layer_keymaps["nav"];
        assert_eq!(map.extra_key_order, ["F14"]);
        assert!(!map.keys.contains_key("F16"));
    }

    #[test]
    fn swapping_layer_keys_exchanges_actions_and_suppression() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Swallow)
            .unwrap();
        document.swap_layer_keys("nav", "KeyH", "KeyJ").unwrap();
        assert_eq!(document.layer_key("nav", "KeyH"), KeyAssignment::Swallow);
        assert_eq!(
            document.layer_key("nav", "KeyJ"),
            KeyAssignment::Action("ArrowLeft".into())
        );
        document.swap_layer_keys("nav", "KeyJ", "KeyK").unwrap();
        assert_eq!(
            document.layer_key("nav", "KeyJ"),
            KeyAssignment::Transparent
        );
        assert_eq!(
            document.layer_key("nav", "KeyK"),
            KeyAssignment::Action("ArrowLeft".into())
        );
        assert!(document.swap_layer_keys("missing", "KeyH", "KeyJ").is_err());
    }

    #[test]
    fn layer_keys_remain_in_memory_until_explicitly_saved() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
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
    fn manual_settings_changes_require_restart_and_do_not_block_layout_edits() {
        let (_dir, mut document) = document(json!({"settings": {"appearance": "dark"}}), LAYOUT);
        document
            .paths
            .save_config(
                r#"{"schemaVersion":1,"settings":{"appearance":"light"},"state":{"editedLayoutId":"user:Test"}}"#,
            )
            .unwrap();
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Swallow)
            .unwrap();
        assert_eq!(document.settings().appearance, Appearance::Dark);
        let restarted = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(restarted.settings().appearance, Appearance::Light);
        document.set_locale(LocalePreference::English).unwrap();
        let restarted = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(restarted.settings().appearance, Appearance::Dark);
        assert_eq!(restarted.settings().locale, LocalePreference::English);
    }

    #[test]
    fn session_edits_do_not_touch_disk() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
        let paths = document.paths().clone();
        let saved = paths.load_user_layout("Test").unwrap();
        document.set_base_tap_action("KeyA", "Enter").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        assert_eq!(paths.load_user_layout("Test").unwrap(), saved);
        let restarted = ConfigDocument::load(paths.clone()).unwrap();
        assert_eq!(restarted.base_tap_action("KeyA"), None);
        assert_eq!(restarted.settings().locale, LocalePreference::English);
        document.save_current_layout_as("Saved").unwrap();
        document.set_base_tap_action("KeyA", "Tab").unwrap();
        let restarted = ConfigDocument::load(paths).unwrap();
        assert_eq!(restarted.base_tap_action("KeyA"), Some("Enter"));
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
            json!({"settings": {"activeLayoutId": "user:Other"}, "state": {"editedLayoutId": "user:Main"}}),
            LAYOUT,
        );
        let save = |document: &ConfigDocument, rules: &str| {
            document
                .paths
                .save_user_layout(
                    "Other",
                    &format!("{{\"schemaVersion\":1,\"rules\":{rules}}}"),
                    true,
                )
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
            r#"[{"key": "KeyA", "tap": "KeyB", "when": {"windows": ["kate"]}}]"#,
        );
        assert!(!uses_titles(&document));
        save(
            &document,
            r#"[{"key": "KeyA", "tap": "KeyB", "when": {"excludeWindows": ["title:Secret"]}}]"#,
        );
        assert!(uses_titles(&document));
        // Disabled rules never run, so they need no titles.
        save(
            &document,
            r#"[{"key": "KeyA", "enabled": false, "tap": "KeyB", "when": {"windows": ["title:x"]}}]"#,
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
            json!({"settings": {"activeLayoutId": "user:Other"}, "state": {"editedLayoutId": "user:Main"}}),
            LAYOUT,
        );
        document
            .paths
            .save_user_layout("Other", r#"{"schemaVersion": 1, "rules": [{"key": "KeyA", "tap": "KeyB"}, {"key": "KeyC", "enabled": false, "tap": "KeyD"}]}"#, true)
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
            .update_settings(|settings| settings.manual_active_layout_id = Some("user:Main".into()))
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
        let (_dir, document) = document(json!({"settings": {"layoutMode": "auto"}}), LAYOUT);
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        assert_eq!(runtime.layout_id, None);
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert!(value["rules"].as_array().unwrap().is_empty());
    }

    #[test]
    fn bundled_layouts_run_in_the_mapper() {
        for bundled in crate::profile::bundled::BUNDLED {
            let (_dir, document) = document(json!({}), bundled.text());
            assert!(
                crate::profile::diagnostics::analyze_rules(&document.config()).is_empty(),
                "{}",
                bundled.name
            );
            let runtime = document
                .runtime_config(&AutoSwitchContext::default())
                .unwrap();
            let value: Value = serde_json::from_str(&runtime.json).unwrap();
            assert!(!value["rules"].as_array().unwrap().is_empty());
        }
        let (_dir, document) = document(json!({}), crate::profile::bundled::BUNDLED[0].text());
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
        let quick = &document.layout().quick_action_pages;
        assert_eq!(quick.len(), 1);
        assert!(quick.iter().all(|page| page.cells.len() == 15));
        assert_eq!(quick[0].cells["KeyQ"].action, "Sleep");
        assert_eq!(quick[0].cells["KeyS"].action, "sys:overview");
        assert_eq!(reparsed.quick_action_pages, *quick);
    }

    #[test]
    fn blocking_rule_issues_refuse_runtime_config() {
        let (_dir, document) = document(
            json!({}),
            r#"{"schemaVersion": 1, "rules": [{"key": "UnknownKey", "tap": "KeyB"}]}"#,
        );
        match document.runtime_config(&AutoSwitchContext::default()) {
            Err(ConfigError::Rules(issues)) => {
                assert!(
                    issues
                        .iter()
                        .all(|issue| issue.code == RuleIssueCode::InvalidTrigger)
                )
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn repeated_triggers_keep_both_rules_in_runtime_order() {
        let (_dir, document) = document(
            json!({}),
            r#"{"schemaVersion": 1, "rules": [{"key": "KeyA", "tap": "KeyB"}, {"key": "KeyA", "tap": "KeyC"}]}"#,
        );
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        let value: Value = serde_json::from_str(&runtime.json).unwrap();
        assert_eq!(value["rules"].as_array().unwrap().len(), 2);
        assert_eq!(value["rules"][0]["tapAction"], "KeyB");
        assert_eq!(value["rules"][1]["tapAction"], "KeyC");
    }

    #[test]
    fn layer_lifecycle_preserves_keymaps_and_detaches_rules() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
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
