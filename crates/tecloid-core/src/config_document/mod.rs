//! Editable application configuration.
//!
//! Global settings live in `config.json`; the current keyboard layout
//! stays in memory. Saved keymaps live in the user
//! library (`keymaps/<name>.json`, id `user:<name>`).
//!
//! Both formats are described in [`crate::config::settings`] and
//! [`crate::config::keymap_file`].

mod activation;

use crate::config::actions::{self, Action, ActionIssue};
use crate::config::diagnostics::RuleIssue;
use crate::config::model::{
    AppSettings, Appearance, KeymapPreset, LocalePreference, ResolvedAppConfig,
};
use crate::config::names::{self, NameIssue, NameKind};
use crate::config::{key_catalog, keymap_file, layer_grid, settings};
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
    Macro(crate::config::macros::MacroIssue),
    Menu(crate::config::menus::MenuIssue),
    /// A layer or keymap name cannot be used.
    Name(NameKind, NameIssue),
    /// Rules have blocking problems; the mapper cannot start.
    Rules(Vec<RuleIssue>),
    /// An extra key of a layer cannot be used.
    ExtraKey(ExtraKeyIssue),
    /// The edited data no longer matches the editor snapshot.
    EditConflict,
}

/// Why a key cannot become an extra key row of a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraKeyIssue {
    /// Not a key code of the key catalog.
    Invalid,
    /// The key is on the layer keyboard grid and is edited there.
    OnGrid,
    /// The layer already has a row for this key.
    Taken,
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
            Self::ExtraKey(issue) => write!(f, "invalid extra key: {issue:?}"),
            Self::EditConflict => write!(f, "configuration changed since editing started"),
        }
    }
}

impl std::error::Error for ConfigError {}

pub use self::activation::RuntimeConfig;
pub use crate::config::layer_edit::KeyAssignment;

#[derive(Clone)]
pub struct ConfigDocument {
    paths: StoragePaths,
    staged: bool,
    keymap_only: bool,
    settings: AppSettings,
    keymap: KeymapPreset,
}

impl ConfigDocument {
    pub fn load(paths: StoragePaths) -> Result<Self, ConfigError> {
        paths.ensure().map_err(ConfigError::Io)?;
        let settings_text = paths.load_config().map_err(ConfigError::Io)?;
        let settings = settings::parse(&settings_text).map_err(ConfigError::Parse)?;
        let keymap = KeymapPreset::initial();
        crate::gamemode::set_settings(&settings.game_mode, settings.in_development);
        Ok(Self {
            staged: false,
            keymap_only: false,
            settings,
            keymap,
            paths,
        })
    }

    pub fn paths(&self) -> &StoragePaths {
        &self.paths
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn keymap(&self) -> &KeymapPreset {
        &self.keymap
    }

    /// Settings and the current keymap combined.
    pub fn config(&self) -> ResolvedAppConfig {
        ResolvedAppConfig::from_parts(
            self.settings.clone(),
            self.keymap.clone(),
            self.settings.editing_keymap_id.as_deref(),
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

    /// Stage settings and keymap changes, saving settings only after the
    /// entire callback succeeds. Library writes and autostart refresh are
    /// separate operations and are rejected inside a transaction.
    pub fn transaction<T>(
        &mut self,
        edit: impl FnOnce(&mut Self) -> Result<T, ConfigError>,
    ) -> Result<T, ConfigError> {
        let mut draft = self.clone();
        draft.staged = true;
        let result = edit(&mut draft)?;
        if draft.settings != self.settings {
            self.update_settings(|settings| *settings = draft.settings.clone())?;
        }
        draft.staged = self.staged;
        *self = draft;
        Ok(result)
    }

    pub(crate) fn require_external_operation(&self) -> Result<(), ConfigError> {
        if self.staged || self.keymap_only {
            Err(ConfigError::Invalid(
                "external operation is not allowed inside a document transaction".into(),
            ))
        } else {
            Ok(())
        }
    }

    pub fn update_settings(
        &mut self,
        edit: impl FnOnce(&mut AppSettings),
    ) -> Result<(), ConfigError> {
        if self.keymap_only {
            return Err(ConfigError::Invalid(
                "settings cannot change inside a keymap edit".into(),
            ));
        }
        let mut updated = self.settings.clone();
        edit(&mut updated);
        settings::validate(&updated).map_err(ConfigError::Invalid)?;
        if self.staged {
            self.settings = updated;
            return Ok(());
        }
        let text = settings::serialize(&updated);
        let autostart_changed = crate::autostart::apply_setting(
            &self.paths,
            self.settings.launch_on_startup,
            updated.launch_on_startup,
        )
        .map_err(ConfigError::Io)?;
        if let Err(error) = self.paths.save_config(&text) {
            if autostart_changed
                && let Err(rollback) =
                    crate::autostart::set_enabled(&self.paths, self.settings.launch_on_startup)
            {
                log::error!("restore autostart registration: {rollback}");
            }
            return Err(ConfigError::Io(error));
        }
        crate::gamemode::set_settings(&updated.game_mode, updated.in_development);
        self.settings = updated;
        Ok(())
    }

    /// Align launch on startup with the system at application start: the
    /// user may have removed or disabled it in the desktop or Windows
    /// settings (see [`crate::autostart::refresh`]). Returns whether the
    /// setting changed.
    pub fn sync_autostart(&mut self) -> Result<bool, ConfigError> {
        self.require_external_operation()?;
        if !crate::autostart::SUPPORTED {
            return Ok(false);
        }
        let registered = crate::autostart::refresh(&self.paths).map_err(ConfigError::Io)?;
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
        self.keymap.base_tap_action(key)
    }

    pub fn set_base_tap_action(&mut self, key: &str, action: &str) -> Result<(), ConfigError> {
        if let Some(issue) = actions::validate(&Action::parse(Some(action)), &self.config()) {
            return Err(ConfigError::InvalidAction(issue));
        }
        if !self.keymap.base_tap_editable(key) {
            return Err(ConfigError::Invalid(format!(
                "key {key} has a rule that requires the full editor"
            )));
        }
        self.update_keymap(|keymap| keymap.set_base_tap(key, action))
    }

    /// Trimmed `name` for a new layer or for renaming layer `except`.
    pub fn layer_name(&self, name: &str, except: Option<&str>) -> Result<String, ConfigError> {
        names::layer_name(&self.keymap, name, except)
            .map_err(|issue| ConfigError::Name(NameKind::Layer, issue))
    }

    /// `name` when a new layer can use it, else the first free `name N`.
    pub fn free_layer_name(&self, name: &str) -> String {
        if self.layer_name(name, None).is_ok() {
            return name.to_owned();
        }
        (2..1000)
            .map(|number| format!("{name} {number}"))
            .find(|candidate| self.layer_name(candidate, None).is_ok())
            .unwrap_or_else(|| name.to_owned())
    }

    pub fn create_layer(&mut self, name: &str, description: &str) -> Result<String, ConfigError> {
        let name = self.layer_name(name, None)?;
        let id = crate::config::ids::generate("layer-");
        self.update_keymap(|keymap| keymap.add_layer(&id, name, optional_text(description)))?;
        Ok(id)
    }

    pub fn rename_layer(
        &mut self,
        id: &str,
        name: &str,
        description: &str,
    ) -> Result<(), ConfigError> {
        let name = self.layer_name(name, Some(id))?;
        self.require_layer(id)?;
        self.update_keymap(|keymap| keymap.rename_layer(id, name, optional_text(description)))
    }

    /// Change only the name, keeping the stored description.
    pub fn set_layer_name(&mut self, id: &str, name: &str) -> Result<(), ConfigError> {
        let name = self.layer_name(name, Some(id))?;
        self.require_layer(id)?;
        self.update_keymap(|keymap| {
            if let Some(layer) = keymap.layers.iter_mut().find(|layer| layer.id == id) {
                layer.name = name;
            }
        })
    }

    /// Change only the description, so a layer whose stored name no longer
    /// passes validation can still be described.
    pub fn set_layer_description(
        &mut self,
        id: &str,
        description: &str,
    ) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        let description = optional_text(description);
        self.update_keymap(|keymap| {
            if let Some(layer) = keymap.layers.iter_mut().find(|layer| layer.id == id) {
                layer.description = description;
            }
        })
    }

    pub fn clone_layer(
        &mut self,
        source_id: &str,
        name: &str,
        description: &str,
    ) -> Result<String, ConfigError> {
        let name = self.layer_name(name, None)?;
        self.require_layer(source_id)?;
        let id = crate::config::ids::generate("layer-");
        self.update_keymap(|keymap| {
            keymap.clone_layer(source_id, &id, name, optional_text(description))
        })?;
        Ok(id)
    }

    pub fn delete_layer(&mut self, id: &str) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        self.update_keymap(|keymap| keymap.remove_layer(id))
    }

    pub fn clear_layer_keys(&mut self, id: &str) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        self.update_keymap(|keymap| keymap.clear_layer_keys(id))
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
        self.update_keymap(|keymap| keymap.get_or_create_layer_keymap(id).extra_key_order = keys)
    }

    fn require_layer(&self, id: &str) -> Result<(), ConfigError> {
        if self.keymap.has_layer(id) {
            Ok(())
        } else {
            Err(ConfigError::Invalid("unknown layer".into()))
        }
    }

    /// Entry of `key` in the keymap of `layer_id`.
    pub fn layer_key(&self, layer_id: &str, key: &str) -> KeyAssignment {
        self.keymap.layer_key(layer_id, key)
    }

    pub fn set_layer_key(
        &mut self,
        layer_id: &str,
        key: &str,
        assignment: KeyAssignment,
    ) -> Result<(), ConfigError> {
        if !self.keymap.has_layer(layer_id) {
            return Err(ConfigError::Invalid(format!(
                "unknown layer \"{layer_id}\""
            )));
        }
        self.check_assignment(&assignment)?;
        self.update_keymap(|keymap| keymap.set_layer_key(layer_id, key, assignment))
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
        self.check_extra_key(layer_id, previous, key)?;
        self.check_assignment(&assignment)?;
        self.update_keymap(|keymap| keymap.set_extra_key(layer_id, previous, key, assignment))
    }

    /// Whether `key` can be the extra key row of `layer_id` that replaces
    /// the row of `previous` (or a new row when it is `None`).
    pub fn check_extra_key(
        &self,
        layer_id: &str,
        previous: Option<&str>,
        key: &str,
    ) -> Result<(), ConfigError> {
        self.require_layer(layer_id)?;
        if !key_catalog::valid_key(key) {
            return Err(ConfigError::ExtraKey(ExtraKeyIssue::Invalid));
        }
        if layer_grid::contains(key) {
            return Err(ConfigError::ExtraKey(ExtraKeyIssue::OnGrid));
        }
        if previous != Some(key) && self.keymap.layer_has_key(layer_id, key) {
            return Err(ConfigError::ExtraKey(ExtraKeyIssue::Taken));
        }
        Ok(())
    }

    /// Whether `key` of `layer_id` keeps its modifiers pressed until the
    /// layer is released.
    pub fn layer_key_holds_modifiers(&self, layer_id: &str, key: &str) -> bool {
        self.keymap.layer_key_holds_modifiers(layer_id, key)
    }

    /// Keep the modifiers of `key`'s keystroke pressed until the layer is
    /// released. Only a key with an action can hold its modifiers.
    pub fn set_layer_key_holds_modifiers(
        &mut self,
        layer_id: &str,
        key: &str,
        hold: bool,
    ) -> Result<(), ConfigError> {
        self.require_layer(layer_id)?;
        if hold && !matches!(self.layer_key(layer_id, key), KeyAssignment::Action(_)) {
            return Err(ConfigError::Invalid(
                "only a key with an action can hold its modifiers".into(),
            ));
        }
        if self.layer_key_holds_modifiers(layer_id, key) == hold {
            return Ok(());
        }
        self.update_keymap(|keymap| keymap.set_layer_key_holds_modifiers(layer_id, key, hold))
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
        self.update_keymap(|keymap| keymap.swap_layer_keys(layer_id, a, b))
    }

    /// Stage commands and a keymap-only edit, committing the working copy only
    /// when every operation succeeds. The callback must not change settings or files.
    pub fn edit_keymap_with_commands<T>(
        &mut self,
        commands: impl IntoIterator<Item = crate::config::model::Command>,
        edit: impl FnOnce(&mut Self) -> Result<T, ConfigError>,
    ) -> Result<T, ConfigError> {
        let mut draft = Self {
            paths: self.paths.clone(),
            staged: self.staged,
            keymap_only: true,
            settings: self.settings.clone(),
            keymap: self.keymap.clone(),
        };
        for command in commands {
            draft.save_command(command)?;
        }
        let result = edit(&mut draft)?;
        self.keymap = draft.keymap;
        Ok(result)
    }

    /// Apply `edit` to a copy of the current keymap.
    pub fn update_keymap(
        &mut self,
        edit: impl FnOnce(&mut KeymapPreset),
    ) -> Result<(), ConfigError> {
        let mut candidate = self.keymap.clone();
        edit(&mut candidate);
        crate::config::ids::validate_keymap(&candidate).map_err(ConfigError::Invalid)?;
        crate::config::timing::validate_keymap(&candidate).map_err(ConfigError::Invalid)?;

        self.keymap = candidate;
        Ok(())
    }

    /// Ids (`user:<name>`) of the keymaps in the user library.
    pub fn keymap_ids(&self) -> Result<Vec<String>, ConfigError> {
        Ok(self
            .paths
            .list_user_keymaps()
            .map_err(ConfigError::Io)?
            .into_iter()
            .map(|name| crate::config::model::user_keymap_id(&name))
            .collect())
    }

    pub fn load_keymap(&self, id: &str) -> Result<KeymapPreset, ConfigError> {
        let name = crate::config::model::user_keymap_name(id)
            .ok_or_else(|| ConfigError::Invalid(format!("unknown keymap id \"{id}\"")))?;
        let text = self.paths.load_user_keymap(name).map_err(ConfigError::Io)?;
        parse_keymap(&text)
    }
}

fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

pub(crate) fn parse_keymap(text: &str) -> Result<KeymapPreset, ConfigError> {
    keymap_file::parse(text)
        .map_err(ConfigError::Parse)?
        .ok_or_else(|| ConfigError::Parse("empty keymap file".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::auto_switch::AutoSwitchContext;
    use crate::config::diagnostics::RuleIssueCode;
    use crate::config::model::KeymapMode;
    use serde_json::{Value, json};
    use std::fs;

    const KEYMAP: &str = r#"{"version": 1, "layers": [{"id": "nav", "name": "Navigation", "keys": {"KeyH": "ArrowLeft"}}], "rules": [{"key": "CapsLock", "hold": {"layer": "nav"}, "tap": "Escape"}]}"#;

    /// A document with an explicitly opened library keymap.
    fn document(settings: Value, keymap: &str) -> (tempfile::TempDir, ConfigDocument) {
        document_named(settings, keymap, "Test")
    }

    fn document_named(
        mut settings: Value,
        keymap: &str,
        name: &str,
    ) -> (tempfile::TempDir, ConfigDocument) {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        settings["version"] = json!(1);
        if settings["settings"]["keymaps"]["manualKeymap"].is_null() {
            settings["settings"]["keymaps"]["manualKeymap"] = json!(format!("user:{name}"));
        }
        paths
            .save_config(&serde_json::to_string_pretty(&settings).unwrap())
            .unwrap();
        paths.save_user_keymap(name, keymap, true).unwrap();
        let mut document = ConfigDocument::load(paths).unwrap();
        document.load_library_for_editing(name).unwrap();
        (dir, document)
    }

    #[test]
    fn restart_keeps_manual_activation_without_opening_an_editor() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        document.set_base_tap_action("KeyA", "Enter").unwrap();
        let loaded = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(loaded.keymap(), &KeymapPreset::initial());
        assert!(loaded.settings().editing_keymap_id.is_none());
        let context = AutoSwitchContext::default();
        assert_eq!(
            loaded.active_keymap_id(&context).unwrap().as_deref(),
            Some("user:Test")
        );
        assert_eq!(
            loaded.active_keymap(&context).unwrap().rules[0]
                .tap_action
                .as_deref(),
            Some("Escape")
        );
        assert!(
            loaded
                .active_keymap(&context)
                .unwrap()
                .rules
                .iter()
                .all(|rule| rule.key != "KeyA")
        );
        let mut automatic = loaded;
        automatic
            .update_settings(|settings| {
                settings.keymap_mode = KeymapMode::Auto;
                settings.auto_default_keymap_id = None;
            })
            .unwrap();
        let automatic = ConfigDocument::load(automatic.paths().clone()).unwrap();
        assert_eq!(automatic.active_keymap_id(&context).unwrap(), None);
        assert_eq!(
            automatic.settings().manual_keymap_id.as_deref(),
            Some("user:Test")
        );
    }

    #[test]
    fn commands_and_assignments_commit_together_or_leave_the_keymap_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut document = ConfigDocument::load(StoragePaths::new(
            dir.path().join("config"),
            dir.path().join("data"),
        ))
        .unwrap();
        document
            .update_settings(|settings| settings.commands_enabled = true)
            .unwrap();
        let command = crate::config::model::Command {
            id: "cmd-test".into(),
            name: "Test".into(),
            posix_script: "true".into(),
            powershell_script: "exit 0".into(),
            working_directory: None,
        };
        let before = document.keymap().clone();
        let failed = document.edit_keymap_with_commands([command.clone()], |config| {
            config.set_base_tap_action("KeyA", "cmd:cmd-test")?;
            config.set_layer_key(
                "missing",
                "KeyB",
                KeyAssignment::Action("cmd:cmd-test".into()),
            )
        });
        assert!(failed.is_err());
        assert_eq!(document.keymap(), &before);
        document
            .edit_keymap_with_commands([command.clone()], |config| {
                config.set_base_tap_action("KeyA", "cmd:cmd-test")
            })
            .unwrap();
        assert_eq!(
            document.keymap().commands.as_slice(),
            std::slice::from_ref(&command)
        );
        assert_eq!(document.base_tap_action("KeyA"), Some("cmd:cmd-test"));
        let before = document.keymap().clone();
        let mut edited = command;
        edited.posix_script = "false".into();
        assert!(
            document
                .edit_keymap_with_commands([edited], |config| {
                    config.set_layer_key("missing", "KeyB", KeyAssignment::Swallow)
                })
                .is_err()
        );
        assert_eq!(document.keymap(), &before);
    }

    #[test]
    fn a_missing_or_damaged_edited_keymap_keeps_the_settings_usable() {
        for damage in ["delete", "corrupt"] {
            let dir = tempfile::tempdir().unwrap();
            let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
            paths
                .save_config(
                    &serde_json::to_string(&json!({
                        "version": 1,
                        "settings": {"keymaps": {"manualKeymap": "user:Test"}, "appearance": "dark"}
                    }))
                    .unwrap(),
                )
                .unwrap();
            paths.save_user_keymap("Test", KEYMAP, true).unwrap();
            match damage {
                "delete" => paths.delete_user_keymap("Test").unwrap(),
                _ => fs::write(paths.keymap_path("Test").unwrap(), "{ not json").unwrap(),
            }

            let mut document = ConfigDocument::load(paths.clone()).unwrap();
            assert_eq!(document.keymap(), &KeymapPreset::initial(), "{damage}");
            assert_eq!(document.settings().appearance, Appearance::Dark);
            assert_eq!(document.settings().editing_keymap_id, None);
            // Saving settings and the fresh keymap never touches the broken file.
            document
                .update_settings(|settings| settings.default_hold_timeout_ms = 300)
                .unwrap();
            if damage == "corrupt" {
                assert_eq!(
                    fs::read_to_string(paths.keymap_path("Test").unwrap()).unwrap(),
                    "{ not json"
                );
            }
            let reloaded = ConfigDocument::load(paths).unwrap();
            assert_eq!(reloaded.keymap(), &KeymapPreset::initial());
            assert_eq!(reloaded.settings().default_hold_timeout_ms, 300);
        }
    }

    #[test]
    fn invalid_identity_and_timing_edits_are_atomic() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        let original = document.config();
        let original_file = document.paths().load_config().unwrap();
        assert!(
            document
                .update_settings(|settings| settings.default_hold_timeout_ms = 0)
                .is_err()
        );
        assert_eq!(document.config(), original);
        assert_eq!(document.paths().load_config().unwrap(), original_file);
        assert!(
            document
                .update_keymap(|keymap| keymap.rules.push(keymap.rules[0].clone()))
                .is_err()
        );
        assert_eq!(document.config(), original);
        assert!(
            document
                .update_keymap(|keymap| keymap.rules[0].hold_timeout_ms = Some(u64::MAX))
                .is_err()
        );
        assert_eq!(document.config(), original);
        assert!(
            document
                .update_settings(|settings| {
                    let item = crate::config::model::AutoRule {
                        id: "duplicate".into(),
                        keymap_id: None,
                        enabled: true,
                        conditions: Default::default(),
                    };
                    settings.auto_rules = vec![item.clone(), item];
                })
                .is_err()
        );
        assert_eq!(document.config(), original);
        assert_eq!(document.paths().load_config().unwrap(), original_file);
    }

    #[test]
    fn non_rule_errors_block_runtime_without_rewriting_the_keymap() {
        for content in [
            r#"{"version":1,"layers":[{"id":"nav","keys":{"KeyA":"macro:missing"}}]}"#,
            r#"{"version":1,"macros":[{"id":"m","steps":["macro:m"]}]}"#,
            r#"{"version":1,"quickMenuPages":[{"cells":{"KeyQ":"cmd:missing"}}]}"#,
        ] {
            let (_dir, document) = document(
                json!({"settings":{"keymaps":{"manualKeymap":"user:Test"}}}),
                content,
            );
            let before = document.paths().load_user_keymap("Test").unwrap();
            assert!(matches!(
                document.runtime_config(&AutoSwitchContext::default()),
                Err(ConfigError::Rules(_))
            ));
            assert_eq!(document.paths().load_user_keymap("Test").unwrap(), before);
        }
    }

    #[test]
    fn reads_settings_and_keymap_from_their_own_files() {
        let (_dir, document) = document(
            json!({"settings": {"devices": {"keyboards": ["/dev/input/by-id/kbd"]}, "appearance": "dark"}}),
            KEYMAP,
        );
        assert_eq!(document.input_device(), Some("/dev/input/by-id/kbd"));
        assert_eq!(document.settings().appearance, Appearance::Dark);
        assert_eq!(document.keymap().rules.len(), 1);
        let config = document.config();
        assert_eq!(
            config.layer_keymaps["nav"].keys["KeyH"].as_deref(),
            Some("ArrowLeft")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn launch_on_startup_follows_the_system_registration() {
        let (dir, mut document) = document(json!({}), KEYMAP);
        let entry = dir.path().join("autostart/dev.bozonx.tecloid.desktop");
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
        assert!(
            !saved.contains("launchAtLogin"),
            "the default is not written"
        );

        // Added outside the application: adopted and pointed at this binary.
        fs::write(&entry, "[Desktop Entry]\nExec=old\n").unwrap();
        assert!(document.sync_autostart().unwrap());
        assert!(document.settings().launch_on_startup);
        assert!(!fs::read_to_string(&entry).unwrap().contains("Exec=old"));
    }

    #[test]
    fn settings_edit_leaves_keymap_alone() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        let keymap_before =
            fs::read_to_string(document.paths.keymaps_dir().join("Test.json")).unwrap();
        document.set_input_device("/dev/input/by-id/other").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        let saved: Value =
            serde_json::from_str(&fs::read_to_string(document.paths.config_path()).unwrap())
                .unwrap();
        assert_eq!(
            saved["version"],
            crate::config::schema::CONFIG_SCHEMA_VERSION
        );
        assert_eq!(
            saved["settings"]["devices"]["keyboards"],
            json!(["/dev/input/by-id/other"])
        );
        assert_eq!(saved["settings"]["locale"], "en");
        assert!(saved.get("rules").is_none());
        assert_eq!(
            fs::read_to_string(document.paths.keymaps_dir().join("Test.json")).unwrap(),
            keymap_before
        );
    }

    #[test]
    fn invalid_settings_are_rejected_without_rewriting_the_file() {
        for text in [
            json!({"version": 1, "settings": {"futureOption": 7}}).to_string(),
            json!({"version": 1, "settings": {"appearance": "blue"}}).to_string(),
            json!({"version": 3, "settings": {}}).to_string(),
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
            r#"{"version": 1, "commands": [{"id": "hello", "sh": "printf hello"}]}"#,
        );
        assert!(!document.settings().commands_enabled);
        document
            .paths
            .save_config(&json!({"version": 1, "settings": {"appearance": "dark"}}).to_string())
            .unwrap();
        document = ConfigDocument::load(document.paths().clone()).unwrap();
        assert!(!document.settings().commands_enabled);
    }

    #[test]
    fn settings_batch_overwrites_external_changes() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
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
        fs::write(document.paths.config_path(), r#"{"version": 1}"#).unwrap();
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
        let (_dir, mut document) = document(json!({}), KEYMAP);
        document.set_base_tap_action("KeyA", "Ctrl+KeyC").unwrap();
        assert_eq!(document.base_tap_action("KeyA"), Some("Ctrl+KeyC"));
        assert_eq!(document.keymap().rules.len(), 2);
        document.set_base_tap_action("KeyA", "Ctrl+KeyV").unwrap();
        assert_eq!(document.base_tap_action("KeyA"), Some("Ctrl+KeyV"));
        assert_eq!(document.keymap().rules.len(), 2);
        assert!(document.set_base_tap_action("CapsLock", "KeyB").is_err());
        assert_eq!(document.keymap().rules.len(), 2);
        let reloaded = ConfigDocument::load(document.paths().clone()).unwrap();
        assert_eq!(reloaded.base_tap_action("KeyA"), None);
    }

    #[test]
    fn additional_key_order_survives_saved_keymap_reload() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        for key in ["F13", "F14", "F15"] {
            document
                .set_layer_key("nav", key, KeyAssignment::Swallow)
                .unwrap();
        }
        document
            .reorder_extra_keys("nav", vec!["F13".into(), "F14".into(), "F15".into()], 0, 2)
            .unwrap();
        document.save_current_keymap_as("Ordered").unwrap();
        let mut restarted = ConfigDocument::load(document.paths.clone()).unwrap();
        restarted.load_library_for_editing("Ordered").unwrap();
        assert_eq!(
            restarted.keymap().layer_keymaps["nav"].extra_key_order,
            ["F14", "F15", "F13"]
        );
        document
            .set_layer_key("nav", "F15", KeyAssignment::Transparent)
            .unwrap();
        assert_eq!(
            document.keymap().layer_keymaps["nav"].extra_key_order,
            ["F14", "F13"]
        );
        let copy = document.clone_layer("nav", "Copy", "").unwrap();
        assert_eq!(
            document.keymap().layer_keymaps[&copy].extra_key_order,
            ["F14", "F13"]
        );
        document.clear_layer_keys("nav").unwrap();
        assert!(
            document.keymap().layer_keymaps["nav"]
                .extra_key_order
                .is_empty()
        );
    }

    #[test]
    fn extra_keys_are_validated_and_keep_their_position() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
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
        let map = &document.keymap().layer_keymaps["nav"];
        assert_eq!(map.extra_key_order, ["F14", "F16"]);
        assert_eq!(map.keys["F16"].as_deref(), Some("F1"));
        assert!(!map.keys.contains_key("F13"));
        document
            .set_extra_key("nav", Some("F16"), "F16", KeyAssignment::Transparent)
            .unwrap();
        let map = &document.keymap().layer_keymaps["nav"];
        assert_eq!(map.extra_key_order, ["F14"]);
        assert!(!map.keys.contains_key("F16"));
    }

    #[test]
    fn extra_key_errors_name_the_problem() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        document
            .set_extra_key("nav", None, "F13", KeyAssignment::Swallow)
            .unwrap();
        let issue = |result: Result<(), ConfigError>| match result {
            Err(ConfigError::ExtraKey(issue)) => Some(issue),
            _ => None,
        };
        assert_eq!(
            issue(document.set_extra_key("nav", None, "F13", KeyAssignment::Swallow)),
            Some(ExtraKeyIssue::Taken)
        );
        assert_eq!(
            issue(document.set_extra_key("nav", None, "KeyQ", KeyAssignment::Swallow)),
            Some(ExtraKeyIssue::OnGrid)
        );
        assert_eq!(
            issue(document.check_extra_key("nav", Some("F13"), "NoSuchKey")),
            Some(ExtraKeyIssue::Invalid)
        );
        assert!(document.check_extra_key("nav", Some("F13"), "F13").is_ok());
        assert!(document.check_extra_key("missing", None, "F14").is_err());
    }

    #[test]
    fn free_layer_names_get_a_number() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        assert_eq!(document.free_layer_name("Copy"), "Copy");
        assert_eq!(document.free_layer_name("Navigation"), "Navigation 2");
        document.create_layer("Navigation 2", "").unwrap();
        assert_eq!(document.free_layer_name("Navigation"), "Navigation 3");
    }

    #[test]
    fn description_is_saved_even_when_the_stored_name_is_invalid() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        document
            .update_keymap(|keymap| keymap.layers[0].name = String::new())
            .unwrap();
        assert!(document.rename_layer("nav", "", "Arrows").is_err());
        document.set_layer_description("nav", " Arrows ").unwrap();
        assert_eq!(
            document.keymap().layers[0].description.as_deref(),
            Some("Arrows")
        );
        document.set_layer_description("nav", "  ").unwrap();
        assert_eq!(document.keymap().layers[0].description, None);
        assert!(document.set_layer_description("missing", "x").is_err());
    }

    #[test]
    fn renaming_keeps_the_description_and_validates_the_name() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        document.set_layer_description("nav", "Arrows").unwrap();
        document.set_layer_name("nav", " Moves ").unwrap();
        assert_eq!(document.keymap().layers[0].name, "Moves");
        assert_eq!(
            document.keymap().layers[0].description.as_deref(),
            Some("Arrows")
        );
        assert!(document.set_layer_name("nav", " ").is_err());
        assert!(document.set_layer_name("missing", "Other").is_err());
        assert_eq!(document.keymap().layers[0].name, "Moves");
    }

    #[test]
    fn swapping_layer_keys_exchanges_actions_and_suppression() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
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
    fn held_modifiers_follow_their_key_and_need_an_action() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        let holds = |document: &ConfigDocument, key| document.layer_key_holds_modifiers("nav", key);
        assert!(
            document
                .set_layer_key_holds_modifiers("nav", "KeyJ", true)
                .is_err()
        );
        document
            .set_layer_key_holds_modifiers("nav", "KeyH", true)
            .unwrap();
        document.swap_layer_keys("nav", "KeyH", "KeyJ").unwrap();
        assert!(!holds(&document, "KeyH") && holds(&document, "KeyJ"));
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Action("Alt+Tab".into()))
            .unwrap();
        assert!(holds(&document, "KeyJ"));
        document
            .set_layer_key("nav", "KeyJ", KeyAssignment::Swallow)
            .unwrap();
        assert!(!holds(&document, "KeyJ"));

        document
            .set_extra_key("nav", None, "F13", KeyAssignment::Action("Alt+Tab".into()))
            .unwrap();
        document
            .set_layer_key_holds_modifiers("nav", "F13", true)
            .unwrap();
        document
            .set_extra_key(
                "nav",
                Some("F13"),
                "F14",
                KeyAssignment::Action("Alt+Tab".into()),
            )
            .unwrap();
        assert!(!holds(&document, "F13") && holds(&document, "F14"));
        document.clear_layer_keys("nav").unwrap();
        assert!(!holds(&document, "F14"));
    }

    #[test]
    fn layer_keys_remain_in_memory_until_explicitly_saved() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
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
        let mut restarted = ConfigDocument::load(document.paths.clone()).unwrap();
        restarted.load_library_for_editing("Test").unwrap();
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
            reloaded.keymap().rules[0].tap_action.as_deref(),
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
    fn manual_settings_changes_require_restart_and_do_not_block_keymap_edits() {
        let (_dir, mut document) = document(json!({"settings": {"appearance": "dark"}}), KEYMAP);
        document
            .paths
            .save_config(r#"{"version":1,"settings":{"appearance":"light"}}"#)
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
        let (_dir, mut document) = document(json!({}), KEYMAP);
        let paths = document.paths().clone();
        let saved = paths.load_user_keymap("Test").unwrap();
        document.set_base_tap_action("KeyA", "Enter").unwrap();
        document.set_locale(LocalePreference::English).unwrap();
        assert_eq!(paths.load_user_keymap("Test").unwrap(), saved);
        let restarted = ConfigDocument::load(paths.clone()).unwrap();
        assert_eq!(restarted.base_tap_action("KeyA"), None);
        assert_eq!(restarted.settings().locale, LocalePreference::English);
        document.save_current_keymap_as("Saved").unwrap();
        document.set_base_tap_action("KeyA", "Tab").unwrap();
        let mut restarted = ConfigDocument::load(paths).unwrap();
        assert!(restarted.settings().editing_keymap_id.is_none());
        restarted.load_library_for_editing("Saved").unwrap();
        assert_eq!(restarted.base_tap_action("KeyA"), Some("Enter"));
    }

    #[test]
    fn missing_files_load_as_a_new_installation() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let document = ConfigDocument::load(paths).unwrap();
        assert_eq!(document.input_device(), None);
        assert_eq!(document.keymap(), &KeymapPreset::initial());
    }

    #[test]
    fn runtime_config_reports_whether_titles_are_needed() {
        let (_dir, mut document) = document_named(
            json!({"settings": {"keymaps": {"manualKeymap": "user:Other"}}}),
            KEYMAP,
            "Main",
        );
        let save = |document: &ConfigDocument, rules: &str| {
            document
                .paths
                .save_user_keymap(
                    "Other",
                    &format!("{{\"version\":1,\"rules\":{rules}}}"),
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
                settings.auto_rules = vec![crate::config::model::AutoRule {
                    id: "r".into(),
                    enabled: true,
                    keymap_id: None,
                    conditions: crate::config::model::KeymapConditionSet {
                        window_patterns: vec!["title:YouTube".into()],
                        ..Default::default()
                    },
                }];
            })
            .unwrap();
        assert!(!uses_titles(&document));
        document
            .update_settings(|settings| settings.keymap_mode = KeymapMode::Auto)
            .unwrap();
        assert!(uses_titles(&document));
        document
            .update_settings(|settings| settings.auto_rules[0].enabled = false)
            .unwrap();
        assert!(!uses_titles(&document));
    }

    #[test]
    fn runtime_config_uses_the_active_library_keymap() {
        let (_dir, mut document) = document_named(
            json!({"settings": {"keymaps": {"manualKeymap": "user:Other"}}}),
            KEYMAP,
            "Main",
        );
        document
            .paths
            .save_user_keymap("Other", r#"{"version": 1, "rules": [{"key": "KeyA", "tap": "KeyB"}, {"key": "KeyC", "enabled": false, "tap": "KeyD"}]}"#, true)
            .unwrap();
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        assert_eq!(runtime.keymap_id.as_deref(), Some("user:Other"));
        let value: Value = serde_json::to_value(&runtime.mapper).unwrap();
        assert_eq!(value["rules"].as_array().unwrap().len(), 1);
        assert_eq!(value["rules"][0]["key"], "KeyA");

        document
            .update_settings(|settings| settings.manual_keymap_id = Some("user:Main".into()))
            .unwrap();
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        let value: Value = serde_json::to_value(&runtime.mapper).unwrap();
        assert_eq!(value["rules"][0]["key"], "CapsLock");
        assert_eq!(value["rules"][0]["tapAction"], "Escape");
        assert_eq!(value["rules"][0]["holdAction"], "");
    }

    #[test]
    fn auto_mode_without_a_match_is_passthrough() {
        let (_dir, document) = document(json!({"settings": {"keymaps": {"mode": "auto"}}}), KEYMAP);
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        assert_eq!(runtime.keymap_id, None);
        let value: Value = serde_json::to_value(&runtime.mapper).unwrap();
        assert!(value["rules"].as_array().unwrap().is_empty());
    }

    #[test]
    fn bundled_keymaps_run_in_the_mapper() {
        for bundled in crate::config::bundled::BUNDLED {
            let (_dir, document) = document(json!({}), bundled.text());
            assert!(
                crate::config::diagnostics::analyze_rules(&document.config()).is_empty(),
                "{}",
                bundled.name
            );
            let runtime = document
                .runtime_config(&AutoSwitchContext::default())
                .unwrap();
            let value: Value = serde_json::to_value(&runtime.mapper).unwrap();
            assert!(!value["rules"].as_array().unwrap().is_empty());
        }
        let (_dir, document) = document(json!({}), crate::config::bundled::BUNDLED[0].text());
        assert!(document.keymap().layers.len() > 1);
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        let value: Value = serde_json::to_value(&runtime.mapper).unwrap();
        assert!(!value["rules"].as_array().unwrap().is_empty());
        let reparsed = keymap_file::parse(&keymap_file::serialize(document.keymap()))
            .unwrap()
            .unwrap();
        assert_eq!(reparsed.layer_keymaps, document.keymap().layer_keymaps);
        assert_eq!(reparsed.layers, document.keymap().layers);
        let quick = &document.keymap().quick_action_pages;
        assert!(!quick.is_empty());
        assert!(quick.iter().all(|page| page.cells.len() == 15));
        let system = quick.iter().find(|page| page.id == "system").unwrap();
        assert_eq!(system.cells["KeyQ"].action, "Sleep");
        assert_eq!(system.cells["KeyS"].action, "PrintScreen");
        assert_eq!(reparsed.quick_action_pages, *quick);
    }

    #[test]
    fn blocking_rule_issues_refuse_runtime_config() {
        let (_dir, document) = document(
            json!({}),
            r#"{"version": 1, "rules": [{"key": "UnknownKey", "tap": "KeyB"}]}"#,
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
            r#"{"version": 1, "rules": [{"key": "KeyA", "tap": "KeyB"}, {"key": "KeyA", "tap": "KeyC"}]}"#,
        );
        let runtime = document
            .runtime_config(&AutoSwitchContext::default())
            .unwrap();
        let value: Value = serde_json::to_value(&runtime.mapper).unwrap();
        assert_eq!(value["rules"].as_array().unwrap().len(), 2);
        assert_eq!(value["rules"][0]["tapAction"], "KeyB");
        assert_eq!(value["rules"][1]["tapAction"], "KeyC");
    }

    #[test]
    fn layer_lifecycle_preserves_keymaps_and_detaches_rules() {
        let (_dir, mut document) = document(json!({}), KEYMAP);
        document
            .set_layer_key("nav", "F13", KeyAssignment::Action("Escape".into()))
            .unwrap();
        let copy = document
            .clone_layer("nav", "Navigation copy", "Copied")
            .unwrap();
        assert_eq!(
            document.keymap().layer_keymaps[&copy].keys["KeyH"].as_deref(),
            Some("ArrowLeft")
        );
        assert_eq!(
            document.keymap().layer_keymaps[&copy].keys["F13"].as_deref(),
            Some("Escape")
        );
        document.clear_layer_keys(&copy).unwrap();
        assert!(document.keymap().layer_keymaps[&copy].keys.is_empty());
        document
            .update_keymap(|keymap| {
                keymap.rules[0].hold_behavior = Some(crate::config::model::HoldBehavior::Layer);
            })
            .unwrap();
        document.delete_layer("nav").unwrap();
        assert_eq!(
            document.keymap().rules[0].hold_behavior(),
            crate::config::model::HoldBehavior::None
        );
        assert!(document.keymap().rules[0].layer_id.is_empty());
        assert!(!document.keymap().layer_keymaps.contains_key("nav"));
        let mut loaded = ConfigDocument::load(document.paths().clone()).unwrap();
        loaded.load_library_for_editing("Test").unwrap();
        assert_eq!(loaded.keymap().layers[0].name, "Navigation");
        document.save_current_keymap_as("Saved").unwrap();
        let mut saved = ConfigDocument::load(document.paths().clone()).unwrap();
        saved.load_library_for_editing("Saved").unwrap();
        assert_eq!(saved.keymap().layers[0].name, "Navigation copy");
        assert!(saved.keymap().rules[0].layer_id.is_empty());
    }
    #[test]
    fn compound_transaction_rolls_back_memory_disk_and_runtime_settings() {
        let (_dir, mut document) = document(json!({}), r#"{"version":1}"#);
        let before_settings = document.settings().clone();
        let before_keymap = document.keymap().clone();
        let disk = std::fs::read(document.paths().config_path()).ok();
        let result: Result<(), ConfigError> = document.transaction(|draft| {
            draft.update_settings(|settings| settings.layer_hint = !settings.layer_hint)?;
            draft.create_layer("Staged", "")?;
            Err(ConfigError::EditConflict)
        });
        assert_eq!(result, Err(ConfigError::EditConflict));
        assert_eq!(document.settings(), &before_settings);
        assert_eq!(document.keymap(), &before_keymap);
        assert_eq!(std::fs::read(document.paths().config_path()).ok(), disk);
        document
            .transaction(|draft| {
                draft.update_settings(|settings| settings.layer_hint = !settings.layer_hint)?;
                draft.create_layer("Committed", "")
            })
            .unwrap();
        assert_ne!(document.settings(), &before_settings);
        assert_eq!(
            document.keymap().layers.len(),
            before_keymap.layers.len() + 1
        );
    }

    #[test]
    fn staged_edits_reject_external_writes_and_keymap_callbacks_reject_settings() {
        let (_dir, mut document) = document(json!({}), r#"{"version":1}"#);
        assert!(
            document
                .transaction(|draft| draft.save_current_keymap_as("Forbidden"))
                .is_err()
        );
        assert!(
            !document
                .keymap_ids()
                .unwrap()
                .contains(&"user:Forbidden".to_owned())
        );
        let before = document.settings().clone();
        assert!(
            document
                .edit_keymap_with_commands([], |draft| draft
                    .update_settings(|settings| settings.layer_hint = true))
                .is_err()
        );
        assert_eq!(document.settings(), &before);
    }
}
