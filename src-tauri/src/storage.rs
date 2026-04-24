use std::fs;
use std::path::PathBuf;

pub struct StoragePaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
}

impl StoragePaths {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            config_dir,
            data_dir,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }

    pub fn settings_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    pub fn ui_state_path(&self) -> PathBuf {
        self.config_dir.join("ui-state.json")
    }

    pub fn current_layout_path(&self) -> PathBuf {
        self.data_dir.join("current-layout.yaml")
    }

    pub fn layouts_dir(&self) -> PathBuf {
        self.data_dir.join("layouts")
    }

    pub fn ensure(&self) -> Result<(), String> {
        fs::create_dir_all(&self.config_dir).map_err(|e| format!("create_dir_all: {e}"))?;
        fs::create_dir_all(&self.data_dir).map_err(|e| format!("create_dir_all: {e}"))?;

        Ok(())
    }

    pub fn load_config(&self) -> Result<String, String> {
        self.ensure()?;
        let path = self.config_path();
        if !path.exists() {
            return Ok(String::new());
        }
        fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
    }

    pub fn save_config(&self, contents: &str) -> Result<(), String> {
        self.ensure()?;
        fs::create_dir_all(&self.config_dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let path = self.config_path();
        let tmp = self.config_dir.join("config.json.tmp");
        fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }

    pub fn load_ui_state(&self) -> Result<String, String> {
        self.ensure()?;
        let path = self.ui_state_path();
        if !path.exists() {
            return Ok(String::new());
        }
        fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
    }

    pub fn save_ui_state(&self, contents: &str) -> Result<(), String> {
        self.ensure()?;
        fs::create_dir_all(&self.config_dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let path = self.ui_state_path();
        let tmp = self.config_dir.join("ui-state.json.tmp");
        fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }

    pub fn load_current_layout(&self) -> Result<String, String> {
        self.ensure()?;
        let path = self.current_layout_path();
        if !path.exists() {
            return Ok(String::new());
        }
        fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
    }

    pub fn save_current_layout(&self, contents: &str) -> Result<(), String> {
        self.ensure()?;
        fs::create_dir_all(&self.data_dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let path = self.current_layout_path();
        let tmp = self.data_dir.join("current-layout.yaml.tmp");
        fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
        Ok(())
    }

    pub fn list_user_layouts(&self) -> Result<Vec<String>, String> {
        self.ensure()?;
        let dir = self.layouts_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| format!("read_dir: {e}"))? {
            let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            if p.extension().and_then(|s| s.to_str()) != Some("yaml") {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                out.push(stem.to_string());
            }
        }
        out.sort_unstable();
        Ok(out)
    }

    pub fn load_user_layout(&self, name: &str) -> Result<String, String> {
        self.ensure()?;
        let path = self.layout_path(name)?;
        if !path.exists() {
            return Err(format!("layout '{name}' not found"));
        }
        fs::read_to_string(&path).map_err(|e| format!("read_to_string: {e}"))
    }

    pub fn save_user_layout(&self, name: &str, contents: &str) -> Result<String, String> {
        self.ensure()?;
        let dir = self.layouts_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let safe = sanitize_layout_name(name)?;
        let path = dir.join(format!("{safe}.yaml"));
        let tmp = dir.join(format!("{safe}.yaml.tmp"));
        fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
        Ok(safe)
    }

    pub fn delete_user_layout(&self, name: &str) -> Result<(), String> {
        self.ensure()?;
        let path = self.layout_path(name)?;
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("remove_file: {e}"))?;
        }
        Ok(())
    }

    pub fn layout_path(&self, name: &str) -> Result<PathBuf, String> {
        let safe = sanitize_layout_name(name)?;
        Ok(self.layouts_dir().join(format!("{safe}.yaml")))
    }
}

pub fn sanitize_layout_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("layout name is empty".into());
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' ') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let out = out.trim_start_matches('.').trim().to_string();
    if out.is_empty() {
        return Err("layout name has no valid characters".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{sanitize_layout_name, StoragePaths};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("lhc-{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn sanitize_layout_name_normalizes_and_rejects_empty_names() {
        assert_eq!(
            sanitize_layout_name(" ../left*hand?control "),
            Ok("_left_hand_control".into())
        );
        assert!(sanitize_layout_name("   ").is_err());
        assert!(sanitize_layout_name("...").is_err());
    }

    #[test]
    fn save_and_load_ui_state_roundtrip() {
        let temp = TempDir::new("storage-ui-state");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        storage
            .save_ui_state("{\"activeTab\":\"keymap\"}")
            .expect("save ui state");

        assert_eq!(
            storage.load_ui_state().expect("load ui state"),
            "{\"activeTab\":\"keymap\"}"
        );
    }

    #[test]
    fn save_and_load_current_layout_roundtrip() {
        let temp = TempDir::new("storage-current-layout");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        storage
            .save_current_layout("name: Current")
            .expect("save current layout");

        assert_eq!(
            storage.load_current_layout().expect("load current layout"),
            "name: Current"
        );
    }

    #[test]
    fn save_and_list_layouts_use_sanitized_names() {
        let temp = TempDir::new("storage-layouts");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        let saved = storage
            .save_user_layout("  .my/layout  ", "name: test")
            .expect("save layout");
        assert_eq!(saved, "my_layout");
        assert_eq!(
            storage.list_user_layouts().expect("list layouts"),
            vec!["my_layout".to_string()]
        );
        assert_eq!(
            storage
                .load_user_layout("  .my/layout  ")
                .expect("load layout"),
            "name: test"
        );
    }
}
