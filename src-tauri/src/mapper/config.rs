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
    pub macros: Vec<Macro>,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Macro {
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    #[serde(default)]
    pub steps: Vec<MacroStep>,
    #[serde(default)]
    pub step_pause_ms: Option<u64>,
    #[serde(default)]
    pub modifier_delay_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MacroStep {
    #[allow(dead_code)]
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub keystroke: String,
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
    #[serde(default)]
    pub double_tap_action: String,
    #[serde(default)]
    pub double_tap_timeout_ms: Option<u64>,
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
    #[serde(default = "default_step_pause")]
    pub default_macro_step_pause_ms: u64,
    #[serde(default = "default_mod_delay")]
    pub default_macro_modifier_delay_ms: u64,
    #[serde(default = "default_double_tap")]
    pub default_double_tap_timeout_ms: u64,
    #[allow(dead_code)]
    #[serde(default)]
    pub input_device_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_hold_timeout_ms: default_hold(),
            default_macro_step_pause_ms: default_step_pause(),
            default_macro_modifier_delay_ms: default_mod_delay(),
            default_double_tap_timeout_ms: default_double_tap(),
            input_device_path: None,
        }
    }
}

fn default_hold() -> u64 {
    200
}

fn default_step_pause() -> u64 {
    20
}

fn default_mod_delay() -> u64 {
    5
}

fn default_double_tap() -> u64 {
    200
}
