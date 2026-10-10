//! Editable application model shared by every shell.
//!
//! These types mirror `types/config.ts`. Serialising [`AppConfig`] with
//! `serde_json` produces the JSON the mapper runtime and the Tauri
//! frontend exchange; the persisted formats (`config.json` settings and
//! YAML layouts) are handled by [`super::settings`] and [`super::layout_file`].

use serde::Serialize;
use std::collections::BTreeMap;

/// Keys that address cells of the Quick and Emoji menus, row by row.
pub const LEFT_HAND_HOTKEYS: [&str; 15] = [
    "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyZ", "KeyX",
    "KeyC", "KeyV", "KeyB",
];

/// First emoji of the standard catalog, used for the default emoji page.
pub const STANDARD_EMOJIS: [&str; 15] = [
    "😀", "😃", "😄", "😁", "😆", "😂", "🤣", "🙂", "🙃", "😉", "😊", "😍", "😘", "😎", "🤔",
];

/// Prefix of ids of layouts stored in the user library.
pub const USER_LAYOUT_PREFIX: &str = "user:";

/// Id of the library layout `name`.
pub fn user_layout_id(name: &str) -> String {
    format!("{USER_LAYOUT_PREFIX}{name}")
}

/// Library name of a layout id, or `None` for ids outside the library.
pub fn user_layout_name(id: &str) -> Option<&str> {
    id.strip_prefix(USER_LAYOUT_PREFIX)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HoldBehavior {
    None,
    Layer,
    Action,
}

/// A rule bound to a physical key. `tap_action` / `hold_action` use the
/// three-state convention: `Some("")` is native passthrough, `None` swallows
/// the key and any other value is an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerRule {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_game_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_layouts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_apps_whitelist: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_apps_blacklist: Option<Vec<String>>,
    pub key: String,
    pub layer_id: String,
    pub tap_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_behavior: Option<HoldBehavior>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub long_hold_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_hold_timeout_ms: Option<u64>,
    pub hold_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_for: Option<String>,
    pub double_tap_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_tap_timeout_ms: Option<u64>,
}

impl LayerRule {
    pub fn hold_behavior(&self) -> HoldBehavior {
        self.hold_behavior.unwrap_or({
            if !self.long_hold_action.is_empty() {
                HoldBehavior::Action
            } else if !self.layer_id.is_empty() {
                HoldBehavior::Layer
            } else {
                HoldBehavior::None
            }
        })
    }

    pub fn new(id: String, key: &str) -> Self {
        Self {
            id,
            enabled: None,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            key: key.into(),
            layer_id: String::new(),
            tap_action: Some(String::new()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: Some(String::new()),
            isolate: None,
            hold_for: None,
            double_tap_action: String::new(),
            hold_timeout_ms: None,
            double_tap_timeout_ms: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled != Some(false)
    }

    pub fn has_trigger(&self) -> bool {
        !self.key.trim().is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerKeymap {
    /// Saved display order of additional keys; absent in older layouts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_key_order: Vec<String>,
    /// Key code → action; `None` swallows the key inside the layer. A
    /// missing entry is transparent.
    pub keys: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroStep {
    pub id: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub steps: Vec<MacroStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_pause_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub name: String,
    pub linux: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAction {
    pub id: String,
    pub name: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickActionPage {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmojiPage {
    pub id: String,
    pub name: String,
    /// Hotkey from [`LEFT_HAND_HOTKEYS`] → inserted text.
    pub cells: BTreeMap<String, String>,
}

impl EmojiPage {
    pub fn default_page() -> Self {
        Self {
            id: "emoji_default".into(),
            name: "Emoji 1".into(),
            cells: LEFT_HAND_HOTKEYS
                .iter()
                .zip(STANDARD_EMOJIS)
                .map(|(key, emoji)| ((*key).into(), emoji.into()))
                .collect(),
        }
    }
}

/// The portable keyboard configuration stored in layout YAML files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPreset {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub layers: Vec<Layer>,
    pub rules: Vec<LayerRule>,
    pub layer_keymaps: BTreeMap<String, LayerKeymap>,
    pub macros: Vec<Macro>,
    pub commands: Vec<Command>,
    pub quick_actions: Vec<QuickAction>,
    pub quick_action_pages: Vec<QuickActionPage>,
    pub emoji_pages: Vec<EmojiPage>,
}

impl LayoutPreset {
    pub fn base_tap_action(&self, key: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|rule| rule.key == key && rule.layer_id.is_empty() && rule.is_enabled())
            .and_then(|rule| rule.tap_action.as_deref())
    }

    /// Preset of a new installation, as `createDefaultConfig()` builds it.
    pub fn initial() -> Self {
        Self {
            quick_action_pages: vec![QuickActionPage {
                id: "quick_default".into(),
                name: "Page 1".into(),
            }],
            emoji_pages: vec![EmojiPage::default_page()],
            ..Self::default()
        }
    }

    pub fn layer_keymap_mut(&mut self, layer_id: &str) -> &mut LayerKeymap {
        self.layer_keymaps.entry(layer_id.into()).or_default()
    }

    /// Number of commands some action of the preset refers to; commands
    /// left behind by reassigned actions are not counted.
    pub fn used_commands(&self) -> usize {
        let refs = self.command_refs();
        self.commands
            .iter()
            .filter(|command| refs.contains(&command.id))
            .count()
    }

    /// Drops commands no action refers to any more: each action owns its
    /// command, so a reassigned action leaves its old command behind.
    pub fn prune_commands(&mut self) {
        let refs = self.command_refs();
        self.commands.retain(|command| refs.contains(&command.id));
    }

    /// Ids of every `cmd:` action anywhere in the preset.
    fn command_refs(&self) -> Vec<String> {
        fn collect(value: &serde_json::Value, refs: &mut Vec<String>) {
            match value {
                serde_json::Value::String(text) => {
                    refs.extend(text.trim().strip_prefix("cmd:").map(str::to_owned))
                }
                serde_json::Value::Array(items) => items.iter().for_each(|v| collect(v, refs)),
                serde_json::Value::Object(map) => map.values().for_each(|v| collect(v, refs)),
                _ => {}
            }
        }
        let actions = serde_json::to_value(Self {
            commands: Vec::new(),
            ..self.clone()
        })
        .unwrap_or_default();
        let mut refs = Vec::new();
        collect(&actions, &mut refs);
        refs
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutConditionSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_mode: Option<String>,
    pub layouts: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
}

impl LayoutConditionSet {
    pub fn is_empty(&self) -> bool {
        self.game_mode.is_none() && self.layouts.is_empty() && self.apps.is_empty()
    }
}

fn is_true(value: &bool) -> bool {
    *value
}

/// One automatic-mode rule: when `conditions` match, `layout_id` is used.
/// Rules are checked in order and the first match wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRule {
    pub id: String,
    /// `None` turns the mapper off (native passthrough).
    pub layout_id: Option<String>,
    /// A disabled rule is kept but never matches.
    #[serde(skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Empty conditions always match.
    #[serde(flatten)]
    pub conditions: LayoutConditionSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameModeProcessMatcher {
    pub id: String,
    pub name: String,
    pub only_active_window: bool,
    pub is_blacklist: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameModeSettings {
    pub use_gamemoded: bool,
    pub use_fullscreen: bool,
    pub process_matchers: Vec<GameModeProcessMatcher>,
}

/// Theme preference; `System` follows the desktop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
    EInk,
}

impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::EInk => "eink",
        }
    }
}

/// UI language preference; `Auto` follows the OS language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum LocalePreference {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en-US")]
    English,
    #[serde(rename = "ru-RU")]
    Russian,
}

impl LocalePreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::English => "en-US",
            Self::Russian => "ru-RU",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    #[default]
    Manual,
    Auto,
}

/// Global settings persisted in `config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub launch_on_startup: bool,
    pub appearance: Appearance,
    pub high_contrast: bool,
    pub reduce_motion: bool,
    pub locale: LocalePreference,
    pub default_hold_timeout_ms: u64,
    pub default_long_hold_timeout_ms: u64,
    pub tap_decision: String,
    pub default_double_tap_timeout_ms: u64,
    pub default_macro_step_pause_ms: u64,
    pub default_macro_modifier_delay_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mouse_device_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_layout_id: Option<String>,
    pub layout_mode: LayoutMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_active_layout_id: Option<String>,
    pub layout_order: Vec<String>,
    pub auto_rules: Vec<AutoRule>,
    /// Layout used in automatic mode when no rule matches; `None` is off.
    pub auto_default_layout_id: Option<String>,
    pub commands_enabled: bool,
    pub game_mode: GameModeSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux_wayland_text_mode: Option<String>,
    pub linux_ydotool_path: String,
    pub linux_xdotool_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_on_startup: false,
            appearance: Appearance::System,
            high_contrast: false,
            reduce_motion: false,
            locale: LocalePreference::Auto,
            default_long_hold_timeout_ms: 1000,
            default_hold_timeout_ms: 200,
            tap_decision: "permissiveHold".into(),
            default_double_tap_timeout_ms: 200,
            default_macro_step_pause_ms: 20,
            default_macro_modifier_delay_ms: 5,
            input_device_path: Some(String::new()),
            input_mouse_device_path: None,
            current_layout_id: None,
            layout_mode: LayoutMode::Manual,
            manual_active_layout_id: None,
            layout_order: Vec::new(),
            auto_rules: Vec::new(),
            auto_default_layout_id: None,
            commands_enabled: false,
            game_mode: GameModeSettings {
                use_gamemoded: true,
                use_fullscreen: false,
                process_matchers: Vec::new(),
            },
            linux_wayland_text_mode: Some("libei".into()),
            linux_ydotool_path: String::new(),
            linux_xdotool_path: String::new(),
        }
    }
}

/// Settings plus the current layout: what the Tauri frontend calls
/// `AppConfig` and what the mapper runtime consumes as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_description: Option<String>,
    pub layers: Vec<Layer>,
    pub rules: Vec<LayerRule>,
    pub layer_keymaps: BTreeMap<String, LayerKeymap>,
    pub macros: Vec<Macro>,
    pub commands: Vec<Command>,
    pub quick_actions: Vec<QuickAction>,
    pub quick_action_pages: Vec<QuickActionPage>,
    pub emoji_pages: Vec<EmojiPage>,
    pub settings: AppSettings,
}

impl AppConfig {
    /// Combine settings with a preset, as `applyPresetToConfig()` does:
    /// the preset's layout id becomes `currentLayoutId` and every layer
    /// gets a keymap.
    pub fn from_parts(
        settings: AppSettings,
        preset: LayoutPreset,
        layout_id: Option<&str>,
    ) -> Self {
        let mut layer_keymaps = preset.layer_keymaps;
        for layer in &preset.layers {
            layer_keymaps.entry(layer.id.clone()).or_default();
        }
        Self {
            version: 1,
            layout_description: preset.description,
            layers: preset.layers,
            rules: preset.rules,
            layer_keymaps,
            macros: preset.macros,
            commands: preset.commands,
            quick_actions: preset.quick_actions,
            quick_action_pages: preset.quick_action_pages,
            emoji_pages: preset.emoji_pages,
            settings: AppSettings {
                current_layout_id: layout_id.map(str::to_owned),
                ..settings
            },
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_commands_skips_orphans() {
        let command = |id: &str| Command {
            id: id.into(),
            name: String::new(),
            linux: "ls".into(),
            working_directory: None,
        };
        let mut rule = LayerRule::new("r1".into(), "KeyA");
        rule.tap_action = Some("cmd:used".into());
        let preset = LayoutPreset {
            rules: vec![rule],
            commands: vec![command("used"), command("orphan")],
            ..LayoutPreset::default()
        };
        assert_eq!(preset.used_commands(), 1);
        let mut preset = preset;
        preset.prune_commands();
        assert_eq!(preset.commands, vec![command("used")]);
    }
}
