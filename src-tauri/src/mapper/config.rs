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
///   null (JSON)          => Swallow
///   ""                   => Native
///   non-empty string       => Action(s)
/// Missing fields default to Native via #[serde(default)].
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
    #[serde(default)]
    #[allow(dead_code)]
    pub windows: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub macos: String,
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
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    pub action: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
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
    pub hold_action: ActionSpec,
    #[serde(default)]
    pub isolate: String,
    #[serde(default)]
    pub hold_timeout_ms: Option<u64>,
    #[serde(default)]
    pub double_tap_action: String,
    #[serde(default)]
    pub double_tap_timeout_ms: Option<u64>,
}
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ExtraKey {
    #[serde(default)]
    #[allow(dead_code)]
    pub id: String,
    #[serde(alias = "name")]
    pub key: String,
    pub action: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct LayerKeymap {
    #[serde(default)]
    pub keys: HashMap<String, Option<String>>,
    #[serde(default)]
    pub isolate: Vec<String>,
    #[serde(default)]
    pub extras: Vec<ExtraKey>,
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
    #[serde(default)]
    #[allow(dead_code)]
    pub input_device_path: Option<String>,
    #[serde(default)]
    pub current_layout_id: Option<String>,
    #[serde(default)]
    pub command_trust: HashMap<String, CommandTrustEntry>,
    #[serde(default)]
    #[allow(dead_code)]
    pub game_mode: GameModeSettings,
    #[serde(default)]
    pub linux_wayland_text_mode: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommandTrustEntry {
    pub fingerprint: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub trusted_at: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameModeSettings {
    #[serde(default)]
    pub use_gamemoded: bool,
    #[serde(default)]
    pub use_fullscreen: bool,
    #[serde(default)]
    pub process_matchers: Vec<GameModeProcessMatcher>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameModeProcessMatcher {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub match_mode: GameModeProcessMatchMode,
    #[serde(default)]
    pub only_active_window: bool,
    #[serde(default)]
    pub is_blacklist: bool,
}

#[derive(Debug, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GameModeProcessMatchMode {
    Exact,
    #[default]
    Substring,
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
            default_hold_timeout_ms: default_hold(),
            default_macro_step_pause_ms: default_step_pause(),
            default_macro_modifier_delay_ms: default_mod_delay(),
            default_double_tap_timeout_ms: default_double_tap(),
            input_device_path: None,
            current_layout_id: None,
            command_trust: HashMap::new(),
            game_mode: GameModeSettings::default(),
            linux_wayland_text_mode: None,
        }
    }
}

impl Settings {
    pub fn commands_trusted(&self, commands: &[Command]) -> bool {
        if commands.is_empty() {
            return true;
        }
        let key = self
            .current_layout_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("custom");
        self.command_trust
            .get(key)
            .is_some_and(|entry| entry.fingerprint == command_fingerprint(commands))
    }
}

fn command_fingerprint(commands: &[Command]) -> String {
    let mut hash: u32 = 0x811c9dc5;
    for command in commands {
        update_hash(&mut hash, command.id.as_bytes());
        update_hash(&mut hash, &[0]);
        update_hash(&mut hash, command.linux.as_bytes());
        update_hash(&mut hash, &[0]);
    }
    format!("{hash:08x}")
}

fn update_hash(hash: &mut u32, bytes: &[u8]) {
    for b in bytes {
        *hash ^= u32::from(*b);
        *hash = hash.wrapping_mul(0x01000193);
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
    fn app_config_deserializes_from_frontend_json() {
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
        assert_eq!(s.default_double_tap_timeout_ms, 200);
        assert!(s.input_device_path.is_none());
        assert!(!s.game_mode.use_gamemoded);
        assert!(!s.game_mode.use_fullscreen);
    }

    #[test]
    fn command_trust_requires_matching_fingerprint() {
        let commands = vec![Command {
            id: "play".into(),
            name: "Play".into(),
            linux: "playerctl play-pause".into(),
            windows: String::new(),
            macos: String::new(),
        }];
        assert_eq!(command_fingerprint(&commands), "4b1e677e");

        let mut settings = Settings {
            current_layout_id: Some("user:test".into()),
            ..Settings::default()
        };
        assert!(!settings.commands_trusted(&commands));

        settings.command_trust.insert(
            "user:test".into(),
            CommandTrustEntry {
                fingerprint: command_fingerprint(&commands),
                trusted_at: "2026-05-04T00:00:00.000Z".into(),
            },
        );
        assert!(settings.commands_trusted(&commands));

        let changed = vec![Command {
            id: "play".into(),
            name: "Play".into(),
            linux: "notify-send changed".into(),
            windows: String::new(),
            macos: String::new(),
        }];
        assert!(!settings.commands_trusted(&changed));
    }

    #[test]
    fn command_fingerprint_regression() {
        let commands = vec![
            Command {
                id: "play".into(),
                name: "Play".into(),
                linux: "playerctl play".into(),
                windows: String::new(),
                macos: String::new(),
            },
            Command {
                id: "pause".into(),
                name: "Pause".into(),
                linux: "playerctl pause".into(),
                windows: String::new(),
                macos: String::new(),
            },
        ];
        let fp = command_fingerprint(&commands);
        assert_eq!(
            fp, "0ab77369",
            "command_fingerprint changed — update the TS side too"
        );
    }
}
