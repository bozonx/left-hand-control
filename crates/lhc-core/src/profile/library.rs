use crate::{
    config_document::{ConfigDocument, ConfigError},
    profile::{
        auto_switch, layout_file,
        model::{LayoutPreset, user_layout_id},
    },
};

/// What a new library layout starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibrarySource<'a> {
    Empty,
    /// The bundled author's layout.
    IvanK,
    /// An existing library layout, by name.
    Copy(&'a str),
}

impl ConfigDocument {
    /// `base`, or `base (2)`, `base (3)`, … — the first name not taken in the library.
    pub fn unique_library_name(&self, base: &str) -> Result<String, ConfigError> {
        let names = self.paths().list_user_layouts().map_err(ConfigError::Io)?;
        let base = base.trim();
        let mut name = base.to_owned();
        let mut suffix = 2;
        while names.contains(&name) {
            name = format!("{base} ({suffix})");
            suffix += 1;
        }
        Ok(name)
    }

    /// Creates `name` in the library; fails if it is already taken.
    pub fn create_library_layout(
        &self,
        name: &str,
        description: &str,
        source: LibrarySource,
    ) -> Result<String, ConfigError> {
        let description = description.trim();
        let mut preset = match source {
            LibrarySource::Empty => LayoutPreset::default(),
            LibrarySource::IvanK => {
                layout_file::parse(include_str!("../../../../public/ivank-layout.yaml"))
                    .map_err(ConfigError::Parse)?
                    .ok_or_else(|| ConfigError::Invalid("Bundled layout is empty".into()))?
            }
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

    pub fn save_current_layout_as(&mut self, name: &str) -> Result<String, ConfigError> {
        self.ensure_files_unchanged()?;
        let saved = self
            .paths()
            .save_user_layout(name, &layout_file::serialize(self.layout()), false)
            .map_err(ConfigError::Io)?;
        let result = self.update_settings(|settings| {
            let old = settings.current_layout_id.clone();
            let new = user_layout_id(&saved);
            settings.current_layout_id = Some(new);
            if settings.manual_active_layout_id == old {
                settings.manual_active_layout_id = settings.current_layout_id.clone();
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
        if self
            .paths()
            .load_user_layout(name)
            .map_err(ConfigError::Io)?
            != expected
        {
            return Err(ConfigError::ExternalChange);
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
            if settings.current_layout_id.as_ref() == Some(&old) {
                settings.current_layout_id = Some(new.clone());
            }
            if settings.manual_active_layout_id.as_ref() == Some(&old) {
                settings.manual_active_layout_id = Some(new.clone());
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
        if self.settings().current_layout_id.as_ref() == Some(&new) {
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
            return Err(ConfigError::ExternalChange);
        }
        self.paths()
            .delete_user_layout(name)
            .map_err(ConfigError::Io)?;
        let id = user_layout_id(name);
        let result = self.update_settings(|settings| {
            settings.layout_order.retain(|item| item != &id);
            settings
                .auto_rules
                .retain(|rule| rule.layout_id.as_ref() != Some(&id));
            if settings.auto_default_layout_id.as_ref() == Some(&id) {
                settings.auto_default_layout_id = None;
            }
            if settings.current_layout_id.as_ref() == Some(&id) {
                settings.current_layout_id = None;
            }
            if settings.manual_active_layout_id.as_ref() == Some(&id) {
                settings.manual_active_layout_id = None;
            }
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
        self.update_layout(|layout| *layout = preset)?;
        self.update_settings(|settings| settings.current_layout_id = Some(user_layout_id(name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(app_id: &str) -> crate::runtime_state::ActiveWindow {
        crate::runtime_state::ActiveWindow {
            app_id: app_id.into(),
            ..Default::default()
        }
    }
    use crate::{
        profile::{
            auto_switch::AutoSwitchContext,
            model::{AutoRule, LayoutConditionSet, LayoutMode},
        },
        storage::StoragePaths,
    };

    #[test]
    fn saving_renaming_and_deleting_preserve_global_command_switch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_user_layout(
                "Test",
                "commands:\n  - id: hello\n    linux: printf hello\n",
                true,
            )
            .unwrap();
        paths
            .save_config(r#"{"settings":{"currentLayoutId":"user:Test"}}"#)
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
            .create_library_layout("Ivan", "", LibrarySource::IvanK)
            .unwrap();
        assert!(!document.load_layout("user:Ivan").unwrap().rules.is_empty());
        assert_eq!(document.unique_library_name("Ivan").unwrap(), "Ivan (2)");
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
        assert!(document.load_layout("user:Empty").unwrap().rules.is_empty());
    }

    #[test]
    fn moving_layout_preserves_other_priorities_in_both_modes() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        for name in ["A", "B", "C", "D"] {
            paths.save_user_layout(name, "rules: []\n", false).unwrap();
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
                .save_user_layout(name, "description: original\nrules: []\n", false)
                .unwrap();
        }
        document
            .update_settings(|settings| {
                settings.manual_active_layout_id = Some("user:A".into());
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
            document.settings().current_layout_id.as_deref(),
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
            Err(ConfigError::ExternalChange)
        );
        document
            .remove_library_layout("C", &paths.load_user_layout("C").unwrap())
            .unwrap();
        let reloaded = ConfigDocument::load(paths).unwrap();
        assert!(reloaded.settings().current_layout_id.is_none());
        assert_eq!(
            reloaded.settings().manual_active_layout_id.as_deref(),
            Some("user:A")
        );
        assert_eq!(reloaded.settings().layout_order, ["user:A"]);
        assert!(reloaded.settings().auto_rules.is_empty());
        assert!(reloaded.settings().auto_default_layout_id.is_none());
    }

    #[test]
    fn auto_runtime_changes_with_context_and_falls_back_to_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        paths
            .save_user_layout("Editor", "rules:\n  - key: KeyA\n    tap: KeyB\n", false)
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
                            apps: vec!["*kate".into()],
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
    fn rename_rolls_back_when_settings_were_changed_externally() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let mut document = ConfigDocument::load(paths.clone()).unwrap();
        let original = "description: original\nrules: []\n";
        paths.save_user_layout("A", original, false).unwrap();
        paths
            .save_config("{\"settings\":{\"appearance\":\"light\"}}")
            .unwrap();
        assert_eq!(
            document.edit_library_metadata("A", "B", "changed", original),
            Err(ConfigError::ExternalChange)
        );
        assert_eq!(paths.load_user_layout("A").unwrap(), original);
        assert!(paths.load_user_layout("B").is_err());
    }
}
