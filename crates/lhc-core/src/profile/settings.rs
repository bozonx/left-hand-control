//! `config.json`: `{ "schemaVersion": 1, "settings": { … }, "state": { … } }`.
//!
//! `settings` holds the user's choices, `state` what the application
//! remembers between runs (the layout open in the editor, library order,
//! the game-mode switch). Both sections are optional and default field by
//! field; unknown fields, wrong types and unknown values are errors, the
//! same as in layout files (see [`super::layout_file`]).

use super::layout_file::{OnOff, SCHEMA_VERSION, check_version};
use super::model::{
    AppSettings, Appearance, AutoRule, GameModeControl, GameModeProcessMatcher, GameModeSettings,
    LayoutConditionSet, LayoutMode, LocalePreference, PasteShortcut, TapDecision, TextMode,
};
use serde::{Deserialize, Serialize};

/// Parse `config.json`; an empty file gives the defaults.
pub fn parse(text: &str) -> Result<AppSettings, String> {
    if text.trim().is_empty() {
        return Ok(AppSettings::default());
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("config.json: {error}"))?;
    check_version(&value, "config.json")?;
    let doc: ConfigDoc =
        serde_json::from_str(text).map_err(|error| format!("config.json: {error}"))?;
    Ok(from_doc(doc))
}

pub fn serialize(settings: &AppSettings) -> String {
    serde_json::to_string_pretty(&to_doc(settings)).unwrap_or_default()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigDoc {
    schema_version: u64,
    #[serde(default)]
    settings: SettingsDoc,
    #[serde(default)]
    state: StateDoc,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct SettingsDoc {
    launch_on_startup: bool,
    /// Start the mapper when the app launches.
    start_mapper_on_launch: bool,
    appearance: Appearance,
    high_contrast: bool,
    reduce_motion: bool,
    layout_osd: bool,
    layer_hint: bool,
    locale: LocalePreference,
    /// Device the mapper grabs; a stable path such as
    /// `/dev/input/by-id/…` on Linux.
    #[serde(skip_serializing_if = "Option::is_none")]
    keyboard_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_device: Option<String>,
    timing: TimingDoc,
    layout_mode: LayoutMode,
    /// Layout used in manual mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    active_layout_id: Option<String>,
    auto_rules: Vec<AutoRuleDoc>,
    /// Layout used in automatic mode when no rule matches; `null` is off.
    auto_default_layout_id: Option<String>,
    commands_enabled: bool,
    game_mode: GameModeDoc,
    linux: LinuxDoc,
}

impl Default for SettingsDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings
    }
}

/// Defaults for rules and macros; a rule or macro may override each.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct TimingDoc {
    hold_ms: u64,
    long_hold_ms: u64,
    double_tap_ms: u64,
    /// Window in which all keys of a multi-key rule must be pressed.
    combo_ms: u64,
    tap_decision: TapDecision,
    macro_step_pause_ms: u64,
    macro_modifier_delay_ms: u64,
    /// A macro run still going after this time is aborted.
    macro_timeout_ms: u64,
}

impl Default for TimingDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.timing
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AutoRuleDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    /// `null` turns the mapper off.
    #[serde(default)]
    layout_id: Option<String>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    enabled: bool,
    /// Empty conditions always match.
    #[serde(default, skip_serializing_if = "AutoWhenDoc::is_empty")]
    when: AutoWhenDoc,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AutoWhenDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    game_mode: Option<OnOff>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keyboard_layouts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    windows: Vec<String>,
}

impl AutoWhenDoc {
    fn is_empty(&self) -> bool {
        self.game_mode.is_none() && self.keyboard_layouts.is_empty() && self.windows.is_empty()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct GameModeDoc {
    detectors: DetectorsDoc,
    processes: Vec<ProcessDoc>,
}

impl Default for GameModeDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.game_mode
    }
}

/// Game detection sources; each platform uses only the ones it supports.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct DetectorsDoc {
    gamemoded: bool,
    fullscreen: bool,
    steam: bool,
    lutris: bool,
    heroic: bool,
    umu: bool,
    gamescope: bool,
    app_category: bool,
    game_bar: bool,
}

impl Default for DetectorsDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.game_mode.detectors
    }
}

/// A process that marks game mode, or with `exclude` one that never does.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProcessDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    #[serde(default = "yes")]
    only_active_window: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    exclude: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct LinuxDoc {
    /// How `text:` actions reach the focused application.
    text_mode: TextMode,
    /// Shortcut the clipboard text modes send to paste.
    paste_shortcut: PasteShortcut,
    #[serde(skip_serializing_if = "String::is_empty")]
    ydotool_path: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    xdotool_path: String,
}

impl Default for LinuxDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.linux
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct StateDoc {
    /// Library layout open in the editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    edited_layout_id: Option<String>,
    /// Library order, also the priority of `activateLayoutN`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    layout_order: Vec<String>,
    /// Auto / On / Off choice of the game-mode switch.
    game_mode: GameModeControl,
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn from_doc(doc: ConfigDoc) -> AppSettings {
    let ConfigDoc {
        settings, state, ..
    } = doc;
    let timing = settings.timing;
    let detectors = settings.game_mode.detectors;
    AppSettings {
        launch_on_startup: settings.launch_on_startup,
        start_mapper_on_launch: settings.start_mapper_on_launch,
        appearance: settings.appearance,
        high_contrast: settings.high_contrast,
        reduce_motion: settings.reduce_motion,
        layout_osd: settings.layout_osd,
        layer_hint: settings.layer_hint,
        locale: settings.locale,
        default_hold_timeout_ms: timing.hold_ms,
        default_long_hold_timeout_ms: timing.long_hold_ms,
        tap_decision: tap_decision_name(timing.tap_decision).into(),
        default_double_tap_timeout_ms: timing.double_tap_ms,
        combo_timeout_ms: timing.combo_ms,
        default_macro_step_pause_ms: timing.macro_step_pause_ms,
        default_macro_modifier_delay_ms: timing.macro_modifier_delay_ms,
        macro_timeout_ms: timing.macro_timeout_ms,
        input_device_path: Some(settings.keyboard_device.unwrap_or_default()),
        input_mouse_device_path: non_empty(settings.mouse_device),
        current_layout_id: non_empty(state.edited_layout_id),
        layout_mode: settings.layout_mode,
        manual_active_layout_id: non_empty(settings.active_layout_id),
        layout_order: state.layout_order,
        auto_rules: settings
            .auto_rules
            .into_iter()
            .enumerate()
            .map(|(index, rule)| AutoRule {
                id: non_empty(rule.id).unwrap_or_else(|| format!("rule-{index}")),
                layout_id: non_empty(rule.layout_id),
                enabled: rule.enabled,
                conditions: LayoutConditionSet {
                    game_mode: rule.when.game_mode.map(|mode| mode.as_str().to_owned()),
                    layouts: rule.when.keyboard_layouts,
                    apps: rule.when.windows,
                },
            })
            .collect(),
        auto_default_layout_id: non_empty(settings.auto_default_layout_id),
        commands_enabled: settings.commands_enabled,
        game_mode: GameModeSettings {
            use_gamemoded: detectors.gamemoded,
            use_fullscreen: detectors.fullscreen,
            use_steam: detectors.steam,
            use_lutris: detectors.lutris,
            use_heroic: detectors.heroic,
            use_umu: detectors.umu,
            use_gamescope: detectors.gamescope,
            use_app_category: detectors.app_category,
            use_game_bar: detectors.game_bar,
            process_matchers: settings
                .game_mode
                .processes
                .into_iter()
                .enumerate()
                .map(|(index, process)| GameModeProcessMatcher {
                    id: non_empty(process.id).unwrap_or_else(|| format!("process-{index}")),
                    name: process.name.trim().into(),
                    only_active_window: process.only_active_window,
                    is_blacklist: process.exclude,
                })
                .collect(),
            control: state.game_mode,
        },
        linux_wayland_text_mode: settings.linux.text_mode,
        linux_paste_shortcut: settings.linux.paste_shortcut,
        linux_ydotool_path: settings.linux.ydotool_path,
        linux_xdotool_path: settings.linux.xdotool_path,
    }
}

fn tap_decision_name(decision: TapDecision) -> &'static str {
    match decision {
        TapDecision::PermissiveHold => "permissiveHold",
        TapDecision::HoldOnOtherKeyPress => "holdOnOtherKeyPress",
    }
}

fn to_doc(settings: &AppSettings) -> ConfigDoc {
    let game_mode = &settings.game_mode;
    let filled = |value: &Option<String>| value.clone().filter(|value| !value.is_empty());
    ConfigDoc {
        schema_version: SCHEMA_VERSION,
        settings: SettingsDoc {
            launch_on_startup: settings.launch_on_startup,
            start_mapper_on_launch: settings.start_mapper_on_launch,
            appearance: settings.appearance,
            high_contrast: settings.high_contrast,
            reduce_motion: settings.reduce_motion,
            layout_osd: settings.layout_osd,
            layer_hint: settings.layer_hint,
            locale: settings.locale,
            keyboard_device: filled(&settings.input_device_path),
            mouse_device: filled(&settings.input_mouse_device_path),
            timing: TimingDoc {
                hold_ms: settings.default_hold_timeout_ms,
                long_hold_ms: settings.default_long_hold_timeout_ms,
                double_tap_ms: settings.default_double_tap_timeout_ms,
                combo_ms: settings.combo_timeout_ms,
                tap_decision: match settings.tap_decision.as_str() {
                    "holdOnOtherKeyPress" => TapDecision::HoldOnOtherKeyPress,
                    _ => TapDecision::PermissiveHold,
                },
                macro_step_pause_ms: settings.default_macro_step_pause_ms,
                macro_modifier_delay_ms: settings.default_macro_modifier_delay_ms,
                macro_timeout_ms: settings.macro_timeout_ms,
            },
            layout_mode: settings.layout_mode,
            active_layout_id: filled(&settings.manual_active_layout_id),
            auto_rules: settings
                .auto_rules
                .iter()
                .map(|rule| AutoRuleDoc {
                    id: Some(rule.id.clone()).filter(|id| !id.is_empty()),
                    layout_id: filled(&rule.layout_id),
                    enabled: rule.enabled,
                    when: AutoWhenDoc {
                        game_mode: OnOff::parse(rule.conditions.game_mode.as_deref()),
                        keyboard_layouts: rule.conditions.layouts.clone(),
                        windows: rule.conditions.apps.clone(),
                    },
                })
                .collect(),
            auto_default_layout_id: filled(&settings.auto_default_layout_id),
            commands_enabled: settings.commands_enabled,
            game_mode: GameModeDoc {
                detectors: DetectorsDoc {
                    gamemoded: game_mode.use_gamemoded,
                    fullscreen: game_mode.use_fullscreen,
                    steam: game_mode.use_steam,
                    lutris: game_mode.use_lutris,
                    heroic: game_mode.use_heroic,
                    umu: game_mode.use_umu,
                    gamescope: game_mode.use_gamescope,
                    app_category: game_mode.use_app_category,
                    game_bar: game_mode.use_game_bar,
                },
                processes: game_mode
                    .process_matchers
                    .iter()
                    .map(|process| ProcessDoc {
                        id: Some(process.id.clone()).filter(|id| !id.is_empty()),
                        name: process.name.clone(),
                        only_active_window: process.only_active_window,
                        exclude: process.is_blacklist,
                    })
                    .collect(),
            },
            linux: LinuxDoc {
                text_mode: settings.linux_wayland_text_mode,
                paste_shortcut: settings.linux_paste_shortcut,
                ydotool_path: settings.linux_ydotool_path.clone(),
                xdotool_path: settings.linux_xdotool_path.clone(),
            },
        },
        state: StateDoc {
            edited_layout_id: filled(&settings.current_layout_id),
            layout_order: settings.layout_order.clone(),
            game_mode: game_mode.control,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn config(settings: Value) -> String {
        json!({"schemaVersion": 1, "settings": settings}).to_string()
    }

    #[test]
    fn missing_settings_use_defaults() {
        assert_eq!(parse("").unwrap(), AppSettings::default());
        assert_eq!(
            parse(r#"{"schemaVersion": 1}"#).unwrap(),
            AppSettings::default()
        );
        assert_eq!(parse(&config(json!({}))).unwrap(), AppSettings::default());
    }

    #[test]
    fn version_is_required_and_checked() {
        assert!(
            parse(r#"{"settings": {}}"#)
                .unwrap_err()
                .contains("schemaVersion")
        );
        assert!(
            parse(r#"{"schemaVersion": 2}"#)
                .unwrap_err()
                .contains("newer")
        );
        assert!(parse(r#"[]"#).is_err());
    }

    #[test]
    fn invalid_or_unknown_values_are_errors() {
        for settings in [
            json!({"appearance": "Dark"}),
            json!({"locale": "de-DE"}),
            json!({"futureOption": 7}),
            json!({"timing": {"holdMs": "fast"}}),
            json!({"timing": {"tapDecision": "sometimes"}}),
            json!({"linux": {"textMode": "magic"}}),
            json!({"gameMode": {"detectors": {"origin": true}}}),
            json!({"gameMode": {"processes": [{"name": "steam", "isBlacklist": true}]}}),
            json!({"autoRules": [{"layoutId": "user:A", "apps": ["kate"]}]}),
        ] {
            assert!(
                parse(&config(settings.clone())).is_err(),
                "accepted {settings}"
            );
        }
        assert!(parse(r#"{"schemaVersion": 1, "extra": true}"#).is_err());
        assert!(parse(r#"{"schemaVersion": 1, "state": {"gameMode": "maybe"}}"#).is_err());
    }

    #[test]
    fn every_setting_round_trips() {
        let settings = AppSettings {
            launch_on_startup: true,
            start_mapper_on_launch: true,
            appearance: Appearance::EInk,
            locale: LocalePreference::Russian,
            default_hold_timeout_ms: 180,
            tap_decision: "holdOnOtherKeyPress".into(),
            input_device_path: Some("/dev/input/by-id/kbd".into()),
            input_mouse_device_path: Some("/dev/input/by-id/mouse".into()),
            current_layout_id: Some("user:Main".into()),
            layout_mode: LayoutMode::Auto,
            manual_active_layout_id: Some("user:Other".into()),
            layout_order: vec!["user:Other".into(), "user:Main".into()],
            auto_rules: vec![AutoRule {
                id: "a".into(),
                layout_id: None,
                enabled: false,
                conditions: LayoutConditionSet {
                    game_mode: Some("on".into()),
                    layouts: vec!["us".into()],
                    apps: vec!["blender".into()],
                },
            }],
            auto_default_layout_id: Some("user:Main".into()),
            commands_enabled: true,
            game_mode: GameModeSettings {
                use_fullscreen: true,
                process_matchers: vec![GameModeProcessMatcher {
                    id: "p".into(),
                    name: "steam".into(),
                    only_active_window: false,
                    is_blacklist: true,
                }],
                control: GameModeControl::On,
                ..AppSettings::default().game_mode
            },
            linux_wayland_text_mode: TextMode::Clipboard,
            linux_paste_shortcut: PasteShortcut::CtrlShiftV,
            linux_ydotool_path: "/usr/bin/ydotool".into(),
            ..AppSettings::default()
        };
        let text = serialize(&settings);
        assert_eq!(parse(&text).unwrap(), settings);
        let raw: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(raw["schemaVersion"], 1);
        assert_eq!(raw["settings"]["timing"]["holdMs"], 180);
        assert_eq!(raw["settings"]["linux"]["pasteShortcut"], "ctrl-shift-v");
        assert_eq!(raw["settings"]["gameMode"]["processes"][0]["exclude"], true);
        assert_eq!(raw["state"]["editedLayoutId"], "user:Main");
        assert_eq!(raw["state"]["gameMode"], "on");
        assert_eq!(raw["settings"]["activeLayoutId"], "user:Other");
    }

    #[test]
    fn partial_sections_keep_other_defaults() {
        let settings = parse(&config(json!({
            "timing": {"holdMs": 300},
            "gameMode": {"detectors": {"lutris": false}, "processes": [{"name": " steam "}]},
        })))
        .unwrap();
        let base = AppSettings::default();
        assert_eq!(settings.default_hold_timeout_ms, 300);
        assert_eq!(
            settings.default_long_hold_timeout_ms,
            base.default_long_hold_timeout_ms
        );
        assert!(!settings.game_mode.use_lutris);
        assert_eq!(settings.game_mode.use_steam, base.game_mode.use_steam);
        let process = &settings.game_mode.process_matchers[0];
        assert_eq!(process.name, "steam");
        assert_eq!(process.id, "process-0");
        assert!(process.only_active_window);
        assert!(!process.is_blacklist);
    }

    #[test]
    fn mapper_start_on_launch_is_off_by_default() {
        assert!(!AppSettings::default().start_mapper_on_launch);
        let settings = parse(&config(json!({"startMapperOnLaunch": true}))).unwrap();
        assert!(settings.start_mapper_on_launch);
    }

    #[test]
    fn manual_activation_is_independent_of_the_edited_layout() {
        let settings =
            parse(r#"{"schemaVersion": 1, "state": {"editedLayoutId": "user:Main"}}"#).unwrap();
        assert_eq!(settings.current_layout_id.as_deref(), Some("user:Main"));
        assert_eq!(settings.manual_active_layout_id, None);
    }

    #[test]
    fn auto_rules_without_ids_get_positional_ids() {
        let settings = parse(&config(json!({
            "autoRules": [
                {"id": "a", "layoutId": "user:A", "when": {"windows": ["blender"]}},
                {"layoutId": null, "enabled": false},
            ],
            "autoDefaultLayoutId": "user:B",
        })))
        .unwrap();
        assert_eq!(settings.auto_rules.len(), 2);
        assert_eq!(settings.auto_rules[0].conditions.apps, ["blender"]);
        assert_eq!(settings.auto_rules[1].id, "rule-1");
        assert_eq!(settings.auto_rules[1].layout_id, None);
        assert!(!settings.auto_rules[1].enabled);
        assert!(settings.auto_rules[1].conditions.is_empty());
        assert_eq!(settings.auto_default_layout_id.as_deref(), Some("user:B"));
    }
}
