// Subset of the frontend AppConfig needed by the Rust key-mapper.
// We deserialize from config.json directly; unknown fields are ignored.

#![cfg(target_os = "linux")]

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub layer_keymaps: HashMap<String, LayerKeymap>,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    #[allow(dead_code)]
    #[serde(default)]
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub layer_id: String,
    #[serde(default)]
    pub tap_action: String,
    #[serde(default)]
    pub hold_timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct LayerKeymap {
    #[serde(default)]
    pub keys: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_hold")]
    pub default_hold_timeout_ms: u64,
    #[allow(dead_code)]
    #[serde(default)]
    pub input_device_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_hold_timeout_ms: default_hold(),
            input_device_path: None,
        }
    }
}

fn default_hold() -> u64 {
    200
}
