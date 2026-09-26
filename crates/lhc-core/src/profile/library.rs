use crate::{
    config_document::{ConfigDocument, ConfigError},
    profile::{auto_switch, layout_file, model::LayoutPreset},
};

impl ConfigDocument {
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
        ids.swap(index, next);
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
        let old = format!("user:{name}");
        let new = format!("user:{saved}");
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
            if let Some(rule) = settings.layout_conditions.remove(&old) {
                settings.layout_conditions.insert(new.clone(), rule);
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
        let id = format!("user:{name}");
        let result = self.update_settings(|settings| {
            settings.layout_order.retain(|item| item != &id);
            settings.layout_conditions.remove(&id);
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
        let preset: LayoutPreset = self.load_layout(&format!("user:{name}"))?;
        self.update_layout(|layout| *layout = preset)?;
        self.update_settings(|settings| settings.current_layout_id = Some(format!("user:{name}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        profile::{
            auto_switch::AutoSwitchContext,
            model::{LayoutConditionRule, LayoutConditionSet, LayoutMode},
        },
        storage::StoragePaths,
    };

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
                settings.layout_conditions.insert(
                    "user:B".into(),
                    LayoutConditionRule {
                        enabled_in_auto: true,
                        ..Default::default()
                    },
                );
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
        assert!(document.settings().layout_conditions.contains_key("user:C"));
        assert!(!document.settings().layout_conditions.contains_key("user:B"));
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
        assert!(reloaded.settings().layout_conditions.is_empty());
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
                settings.layout_conditions.insert(
                    "user:Editor".into(),
                    LayoutConditionRule {
                        enabled_in_auto: true,
                        whitelist: Some(LayoutConditionSet {
                            game_mode: None,
                            layouts: vec!["us".into()],
                            apps: vec!["kate".into()],
                        }),
                        blacklist: Some(LayoutConditionSet {
                            game_mode: Some("on".into()),
                            layouts: vec![],
                            apps: vec![],
                        }),
                    },
                );
            })
            .unwrap();
        let mut context = AutoSwitchContext {
            system_layout: Some("us".into()),
            window_app_id: Some("org.kde.kate".into()),
            game_mode_detection_enabled: true,
            ..Default::default()
        };
        let active = document.runtime_config(&context).unwrap();
        assert_eq!(active.layout_id.as_deref(), Some("user:Editor"));
        for change in 0..3 {
            match change {
                0 => context.window_app_id = Some("browser".into()),
                1 => {
                    context.window_app_id = Some("kate".into());
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
