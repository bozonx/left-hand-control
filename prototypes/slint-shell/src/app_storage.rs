use lhc_core::{StoragePaths, mapper_config::AppConfig};
use serde_json::Value;
use std::path::PathBuf;

#[cfg(not(debug_assertions))]
const APP_ID: &str = "dev.bozonx.left-hand-control";

pub fn paths() -> Result<StoragePaths, String> {
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

pub fn load_config() -> Result<(Value, AppConfig), String> {
    let raw = paths()?.load_config()?;
    let value: Value = if raw.trim().is_empty() {
        serde_json::json!({"version": 1, "rules": [], "settings": {}})
    } else {
        serde_json::from_str(&raw).map_err(|error| format!("parse config.json: {error}"))?
    };
    let config = parse_config(&value)?;
    Ok((value, config))
}

pub fn parse_config(value: &Value) -> Result<AppConfig, String> {
    let config: AppConfig = serde_json::from_value(value.clone())
        .map_err(|error| format!("parse config.json: {error}"))?;
    #[cfg(target_os = "linux")]
    lhc_core::mapper::validation::validate_config(&config)?;
    Ok(config)
}

pub fn save_config(value: &Value) -> Result<(), String> {
    parse_config(value)?;
    let raw = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    paths()?.save_config(&raw)
}
