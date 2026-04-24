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

    pub fn save_user_layout(
        &self,
        name: &str,
        contents: &str,
        overwrite: bool,
    ) -> Result<String, String> {
        self.ensure()?;
        let dir = self.layouts_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let safe = validate_layout_name(name)?;
        let path = dir.join(format!("{safe}.yaml"));
        if path.exists() && !overwrite {
            return Err(format!("layout '{safe}' already exists"));
        }
        let tmp = dir.join(format!("{safe}.yaml.tmp"));
        fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
        Ok(safe)
    }

    pub fn rename_user_layout(
        &self,
        old_name: &str,
        new_name: &str,
        contents: &str,
        overwrite: bool,
    ) -> Result<String, String> {
        self.ensure()?;
        let old_path = self.layout_path(old_name)?;
        if !old_path.exists() {
            return Err(format!("layout '{old_name}' not found"));
        }
        let new_safe = validate_layout_name(new_name)?;
        let dir = self.layouts_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let new_path = dir.join(format!("{new_safe}.yaml"));
        if new_path.exists() && old_path != new_path && !overwrite {
            return Err(format!("layout '{new_safe}' already exists"));
        }
        let tmp = dir.join(format!("{new_safe}.yaml.tmp"));
        fs::write(&tmp, contents.as_bytes()).map_err(|e| format!("write tmp: {e}"))?;
        if new_path.exists() && old_path != new_path {
            fs::remove_file(&new_path).map_err(|e| format!("remove_file: {e}"))?;
        }
        fs::rename(&tmp, &new_path).map_err(|e| format!("rename: {e}"))?;
        if old_path != new_path && old_path.exists() {
            fs::remove_file(&old_path).map_err(|e| format!("remove_file: {e}"))?;
        }
        Ok(new_safe)
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
        let safe = validate_layout_name(name)?;
        Ok(self.layouts_dir().join(format!("{safe}.yaml")))
    }
}

pub fn validate_layout_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("layout name is empty".into());
    }
    for ch in trimmed.chars() {
        if ch.is_control() || matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        {
            return Err("layout name contains invalid filename characters".into());
        }
    }
    if trimmed == "." || trimmed == ".." {
        return Err("layout name is reserved".into());
    }
    if trimmed.starts_with('.') {
        return Err("layout name cannot start with '.'".into());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{validate_layout_name, StoragePaths};
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
    fn validate_layout_name_rejects_invalid_names() {
        assert_eq!(validate_layout_name(" Left hand "), Ok("Left hand".into()));
        assert!(validate_layout_name("   ").is_err());
        assert!(validate_layout_name("...").is_err());
        assert!(validate_layout_name("left/hand").is_err());
    }

    #[test]
    fn save_and_load_ui_state_roundtrip() {
        let temp = TempDir::new("storage-ui-state");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        storage
            .save_ui_state("{\"selectedLayerId\":\"nav\"}")
            .expect("save ui state");

        assert_eq!(
            storage.load_ui_state().expect("load ui state"),
            "{\"selectedLayerId\":\"nav\"}"
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
            .save_user_layout("My layout", "description: test", false)
            .expect("save layout");
        assert_eq!(saved, "My layout");
        assert_eq!(
            storage.list_user_layouts().expect("list layouts"),
            vec!["My layout".to_string()]
        );
        assert_eq!(
            storage
                .load_user_layout("My layout")
                .expect("load layout"),
            "description: test"
        );
    }

    #[test]
    fn rename_layout_overwrites_when_requested() {
        let temp = TempDir::new("storage-layouts-rename");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        storage
            .save_user_layout("Old", "description: one", false)
            .expect("save old");
        storage
            .save_user_layout("New", "description: two", false)
            .expect("save new");

        assert!(storage
            .rename_user_layout("Old", "New", "description: updated", false)
            .is_err());

        let renamed = storage
            .rename_user_layout("Old", "New", "description: updated", true)
            .expect("rename");
        assert_eq!(renamed, "New");
        assert_eq!(
            storage.load_user_layout("New").expect("load new"),
            "description: updated"
        );
        assert!(storage.load_user_layout("Old").is_err());
    }
}
