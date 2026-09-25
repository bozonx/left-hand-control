#[cfg(debug_assertions)]
use std::path::PathBuf;
#[cfg(not(debug_assertions))]
use tauri::Manager;

pub use lhc_core::storage::StoragePaths;

#[cfg_attr(debug_assertions, allow(unused_variables))]
pub fn resolve_storage_paths(app: &tauri::AppHandle) -> Result<StoragePaths, String> {
    #[cfg(debug_assertions)]
    {
        let base = if let Ok(dev_dir) = std::env::var("LHC_DEV_DIR") {
            let path = PathBuf::from(dev_dir);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map_err(|e| format!("resolve current_dir: {e}"))?
                    .join(path)
            }
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .ok_or_else(|| "resolve repo root from CARGO_MANIFEST_DIR".to_string())?
                .join(".dev-files")
        };
        return Ok(StoragePaths::new(base.join("config"), base.join("data")));
    }

    #[cfg(not(debug_assertions))]
    {
        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|e| format!("resolve app_config_dir: {e}"))?;
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("resolve app_data_dir: {e}"))?;
        Ok(StoragePaths::new(config_dir, data_dir))
    }
}
