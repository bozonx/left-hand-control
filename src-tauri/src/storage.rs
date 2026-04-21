use std::fs;
use std::path::{Path, PathBuf};

pub struct StoragePaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    legacy_dir: Option<PathBuf>,
}

impl StoragePaths {
    pub fn new(config_dir: PathBuf, data_dir: PathBuf, legacy_dir: Option<PathBuf>) -> Self {
        Self {
            config_dir,
            data_dir,
            legacy_dir,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }

    pub fn layouts_dir(&self) -> PathBuf {
        self.data_dir.join("layouts")
    }

    pub fn ensure(&self) -> Result<(), String> {
        fs::create_dir_all(&self.config_dir).map_err(|e| format!("create_dir_all: {e}"))?;
        fs::create_dir_all(&self.data_dir).map_err(|e| format!("create_dir_all: {e}"))?;

        if let Some(legacy_dir) = &self.legacy_dir {
            migrate_file_if_missing(
                &legacy_dir.join("config.json"),
                &self.config_dir.join("config.json"),
            )?;
            migrate_layouts_if_missing(&legacy_dir.join("layouts"), &self.layouts_dir())?;
        }

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

fn migrate_file_if_missing(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() || !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create_dir_all: {e}"))?;
    }
    fs::copy(from, to).map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))?;
    Ok(())
}

fn migrate_layouts_if_missing(from_dir: &Path, to_dir: &Path) -> Result<(), String> {
    if !from_dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(to_dir).map_err(|e| format!("create_dir_all: {e}"))?;
    for entry in fs::read_dir(from_dir).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
        let from = entry.path();
        if !from.is_file() {
            continue;
        }
        let to = to_dir.join(entry.file_name());
        migrate_file_if_missing(&from, &to)?;
    }
    Ok(())
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
    fn ensure_migrates_legacy_files_once() {
        let temp = TempDir::new("storage-migrate");
        let legacy = temp.path().join("legacy");
        let config_dir = temp.path().join("config");
        let data_dir = temp.path().join("data");
        fs::create_dir_all(legacy.join("layouts")).expect("create legacy layouts");
        fs::write(legacy.join("config.json"), b"{\"v\":1}").expect("write legacy config");
        fs::write(legacy.join("layouts").join("vim.yaml"), b"name: vim")
            .expect("write legacy layout");

        let storage = StoragePaths::new(config_dir.clone(), data_dir.clone(), Some(legacy));
        storage.ensure().expect("ensure storage");

        assert_eq!(
            fs::read_to_string(config_dir.join("config.json")).expect("read config"),
            "{\"v\":1}"
        );
        assert_eq!(
            fs::read_to_string(data_dir.join("layouts").join("vim.yaml")).expect("read layout"),
            "name: vim"
        );
    }

    #[test]
    fn save_and_list_layouts_use_sanitized_names() {
        let temp = TempDir::new("storage-layouts");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"), None);

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
