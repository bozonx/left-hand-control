//! Layout library: creating, copying, renaming and deleting user layouts.

use crate::{
    config_document::{ConfigDocument, ConfigError},
    profile::{
        auto_switch,
        bundled::BundledLayout,
        layout_file,
        model::{LayoutPreset, user_layout_id},
        names::NameKind,
    },
};

/// Read one metadata snapshot per listed layout, preserving failures per file.
/// Callers may load this snapshot on a worker without sharing a document.
pub fn load_library_entries(
    paths: &crate::storage::StoragePaths,
    names: &[String],
) -> Vec<Result<super::model::LayoutPreset, crate::config_document::ConfigError>> {
    use crate::config_document::ConfigError;
    names
        .iter()
        .map(|name| {
            let text = paths.load_user_layout(name).map_err(ConfigError::Io)?;
            super::layout_file::parse(&text)
                .map_err(ConfigError::Parse)?
                .ok_or_else(|| ConfigError::Parse("empty layout file".into()))
        })
        .collect()
}

/// What a new library layout starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibrarySource<'a> {
    Empty,
    /// A bundled layout.
    Bundled(&'static BundledLayout),
    /// An existing library layout, by name.
    Copy(&'a str),
}

/// Next step when saving the working copy back to its library entry.
#[derive(Debug, PartialEq, Eq)]
pub enum LibrarySaveTarget {
    NeedsName,
    Conflict,
    Existing(String),
}

fn normalized(text: &str) -> Option<String> {
    layout_file::parse(text)
        .ok()
        .flatten()
        .map(|layout| layout_file::serialize(&layout))
}

impl ConfigDocument {
    /// Resolve a normal save without letting the shell interpret file conflicts.
    pub fn library_save_target(&self, baseline: Option<(&str, &str)>) -> LibrarySaveTarget {
        let Some(name) = self
            .settings()
            .editing_layout_id
            .as_deref()
            .and_then(super::model::user_layout_name)
        else {
            return LibrarySaveTarget::NeedsName;
        };
        if self.can_save_library_layout(name, baseline) {
            LibrarySaveTarget::Existing(name.to_owned())
        } else {
            LibrarySaveTarget::Conflict
        }
    }

    /// Whether saving the working copy as `name` loses nothing on disk: the
    /// file is what the working copy was opened from, or already holds it.
    fn can_save_library_layout(&self, name: &str, baseline: Option<(&str, &str)>) -> bool {
        let config = self;
        let Ok(disk) = config.paths().load_user_layout(name) else {
            return false;
        };
        let Some(disk_layout) = normalized(&disk) else {
            return false;
        };
        let opened = baseline
            .filter(|(baseline, _)| *baseline == name)
            .map(|(_, text)| text);
        if opened
            .is_some_and(|text| text == disk || normalized(text).as_ref() == Some(&disk_layout))
        {
            return true;
        }
        let mut current = config.layout().clone();
        current.remove_unused_commands();
        normalized(&layout_file::serialize(&current)).as_ref() == Some(&disk_layout)
    }

    /// Save the working copy over an existing library file, returning its contents.
    pub fn overwrite_library_layout(&mut self, name: &str) -> Result<String, ConfigError> {
        let mut draft = self.clone();
        draft.update_layout(LayoutPreset::remove_unused_commands)?;
        let text = layout_file::serialize(draft.layout());
        draft
            .paths()
            .save_user_layout(name, &text, true)
            .map_err(ConfigError::Io)?;
        *self = draft;
        Ok(text)
    }

    /// `base`, or `base (2)`, `base (3)`, … — the first name not taken in the library.
    pub fn unique_library_name(&self, base: &str) -> Result<String, ConfigError> {
        let names = self.paths().list_user_layouts().map_err(ConfigError::Io)?;
        super::names::unique_layout_name(base, &names)
            .map_err(|issue| ConfigError::Name(NameKind::Layout, issue))
    }

    /// Creates `name` in the library; fails if it is already taken.
    pub fn create_library_layout(
        &self,
        name: &str,
        description: &str,
        source: LibrarySource,
    ) -> Result<String, ConfigError> {
        let name = &self.checked_layout_name(name, None)?;
        let description = description.trim();
        let mut preset = match source {
            LibrarySource::Empty => LayoutPreset::default(),
            LibrarySource::Bundled(bundled) => bundled.layout().map_err(ConfigError::Parse)?,
            LibrarySource::Copy(from) => {
                let text = self
                    .paths()
                    .load_user_layout(from)
                    .map_err(ConfigError::Io)?;
                if description.is_empty() {
                    // Byte-for-byte copy keeps whatever the source file holds.
                    return self
                        .paths()
                        .save_user_layout(name, &text, false)
                        .map_err(ConfigError::Io);
                }
                layout_file::parse(&text)
                    .map_err(ConfigError::Parse)?
                    .unwrap_or_default()
            }
        };
        if !description.is_empty() {
            preset.description = Some(description.to_owned());
        }
        self.paths()
            .save_user_layout(name, &layout_file::serialize(&preset), false)
            .map_err(ConfigError::Io)
    }

    /// Validate a layout name against the current directory before writing.
    pub fn checked_layout_name(
        &self,
        name: &str,
        except: Option<&str>,
    ) -> Result<String, ConfigError> {
        let names = self.paths().list_user_layouts().map_err(ConfigError::Io)?;
        super::names::layout_name(name, except, names.iter().map(String::as_str))
            .map_err(|issue| ConfigError::Name(NameKind::Layout, issue))
    }

    /// Save an edited working copy under a new name, dropping unreferenced commands.
    pub fn save_working_layout_as(&mut self, name: &str) -> Result<(String, String), ConfigError> {
        let mut draft = self.clone();
        draft.update_layout(LayoutPreset::remove_unused_commands)?;
        let text = layout_file::serialize(draft.layout());
        let saved = draft.save_current_layout_as(name)?;
        *self = draft;
        Ok((saved, text))
    }

    pub fn save_current_layout_as(&mut self, name: &str) -> Result<String, ConfigError> {
        let name = &self.checked_layout_name(name, None)?;
        let saved = self
            .paths()
            .save_user_layout(name, &layout_file::serialize(self.layout()), false)
            .map_err(ConfigError::Io)?;
        let result = self.update_settings(|settings| {
            let old = settings.editing_layout_id.clone();
            let new = user_layout_id(&saved);
            settings.editing_layout_id = Some(new);
            if settings.manual_layout_id == old {
                settings.manual_layout_id = settings.editing_layout_id.clone();
            }
        });
        if let Err(error) = result {
            self.paths()
                .delete_user_layout(&saved)
                .map_err(ConfigError::Io)?;
            return Err(error);
        }
        Ok(saved)
    }

    pub fn ordered_layout_ids(&self) -> Result<Vec<String>, ConfigError> {
        Ok(auto_switch::order_layout_ids(
            &self.layout_ids()?,
            &self.settings().layout_order,
        ))
    }

    pub fn move_library_layout(&mut self, id: &str, delta: i32) -> Result<(), ConfigError> {
        let mut ids = self.ordered_layout_ids()?;
        let index = ids
            .iter()
            .position(|item| item == id)
            .ok_or_else(|| ConfigError::Invalid("Unknown layout".into()))?;
        let next = (index as i64 + i64::from(delta)).clamp(0, ids.len() as i64 - 1) as usize;
        let moved = ids.remove(index);
        ids.insert(next, moved);
        self.update_settings(|settings| settings.layout_order = ids)
    }

    pub fn edit_library_metadata(
        &mut self,
        name: &str,
        new_name: &str,
        description: &str,
        expected: &str,
    ) -> Result<String, ConfigError> {
        let new_name = &self.checked_layout_name(new_name, Some(name))?;
        if self
            .paths()
            .load_user_layout(name)
            .map_err(ConfigError::Io)?
            != expected
        {
            return Err(ConfigError::EditConflict);
        }
        let mut preset = layout_file::parse(expected)
            .map_err(ConfigError::Parse)?
            .unwrap_or_default();
        preset.description =
            (!description.trim().is_empty()).then(|| description.trim().to_owned());
        let text = layout_file::serialize(&preset);
        let saved = self
            .paths()
            .rename_user_layout(name, new_name, &text, false)
            .map_err(ConfigError::Io)?;
        let old = user_layout_id(name);
        let new = user_layout_id(&saved);
        let result = self.update_settings(|settings| {
            if settings.editing_layout_id.as_ref() == Some(&old) {
                settings.editing_layout_id = Some(new.clone());
            }
            if settings.manual_layout_id.as_ref() == Some(&old) {
                settings.manual_layout_id = Some(new.clone());
            }
            for id in &mut settings.layout_order {
                if id == &old {
                    *id = new.clone();
                }
            }
            for target in settings
                .auto_rules
                .iter_mut()
                .map(|rule| &mut rule.layout_id)
                .chain([&mut settings.auto_default_layout_id])
            {
                if target.as_ref() == Some(&old) {
                    *target = Some(new.clone());
                }
            }
        });
        if let Err(error) = result {
            self.paths()
                .rename_user_layout(&saved, name, expected, false)
                .map_err(ConfigError::Io)?;
            return Err(error);
        }
        if self.settings().editing_layout_id.as_ref() == Some(&new) {
            self.update_layout(|layout| layout.description = preset.description)?;
        }
        Ok(saved)
    }

    pub fn remove_library_layout(&mut self, name: &str, expected: &str) -> Result<(), ConfigError> {
        if self
            .paths()
            .load_user_layout(name)
            .map_err(ConfigError::Io)?
            != expected
        {
            return Err(ConfigError::EditConflict);
        }
        self.paths()
            .delete_user_layout(name)
            .map_err(ConfigError::Io)?;
        let id = user_layout_id(name);
        // Deleting the single-mode layout hands activation to the first
        // remaining one; auto mode re-chooses when the mapper is updated.
        let result = self.ordered_layout_ids().and_then(|remaining| {
            self.update_settings(|settings| {
                settings.layout_order.retain(|item| item != &id);
                settings
                    .auto_rules
                    .retain(|rule| rule.layout_id.as_ref() != Some(&id));
                if settings.auto_default_layout_id.as_ref() == Some(&id) {
                    settings.auto_default_layout_id = None;
                }
                if settings.editing_layout_id.as_ref() == Some(&id) {
                    settings.editing_layout_id = None;
                }
                if settings.manual_layout_id.as_ref() == Some(&id) {
                    settings.manual_layout_id = remaining.first().cloned();
                }
            })
        });
        if result.is_err() {
            self.paths()
                .save_user_layout(name, expected, false)
                .map_err(ConfigError::Io)?;
        }
        result
    }

    pub fn load_library_for_editing(&mut self, name: &str) -> Result<(), ConfigError> {
        let preset: LayoutPreset = self.load_layout(&user_layout_id(name))?;
        // Validate before saving the identity, so a failed open changes neither part.
        super::ids::validate_layout(&preset).map_err(ConfigError::Invalid)?;
        super::timing::validate_layout(&preset).map_err(ConfigError::Invalid)?;
        self.update_settings(|settings| settings.editing_layout_id = Some(user_layout_id(name)))?;
        self.update_layout(|layout| *layout = preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::names::NameIssue;

    #[test]
    fn metadata_snapshot_preserves_invalid_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths =
            crate::storage::StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_user_layout(
                "Valid",
                &layout_file::serialize(&LayoutPreset::initial()),
                false,
            )
            .unwrap();
        std::fs::write(paths.layout_path("Broken").unwrap(), "not json").unwrap();
        let names = vec!["Valid".into(), "Broken".into(), "Missing".into()];
        let loaded = load_library_entries(&paths, &names);
        assert!(loaded[0].is_ok());
        assert!(matches!(&loaded[1], Err(ConfigError::Parse(_))));
        assert!(matches!(&loaded[2], Err(ConfigError::Io(_))));
    }

    #[test]
    fn failed_saves_preserve_commands_and_working_copy() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_user_layout(
                "Original",
                r#"{"schemaVersion":1,"commands":[{"id":"unused","unix":"true"}]}"#,
                false,
            )
            .unwrap();
        paths
            .save_config(r#"{"schemaVersion":1,"state":{"editedLayoutId":"user:Original"}}"#)
            .unwrap();
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        let before = doc.layout().clone();
        let settings = doc.settings().clone();
        let original = paths.load_user_layout("Original").unwrap();

        assert!(doc.save_working_layout_as("Original").is_err());
        assert_eq!(doc.layout(), &before);
        assert!(doc.save_working_layout_as("invalid/name").is_err());
        assert_eq!(doc.layout(), &before);

        let blocked = paths.layout_path("Blocked").unwrap();
        std::fs::create_dir(&blocked).unwrap();
        assert!(doc.overwrite_library_layout("Blocked").is_err());
        assert_eq!(doc.layout(), &before);

        std::fs::remove_file(paths.config_path()).unwrap();
        std::fs::create_dir(paths.config_path()).unwrap();
        assert!(doc.save_working_layout_as("New").is_err());
        assert_eq!(doc.layout(), &before);
        assert_eq!(doc.settings(), &settings);
        assert!(!paths.layout_path("New").unwrap().exists());
        assert_eq!(paths.load_user_layout("Original").unwrap(), original);

        std::fs::remove_dir(paths.config_path()).unwrap();
        doc.save_working_layout_as("New").unwrap();
        assert!(doc.layout().commands.is_empty());
        assert!(doc.load_layout("user:New").unwrap().commands.is_empty());
    }

    #[test]
    fn suggested_names_fit_character_and_byte_limits() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let doc = ConfigDocument::load(paths.clone()).unwrap();
        for base in ["a".repeat(128), "я".repeat(125), "界".repeat(83)] {
            paths
                .save_user_layout(&base, r#"{"schemaVersion":1}"#, false)
                .unwrap();
            for suffix in 2..=12 {
                let name = doc.unique_library_name(&base).unwrap();
                assert!(name.ends_with(&format!(" ({suffix})")));
                assert!(crate::storage::validate_layout_name(&name).is_ok());
                paths
                    .save_user_layout(&name, r#"{"schemaVersion":1}"#, false)
                    .unwrap();
            }
        }
        assert_eq!(
            doc.unique_library_name(" "),
            Err(ConfigError::Name(NameKind::Layout, NameIssue::Empty))
        );
        assert_eq!(
            doc.unique_library_name("bad/name"),
            Err(ConfigError::Name(NameKind::Layout, NameIssue::Invalid))
        );
    }

    #[test]
    fn failed_open_preserves_the_working_copy_and_its_identity() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        doc.update_layout(|layout| layout.description = Some("working A".into()))
            .unwrap();
        doc.save_current_layout_as("A").unwrap();
        paths
            .save_user_layout("B", r#"{"schemaVersion":1,"description":"B"}"#, false)
            .unwrap();
        let before = doc.layout().clone();
        std::fs::remove_file(paths.config_path()).unwrap();
        std::fs::create_dir(paths.config_path()).unwrap();

        assert!(doc.load_library_for_editing("B").is_err());
        assert_eq!(doc.layout(), &before);
        assert_eq!(doc.settings().editing_layout_id.as_deref(), Some("user:A"));

        std::fs::remove_dir(paths.config_path()).unwrap();
        doc.load_library_for_editing("B").unwrap();
        assert_eq!(doc.layout().description.as_deref(), Some("B"));
        assert_eq!(doc.settings().editing_layout_id.as_deref(), Some("user:B"));
    }

    fn window(app_id: &str) -> crate::runtime_state::ActiveWindow {
        crate::runtime_state::ActiveWindow {
            app_id: app_id.into(),
            ..Default::default()
        }
    }
    use crate::{
        profile::{
            auto_switch::AutoSwitchContext,
            bundled::BUNDLED,
            model::{AutoRule, LayoutConditionSet, LayoutMode},
        },
        storage::StoragePaths,
    };

    #[test]
    fn save_target_detects_external_changes_and_accepts_unchanged_baselines() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut doc = ConfigDocument::load(paths).unwrap();
        assert_eq!(doc.library_save_target(None), LibrarySaveTarget::NeedsName);
        doc.save_current_layout_as("Saved").unwrap();
        let original = doc.paths().load_user_layout("Saved").unwrap();
        doc.update_layout(|layout| layout.description = Some("working edit".into()))
            .unwrap();
        assert_eq!(
            doc.library_save_target(Some(("Saved", &original))),
            LibrarySaveTarget::Existing("Saved".into())
        );
        let mut external = doc.layout().clone();
        external.description = Some("external edit".into());
        let external = layout_file::serialize(&external);
        doc.paths()
            .save_user_layout("Saved", &external, true)
            .unwrap();
        assert_eq!(
            doc.library_save_target(Some(("Saved", &original))),
            LibrarySaveTarget::Conflict
        );
        assert_eq!(doc.paths().load_user_layout("Saved").unwrap(), external);
        let saved = doc.overwrite_library_layout("Saved").unwrap();
        assert_eq!(doc.paths().load_user_layout("Saved").unwrap(), saved);
        assert_eq!(
            doc.library_save_target(None),
            LibrarySaveTarget::Existing("Saved".into())
        );
    }

    #[test]
    fn portable_metadata_survives_library_operations() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let text = r#"{"schemaVersion":1,"$schema":"https://example.com/layout.json",
            "meta":{"name":"Original","author":"Ivan K","version":"1.0"}}"#;
        paths
            .save_user_layout("Downloaded (2)", text, false)
            .unwrap();
        paths
            .save_config(
                r#"{"schemaVersion":1,"$schema":"https://example.com/config.json",
            "state":{"editedLayoutId":"user:Downloaded (2)"}}"#,
            )
            .unwrap();
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        doc.save_current_layout_as("Saved").unwrap();
        doc.create_library_layout("Copy", "New description", LibrarySource::Copy("Saved"))
            .unwrap();
        let expected = paths.load_user_layout("Copy").unwrap();
        doc.edit_library_metadata("Copy", "Renamed", "Edited description", &expected)
            .unwrap();
        let original = layout_file::parse(text).unwrap().unwrap();
        for name in ["Saved", "Renamed"] {
            let saved = doc.load_layout(&user_layout_id(name)).unwrap();
            assert_eq!(saved.meta, original.meta);
            assert_eq!(saved.schema, original.schema);
        }
        let config = super::super::settings::parse(&paths.load_config().unwrap()).unwrap();
        assert_eq!(
            config.schema.as_deref(),
            Some("https://example.com/config.json")
        );
    }

    #[test]
    fn saving_renaming_and_deleting_preserve_global_command_switch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_user_layout(
                "Test",
                r#"{"schemaVersion": 1, "commands": [{"id": "hello", "unix": "printf hello"}]}"#,
                true,
            )
            .unwrap();
        paths
            .save_config(
                r#"{"schemaVersion":1,"settings":{"commandsEnabled":true},"state":{"editedLayoutId":"user:Test"}}"#,
            )
            .unwrap();
        let mut doc = ConfigDocument::load(paths.clone()).unwrap();
        doc.save_current_layout_as("Hello").unwrap();
        let text = paths.load_user_layout("Hello").unwrap();
        doc.edit_library_metadata("Hello", "Renamed", "", &text)
            .unwrap();
        let text = paths.load_user_layout("Renamed").unwrap();
        doc.remove_library_layout("Renamed", &text).unwrap();
        assert!(doc.settings().commands_enabled);
    }

    #[test]
    fn create_from_sources_keeps_names_unique() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let document = ConfigDocument::load(paths.clone()).unwrap();
        assert_eq!(document.unique_library_name(" New ").unwrap(), "New");
        let ivan = document
            .create_library_layout("Ivan", "", LibrarySource::Bundled(&BUNDLED[0]))
            .unwrap();
        assert!(!document.load_layout("user:Ivan").unwrap().rules.is_empty());
        assert_eq!(document.unique_library_name("Ivan").unwrap(), "Ivan (2)");
        assert_eq!(document.unique_library_name("ivan").unwrap(), "ivan (2)");
        assert_eq!(
            document.create_library_layout("IVAN", "", LibrarySource::Empty),
            Err(ConfigError::Name(NameKind::Layout, NameIssue::Taken))
        );
        assert_eq!(
            document.checked_layout_name("IVAN", Some("Ivan")),
            Ok("IVAN".into())
        );
        assert!(
            document
                .create_library_layout(&ivan, "", LibrarySource::Empty)
                .is_err()
        );
        document
            .create_library_layout("Copy", " mine ", LibrarySource::Copy("Ivan"))
            .unwrap();
        let copy = document.load_layout("user:Copy").unwrap();
        assert_eq!(copy.description.as_deref(), Some("mine"));
        assert_eq!(
            copy.rules.len(),
            document.load_layout("user:Ivan").unwrap().rules.len()
        );
        document
            .create_library_layout("Empty", "", LibrarySource::Empty)
            .unwrap();
        let empty = document.load_layout("user:Empty").unwrap();
        assert!(empty.rules.is_empty());
        assert!(empty.emoji_pages.iter().all(|page| page.cells.is_empty()));
        assert!(
            empty
                .quick_action_pages
                .iter()
                .all(|page| page.cells.is_empty())
        );
    }

    #[test]
    fn moving_layout_preserves_other_priorities_in_both_modes() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        for name in ["A", "B", "C", "D"] {
            paths
                .save_user_layout(name, r#"{"schemaVersion": 1}"#, false)
                .unwrap();
        }
        document.move_library_layout("user:A", 3).unwrap();
        assert_eq!(
            document.ordered_layout_ids().unwrap(),
            ["user:B", "user:C", "user:D", "user:A"]
        );
        document
            .update_settings(|settings| settings.layout_mode = LayoutMode::Auto)
            .unwrap();
        assert_eq!(
            document.ordered_layout_ids().unwrap(),
            ["user:B", "user:C", "user:D", "user:A"]
        );
        document.move_library_layout("user:A", -2).unwrap();
        assert_eq!(
            document.ordered_layout_ids().unwrap(),
            ["user:B", "user:A", "user:C", "user:D"]
        );
        document
            .update_settings(|settings| settings.layout_mode = LayoutMode::Manual)
            .unwrap();
        assert_eq!(
            document.ordered_layout_ids().unwrap(),
            ["user:B", "user:A", "user:C", "user:D"]
        );
        let reloaded = ConfigDocument::load(paths).unwrap();
        assert_eq!(
            reloaded.ordered_layout_ids().unwrap(),
            document.ordered_layout_ids().unwrap()
        );
    }

    #[test]
    fn editing_rename_delete_preserve_activation_and_references() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        for name in ["A", "B"] {
            paths
                .save_user_layout(
                    name,
                    r#"{"schemaVersion": 1, "description": "original"}"#,
                    false,
                )
                .unwrap();
        }
        document
            .update_settings(|settings| {
                settings.manual_layout_id = Some("user:A".into());
                settings.layout_order = vec!["user:A".into(), "user:B".into()];
                settings.auto_rules = vec![AutoRule {
                    id: "r".into(),
                    enabled: true,
                    layout_id: Some("user:B".into()),
                    conditions: LayoutConditionSet::default(),
                }];
                settings.auto_default_layout_id = Some("user:B".into());
            })
            .unwrap();
        document.load_library_for_editing("B").unwrap();
        assert_eq!(
            document
                .active_layout_id(&AutoSwitchContext::default())
                .unwrap()
                .as_deref(),
            Some("user:A")
        );
        let original = paths.load_user_layout("B").unwrap();
        document
            .edit_library_metadata("B", "C", "Описание", &original)
            .unwrap();
        assert_eq!(
            document
                .active_layout(&AutoSwitchContext::default())
                .unwrap()
                .description
                .as_deref(),
            Some("original")
        );
        assert_eq!(
            document.settings().editing_layout_id.as_deref(),
            Some("user:C")
        );
        assert_eq!(
            document.settings().auto_rules[0].layout_id.as_deref(),
            Some("user:C")
        );
        assert_eq!(
            document.settings().auto_default_layout_id.as_deref(),
            Some("user:C")
        );
        assert_eq!(document.layout().description.as_deref(), Some("Описание"));
        document.move_library_layout("user:C", -1).unwrap();
        assert_eq!(document.ordered_layout_ids().unwrap(), ["user:C", "user:A"]);
        assert_eq!(
            document.remove_library_layout("C", &original),
            Err(ConfigError::EditConflict)
        );
        document
            .remove_library_layout("C", &paths.load_user_layout("C").unwrap())
            .unwrap();
        let reloaded = ConfigDocument::load(paths).unwrap();
        assert!(reloaded.settings().editing_layout_id.is_none());
        assert_eq!(
            reloaded.settings().manual_layout_id.as_deref(),
            Some("user:A")
        );
        assert_eq!(reloaded.settings().layout_order, ["user:A"]);
        assert!(reloaded.settings().auto_rules.is_empty());
        assert!(reloaded.settings().auto_default_layout_id.is_none());
    }

    #[test]
    fn deleting_single_mode_layout_activates_first_remaining() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        for name in ["A", "B", "C"] {
            paths
                .save_user_layout(name, r#"{"schemaVersion": 1}"#, false)
                .unwrap();
        }
        document
            .update_settings(|settings| {
                settings.manual_layout_id = Some("user:B".into());
                settings.layout_order = vec!["user:C".into(), "user:B".into(), "user:A".into()];
            })
            .unwrap();
        document
            .remove_library_layout("A", &paths.load_user_layout("A").unwrap())
            .unwrap();
        assert_eq!(
            document.settings().manual_layout_id.as_deref(),
            Some("user:B")
        );
        document
            .remove_library_layout("B", &paths.load_user_layout("B").unwrap())
            .unwrap();
        assert_eq!(
            document.settings().manual_layout_id.as_deref(),
            Some("user:C")
        );
        document
            .remove_library_layout("C", &paths.load_user_layout("C").unwrap())
            .unwrap();
        assert!(document.settings().manual_layout_id.is_none());
    }

    #[test]
    fn auto_runtime_changes_with_context_and_falls_back_to_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        paths
            .save_user_layout(
                "Editor",
                r#"{"schemaVersion": 1, "rules": [{"key": "KeyA", "tap": "KeyB"}]}"#,
                false,
            )
            .unwrap();
        document
            .update_settings(|settings| {
                settings.layout_mode = LayoutMode::Auto;
                settings.auto_rules = vec![
                    AutoRule {
                        id: "games".into(),
                        enabled: true,
                        layout_id: None,
                        conditions: LayoutConditionSet {
                            game_mode: Some("on".into()),
                            ..Default::default()
                        },
                    },
                    AutoRule {
                        id: "editor".into(),
                        enabled: true,
                        layout_id: Some("user:Editor".into()),
                        conditions: LayoutConditionSet {
                            game_mode: None,
                            layouts: vec!["us".into()],
                            window_patterns: vec!["*kate".into()],
                            excluded_window_patterns: vec![],
                        },
                    },
                ];
            })
            .unwrap();
        let mut context = AutoSwitchContext {
            system_layout: Some("us".into()),
            window: Some(window("org.kde.kate")),
            ..Default::default()
        };
        let active = document.runtime_config(&context).unwrap();
        assert_eq!(active.layout_id.as_deref(), Some("user:Editor"));
        for change in 0..3 {
            match change {
                0 => context.window = Some(window("browser")),
                1 => {
                    context.window = Some(window("org.kde.kate"));
                    context.system_layout = Some("ru".into());
                }
                _ => {
                    context.system_layout = Some("us".into());
                    context.game_mode_active = true;
                }
            }
            let runtime = document.runtime_config(&context).unwrap();
            assert!(runtime.layout_id.is_none());
            let raw: serde_json::Value = serde_json::from_str(&runtime.json).unwrap();
            assert!(raw["rules"].as_array().unwrap().is_empty());
        }
    }

    #[test]
    fn rename_saves_settings_from_the_running_document() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        let original = r#"{"schemaVersion": 1, "description": "original"}"#;
        paths.save_user_layout("A", original, false).unwrap();
        paths
            .save_config(r#"{"schemaVersion":1,"settings":{"appearance":"light"}}"#)
            .unwrap();
        document
            .edit_library_metadata("A", "B", "changed", original)
            .unwrap();
        assert!(paths.load_user_layout("A").is_err());
        assert!(paths.load_user_layout("B").unwrap().contains("changed"));
        let restarted = ConfigDocument::load(paths).unwrap();
        assert_eq!(
            restarted.settings().appearance,
            document.settings().appearance
        );
    }
}
