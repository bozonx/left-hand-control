use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Application identifier; must match `identifier` in `src-tauri/tauri.conf.json`
/// so both shells share the same config and data directories.
pub const APP_ID: &str = "dev.bozonx.left-hand-control";

#[derive(Debug, Clone)]
pub struct StoragePaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
}

impl StoragePaths {
    /// Resolve the storage location shared by every shell.
    ///
    /// Debug builds use `$LHC_DEV_DIR` (relative paths resolve against the
    /// current directory) or `<repo>/dev-files`, so development never
    /// touches the user's real configuration. Inside it `<os>/` mirrors the
    /// user's home directory, e.g. `dev-files/linux/.config/<APP_ID>/`.
    /// Release builds use the platform config/data directories under
    /// [`APP_ID`], which are the same directories Tauri's
    /// `app_config_dir()` / `app_data_dir()` return.
    pub fn resolve() -> Result<Self, String> {
        if cfg!(debug_assertions) {
            let base = dev_base_dir(std::env::var_os("LHC_DEV_DIR").map(PathBuf::from))?;
            return Ok(Self::dev(&base, std::env::consts::OS));
        }
        let config_dir = dirs::config_dir()
            .ok_or_else(|| "resolve configuration directory".to_string())?
            .join(APP_ID);
        let data_dir = dirs::data_dir()
            .ok_or_else(|| "resolve data directory".to_string())?
            .join(APP_ID);
        Ok(Self::new(config_dir, data_dir))
    }

    /// Development layout for `os` under `base`: the per-OS home mirror.
    pub fn dev(base: &Path, os: &str) -> Self {
        let (folder, config, data) = dev_home_layout(os);
        let home = base.join(folder);
        Self::new(home.join(config).join(APP_ID), home.join(data).join(APP_ID))
    }

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

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
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
        write_atomic(&self.config_path(), contents.as_bytes())
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
        write_atomic(&self.ui_state_path(), contents.as_bytes())
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
        write_atomic(&self.current_layout_path(), contents.as_bytes())
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
            return Err(format!("Layout \"{name}\" not found"));
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
            return Err(format!("Layout \"{safe}\" already exists"));
        }
        write_atomic(&path, contents.as_bytes())?;
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
            return Err(format!("Layout \"{old_name}\" not found"));
        }
        let new_safe = validate_layout_name(new_name)?;
        let dir = self.layouts_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("create_dir_all: {e}"))?;
        let new_path = dir.join(format!("{new_safe}.yaml"));
        if new_path.exists() && old_path != new_path && !overwrite {
            return Err(format!("Layout \"{new_safe}\" already exists"));
        }
        let tmp = unique_tmp_path(&new_path);
        write_tmp_synced(&tmp, contents.as_bytes())?;
        if new_path.exists() && old_path != new_path {
            fs::remove_file(&new_path).map_err(|e| format!("remove_file: {e}"))?;
        }
        if let Err(e) = fs::rename(&tmp, &new_path) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("rename: {e}"));
        }
        sync_parent_dir(&new_path)?;
        if old_path != new_path && old_path.exists() {
            fs::remove_file(&old_path).map_err(|e| format!("remove_file: {e}"))?;
            sync_parent_dir(&old_path)?;
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

/// Why [`TrackedFile::write`] refused to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// Another process changed the file after it was last read or written.
    ExternalChange,
    Io(String),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExternalChange => {
                write!(f, "{EXTERNAL_CHANGE}: file was changed by another process")
            }
            Self::Io(error) => f.write_str(error),
        }
    }
}

/// Marker at the start of errors returned to the frontend when a save was
/// refused because the file changed on disk.
pub const EXTERNAL_CHANGE: &str = "EXTERNAL_CHANGE";

/// A file that remembers the contents it was last read or written with and
/// refuses to overwrite contents another process wrote since then. Both
/// shells edit the same files, so every save goes through this check.
#[derive(Debug, Clone)]
pub struct TrackedFile {
    path: PathBuf,
    known: String,
    /// Metadata of the file taken no later than reading `known`, or `None`
    /// when unknown; lets [`Self::changed`] skip reading an untouched file.
    stamp: Option<Stamp>,
}

/// Modification time and size of a file; `None` when it does not exist.
type Stamp = Option<(std::time::SystemTime, u64)>;

fn stamp(path: &Path) -> Option<Stamp> {
    match fs::metadata(path) {
        Ok(meta) => meta.modified().ok().map(|time| Some((time, meta.len()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(None),
        Err(_) => None,
    }
}

impl TrackedFile {
    /// Read `path`; a missing file reads as an empty string.
    pub fn open(path: PathBuf) -> Result<(Self, String), String> {
        let stamp = stamp(&path);
        let contents = read_or_empty(&path)?;
        Ok((
            Self {
                path,
                known: contents.clone(),
                stamp,
            },
            contents,
        ))
    }

    /// Contents on disk when they differ from the last known ones. A file
    /// whose modification time and size did not change is not read.
    pub fn changed(&mut self) -> Result<Option<String>, String> {
        let before = stamp(&self.path);
        if before.is_some() && before == self.stamp {
            return Ok(None);
        }
        let current = read_or_empty(&self.path)?;
        if current == self.known {
            // Taken before the read, so a write after it changes the stamp.
            self.stamp = before;
            return Ok(None);
        }
        Ok(Some(current))
    }

    /// Accept `contents` (as returned by [`Self::changed`]) as read.
    pub fn mark_read(&mut self, contents: String) {
        self.stamp = None;
        self.known = contents;
    }

    /// Write unless the file changed since it was last read or written.
    /// Writing the contents that are already on disk is a no-op.
    pub fn write(&mut self, contents: &str) -> Result<(), WriteError> {
        let current = read_or_empty(&self.path).map_err(WriteError::Io)?;
        if current == contents {
            self.stamp = None;
            self.known = current;
            return Ok(());
        }
        if current != self.known {
            return Err(WriteError::ExternalChange);
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| WriteError::Io(format!("create_dir_all: {e}")))?;
        }
        write_atomic(&self.path, contents.as_bytes()).map_err(WriteError::Io)?;
        self.stamp = None;
        self.known = contents.to_owned();
        Ok(())
    }
}

fn read_or_empty(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("read {}: {error}", path.display())),
    }
}

pub fn validate_layout_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Layout name cannot be empty".into());
    }
    if trimmed.chars().count() > 128 {
        return Err("Layout name is too long (max 128 characters)".into());
    }
    for ch in trimmed.chars() {
        if ch.is_control() || matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            return Err("layout name contains invalid filename characters".into());
        }
    }
    if trimmed == "." || trimmed == ".." {
        return Err("Layout name is reserved".into());
    }
    if trimmed.starts_with('.') {
        return Err("Layout name cannot start with a dot".into());
    }
    // Windows rejects file names ending in a dot; keep names portable.
    if trimmed.ends_with('.') {
        return Err("Layout name cannot end with a dot".into());
    }
    Ok(trimmed.to_string())
}

/// Per-call unique sibling path for atomic writes. A fixed `.tmp` name
/// would let two concurrent saves of the same file interleave their
/// writes and rename a corrupted mix into place.
fn unique_tmp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{}.{n}.tmp", std::process::id()));
    path.with_file_name(name)
}

/// Home-relative config and data directories per OS, matching `dirs`.
fn dev_home_layout(os: &str) -> (&str, &str, &str) {
    match os {
        "windows" => ("windows", "AppData/Roaming", "AppData/Roaming"),
        "macos" => (
            "macos",
            "Library/Application Support",
            "Library/Application Support",
        ),
        "linux" => ("linux", ".config", ".local/share"),
        other => (other, ".config", ".local/share"),
    }
}

fn dev_base_dir(override_dir: Option<PathBuf>) -> Result<PathBuf, String> {
    match override_dir {
        Some(path) if path.is_absolute() => Ok(path),
        Some(path) => Ok(std::env::current_dir()
            .map_err(|e| format!("resolve current_dir: {e}"))?
            .join(path)),
        None => Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| "resolve repository root".to_string())?
            .join("dev-files")),
    }
}

pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    let tmp = unique_tmp_path(path);
    write_tmp_synced(&tmp, contents)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("rename: {e}"));
    }
    sync_parent_dir(path)
}

fn write_tmp_synced(tmp: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = File::create(tmp).map_err(|e| format!("create tmp: {e}"))?;
    file.write_all(contents)
        .map_err(|e| format!("write tmp: {e}"))?;
    file.sync_all().map_err(|e| format!("sync tmp: {e}"))
}

fn sync_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| format!("sync dir: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn dev_base_dir_defaults_to_repo_dev_files() {
        let base = super::dev_base_dir(None).unwrap();
        assert!(base.ends_with("dev-files"));
        assert!(base.parent().unwrap().join("Cargo.toml").exists());
    }

    #[test]
    fn dev_paths_mirror_home_per_os() {
        let base = std::path::Path::new("/repo/dev-files");
        let linux = StoragePaths::dev(base, "linux");
        assert_eq!(
            linux.config_path(),
            PathBuf::from("/repo/dev-files/linux/.config/dev.bozonx.left-hand-control/config.json")
        );
        assert_eq!(
            linux.current_layout_path(),
            PathBuf::from(
                "/repo/dev-files/linux/.local/share/dev.bozonx.left-hand-control/current-layout.yaml"
            )
        );
        let windows = StoragePaths::dev(base, "windows");
        assert_eq!(
            windows.layouts_dir(),
            PathBuf::from(
                "/repo/dev-files/windows/AppData/Roaming/dev.bozonx.left-hand-control/layouts"
            )
        );
        let macos = StoragePaths::dev(base, "macos");
        assert!(
            macos
                .config_path()
                .starts_with("/repo/dev-files/macos/Library/Application Support")
        );
    }

    #[test]
    fn dev_base_dir_keeps_absolute_override() {
        let base = super::dev_base_dir(Some("/tmp/lhc-dev".into())).unwrap();
        assert_eq!(base, std::path::PathBuf::from("/tmp/lhc-dev"));
    }

    use super::{StoragePaths, validate_layout_name};
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
    fn tracked_file_refuses_to_overwrite_external_changes() {
        use super::{TrackedFile, WriteError};
        let temp = TempDir::new("tracked");
        let path = temp.path().join("nested/config.json");
        let (mut file, contents) = TrackedFile::open(path.clone()).unwrap();
        assert_eq!(contents, "");
        file.write("one").unwrap();
        assert_eq!(file.changed().unwrap(), None);
        fs::write(&path, "two").unwrap();
        assert_eq!(file.write("three"), Err(WriteError::ExternalChange));
        assert_eq!(file.write("two"), Ok(()), "identical contents are accepted");
        file.write("three").unwrap();
        fs::write(&path, "four").unwrap();
        let changed = file.changed().unwrap().unwrap();
        file.mark_read(changed);
        file.write("five").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "five");
    }

    #[test]
    fn tracked_file_notices_changes_after_unchanged_polls() {
        use super::TrackedFile;
        let temp = TempDir::new("tracked-stamp");
        let path = temp.path().join("config.json");
        fs::write(&path, "one").unwrap();
        let (mut file, _) = TrackedFile::open(path.clone()).unwrap();
        assert_eq!(file.changed().unwrap(), None);
        assert_eq!(file.changed().unwrap(), None, "untouched file is skipped");
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&path, "two").unwrap();
        assert_eq!(file.changed().unwrap().as_deref(), Some("two"));
        file.mark_read("two".into());
        assert_eq!(file.changed().unwrap(), None);
        fs::remove_file(&path).unwrap();
        assert_eq!(file.changed().unwrap().as_deref(), Some(""));
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
            storage.load_user_layout("My layout").expect("load layout"),
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

        assert!(
            storage
                .rename_user_layout("Old", "New", "description: updated", false)
                .is_err()
        );

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
