use lhc_core::{StoragePaths, mapper_config::AppConfig};
use std::path::PathBuf;

#[cfg(not(debug_assertions))]
const APP_ID: &str = "dev.bozonx.left-hand-control";

fn paths() -> Result<StoragePaths, String> {
    #[cfg(debug_assertions)]
    {
        let base = if let Ok(value) = std::env::var("LHC_DEV_DIR") {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map_err(|error| format!("resolve current directory: {error}"))?
                    .join(path)
            }
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|path| path.parent())
                .ok_or_else(|| "resolve repository directory".to_string())?
                .join(".dev-files")
        };
        Ok(StoragePaths::new(base.join("config"), base.join("data")))
    }

    #[cfg(not(debug_assertions))]
    {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| "resolve configuration directory".to_string())?
            .join(APP_ID);
        let data_dir = dirs::data_dir()
            .ok_or_else(|| "resolve data directory".to_string())?
            .join(APP_ID);
        Ok(StoragePaths::new(config_dir, data_dir))
    }
}

pub fn config_status() -> Result<String, String> {
    let storage = paths()?;
    let raw = storage.load_config()?;
    if raw.trim().is_empty() {
        return Ok("Конфигурация пока не создана".into());
    }
    let config: AppConfig =
        serde_json::from_str(&raw).map_err(|error| format!("parse config.json: {error}"))?;
    #[cfg(target_os = "linux")]
    lhc_core::mapper::validation::validate_config(&config)?;
    Ok(format!(
        "Конфигурация загружена: {} правил",
        config.rules.len()
    ))
}
