//! Mapper configuration: rules, layer keymaps, macros, commands and settings as
//! the engine receives them.

pub use crate::profile::model::TapDecision;
use crate::profile::model::{PasteShortcut, TextMode};
use crate::profile::timing;
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
///   null (JSON)          => Swallow
///   ""                   => Native
///   non-empty string       => Action(s)
/// Missing fields default to Native via #[serde(default)].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Debug, Deserialize, Default, Clone, PartialEq, Eq)]
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

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    #[serde(default)]
    pub linux: String,
    #[serde(default)]
    pub windows: String,
    #[serde(default)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Macro {
    pub id: String,
    #[serde(default)]
    pub steps: Vec<MacroStep>,
    #[serde(default)]
    pub step_pause_ms: Option<u64>,
    #[serde(default)]
    pub modifier_delay_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MacroStep {
    #[serde(default)]
    pub action: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub condition_game_mode: Option<GameModeCondition>,
    #[serde(default)]
    pub condition_layouts: Option<Vec<String>>,
    #[serde(default)]
    pub condition_apps_whitelist: Option<Vec<String>>,
    #[serde(default)]
    pub condition_apps_blacklist: Option<Vec<String>>,
    pub key: String,
    #[serde(default)]
    pub layer_id: String,
    #[serde(default)]
    pub tap_action: ActionSpec,
    #[serde(default)]
    pub hold_behavior: Option<crate::profile::model::HoldBehavior>,
    #[serde(default)]
    pub long_hold_action: String,
    #[serde(default)]
    pub long_hold_timeout_ms: Option<u64>,
    #[serde(default)]
    pub hold_action: ActionSpec,
    #[serde(default)]
    pub hold_timeout_ms: Option<u64>,
    #[serde(default)]
    pub tap_decision: Option<TapDecision>,
    #[serde(default)]
    pub double_tap_action: String,
    #[serde(default)]
    pub double_tap_timeout_ms: Option<u64>,
}
#[derive(Debug, Deserialize, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LayerKeymap {
    #[serde(default)]
    pub keys: HashMap<String, Option<String>>,
    /// Keys whose keystroke modifiers stay pressed until the layer ends.
    #[serde(default)]
    pub hold_modifiers: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Development gate supplied by the application runtime projection.
    #[serde(default)]
    pub in_development: Option<bool>,
    #[serde(default = "default_hold")]
    pub default_hold_timeout_ms: u64,
    #[serde(default = "default_long_hold")]
    pub default_long_hold_timeout_ms: u64,
    #[serde(default)]
    pub tap_decision: TapDecision,
    #[serde(default = "default_step_pause")]
    pub default_macro_step_pause_ms: u64,
    #[serde(default = "default_mod_delay")]
    pub default_macro_modifier_delay_ms: u64,
    #[serde(default = "default_macro_timeout")]
    pub macro_timeout_ms: u64,
    #[serde(default = "default_double_tap")]
    pub default_double_tap_timeout_ms: u64,
    #[serde(default = "default_combo_timeout")]
    pub combo_timeout_ms: u64,
    #[serde(default)]
    pub current_layout_id: Option<String>,
    #[serde(default)]
    pub commands_enabled: bool,
    #[serde(default)]
    pub linux_wayland_text_mode: TextMode,
    #[serde(default)]
    pub linux_paste_shortcut: PasteShortcut,
    #[serde(default = "default_terminal_paste")]
    pub linux_terminal_paste_shortcut: Option<PasteShortcut>,
    #[serde(default)]
    pub linux_ydotool_path: Option<String>,
    #[serde(default)]
    pub linux_xdotool_path: Option<String>,
}

/// Three-valued condition on game-mode state for a rule.
/// `Ignore` (the default when the field is absent) means the rule always
/// fires regardless of game-mode state.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameModeCondition {
    On,
    Off,
    Ignore,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            in_development: None,
            default_long_hold_timeout_ms: default_long_hold(),
            default_hold_timeout_ms: default_hold(),
            tap_decision: TapDecision::default(),
            default_macro_step_pause_ms: default_step_pause(),
            default_macro_modifier_delay_ms: default_mod_delay(),
            macro_timeout_ms: default_macro_timeout(),
            default_double_tap_timeout_ms: default_double_tap(),
            combo_timeout_ms: default_combo_timeout(),
            current_layout_id: None,
            commands_enabled: false,
            linux_wayland_text_mode: TextMode::Libei,
            linux_paste_shortcut: PasteShortcut::ShiftInsert,
            linux_terminal_paste_shortcut: default_terminal_paste(),
            linux_ydotool_path: None,
            linux_xdotool_path: None,
        }
    }
}

fn default_terminal_paste() -> Option<PasteShortcut> {
    Some(PasteShortcut::CtrlShiftV)
}

fn default_long_hold() -> u64 {
    timing::DEFAULT_LONG_HOLD_MS
}

fn default_hold() -> u64 {
    timing::DEFAULT_HOLD_MS
}

fn default_step_pause() -> u64 {
    timing::DEFAULT_MACRO_STEP_PAUSE_MS
}

fn default_mod_delay() -> u64 {
    timing::DEFAULT_MACRO_MODIFIER_DELAY_MS
}

fn default_macro_timeout() -> u64 {
    timing::DEFAULT_MACRO_TIMEOUT_MS
}

fn default_double_tap() -> u64 {
    timing::DEFAULT_DOUBLE_TAP_MS
}

fn default_combo_timeout() -> u64 {
    timing::DEFAULT_COMBO_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_null_action_spec_is_swallow() {
        let spec: ActionSpec = serde_json::from_str("null").unwrap();
        assert!(matches!(spec, ActionSpec::Swallow));
    }

    #[test]
    fn deserialize_empty_string_action_spec_is_native() {
        let spec: ActionSpec = serde_json::from_str(r#""""#).unwrap();
        assert!(matches!(spec, ActionSpec::Native));
    }

    #[test]
    fn deserialize_nonempty_string_action_spec_is_action() {
        let spec: ActionSpec = serde_json::from_str(r#""Escape""#).unwrap();
        assert!(matches!(spec, ActionSpec::Action(ref s) if s == "Escape"));
    }

    #[test]
    fn missing_action_spec_field_defaults_to_native() {
        let rule: Rule = serde_json::from_str(r#"{"key":"KeyQ"}"#).unwrap();
        assert!(matches!(rule.tap_action, ActionSpec::Native));
        assert!(matches!(rule.hold_action, ActionSpec::Native));
    }

    #[test]
    fn rule_enabled_defaults_to_true() {
        let rule: Rule = serde_json::from_str(r#"{"key":"KeyQ"}"#).unwrap();
        assert!(rule.enabled);
    }

    #[test]
    fn app_config_deserializes_from_runtime_json() {
        let json = r#"{
            "rules":[{"key":"KeyQ","tapAction":"Escape","holdAction":""}],
            "layerKeymaps":{},
            "macros":[],
            "commands":[],
            "settings":{"defaultHoldTimeoutMs":300}
        }"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.rules.len(), 1);
        assert_eq!(cfg.rules[0].key, "KeyQ");
        assert!(matches!(cfg.rules[0].tap_action, ActionSpec::Action(ref s) if s == "Escape"));
        assert!(matches!(cfg.rules[0].hold_action, ActionSpec::Native));
        assert_eq!(cfg.settings.default_hold_timeout_ms, 300);
    }

    #[test]
    fn default_settings_values() {
        let s = Settings::default();
        assert_eq!(s.default_hold_timeout_ms, 200);
        assert_eq!(s.default_macro_step_pause_ms, 20);
        assert_eq!(s.default_macro_modifier_delay_ms, 5);
        assert_eq!(s.macro_timeout_ms, 10_000);
        assert_eq!(s.default_double_tap_timeout_ms, 200);
        assert_eq!(s.combo_timeout_ms, 50);
        assert_eq!(s.tap_decision, TapDecision::PermissiveHold);
        assert_eq!(s.linux_wayland_text_mode, TextMode::Libei);
        assert_eq!(s.linux_paste_shortcut, PasteShortcut::ShiftInsert);
        assert_eq!(
            s.linux_terminal_paste_shortcut,
            Some(PasteShortcut::CtrlShiftV)
        );
        assert!(s.linux_ydotool_path.is_none());
    }
}
