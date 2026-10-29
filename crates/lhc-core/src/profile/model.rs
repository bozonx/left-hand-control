//! Editable application model used by the shell and mapper.
//!
//! Serialising [`ResolvedAppConfig`] with `serde_json` produces the JSON consumed
//! by the mapper runtime; the persisted formats (`config.json` settings and
//! JSON layouts) are handled by [`super::settings`] and [`super::layout_file`].

use super::timing;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How the mapper resolves tap vs hold when another key is pressed while a
/// tap/hold rule key is still held and undecided.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum TapDecision {
    /// QMK-style permissive hold: buffer the interrupting key and only
    /// commit the hold on a nested release or timeout.
    #[default]
    PermissiveHold,
    /// Behaviour: any other key press commits the hold immediately.
    HoldOnOtherKeyPress,
}

/// Keys that address cells of the Quick and Emoji menus, row by row.
pub const LEFT_HAND_HOTKEYS: [&str; 15] = [
    "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyZ", "KeyX",
    "KeyC", "KeyV", "KeyB",
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
pub struct KeyRule {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition_game_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "conditionLayouts")]
    pub condition_keyboard_layouts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "conditionAppsWhitelist")]
    pub condition_window_patterns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "conditionAppsBlacklist")]
    pub condition_excluded_window_patterns: Option<Vec<String>>,
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
    pub double_tap_action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_decision: Option<TapDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_tap_timeout_ms: Option<u64>,
}

impl KeyRule {
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
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: key.into(),
            layer_id: String::new(),
            tap_action: Some(String::new()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: Some(String::new()),
            double_tap_action: String::new(),
            hold_timeout_ms: None,
            tap_decision: None,
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
    /// Saved display order of additional keys.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_key_order: Vec<String>,
    /// Key code → action; `None` swallows the key inside the layer. A
    /// missing entry is transparent.
    pub keys: BTreeMap<String, Option<String>>,
    /// Keys whose keystroke modifiers stay pressed until the layer is
    /// released (an Alt+Tab key keeps Alt down for the window switcher).
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub hold_modifiers: BTreeSet<String>,
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
    /// POSIX `sh` script, run on Linux and macOS.
    #[serde(rename = "linux")]
    pub posix_script: String,
    /// PowerShell script, run on Windows.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(rename = "windows")]
    pub powershell_script: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

impl Command {
    /// The script this operating system runs.
    pub fn script(&self) -> &str {
        if cfg!(target_os = "windows") {
            &self.powershell_script
        } else {
            &self.posix_script
        }
    }

    /// Name shown for the command: its name, or the script when unnamed.
    pub fn display_name(&self) -> &str {
        display_name(&self.name, self.script())
    }

    /// Replace the script this operating system runs.
    pub fn set_script(&mut self, script: String) {
        if cfg!(target_os = "windows") {
            self.powershell_script = script;
        } else {
            self.posix_script = script;
        }
    }
}

/// `name`, or `fallback` when the name is empty.
pub fn display_name<'a>(name: &'a str, fallback: &'a str) -> &'a str {
    if name.is_empty() { fallback } else { name }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAction {
    /// Caption in the menu; the action is shown when it is empty.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickActionPage {
    pub id: String,
    /// Optional caption; an empty page is shown by its number.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Hotkey from [`LEFT_HAND_HOTKEYS`] → action; empty cells are absent.
    pub cells: BTreeMap<String, QuickAction>,
}

impl QuickActionPage {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            cells: BTreeMap::new(),
        }
    }

    pub fn default_page() -> Self {
        Self::new("page-default".into(), String::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmojiPage {
    pub id: String,
    /// Optional caption; an empty page is shown by its number.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Hotkey from [`LEFT_HAND_HOTKEYS`] → inserted text.
    pub cells: BTreeMap<String, String>,
}

impl EmojiPage {
    pub fn default_page() -> Self {
        Self {
            id: "emoji-default".into(),
            name: String::new(),
            cells: BTreeMap::new(),
        }
    }
}

/// Portable author information, independent of the local library filename.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutMetadata {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::schema::non_null"
    )]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::schema::non_null"
    )]
    pub author: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::schema::non_null"
    )]
    pub version: Option<String>,
}

/// The portable keyboard configuration stored in layout JSON files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPreset {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<LayoutMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub layers: Vec<Layer>,
    pub rules: Vec<KeyRule>,
    pub layer_keymaps: BTreeMap<String, LayerKeymap>,
    pub macros: Vec<Macro>,
    pub commands: Vec<Command>,
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
            quick_action_pages: vec![QuickActionPage::default_page()],
            emoji_pages: vec![EmojiPage::default_page()],
            ..Self::default()
        }
    }

    pub fn get_or_create_layer_keymap(&mut self, layer_id: &str) -> &mut LayerKeymap {
        self.layer_keymaps.entry(layer_id.into()).or_default()
    }

    /// Number of commands some action of the preset refers to; commands
    /// left behind by reassigned actions are not counted.
    pub fn used_command_count(&self) -> usize {
        let refs = self.command_refs();
        self.commands
            .iter()
            .filter(|command| refs.contains(&command.id))
            .count()
    }

    /// Drops commands no action refers to any more: each action owns its
    /// command, so a reassigned action leaves its old command behind.
    pub fn remove_unused_commands(&mut self) {
        let refs = self.command_refs();
        self.commands.retain(|command| refs.contains(&command.id));
    }

    /// Ids of every `cmd:` action anywhere in the preset.
    fn command_refs(&self) -> Vec<String> {
        let mut refs = Vec::new();
        self.for_each_action(|_, raw| {
            if let super::actions::Action::Command(id) = super::actions::Action::parse(Some(raw)) {
                refs.push(id);
            }
        });
        refs
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutConditionSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_mode: Option<String>,
    pub layouts: Vec<String>,
    /// Window patterns, see [`super::app_match`].
    #[serde(rename = "windows", skip_serializing_if = "Vec::is_empty")]
    pub window_patterns: Vec<String>,
    /// Window patterns that block the match.
    #[serde(rename = "excludeWindows", skip_serializing_if = "Vec::is_empty")]
    pub excluded_window_patterns: Vec<String>,
}

impl LayoutConditionSet {
    pub fn is_empty(&self) -> bool {
        self.game_mode.is_none()
            && self.layouts.is_empty()
            && self.window_patterns.is_empty()
            && self.excluded_window_patterns.is_empty()
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
    /// Known-game sources, see [`crate::gamemode::GameSource`]. Each
    /// platform uses only the ones it supports.
    pub use_steam: bool,
    pub use_lutris: bool,
    pub use_heroic: bool,
    pub use_umu: bool,
    pub use_gamescope: bool,
    pub use_app_category: bool,
    pub use_game_bar: bool,
    /// Games from the sources above count only while their window is
    /// active; sources that cannot name the game's processes always count.
    pub foreground_only: bool,
    pub process_matchers: Vec<GameModeProcessMatcher>,
    /// Persisted Auto/On/Off choice, restored at startup.
    pub control: GameModeControl,
}

/// Who decides whether game mode is active: detection or the user.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameModeControl {
    #[default]
    Auto,
    On,
    Off,
}

/// Theme preference; `System` follows the desktop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalePreference {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en-US")]
    English,
    #[serde(rename = "ru-RU")]
    Russian,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "pt")]
    Portuguese,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "tr")]
    Turkish,
    #[serde(rename = "uk")]
    Ukrainian,
    #[serde(rename = "cs")]
    Czech,
    #[serde(rename = "hu")]
    Hungarian,
    #[serde(rename = "ro")]
    Romanian,
    #[serde(rename = "el")]
    Greek,
    #[serde(rename = "hr")]
    Croatian,
    #[serde(rename = "ka")]
    Georgian,
    #[serde(rename = "kk")]
    Kazakh,
    #[serde(rename = "zh-Hans")]
    ChineseSimplified,
    #[serde(rename = "zh-Hant")]
    ChineseTraditional,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "ko")]
    Korean,
}

impl LocalePreference {
    /// Every choice, in the order the settings page lists them.
    pub const ALL: [Self; 22] = [
        Self::Auto,
        Self::English,
        Self::Russian,
        Self::German,
        Self::French,
        Self::Spanish,
        Self::Italian,
        Self::Portuguese,
        Self::Polish,
        Self::Turkish,
        Self::Ukrainian,
        Self::Czech,
        Self::Hungarian,
        Self::Romanian,
        Self::Greek,
        Self::Croatian,
        Self::Georgian,
        Self::Kazakh,
        Self::ChineseSimplified,
        Self::ChineseTraditional,
        Self::Japanese,
        Self::Korean,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::English => "en-US",
            Self::Russian => "ru-RU",
            Self::German => "de",
            Self::French => "fr",
            Self::Spanish => "es",
            Self::Italian => "it",
            Self::Portuguese => "pt",
            Self::Polish => "pl",
            Self::Turkish => "tr",
            Self::Ukrainian => "uk",
            Self::Czech => "cs",
            Self::Hungarian => "hu",
            Self::Romanian => "ro",
            Self::Greek => "el",
            Self::Croatian => "hr",
            Self::Georgian => "ka",
            Self::Kazakh => "kk",
            Self::ChineseSimplified => "zh-Hans",
            Self::ChineseTraditional => "zh-Hant",
            Self::Japanese => "ja",
            Self::Korean => "ko",
        }
    }
}

/// How `text:` actions reach the focused application on Linux.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextMode {
    /// libei; pastes through the clipboard when the layout lacks a character.
    #[default]
    Libei,
    /// libei only.
    LibeiPure,
    /// RemoteDesktop portal `NotifyKeyboardKeycode`.
    Keycode,
    /// MIME-preserving clipboard ownership and Shift+Insert through the portal.
    Clipboard,
    Ydotool,
    Xdotool,
}

impl TextMode {
    /// Every mode, in the order the settings page lists them.
    pub const ALL: [Self; 6] = [
        Self::Libei,
        Self::LibeiPure,
        Self::Keycode,
        Self::Clipboard,
        Self::Ydotool,
        Self::Xdotool,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Libei => "libei",
            Self::LibeiPure => "libei-pure",
            Self::Keycode => "keycode",
            Self::Clipboard => "clipboard",
            Self::Ydotool => "ydotool",
            Self::Xdotool => "xdotool",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == value)
    }
}

/// Shortcut that pastes clipboard text into the focused application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PasteShortcut {
    /// Common in GUI apps and terminals; some terminals paste the primary selection.
    #[default]
    ShiftInsert,
    CtrlV,
    /// Terminal paste.
    CtrlShiftV,
}

impl PasteShortcut {
    /// Every shortcut, in the order the settings page lists them.
    pub const ALL: [Self; 3] = [Self::ShiftInsert, Self::CtrlV, Self::CtrlShiftV];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ShiftInsert => "shift-insert",
            Self::CtrlV => "ctrl-v",
            Self::CtrlShiftV => "ctrl-shift-v",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|shortcut| shortcut.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Expose features that are still in development.
    pub in_development: bool,
    pub launch_on_startup: bool,
    /// Start the mapper when the app launches, whether by autostart or
    /// manually.
    pub start_mapper_on_launch: bool,
    pub appearance: Appearance,
    pub high_contrast: bool,
    pub reduce_motion: bool,
    /// Show an on-screen display when the active layout changes.
    pub layout_osd: bool,
    /// While a layer is held, show its keys in an overlay above all windows.
    pub layer_hint: bool,
    pub locale: LocalePreference,
    pub default_hold_timeout_ms: u64,
    pub default_long_hold_timeout_ms: u64,
    pub tap_decision: TapDecision,
    pub default_double_tap_timeout_ms: u64,
    /// Window in which all keys of a multi-key rule must be pressed.
    #[serde(rename = "comboTimeoutMs")]
    pub trigger_chord_timeout_ms: u64,
    pub default_macro_step_pause_ms: u64,
    pub default_macro_modifier_delay_ms: u64,
    /// A macro run still going after this time is aborted.
    pub macro_timeout_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mouse_device_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "currentLayoutId")]
    pub editing_layout_id: Option<String>,
    pub layout_mode: LayoutMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "manualActiveLayoutId")]
    pub manual_layout_id: Option<String>,
    pub layout_order: Vec<String>,
    pub auto_rules: Vec<AutoRule>,
    /// Layout used in automatic mode when no rule matches; `None` is off.
    pub auto_default_layout_id: Option<String>,
    pub commands_enabled: bool,
    pub game_mode: GameModeSettings,
    pub linux_wayland_text_mode: TextMode,
    /// Shortcut the clipboard text modes send to paste.
    pub linux_paste_shortcut: PasteShortcut,
    /// Paste shortcut for terminal windows; `None` uses `linux_paste_shortcut`.
    pub linux_terminal_paste_shortcut: Option<PasteShortcut>,
    pub linux_ydotool_path: String,
    pub linux_xdotool_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema: None,
            in_development: false,
            launch_on_startup: false,
            start_mapper_on_launch: false,
            appearance: Appearance::System,
            high_contrast: false,
            reduce_motion: false,
            layout_osd: false,
            layer_hint: false,
            locale: LocalePreference::Auto,
            default_long_hold_timeout_ms: timing::DEFAULT_LONG_HOLD_MS,
            default_hold_timeout_ms: timing::DEFAULT_HOLD_MS,
            tap_decision: TapDecision::default(),
            default_double_tap_timeout_ms: timing::DEFAULT_DOUBLE_TAP_MS,
            trigger_chord_timeout_ms: timing::DEFAULT_TRIGGER_CHORD_MS,
            default_macro_step_pause_ms: timing::DEFAULT_MACRO_STEP_PAUSE_MS,
            default_macro_modifier_delay_ms: timing::DEFAULT_MACRO_MODIFIER_DELAY_MS,
            macro_timeout_ms: timing::DEFAULT_MACRO_TIMEOUT_MS,
            input_device_path: Some(String::new()),
            input_mouse_device_path: None,
            editing_layout_id: None,
            layout_mode: LayoutMode::Manual,
            manual_layout_id: None,
            layout_order: Vec::new(),
            auto_rules: Vec::new(),
            auto_default_layout_id: None,
            commands_enabled: false,
            game_mode: GameModeSettings {
                use_gamemoded: cfg!(target_os = "linux"),
                use_fullscreen: false,
                use_steam: cfg!(any(target_os = "linux", target_os = "windows")),
                use_lutris: cfg!(target_os = "linux"),
                use_heroic: cfg!(target_os = "linux"),
                use_umu: cfg!(target_os = "linux"),
                use_gamescope: cfg!(target_os = "linux"),
                use_app_category: cfg!(target_os = "macos"),
                use_game_bar: false,
                foreground_only: true,
                process_matchers: Vec::new(),
                control: GameModeControl::Auto,
            },
            linux_wayland_text_mode: TextMode::Libei,
            linux_paste_shortcut: PasteShortcut::ShiftInsert,
            linux_terminal_paste_shortcut: Some(PasteShortcut::CtrlShiftV),
            linux_ydotool_path: String::new(),
            linux_xdotool_path: String::new(),
        }
    }
}

/// Settings plus the current layout, consumed by the mapper runtime as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedAppConfig {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout_description: Option<String>,
    pub layers: Vec<Layer>,
    pub rules: Vec<KeyRule>,
    pub layer_keymaps: BTreeMap<String, LayerKeymap>,
    pub macros: Vec<Macro>,
    pub commands: Vec<Command>,
    pub quick_action_pages: Vec<QuickActionPage>,
    pub emoji_pages: Vec<EmojiPage>,
    pub settings: AppSettings,
}

impl ResolvedAppConfig {
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
            quick_action_pages: preset.quick_action_pages,
            emoji_pages: preset.emoji_pages,
            settings: AppSettings {
                editing_layout_id: layout_id.map(str::to_owned),
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
    fn renamed_model_fields_preserve_runtime_json_contract() {
        let mut rule = KeyRule::new("r1".into(), "KeyA");
        rule.condition_keyboard_layouts = Some(vec!["us".into()]);
        rule.condition_window_patterns = Some(vec!["konsole".into()]);
        rule.condition_excluded_window_patterns = Some(vec!["title:private".into()]);
        let layout = LayoutPreset {
            rules: vec![rule],
            commands: vec![Command {
                id: "c1".into(),
                name: "Command".into(),
                posix_script: "printf test".into(),
                powershell_script: "Write-Output test".into(),
                working_directory: None,
            }],
            ..LayoutPreset::default()
        };
        let settings = AppSettings {
            manual_layout_id: Some("user:Manual".into()),
            trigger_chord_timeout_ms: 80,
            ..AppSettings::default()
        };
        let config = ResolvedAppConfig::from_parts(settings, layout, Some("user:Editing"));
        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["settings"]["currentLayoutId"], "user:Editing");
        assert_eq!(value["settings"]["manualActiveLayoutId"], "user:Manual");
        assert_eq!(value["settings"]["comboTimeoutMs"], 80);
        assert_eq!(value["rules"][0]["conditionLayouts"][0], "us");
        assert_eq!(value["rules"][0]["conditionAppsWhitelist"][0], "konsole");
        assert_eq!(
            value["rules"][0]["conditionAppsBlacklist"][0],
            "title:private"
        );
        assert_eq!(value["commands"][0]["linux"], "printf test");
        assert_eq!(value["commands"][0]["windows"], "Write-Output test");

        let mapper: crate::mapper::config::MapperConfig = serde_json::from_value(value).unwrap();
        assert_eq!(
            mapper.settings.editing_layout_id.as_deref(),
            Some("user:Editing")
        );
        assert_eq!(mapper.settings.trigger_chord_timeout_ms, 80);
        assert_eq!(
            mapper.rules[0].condition_window_patterns,
            config.rules[0].condition_window_patterns
        );
        assert_eq!(mapper.commands[0].posix_script, "printf test");
        assert_eq!(mapper.commands[0].powershell_script, "Write-Output test");
    }

    #[test]
    fn used_commands_skips_orphans() {
        let command = |id: &str| Command {
            id: id.into(),
            name: String::new(),
            posix_script: "ls".into(),
            powershell_script: "ls".into(),
            working_directory: None,
        };
        let mut rule = KeyRule::new("r1".into(), "KeyA");
        rule.tap_action = Some("cmd:used".into());
        let preset = LayoutPreset {
            rules: vec![rule],
            commands: vec![command("used"), command("orphan")],
            ..LayoutPreset::default()
        };
        assert_eq!(preset.used_command_count(), 1);
        let mut preset = preset;
        preset.remove_unused_commands();
        assert_eq!(preset.commands, vec![command("used")]);
    }
}
