// Subset of the frontend AppConfig needed by the Rust key-mapper.
// We deserialize from config.json directly; unknown fields are ignored.

#![cfg(target_os = "linux")]

use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

/// Three-state spec for a rule's tap / hold behaviour.
///   * Native  — act like the physical key itself on that event.
///   * Swallow — nothing happens.
///   * Action  — user-defined action string (for tap: any action;
///     for hold: a keystroke like "ControlLeft" or
///     "ControlLeft+ShiftLeft" that is held down while the physical key
///     is held).
///
/// Deserialization rules (from JSON — the only format Rust reads):
///   missing field      => Native  (via `#[serde(default)]`)
///   null               => Swallow
///   ""                 => Native
///   non-empty string   => Action(s)
#[derive(Debug, Clone, Default)]
pub enum ActionSpec {
    #[default]
    Native,
    Swallow,
    Action(String),
}

impl<'de> Deserialize<'de> for ActionSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Option<String>: null => None, "" => Some(""), "X" => Some("X").
        let v: Option<String> = Option::<String>::deserialize(deserializer)?;
        Ok(match v {
            None => ActionSpec::Swallow,
            Some(s) if s.is_empty() => ActionSpec::Native,
            Some(s) => ActionSpec::Action(s),
        })
    }
}

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
    pub commands: Vec<Command>,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    #[serde(default)]
    pub linux: String,
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
    pub tap_action: ActionSpec,
    #[serde(default)]
    pub hold_action: ActionSpec,
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
    pub keys: HashMap<String, Option<String>>,
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
