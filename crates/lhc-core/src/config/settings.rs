//! `config.json`: `{ "version": 1, "settings": { … }, "state": { … } }`.
//!
//! `settings` holds the user's choices, `state` what the application
//! remembers between runs (the profile open in the editor, the game-mode
//! switch). Both sections are optional and default field by field; unknown
//! fields, wrong types and unknown values are errors, the same as in profile
//! files (see [`super::profile_file`]). Only values that differ from the
//! defaults are written, so a later release can change a default for
//! everyone who never chose otherwise.

use super::model::{
    AppSettings, Appearance, AutoRule, GameModeControl, GameModeProcessMatcher, GameModeSettings,
    LocalePreference, PasteShortcut, ProfileMode, TapDecision, TextMode,
};
use super::profile_file::WhenDoc;
use super::schema::{self, CONFIG_SCHEMA_VERSION};
use crate::storage::CONFIG_FILE;
use serde::{Deserialize, Serialize};

/// Parse `config.json`; an empty file gives the defaults.
pub fn parse(text: &str) -> Result<AppSettings, String> {
    if text.trim().is_empty() {
        return Ok(AppSettings::default());
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("{CONFIG_FILE}: {error}"))?;
    let settings = match schema::read_version(&value, CONFIG_FILE, CONFIG_SCHEMA_VERSION)? {
        1 => from_doc(
            serde_json::from_str::<ConfigDoc>(text)
                .map_err(|error| schema::read_error(CONFIG_FILE, &error))?,
        )?,
        version => {
            return Err(format!(
                "{CONFIG_FILE}: no reader for schema version {version}"
            ));
        }
    };
    validate(&settings)?;
    Ok(settings)
}

/// JSON Schema of `config.json`, published under `schemas/`.
#[cfg(test)]
pub(crate) fn json_schema() -> schemars::Schema {
    schemars::schema_for!(ConfigDoc)
}

pub fn serialize(settings: &AppSettings) -> String {
    let defaults = serde_json::to_value(to_doc(&AppSettings::default())).unwrap_or_default();
    let mut value = serde_json::to_value(to_doc(settings)).unwrap_or_default();
    if let Some(object) = value.as_object_mut() {
        for section in ["settings", "state"] {
            if let (Some(current), Some(default)) = (object.get_mut(section), defaults.get(section))
            {
                prune_defaults(current, default);
            }
            if object
                .get(section)
                .and_then(serde_json::Value::as_object)
                .is_some_and(serde_json::Map::is_empty)
            {
                object.remove(section);
            }
        }
    }
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// Drop the fields of `value` that equal those of `default`, descending
/// into nested objects; lists are compared whole. Every section reads a
/// missing field as its default, so the result parses to the same settings.
fn prune_defaults(value: &mut serde_json::Value, default: &serde_json::Value) {
    let (Some(object), Some(default)) = (value.as_object_mut(), default.as_object()) else {
        return;
    };
    object.retain(|key, field| {
        let Some(default) = default.get(key) else {
            return true;
        };
        if field == default {
            return false;
        }
        prune_defaults(field, default);
        !field.as_object().is_some_and(serde_json::Map::is_empty)
    });
}

#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigDoc {
    #[cfg_attr(test, schemars(extend("const" = CONFIG_SCHEMA_VERSION)))]
    version: u64,
    #[serde(
        rename = "$schema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    #[cfg_attr(test, schemars(with = "String"))]
    schema: Option<String>,
    #[serde(default)]
    settings: SettingsDoc,
    #[serde(default)]
    state: StateDoc,
}

#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct SettingsDoc {
    /// Shows interface features that are still in development.
    experimental: bool,
    launch_on_startup: bool,
    /// Start the mapper when the app launches.
    start_mapper_on_launch: bool,
    appearance: Appearance,
    high_contrast: bool,
    reduce_motion: bool,
    profile_osd: bool,
    layer_hint: bool,
    locale: LocalePreference,
    #[serde(skip_serializing_if = "DevicesDoc::is_empty")]
    devices: DevicesDoc,
    timing: TimingDoc,
    profiles: ProfilesDoc,
    commands_enabled: bool,
    game_mode: GameModeDoc,
    linux: LinuxDoc,
}

impl Default for SettingsDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings
    }
}

/// Devices the mapper grabs, as stable paths such as `/dev/input/by-id/…`
/// on Linux. Lists leave room for several devices; this version grabs at
/// most one of each kind.
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct DevicesDoc {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keyboards: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mice: Vec<String>,
}

impl DevicesDoc {
    fn is_empty(&self) -> bool {
        self.keyboards.is_empty() && self.mice.is_empty()
    }

    /// The single device of `list`, checked.
    fn single(list: Vec<String>, what: &str) -> Result<Option<String>, String> {
        if list.len() > 1 {
            return Err(format!(
                "settings.devices.{what}: this version supports at most one device"
            ));
        }
        match list.into_iter().next() {
            Some(path) if path.trim().is_empty() => Err(format!(
                "settings.devices.{what}: device path must not be empty"
            )),
            path => Ok(path),
        }
    }
}

/// Which library profile the mapper uses.
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct ProfilesDoc {
    mode: ProfileMode,
    /// Profile used in manual mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    manual_profile: Option<String>,
    /// Library order: the slots of `activateProfileN` and the order of the
    /// list. Behaviour, not remembered state, so it lives in settings.
    order: Vec<String>,
    auto: AutoDoc,
}

impl Default for ProfilesDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.profiles
    }
}

/// Automatic mode: the first matching rule wins, else `fallback`.
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct AutoDoc {
    rules: Vec<AutoRuleDoc>,
    /// Profile used when no rule matches; `null` turns the mapper off.
    fallback: Option<String>,
}

impl Default for AutoDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.profiles.auto
    }
}

/// Defaults for rules and macros; a rule or macro may override each.
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct TimingDoc {
    #[cfg_attr(test, schemars(range(min = super::timing::TAP_HOLD.min, max = super::timing::TAP_HOLD.max)))]
    hold_ms: u64,
    #[cfg_attr(test, schemars(range(min = super::timing::LONG_HOLD.min, max = super::timing::LONG_HOLD.max)))]
    long_hold_ms: u64,
    #[cfg_attr(test, schemars(range(min = super::timing::TAP_HOLD.min, max = super::timing::TAP_HOLD.max)))]
    double_tap_ms: u64,
    /// Window in which all keys of a multi-key rule must be pressed.
    #[cfg_attr(test, schemars(range(min = super::timing::TRIGGER_CHORD.min, max = super::timing::TRIGGER_CHORD.max)))]
    combo_ms: u64,
    tap_decision: TapDecision,
    #[cfg_attr(test, schemars(range(min = super::timing::MACRO_DELAY.min, max = super::timing::MACRO_DELAY.max)))]
    macro_step_pause_ms: u64,
    #[cfg_attr(test, schemars(range(min = super::timing::MACRO_DELAY.min, max = super::timing::MACRO_DELAY.max)))]
    macro_modifier_delay_ms: u64,
    /// A macro run still going after this time is aborted.
    #[cfg_attr(test, schemars(range(min = super::timing::MACRO_TIMEOUT.min, max = super::timing::MACRO_TIMEOUT.max)))]
    macro_timeout_ms: u64,
}

impl Default for TimingDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.timing
    }
}

#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AutoRuleDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, schemars(regex(pattern = super::schema::ID_PATTERN)))]
    id: Option<String>,
    /// Required; an explicit `null` turns the mapper off, so a forgotten
    /// field is an error rather than a silent "off".
    #[serde(deserialize_with = "nullable")]
    profile: Option<String>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    enabled: bool,
    /// Empty conditions always match.
    #[serde(default, skip_serializing_if = "WhenDoc::is_empty")]
    when: WhenDoc,
}

/// `linux.terminalPasteShortcut`: a paste chord, or `"sameAsPaste"` (`None`)
/// to use `linux.pasteShortcut`.
#[derive(PartialEq)]
struct TerminalPasteDoc(Option<PasteShortcut>);

const SAME_AS_PASTE: &str = "sameAsPaste";

impl Serialize for TerminalPasteDoc {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(shortcut) => shortcut.serialize(serializer),
            None => serializer.serialize_str(SAME_AS_PASTE),
        }
    }
}

impl<'de> Deserialize<'de> for TerminalPasteDoc {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value == SAME_AS_PASTE {
            return Ok(Self(None));
        }
        PasteShortcut::deserialize(serde::de::value::StrDeserializer::<D::Error>::new(&value))
            .map(|shortcut| Self(Some(shortcut)))
    }
}

#[cfg(test)]
impl schemars::JsonSchema for TerminalPasteDoc {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TerminalPasteShortcut".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut values: Vec<&str> = PasteShortcut::ALL.map(PasteShortcut::as_str).into();
        values.push(SAME_AS_PASTE);
        schemars::json_schema!({ "type": "string", "enum": values })
    }
}

/// A required field that may be `null`.
fn nullable<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct GameModeDoc {
    detectors: DetectorsDoc,
    /// Games a launcher or the system reports count only while their
    /// window is active.
    foreground_only: bool,
    processes: Vec<ProcessDoc>,
}

impl Default for GameModeDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.game_mode
    }
}

/// Game detection sources; each platform uses only the ones it supports.
#[cfg_attr(test, derive(schemars::JsonSchema))]
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
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProcessDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, schemars(regex(pattern = super::schema::ID_PATTERN)))]
    id: Option<String>,
    name: String,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    foreground_only: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    exclude: bool,
}

#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct LinuxDoc {
    /// How `text:` actions reach the focused application.
    text_mode: TextMode,
    /// Shortcut the clipboard text modes send to paste, as a key chord.
    paste_shortcut: PasteShortcut,
    /// Paste shortcut for terminal windows, or `sameAsPaste`.
    terminal_paste_shortcut: TerminalPasteDoc,
    /// Missing: looked up in `PATH`. An empty string is an error, so
    /// "no value" has one spelling.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "program_path"
    )]
    #[cfg_attr(test, schemars(with = "String", length(min = 1)))]
    ydotool_path: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "program_path"
    )]
    #[cfg_attr(test, schemars(with = "String", length(min = 1)))]
    xdotool_path: Option<String>,
}

/// A program path: absent or a non-blank string; `null` and `""` are errors.
fn program_path<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let path = String::deserialize(deserializer)?;
    if path.trim().is_empty() {
        return Err(serde::de::Error::custom(
            "program path must not be empty; omit the field to search PATH",
        ));
    }
    Ok(Some(path))
}

impl Default for LinuxDoc {
    fn default() -> Self {
        to_doc(&AppSettings::default()).settings.linux
    }
}

#[cfg_attr(test, derive(schemars::JsonSchema))]
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct StateDoc {
    /// Library profile open in the editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    edited_profile: Option<String>,
    /// Auto / On / Off choice of the game-mode switch.
    game_mode_switch: GameModeControl,
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

/// A profile reference: a profile id or `null`, never an empty string.
fn profile_ref(value: Option<String>, what: &str) -> Result<Option<String>, String> {
    match value {
        Some(id) if id.trim().is_empty() => Err(format!("{what}: profile id must not be empty")),
        value => Ok(value),
    }
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    super::ids::unique(
        settings.auto_rules.iter().map(|item| item.id.as_str()),
        "settings.profiles.auto.rules",
    )?;
    super::ids::unique(
        settings
            .game_mode
            .process_matchers
            .iter()
            .map(|item| item.id.as_str()),
        "settings.gameMode.processes",
    )?;
    super::timing::validate_settings(settings)
}

fn from_doc(mut doc: ConfigDoc) -> Result<AppSettings, String> {
    let mut ids: Vec<_> = doc
        .settings
        .profiles
        .auto
        .rules
        .iter_mut()
        .map(|item| item.id.take())
        .collect();
    super::ids::complete(&mut ids, "rule-", "settings.profiles.auto.rules")?;
    for (item, id) in doc.settings.profiles.auto.rules.iter_mut().zip(ids) {
        item.id = id;
    }
    let mut ids: Vec<_> = doc
        .settings
        .game_mode
        .processes
        .iter_mut()
        .map(|item| item.id.take())
        .collect();
    super::ids::complete(&mut ids, "process-", "settings.gameMode.processes")?;
    for (item, id) in doc.settings.game_mode.processes.iter_mut().zip(ids) {
        item.id = id;
    }
    let ConfigDoc {
        settings, state, ..
    } = doc;
    let timing = settings.timing;
    let detectors = settings.game_mode.detectors;
    let keyboard_device = DevicesDoc::single(settings.devices.keyboards, "keyboards")?;
    let mouse_device = DevicesDoc::single(settings.devices.mice, "mice")?;
    let profiles = settings.profiles;
    let mut auto_rules = Vec::new();
    for (index, rule) in profiles.auto.rules.into_iter().enumerate() {
        auto_rules.push(AutoRule {
            id: rule.id.unwrap_or_default(),
            profile_id: profile_ref(
                rule.profile,
                &format!("settings.profiles.auto.rules[{index}]"),
            )?,
            enabled: rule.enabled,
            conditions: rule.when.into_set()?.normalized(),
        });
    }
    Ok(AppSettings {
        schema: doc.schema,
        in_development: settings.experimental,
        launch_on_startup: settings.launch_on_startup,
        start_mapper_on_launch: settings.start_mapper_on_launch,
        appearance: settings.appearance,
        high_contrast: settings.high_contrast,
        reduce_motion: settings.reduce_motion,
        profile_osd: settings.profile_osd,
        layer_hint: settings.layer_hint,
        locale: settings.locale,
        default_hold_timeout_ms: timing.hold_ms,
        default_long_hold_timeout_ms: timing.long_hold_ms,
        tap_decision: timing.tap_decision,
        default_double_tap_timeout_ms: timing.double_tap_ms,
        trigger_chord_timeout_ms: timing.combo_ms,
        default_macro_step_pause_ms: timing.macro_step_pause_ms,
        default_macro_modifier_delay_ms: timing.macro_modifier_delay_ms,
        macro_timeout_ms: timing.macro_timeout_ms,
        input_device_path: Some(keyboard_device.unwrap_or_default()),
        input_mouse_device_path: mouse_device,
        editing_profile_id: non_empty(state.edited_profile),
        profile_mode: profiles.mode,
        manual_profile_id: non_empty(profiles.manual_profile),
        profile_order: profiles.order,
        auto_rules,
        auto_default_profile_id: profile_ref(
            profiles.auto.fallback,
            "settings.profiles.auto.fallback",
        )?,
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
            foreground_only: settings.game_mode.foreground_only,
            process_matchers: settings
                .game_mode
                .processes
                .into_iter()
                .map(|process| GameModeProcessMatcher {
                    id: process.id.unwrap_or_default(),
                    name: process.name.trim().into(),
                    only_active_window: process.foreground_only,
                    is_blacklist: process.exclude,
                })
                .collect(),
            control: state.game_mode_switch,
        },
        linux_wayland_text_mode: settings.linux.text_mode,
        linux_paste_shortcut: settings.linux.paste_shortcut,
        linux_terminal_paste_shortcut: settings.linux.terminal_paste_shortcut.0,
        linux_ydotool_path: settings.linux.ydotool_path.unwrap_or_default(),
        linux_xdotool_path: settings.linux.xdotool_path.unwrap_or_default(),
    })
}

fn to_doc(settings: &AppSettings) -> ConfigDoc {
    let game_mode = &settings.game_mode;
    let filled = |value: &Option<String>| value.clone().filter(|value| !value.is_empty());
    ConfigDoc {
        version: CONFIG_SCHEMA_VERSION,
        schema: settings.schema.clone(),
        settings: SettingsDoc {
            experimental: settings.in_development,
            launch_on_startup: settings.launch_on_startup,
            start_mapper_on_launch: settings.start_mapper_on_launch,
            appearance: settings.appearance,
            high_contrast: settings.high_contrast,
            reduce_motion: settings.reduce_motion,
            profile_osd: settings.profile_osd,
            layer_hint: settings.layer_hint,
            locale: settings.locale,
            devices: DevicesDoc {
                keyboards: filled(&settings.input_device_path).into_iter().collect(),
                mice: filled(&settings.input_mouse_device_path)
                    .into_iter()
                    .collect(),
            },
            timing: TimingDoc {
                hold_ms: settings.default_hold_timeout_ms,
                long_hold_ms: settings.default_long_hold_timeout_ms,
                double_tap_ms: settings.default_double_tap_timeout_ms,
                combo_ms: settings.trigger_chord_timeout_ms,
                tap_decision: settings.tap_decision,
                macro_step_pause_ms: settings.default_macro_step_pause_ms,
                macro_modifier_delay_ms: settings.default_macro_modifier_delay_ms,
                macro_timeout_ms: settings.macro_timeout_ms,
            },
            profiles: ProfilesDoc {
                mode: settings.profile_mode,
                manual_profile: filled(&settings.manual_profile_id),
                order: settings.profile_order.clone(),
                auto: AutoDoc {
                    rules: settings
                        .auto_rules
                        .iter()
                        .map(|rule| AutoRuleDoc {
                            id: Some(rule.id.clone()).filter(|id| !id.is_empty()),
                            profile: filled(&rule.profile_id),
                            enabled: rule.enabled,
                            when: WhenDoc::from_set(&rule.conditions),
                        })
                        .collect(),
                    fallback: filled(&settings.auto_default_profile_id),
                },
            },
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
                foreground_only: game_mode.foreground_only,
                processes: game_mode
                    .process_matchers
                    .iter()
                    .map(|process| ProcessDoc {
                        id: Some(process.id.clone()).filter(|id| !id.is_empty()),
                        name: process.name.clone(),
                        foreground_only: process.only_active_window,
                        exclude: process.is_blacklist,
                    })
                    .collect(),
            },
            linux: LinuxDoc {
                text_mode: settings.linux_wayland_text_mode,
                paste_shortcut: settings.linux_paste_shortcut,
                terminal_paste_shortcut: TerminalPasteDoc(settings.linux_terminal_paste_shortcut),
                ydotool_path: Some(settings.linux_ydotool_path.clone())
                    .filter(|path| !path.trim().is_empty()),
                xdotool_path: Some(settings.linux_xdotool_path.clone())
                    .filter(|path| !path.trim().is_empty()),
            },
        },
        state: StateDoc {
            edited_profile: filled(&settings.editing_profile_id),
            game_mode_switch: game_mode.control,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn config(settings: Value) -> String {
        json!({"version": 1, "settings": settings}).to_string()
    }

    #[test]
    fn all_interface_locales_persist_without_changing_schema_version() {
        for code in [
            "auto", "en", "ru", "de", "fr", "es", "it", "pt", "pl", "tr", "uk", "cs", "hu", "ro",
            "el", "hr", "ka", "kk", "zh-Hans", "zh-Hant", "ja", "ko",
        ] {
            let settings = parse(&config(json!({"locale": code}))).unwrap();
            assert_eq!(settings.locale.as_str(), code);
            let saved = serialize(&settings);
            let raw: Value = serde_json::from_str(&saved).unwrap();
            assert_eq!(raw["version"], 1);
            // The default is not written.
            let expected = if code == "auto" {
                Value::Null
            } else {
                json!(code)
            };
            assert_eq!(raw["settings"]["locale"], expected);
            assert_eq!(parse(&saved).unwrap().locale, settings.locale);
        }
    }

    #[test]
    fn editor_schema_is_preserved_and_strictly_typed() {
        let text = r#"{"version":1,"$schema":"https://example.com/config.schema.json"}"#;
        let mut settings = parse(text).unwrap();
        settings.high_contrast = true;
        let saved = serialize(&settings);
        assert_eq!(parse(&saved).unwrap(), settings);
        let raw: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(raw["$schema"], "https://example.com/config.schema.json");
        for value in ["null", "1", "{}"] {
            assert!(parse(&format!(r#"{{"version":1,"$schema":{value}}}"#)).is_err());
        }
    }

    #[test]
    fn experimental_flag_is_optional_and_removed_when_disabled() {
        let mut settings = parse(&config(json!({}))).unwrap();
        assert!(!settings.in_development);
        let raw: Value = serde_json::from_str(&serialize(&settings)).unwrap();
        assert!(raw.get("settings").is_none());
        settings.in_development = true;
        let saved = serialize(&settings);
        assert!(parse(&saved).unwrap().in_development);
        settings.in_development = false;
        let raw: Value = serde_json::from_str(&serialize(&settings)).unwrap();
        assert!(raw.get("settings").is_none());
    }

    #[test]
    fn only_values_differing_from_defaults_are_written() {
        let raw: Value = serde_json::from_str(&serialize(&AppSettings::default())).unwrap();
        assert_eq!(raw, json!({"version": 1}));
        let settings = AppSettings {
            default_hold_timeout_ms: 300,
            linux_terminal_paste_shortcut: None,
            ..AppSettings::default()
        };
        let text = serialize(&settings);
        let raw: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            raw,
            json!({"version": 1, "settings": {
                "timing": {"holdMs": 300},
                "linux": {"terminalPasteShortcut": "sameAsPaste"},
            }})
        );
        assert_eq!(parse(&text).unwrap(), settings);
    }

    #[test]
    fn ids_are_unique_and_generated_without_collisions() {
        for field in ["autoRules", "processes"] {
            let items = if field == "autoRules" {
                serde_json::json!([{"id":"x","profile":null},{"id":"x","profile":null}])
            } else {
                serde_json::json!([{"id":"x","name":"a"},{"id":"x","name":"b"}])
            };
            let settings = if field == "autoRules" {
                serde_json::json!({"profiles":{"auto":{"rules":items}}})
            } else {
                serde_json::json!({"gameMode":{field:items}})
            };
            assert!(
                parse(&config(settings))
                    .unwrap_err()
                    .contains("duplicate id")
            );
        }
        let settings = parse(&config(serde_json::json!({
            "profiles":{"auto":{"rules":[{"profile":null}, {"id":"rule-1","profile":null}]}},
            "gameMode":{"processes":[{"name":"a"},{"id":"process-1","name":"b"}]}
        })))
        .unwrap();
        assert_eq!(settings.auto_rules[0].id, "rule-2");
        assert_eq!(settings.game_mode.process_matchers[0].id, "process-2");
        assert_eq!(parse(&serialize(&settings)).unwrap(), settings);
    }

    #[test]
    fn settings_timing_boundaries_are_enforced() {
        use super::super::timing::*;
        for (field, range) in [
            ("holdMs", TAP_HOLD),
            ("doubleTapMs", TAP_HOLD),
            ("longHoldMs", LONG_HOLD),
            ("comboMs", TRIGGER_CHORD),
            ("macroStepPauseMs", MACRO_DELAY),
            ("macroModifierDelayMs", MACRO_DELAY),
            ("macroTimeoutMs", MACRO_TIMEOUT),
        ] {
            for value in [0, range.min, range.max, range.max + 1, u64::MAX] {
                let text = config(serde_json::json!({"timing":{field:value}}));
                assert_eq!(
                    parse(&text).is_ok(),
                    range.contains(value),
                    "{field}: {value}"
                );
            }
        }
    }

    #[test]
    fn missing_settings_use_defaults() {
        assert_eq!(parse("").unwrap(), AppSettings::default());
        assert_eq!(parse(r#"{"version": 1}"#).unwrap(), AppSettings::default());
        assert_eq!(parse(&config(json!({}))).unwrap(), AppSettings::default());
    }

    #[test]
    fn version_is_required_and_checked() {
        assert!(
            parse(r#"{"settings": {}}"#)
                .unwrap_err()
                .contains("version")
        );
        assert!(parse(r#"{"version": 2}"#).unwrap_err().contains("newer"));
        assert!(parse(r#"[]"#).is_err());
    }

    #[test]
    fn invalid_or_unknown_values_are_errors() {
        for settings in [
            json!({"appearance": "Dark"}),
            json!({"locale": "de-DE"}),
            json!({"locale": "en-US"}),
            json!({"appearance": "eink"}),
            json!({"linux": {"textMode": "libei-pure"}}),
            json!({"linux": {"pasteShortcut": "shift-insert"}}),
            json!({"linux": {"pasteShortcut": "Alt+KeyV"}}),
            json!({"inDevelopment": true}),
            json!({"activeProfileId": "user:A"}),
            json!({"futureOption": 7}),
            json!({"timing": {"holdMs": "fast"}}),
            json!({"timing": {"tapDecision": "sometimes"}}),
            json!({"linux": {"textMode": "magic"}}),
            json!({"gameMode": {"detectors": {"origin": true}}}),
            json!({"gameMode": {"processes": [{"name": "steam", "isBlacklist": true}]}}),
            json!({"profiles": {"auto": {"rules": [{"profile": "user:A", "window_patterns": ["kate"]}]}}}),
            json!({"profiles": {"auto": {"rules": [{"when": {"gameMode": "on"}}]}}}),
            json!({"profiles": {"auto": {"rules": [{"profile": ""}]}}}),
            json!({"profiles": {"auto": {"fallback": " "}}}),
            json!({"profiles": {"auto": {"rules": [{"id": "a b", "profile": null}]}}}),
            json!({"profiles": {"active": "user:A"}}),
            json!({"keyboardDevice": "/dev/input/event1"}),
            json!({"devices": {"keyboards": ["/dev/a", "/dev/b"]}}),
            json!({"devices": {"mice": [""]}}),
            json!({"devices": {"tablets": []}}),
        ] {
            assert!(
                parse(&config(settings.clone())).is_err(),
                "accepted {settings}"
            );
        }
        assert!(parse(r#"{"version": 1, "extra": true}"#).is_err());
        assert!(parse(r#"{"version": 1, "state": {"gameModeSwitch": "maybe"}}"#).is_err());
        assert!(parse(r#"{"version": 1, "state": {"gameMode": "on"}}"#).is_err());
    }

    #[test]
    fn every_setting_round_trips() {
        let settings = AppSettings {
            launch_on_startup: true,
            start_mapper_on_launch: true,
            appearance: Appearance::EInk,
            locale: LocalePreference::Russian,
            default_hold_timeout_ms: 180,
            tap_decision: TapDecision::HoldOnOtherKeyPress,
            input_device_path: Some("/dev/input/by-id/kbd".into()),
            input_mouse_device_path: Some("/dev/input/by-id/mouse".into()),
            editing_profile_id: Some("user:Main".into()),
            profile_mode: ProfileMode::Auto,
            manual_profile_id: Some("user:Other".into()),
            profile_order: vec!["user:Other".into(), "user:Main".into()],
            auto_rules: vec![AutoRule {
                id: "a".into(),
                profile_id: None,
                enabled: false,
                conditions: super::super::model::ProfileConditionSet {
                    game_mode: Some(crate::config::model::GameModeCondition::On),
                    keyboard_layouts: vec!["us".into()],
                    window_patterns: vec!["blender".into()],
                    excluded_window_patterns: vec!["title:Preferences".into()],
                },
            }],
            auto_default_profile_id: Some("user:Main".into()),
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
            linux_terminal_paste_shortcut: None,
            linux_ydotool_path: "/usr/bin/ydotool".into(),
            ..AppSettings::default()
        };
        let text = serialize(&settings);
        assert_eq!(parse(&text).unwrap(), settings);
        let raw: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(raw["version"], 1);
        assert_eq!(raw["settings"]["timing"]["holdMs"], 180);
        assert_eq!(raw["settings"]["linux"]["pasteShortcut"], "Ctrl+Shift+KeyV");
        assert_eq!(raw["settings"]["appearance"], "eInk");
        assert_eq!(raw["settings"]["locale"], "ru");
        assert_eq!(raw["settings"]["linux"]["textMode"], "clipboard");
        assert_eq!(
            raw["settings"]["linux"]["terminalPasteShortcut"],
            "sameAsPaste"
        );
        assert_eq!(raw["settings"]["gameMode"]["processes"][0]["exclude"], true);
        assert_eq!(raw["state"]["editedProfile"], "user:Main");
        assert_eq!(raw["state"]["gameModeSwitch"], "on");
        let profiles = &raw["settings"]["profiles"];
        assert_eq!(profiles["mode"], "auto");
        assert_eq!(profiles["order"], json!(["user:Other", "user:Main"]));
        assert!(raw["state"].get("profileOrder").is_none());
        assert_eq!(
            profiles["auto"]["rules"][0]["when"]["excludeWindows"],
            json!(["title:Preferences"])
        );
        assert_eq!(profiles["auto"]["rules"][0]["profile"], Value::Null);
        assert_eq!(profiles["auto"]["fallback"], "user:Main");
        assert_eq!(
            raw["settings"]["devices"],
            json!({"keyboards": ["/dev/input/by-id/kbd"], "mice": ["/dev/input/by-id/mouse"]})
        );
        assert_eq!(profiles["manualProfile"], "user:Other");
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
        assert_eq!(process.id, "process-1");
        assert!(process.only_active_window);
        assert!(!process.is_blacklist);
    }

    #[test]
    fn terminal_paste_shortcut_defaults_and_can_follow_the_main_one() {
        let shortcut = |linux: Value| {
            parse(&config(json!({ "linux": linux })))
                .unwrap()
                .linux_terminal_paste_shortcut
        };
        assert_eq!(
            shortcut(json!({"pasteShortcut": "Ctrl+KeyV"})),
            Some(PasteShortcut::CtrlShiftV)
        );
        assert_eq!(
            shortcut(json!({"terminalPasteShortcut": "sameAsPaste"})),
            None
        );
        assert_eq!(
            shortcut(json!({"terminalPasteShortcut": "Shift+Insert"})),
            Some(PasteShortcut::ShiftInsert)
        );
        // Any modifier order and spelling names the same chord.
        assert_eq!(
            shortcut(json!({"terminalPasteShortcut": "ShiftLeft+Ctrl+KeyV"})),
            Some(PasteShortcut::CtrlShiftV)
        );
        let null = config(json!({"linux": {"terminalPasteShortcut": null}}));
        assert!(parse(&null).is_err());
    }

    #[test]
    fn program_paths_are_omitted_rather_than_empty() {
        for bad in [json!(""), json!("  "), Value::Null] {
            assert!(parse(&config(json!({"linux": {"ydotoolPath": bad}}))).is_err());
        }
        let settings = parse(&config(json!({"linux": {"xdotoolPath": "/opt/xdotool"}}))).unwrap();
        assert_eq!(settings.linux_xdotool_path, "/opt/xdotool");
        assert_eq!(settings.linux_ydotool_path, "");
        let raw: Value = serde_json::from_str(&serialize(&AppSettings::default())).unwrap();
        assert!(raw.get("settings").is_none());
    }

    #[test]
    fn mapper_start_on_launch_is_off_by_default() {
        assert!(!AppSettings::default().start_mapper_on_launch);
        let settings = parse(&config(json!({"startMapperOnLaunch": true}))).unwrap();
        assert!(settings.start_mapper_on_launch);
    }

    #[test]
    fn manual_activation_is_independent_of_the_edited_profile() {
        let settings = parse(r#"{"version": 1, "state": {"editedProfile": "user:Main"}}"#).unwrap();
        assert_eq!(settings.editing_profile_id.as_deref(), Some("user:Main"));
        assert_eq!(settings.manual_profile_id, None);
    }

    #[test]
    fn auto_rules_without_ids_get_deterministic_ids() {
        let settings = parse(&config(json!({"profiles": {"auto": {
            "rules": [
                {"id": "a", "profile": "user:A", "when": {"windows": ["blender"]}},
                {"profile": null, "enabled": false},
            ],
            "fallback": "user:B",
        }}})))
        .unwrap();
        assert_eq!(settings.auto_rules.len(), 2);
        assert_eq!(
            settings.auto_rules[0].conditions.window_patterns,
            ["blender"]
        );
        assert_eq!(settings.auto_rules[1].id, "rule-1");
        assert_eq!(settings.auto_rules[1].profile_id, None);
        assert!(!settings.auto_rules[1].enabled);
        assert!(settings.auto_rules[1].conditions.is_empty());
        assert_eq!(settings.auto_default_profile_id.as_deref(), Some("user:B"));
    }
}
