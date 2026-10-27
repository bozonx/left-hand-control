//! Editable application configuration.
//!
//! Global settings live in `config.json`; the current keyboard layout
//! stays in memory. Saved layouts live in the user
//! library (`layouts/<name>.json`, id `user:<name>`).
//!
//! Both formats are described in [`crate::profile::settings`] and
//! [`crate::profile::layout_file`].

mod activation;

use crate::profile::actions::{self, Action, ActionIssue};
use crate::profile::diagnostics::RuleIssue;
use crate::profile::model::{
    AppSettings, Appearance, LayoutPreset, LocalePreference, ResolvedAppConfig,
};
use crate::profile::names::{self, NameIssue, NameKind};
use crate::profile::{key_catalog, layout_file, settings};
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

pub use self::activation::RuntimeConfig;
pub use crate::profile::layer_edit::KeyAssignment;

pub struct ConfigDocument {
    paths: StoragePaths,
    settings: AppSettings,
    layout: LayoutPreset,
    /// Why the layout last opened for editing could not be read at load.
    layout_load_error: Option<ConfigError>,
}

impl ConfigDocument {
    pub fn load(paths: StoragePaths) -> Result<Self, ConfigError> {
        paths.ensure().map_err(ConfigError::Io)?;
        let settings_text = paths.load_config().map_err(ConfigError::Io)?;
        let mut settings = settings::parse(&settings_text).map_err(ConfigError::Parse)?;
        let mut layout_load_error = None;
        let layout = match settings.editing_layout_id.as_deref() {
            Some(id) => match load_current_layout(&paths, id) {
                Ok(layout) => layout,
                Err(error) => {
                    // A deleted or damaged library file must not take the
                    // global settings down with it. The editor starts from
                    // a fresh layout that is not linked to the broken file,
                    // so saving it never overwrites that file.
                    log::warn!("layout {id} cannot be opened for editing: {error}");
                    settings.editing_layout_id = None;
                    layout_load_error = Some(error);
                    LayoutPreset::initial()
                }
            },
            None => LayoutPreset::initial(),
        };
        crate::gamemode::set_settings(&settings.game_mode, settings.in_development);
        Ok(Self {
            settings,
            layout,
            paths,
            layout_load_error,
        })
    }

    /// Why the layout that was open for editing could not be read when the
    /// configuration loaded; the editor then started from a fresh layout.
    pub fn layout_load_error(&self) -> Option<&ConfigError> {
        self.layout_load_error.as_ref()
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
    pub fn config(&self) -> ResolvedAppConfig {
        ResolvedAppConfig::from_parts(
            self.settings.clone(),
            self.layout.clone(),
            self.settings.editing_layout_id.as_deref(),
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
        settings::validate(&updated).map_err(ConfigError::Invalid)?;
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
        self.layout.base_tap_action(key)
    }

    pub fn set_base_tap_action(&mut self, key: &str, action: &str) -> Result<(), ConfigError> {
        if let Some(issue) = actions::validate(&Action::parse(Some(action)), &self.config()) {
            return Err(ConfigError::InvalidAction(issue));
        }
        if !self.layout.base_tap_editable(key) {
            return Err(ConfigError::Invalid(format!(
                "key {key} has a rule that requires the full editor"
            )));
        }
        self.update_layout(|layout| layout.set_base_tap(key, action))
    }

    /// Trimmed `name` for a new layer or for renaming layer `except`.
    pub fn layer_name(&self, name: &str, except: Option<&str>) -> Result<String, ConfigError> {
        names::layer_name(&self.layout, name, except)
            .map_err(|issue| ConfigError::Name(NameKind::Layer, issue))
    }

    pub fn create_layer(&mut self, name: &str, description: &str) -> Result<String, ConfigError> {
        let name = self.layer_name(name, None)?;
        let id = crate::profile::ids::generate("layer-");
        self.update_layout(|layout| layout.add_layer(&id, name, optional_text(description)))?;
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
        self.update_layout(|layout| layout.rename_layer(id, name, optional_text(description)))
    }

    pub fn clone_layer(
        &mut self,
        source_id: &str,
        name: &str,
        description: &str,
    ) -> Result<String, ConfigError> {
        let name = self.layer_name(name, None)?;
        self.require_layer(source_id)?;
        let id = crate::profile::ids::generate("layer-");
        self.update_layout(|layout| {
            layout.clone_layer(source_id, &id, name, optional_text(description))
        })?;
        Ok(id)
    }

    pub fn delete_layer(&mut self, id: &str) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        self.update_layout(|layout| layout.remove_layer(id))
    }

    pub fn clear_layer_keys(&mut self, id: &str) -> Result<(), ConfigError> {
        self.require_layer(id)?;
        self.update_layout(|layout| layout.clear_layer_keys(id))
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
        self.update_layout(|layout| layout.get_or_create_layer_keymap(id).extra_key_order = keys)
    }

    fn require_layer(&self, id: &str) -> Result<(), ConfigError> {
        if self.layout.has_layer(id) {
            Ok(())
        } else {
            Err(ConfigError::Invalid("unknown layer".into()))
        }
    }

    /// Entry of `key` in the keymap of `layer_id`.
    pub fn layer_key(&self, layer_id: &str, key: &str) -> KeyAssignment {
        self.layout.layer_key(layer_id, key)
    }

    pub fn set_layer_key(
        &mut self,
        layer_id: &str,
        key: &str,
        assignment: KeyAssignment,
    ) -> Result<(), ConfigError> {
        if !self.layout.has_layer(layer_id) {
            return Err(ConfigError::Invalid(format!(
                "unknown layer \"{layer_id}\""
            )));
        }
        self.check_assignment(&assignment)?;
        self.update_layout(|layout| layout.set_layer_key(layer_id, key, assignment))
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
        if previous != Some(key) && self.layout.layer_has_key(layer_id, key) {
            return Err(ConfigError::Invalid("key already assigned".into()));
        }
        self.check_assignment(&assignment)?;
        self.update_layout(|layout| layout.set_extra_key(layer_id, previous, key, assignment))
    }

    /// Whether `key` of `layer_id` keeps its modifiers pressed until the
    /// layer is released.
    pub fn layer_key_holds_modifiers(&self, layer_id: &str, key: &str) -> bool {
        self.layout.layer_key_holds_modifiers(layer_id, key)
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
        self.update_layout(|layout| layout.set_layer_key_holds_modifiers(layer_id, key, hold))
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
        self.update_layout(|layout| layout.swap_layer_keys(layer_id, a, b))
    }

    /// Stage commands and a layout-only edit, committing the working copy only
    /// when every operation succeeds. The callback must not change settings or files.
    pub fn edit_layout_with_commands<T>(
        &mut self,
        commands: impl IntoIterator<Item = crate::profile::model::Command>,
        edit: impl FnOnce(&mut Self) -> Result<T, ConfigError>,
    ) -> Result<T, ConfigError> {
        let mut draft = Self {
            paths: self.paths.clone(),
            settings: self.settings.clone(),
            layout: self.layout.clone(),
            layout_load_error: self.layout_load_error.clone(),
        };
        for command in commands {
            draft.save_command(command)?;
        }
        let result = edit(&mut draft)?;
        self.layout = draft.layout;
        Ok(result)
    }

    /// Apply `edit` to a copy of the current layout.
    pub fn update_layout(
        &mut self,
        edit: impl FnOnce(&mut LayoutPreset),
    ) -> Result<(), ConfigError> {
        let mut candidate = self.layout.clone();
        edit(&mut candidate);
        crate::profile::ids::validate_layout(&candidate).map_err(ConfigError::Invalid)?;
        crate::profile::timing::validate_layout(&candidate).map_err(ConfigError::Invalid)?;

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
}

fn load_current_layout(paths: &StoragePaths, id: &str) -> Result<LayoutPreset, ConfigError> {
    let name = crate::profile::model::user_layout_name(id)
        .ok_or_else(|| ConfigError::Invalid(format!("unknown layout id \"{id}\"")))?;
    parse_layout(&paths.load_user_layout(name).map_err(ConfigError::Io)?)
}

fn optional_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

fn parse_layout(text: &str) -> Result<LayoutPreset, ConfigError> {
    Ok(layout_file::parse(text)
        .map_err(ConfigError::Parse)?
        .unwrap_or_else(LayoutPreset::initial))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::auto_switch::AutoSwitchContext;
    use crate::profile::diagnostics::RuleIssueCode;
    use crate::profile::model::LayoutMode;
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
    fn commands_and_assignments_commit_together_or_leave_the_layout_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut document = ConfigDocument::load(StoragePaths::new(
            dir.path().join("config"),
            dir.path().join("data"),
        ))
        .unwrap();
        document
            .update_settings(|settings| settings.commands_enabled = true)
            .unwrap();
        let command = crate::profile::model::Command {
            id: "cmd-test".into(),
            name: "Test".into(),
            posix_script: "true".into(),
            powershell_script: "exit 0".into(),
            working_directory: None,
        };
        let before = document.layout().clone();
        let failed = document.edit_layout_with_commands([command.clone()], |config| {
            config.set_base_tap_action("KeyA", "cmd:cmd-test")?;
            config.set_layer_key(
                "missing",
                "KeyB",
                KeyAssignment::Action("cmd:cmd-test".into()),
            )
        });
        assert!(failed.is_err());
        assert_eq!(document.layout(), &before);
        document
            .edit_layout_with_commands([command.clone()], |config| {
                config.set_base_tap_action("KeyA", "cmd:cmd-test")
            })
            .unwrap();
        assert_eq!(
            document.layout().commands.as_slice(),
            std::slice::from_ref(&command)
        );
        assert_eq!(document.base_tap_action("KeyA"), Some("cmd:cmd-test"));
        let before = document.layout().clone();
        let mut edited = command;
        edited.posix_script = "false".into();
        assert!(
            document
                .edit_layout_with_commands([edited], |config| {
                    config.set_layer_key("missing", "KeyB", KeyAssignment::Swallow)
                })
                .is_err()
        );
        assert_eq!(document.layout(), &before);
    }

    #[test]
    fn a_missing_or_damaged_edited_layout_keeps_the_settings_usable() {
        for damage in ["delete", "corrupt"] {
            let dir = tempfile::tempdir().unwrap();
            let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
            paths
                .save_config(
                    &serde_json::to_string(&json!({
                        "schemaVersion": 1,
                        "state": {"editedLayoutId": "user:Test"},
                        "settings": {"activeLayoutId": "user:Test", "appearance": "dark"}
                    }))
                    .unwrap(),
                )
                .unwrap();
            paths.save_user_layout("Test", LAYOUT, true).unwrap();
            match damage {
                "delete" => paths.delete_user_layout("Test").unwrap(),
                _ => fs::write(paths.layout_path("Test").unwrap(), "{ not json").unwrap(),
            }

            let mut document = ConfigDocument::load(paths.clone()).unwrap();
            assert!(document.layout_load_error().is_some(), "{damage}");
            assert_eq!(document.settings().appearance, Appearance::Dark);
            assert_eq!(document.settings().editing_layout_id, None);
            // Saving settings and the fresh layout never touches the broken file.
            document
                .update_settings(|settings| settings.default_hold_timeout_ms = 300)
                .unwrap();
            if damage == "corrupt" {
                assert_eq!(
                    fs::read_to_string(paths.layout_path("Test").unwrap()).unwrap(),
                    "{ not json"
                );
            }
            let reloaded = ConfigDocument::load(paths).unwrap();
            assert!(reloaded.layout_load_error().is_none());
            assert_eq!(reloaded.settings().default_hold_timeout_ms, 300);
        }
    }

    #[test]
    fn invalid_identity_and_timing_edits_are_atomic() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
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
                .update_layout(|layout| layout.rules.push(layout.rules[0].clone()))
                .is_err()
        );
        assert_eq!(document.config(), original);
        assert!(
            document
                .update_layout(|layout| layout.rules[0].hold_timeout_ms = Some(u64::MAX))
                .is_err()
        );
        assert_eq!(document.config(), original);
        assert!(
            document
                .update_settings(|settings| {
                    let item = crate::profile::model::AutoRule {
                        id: "duplicate".into(),
                        layout_id: None,
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
    fn non_rule_errors_block_runtime_without_rewriting_the_layout() {
        for content in [
            r#"{"schemaVersion":1,"layers":[{"id":"nav","keys":{"KeyA":"macro:missing"}}]}"#,
            r#"{"schemaVersion":1,"macros":[{"id":"m","steps":["macro:m"]}]}"#,
            r#"{"schemaVersion":1,"quickActionPages":[{"cells":{"KeyQ":"cmd:missing"}}]}"#,
        ] {
            let (_dir, document) =
                document(json!({"settings":{"activeLayoutId":"user:Test"}}), content);
            let before = document.paths().load_user_layout("Test").unwrap();
            assert!(matches!(
                document.runtime_config(&AutoSwitchContext::default()),
                Err(ConfigError::Rules(_))
            ));
            assert_eq!(document.paths().load_user_layout("Test").unwrap(), before);
        }
    }

    #[test]
    fn reads_settings_and_layout_from_their_own_files() {
        let (_dir, document) = document(
            json!({"settings": {"devices": {"keyboards": ["/dev/input/by-id/kbd"]}, "appearance": "dark"}}),
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
            saved["settings"]["devices"]["keyboards"],
            json!(["/dev/input/by-id/other"])
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
    fn held_modifiers_follow_their_key_and_need_an_action() {
        let (_dir, mut document) = document(json!({}), LAYOUT);
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
                        window_patterns: vec!["title:YouTube".into()],
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
            .update_settings(|settings| settings.manual_layout_id = Some("user:Main".into()))
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
        assert!(!quick.is_empty());
        assert!(quick.iter().all(|page| page.cells.len() == 15));
        assert_eq!(quick[0].cells["KeyQ"].action, "Sleep");
        assert_eq!(quick[0].cells["KeyS"].action, "PrintScreen");
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
