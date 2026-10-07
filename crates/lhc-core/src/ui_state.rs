use crate::storage::StoragePaths;
use serde_json::{Value, json};

pub struct UiState {
    paths: StoragePaths,
    value: Value,
}

impl UiState {
    pub fn load(paths: StoragePaths) -> Self {
        let value = Self::read(&paths).unwrap_or_else(|error| {
            log::warn!("UI state: {error}");
            json!({})
        });
        Self { paths, value }
    }

    fn read(paths: &StoragePaths) -> Result<Value, String> {
        let text = paths.load_ui_state()?;
        if text.trim().is_empty() {
            return Ok(json!({}));
        }
        let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        if !value.is_object() {
            return Err("ui-state.json must contain an object".into());
        }
        Ok(value)
    }

    pub fn selected_layer_id(&self) -> &str {
        self.value["selectedLayerId"].as_str().unwrap_or_default()
    }

    pub fn label_mode(&self) -> i32 {
        match self.value["keyLabelMode"].as_str() {
            Some("code") => 1,
            Some("numeric") => 2,
            _ => 0,
        }
    }

    pub fn update(&mut self, layer: Option<&str>, mode: Option<i32>) -> Result<(), String> {
        let mut value = Self::read(&self.paths)?;
        if let Some(layer) = layer {
            value["selectedLayerId"] = json!(layer);
        }
        if let Some(mode) = mode {
            value["keyLabelMode"] = json!(match mode {
                1 => "code",
                2 => "numeric",
                _ => "label",
            });
        }
        self.paths
            .save_ui_state(&serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?)?;
        self.value = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_shared_fields_and_preserves_external_ui_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        paths
            .save_ui_state(
                r#"{"selectedLayerId":"nav","keyLabelMode":"numeric","homeHelpOpen":false}"#,
            )
            .unwrap();
        let mut state = UiState::load(paths.clone());
        assert_eq!(state.selected_layer_id(), "nav");
        assert_eq!(state.label_mode(), 2);
        paths
            .save_ui_state(r#"{"homeHelpOpen":true,"future":7}"#)
            .unwrap();
        state.update(Some("symbols"), Some(1)).unwrap();
        let value = UiState::read(&paths).unwrap();
        assert_eq!(value["homeHelpOpen"], true);
        assert_eq!(value["future"], 7);
        let state = UiState::load(paths);
        assert_eq!(state.selected_layer_id(), "symbols");
        assert_eq!(state.label_mode(), 1);
    }
}
