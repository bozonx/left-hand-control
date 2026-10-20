use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Stable application identifier used for config and data directories.
pub const APP_ID: &str = "dev.bozonx.left-hand-control";

#[derive(Debug, Clone)]
pub struct StoragePaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
}

impl StoragePaths {
    /// Resolve the application storage location.
    ///
    /// An explicit `$LHC_DEV_DIR` overrides storage in every build (relative
    /// paths resolve against the current directory). Debug builds otherwise
    /// use `<repo>/dev-files`, so development never
    /// touches the user's real configuration. Inside it `<os>/` mirrors the
    /// user's home directory, e.g. `dev-files/linux/.config/<APP_ID>/`.
    /// Without an override, release builds use the platform config/data directories under
    /// [`APP_ID`].
    pub fn resolve() -> Result<Self, String> {
        let override_dir = std::env::var_os("LHC_DEV_DIR").map(PathBuf::from);
        if cfg!(debug_assertions) || override_dir.is_some() {
            let base = dev_base_dir(override_dir)?;
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
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
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
        let path = dir.join(format!("{safe}.json"));
        if path.exists() && !overwrite {
            return Err(format!("Layout \"{safe}\" already exists"));
        }
        write_file(&path, contents.as_bytes(), overwrite)?;
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
        let new_path = dir.join(format!("{new_safe}.json"));
        if new_path.exists() && old_path != new_path && !overwrite {
            return Err(format!("Layout \"{new_safe}\" already exists"));
        }
        write_file(
            &new_path,
            contents.as_bytes(),
            overwrite || old_path == new_path,
        )?;
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
        Ok(self.layouts_dir().join(format!("{safe}.json")))
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
    write_file(path, contents, true)
}

fn write_file(path: &Path, contents: &[u8], overwrite: bool) -> Result<(), String> {
    let mut tmp = TemporaryWrite::create(path)?;
    tmp.write(contents)?;
    if overwrite {
        fs::rename(&tmp.path, path).map_err(|e| format!("rename: {e}"))?;
    } else {
        // A hard link publishes the completed file without replacing a destination
        // created after the caller's existence check.
        fs::hard_link(&tmp.path, path).map_err(|e| format!("publish new file: {e}"))?;
        fs::remove_file(&tmp.path).map_err(|e| format!("remove tmp: {e}"))?;
    }
    sync_parent_dir(path)
}

struct TemporaryWrite {
    path: PathBuf,
    file: Option<File>,
}

impl TemporaryWrite {
    fn create(destination: &Path) -> Result<Self, String> {
        for _ in 0..128 {
            let path = unique_tmp_path(destination);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("create tmp: {error}")),
            }
        }
        Err("could not create a unique temporary file".into())
    }

    fn write(&mut self, contents: &[u8]) -> Result<(), String> {
        let mut file = self
            .file
            .take()
            .ok_or_else(|| "temporary file already written".to_owned())?;
        file.write_all(contents)
            .map_err(|e| format!("write tmp: {e}"))?;
        file.sync_all().map_err(|e| format!("sync tmp: {e}"))
    }
}

impl Drop for TemporaryWrite {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn sync_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| format!("sync dir: {e}"))
}

#[cfg(not(unix))]
fn sync_parent_dir(_path: &Path) -> Result<(), String> {
    // File::open cannot portably open directories for syncing on Windows.
    // The file itself is synced before publication on every platform.
    Ok(())
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
            linux.layouts_dir().join("Test.json"),
            PathBuf::from(
                "/repo/dev-files/linux/.local/share/dev.bozonx.left-hand-control/layouts/Test.json"
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
        let absolute = std::env::temp_dir().join("lhc-dev");
        let base = super::dev_base_dir(Some(absolute.clone())).unwrap();
        assert_eq!(base, absolute);
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
    fn validate_layout_name_rejects_invalid_names() {
        assert_eq!(validate_layout_name(" Left hand "), Ok("Left hand".into()));
        assert!(validate_layout_name("   ").is_err());
        assert!(validate_layout_name("...").is_err());
        assert!(validate_layout_name("left/hand").is_err());
    }

    #[test]
    fn save_and_list_layouts_use_sanitized_names() {
        let temp = TempDir::new("storage-layouts");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        let saved = storage
            .save_user_layout("My layout", r#"{"description":"test"}"#, false)
            .expect("save layout");
        assert_eq!(saved, "My layout");
        assert_eq!(
            storage.list_user_layouts().expect("list layouts"),
            vec!["My layout".to_string()]
        );
        assert_eq!(
            storage.load_user_layout("My layout").expect("load layout"),
            r#"{"description":"test"}"#
        );
    }

    #[test]
    fn rename_layout_overwrites_when_requested() {
        let temp = TempDir::new("storage-layouts-rename");
        let storage = StoragePaths::new(temp.path().join("config"), temp.path().join("data"));

        storage
            .save_user_layout("Old", r#"{"description":"one"}"#, false)
            .expect("save old");
        storage
            .save_user_layout("New", r#"{"description":"two"}"#, false)
            .expect("save new");

        assert!(
            storage
                .rename_user_layout("Old", "New", r#"{"description":"updated"}"#, false)
                .is_err()
        );

        let renamed = storage
            .rename_user_layout("Old", "New", r#"{"description":"updated"}"#, true)
            .expect("rename");
        assert_eq!(renamed, "New");
        assert_eq!(
            storage.load_user_layout("New").expect("load new"),
            r#"{"description":"updated"}"#
        );
        assert!(storage.load_user_layout("Old").is_err());
    }

    #[test]
    fn failed_replacement_preserves_destination_and_cleans_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("config.json");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), "original").unwrap();
        assert!(super::write_atomic(&destination, b"replacement").is_err());
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "original"
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn new_file_publication_never_overwrites_an_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("layout.json");
        fs::write(&destination, "external").unwrap();
        assert!(super::write_file(&destination, b"replacement", false).is_err());
        assert_eq!(fs::read_to_string(destination).unwrap(), "external");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn temporary_write_is_removed_when_abandoned() {
        let dir = tempfile::tempdir().unwrap();
        let temporary = super::TemporaryWrite::create(&dir.path().join("config.json")).unwrap();
        let path = temporary.path.clone();
        assert!(path.exists());
        drop(temporary);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn saved_files_are_private_even_when_replacing_a_public_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        super::write_atomic(&path, b"private").unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn concurrent_layout_creations_publish_exactly_one_complete_file() {
        use std::sync::{Arc, Barrier};
        let dir = tempfile::tempdir().unwrap();
        let storage = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
        let barrier = Arc::new(Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|index| {
                let storage = storage.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let contents = format!("writer-{index}").repeat(1024);
                    barrier.wait();
                    storage
                        .save_user_layout("Shared", &contents, false)
                        .ok()
                        .map(|_| contents)
                })
            })
            .collect();
        let winners: Vec<_> = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(winners.len(), 1);
        assert_eq!(storage.load_user_layout("Shared").unwrap(), winners[0]);
        assert_eq!(fs::read_dir(storage.layouts_dir()).unwrap().count(), 1);
    }
}
