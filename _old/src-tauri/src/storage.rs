pub use lhc_core::storage::StoragePaths;
use lhc_core::storage::TrackedFile;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn resolve_storage_paths() -> Result<StoragePaths, String> {
    StoragePaths::resolve()
}

/// Files the frontend edits, with the contents it last saw. Another shell
/// (Slint) edits the same files, so saves are refused with an
/// `EXTERNAL_CHANGE` error instead of overwriting its changes.
static TRACKED: Mutex<Option<HashMap<PathBuf, TrackedFile>>> = Mutex::new(None);

fn with_tracked<T>(
    path: &Path,
    f: impl FnOnce(&mut TrackedFile) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = TRACKED
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())?;
    let files = guard.get_or_insert_with(HashMap::new);
    if !files.contains_key(path) {
        let (file, _) = TrackedFile::open(path.to_path_buf())?;
        files.insert(path.to_path_buf(), file);
    }
    let file = files
        .get_mut(path)
        .ok_or_else(|| "tracked file missing".to_string())?;
    f(file)
}

pub fn read_tracked(path: &Path) -> Result<String, String> {
    let (file, contents) = TrackedFile::open(path.to_path_buf())?;
    let mut guard = TRACKED
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())?;
    guard
        .get_or_insert_with(HashMap::new)
        .insert(path.to_path_buf(), file);
    Ok(contents)
}

pub fn write_tracked(path: &Path, contents: &str) -> Result<(), String> {
    with_tracked(path, |file| {
        file.write(contents).map_err(|error| error.to_string())
    })
}

/// Whether any of `paths` changed on disk since the frontend read them.
pub fn changed_on_disk(paths: &[PathBuf]) -> Result<bool, String> {
    for path in paths {
        if with_tracked(path, |file| file.changed())?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

static LIBRARY_NAMES: Mutex<Option<Vec<String>>> = Mutex::new(None);

pub fn remember_layouts(names: &[String]) -> Result<(), String> {
    *LIBRARY_NAMES
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())? = Some(names.to_vec());
    Ok(())
}

pub fn library_changed(paths: &StoragePaths) -> Result<bool, String> {
    let names = paths.list_user_layouts()?;
    if LIBRARY_NAMES
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())?
        .as_ref()
        .is_some_and(|previous| previous != &names)
    {
        return Ok(true);
    }
    let tracked: Vec<PathBuf> = TRACKED
        .lock()
        .map_err(|_| "storage lock poisoned".to_string())?
        .as_ref()
        .map(|files| {
            files
                .keys()
                .filter(|path| path.parent() == Some(paths.layouts_dir().as_path()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    changed_on_disk(&tracked)
}
