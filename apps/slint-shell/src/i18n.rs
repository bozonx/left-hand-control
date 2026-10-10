//! User-visible text produced by Rust.
//!
//! Status and error messages are sent to Slint as ids with arguments and
//! translated by `Locale.text()` in `ui/i18n.slint` through `@tr`, so they
//! follow language switches without Rust re-sending them.
//!
//! Text shown outside Slint (tray menu) or built from data (action catalog
//! names) is translated with [`tr`] from the same PO catalog that Slint
//! bundles, so every translation lives in `translations/`.

use crate::ui::Message;
use lhc_core::config_document::ConfigError;
use lhc_core::profile::actions::ActionIssue;
use lhc_core::profile::diagnostics::{RuleIssue, RuleIssueCode};
use lhc_core::profile::model::LocalePreference;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English,
    Russian,
}

impl Language {
    /// Code of the bundled Slint translation.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
        }
    }

    pub fn select_bundled(self) {
        if let Err(error) = select_ui_language(self.code()) {
            log::error!("select translation: {error}");
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "ru" => Some(Self::Russian),
            _ => None,
        }
    }

    /// Resolve the configured preference; `auto` follows the OS language
    /// and falls back to English.
    pub fn resolve(preference: LocalePreference) -> Self {
        match preference {
            LocalePreference::English => Self::English,
            LocalePreference::Russian => Self::Russian,
            LocalePreference::Auto => ["LC_ALL", "LC_MESSAGES", "LANG"]
                .iter()
                .filter_map(|name| std::env::var(name).ok())
                .find(|value| !value.is_empty())
                .filter(|value| value.starts_with("ru"))
                .map_or(Self::English, |_| Self::Russian),
        }
    }
}

const RUSSIAN_PO: &str = include_str!("../translations/ru/LC_MESSAGES/slint-shell.po");
const RUSSIAN_MO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ru.mo"));

pub fn select_ui_language(code: &str) -> Result<(), String> {
    let language =
        Language::from_code(code).ok_or_else(|| format!("unknown UI language: {code}"))?;
    static RUSSIAN: OnceLock<Result<Arc<tr::MoTranslator>, String>> = OnceLock::new();
    let translator: Option<Box<dyn i_slint_core::translations::Translator>> =
        if language == Language::Russian {
            let russian = RUSSIAN
                .get_or_init(|| {
                    tr::MoTranslator::from_vec_u8(RUSSIAN_MO.to_vec())
                        .map(Arc::new)
                        .map_err(|error| error.to_string())
                })
                .as_ref()
                .map_err(Clone::clone)?;
            Some(Box::new(russian.clone()))
        } else {
            None
        };
    i_slint_core::context::with_global_context(
        || Err("initialize a Slint window before selecting the UI language".into()),
        |context| context.set_external_translator(translator),
    )
    .map_err(|error| error.to_string())?;
    match slint::select_bundled_translation(code) {
        Ok(()) | Err(slint::SelectBundledTranslationError::NoTranslationsBundled) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Translate an English source string. `{n}` is replaced with `n`.
pub fn tr(language: Language, source: &str, n: Option<u32>) -> String {
    static RUSSIAN: OnceLock<HashMap<String, String>> = OnceLock::new();
    let text = match language {
        Language::English => source,
        Language::Russian => RUSSIAN
            .get_or_init(|| parse_po(RUSSIAN_PO))
            .get(source)
            .map_or(source, String::as_str),
    };
    match n {
        Some(n) => text.replace("{n}", &n.to_string()),
        None => text.to_owned(),
    }
}

/// Singular `msgid` → `msgstr` pairs of a PO file.
fn parse_po(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut id: Option<String> = None;
    let mut current: Option<(bool, String)> = None;
    let mut contextual = false;
    let mut finish =
        |current: &mut Option<(bool, String)>, id: &mut Option<String>| match current.take() {
            Some((false, value)) => *id = Some(value),
            Some((true, value)) => {
                if let Some(id) = id.take()
                    && !id.is_empty()
                    && !value.is_empty()
                {
                    out.insert(id, value);
                }
            }
            None => {}
        };
    for line in text.lines().map(str::trim) {
        if line.starts_with("msgctxt ") {
            finish(&mut current, &mut id);
            contextual = true;
            continue;
        }
        if line.is_empty() {
            contextual = false;
        }
        if contextual {
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgid ") {
            finish(&mut current, &mut id);
            current = Some((false, unquote(rest)));
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            finish(&mut current, &mut id);
            current = Some((true, unquote(rest)));
        } else if line.starts_with('"') {
            if let Some((_, value)) = &mut current {
                value.push_str(&unquote(line));
            }
        } else {
            finish(&mut current, &mut id);
            if !line.starts_with("msgstr[") {
                id = None;
            }
        }
    }
    finish(&mut current, &mut id);
    out
}

fn unquote(value: &str) -> String {
    let inner = value.trim().strip_prefix('"').unwrap_or(value);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    None,
    Copied,
    LayoutCreated(String),
    LayoutSaved(String),
    LayoutDeleted(String),
    LayoutActivated(String),
    LayoutDiscarded(String),
    MapperRunning(Option<String>),
    MapperStopped(Option<String>),
    MapperStarting,
    ConfigLoaded(usize),
    ConfigSaved(usize),
    ConfigReloaded(usize),
    ConfigUnavailable(String),
    ConfigExternalChange,
    DeviceSaved(String),
    LoadConfigFirst,
    SelectInputDevice,
    WorkerUnavailable(String),
    WorkerRestarting,
    WorkerRestartLimit,
    WorkerNotStarted,
    WorkerError(String),
    SavedMapperNotUpdated(String),
    ActionSaved,
    SettingsSaved,
    CapabilityAvailable,
    CapabilityUnavailable,
    CapabilityUnsupported,
    GameModeAutoDisabled,
    GameModeAutoUnavailable,
    GameModeAutoInactive,
    GameModeDaemonActive,
    GameModeProcessActive(String),
    GameModeFullscreenActive,
    GameModeExcluded(String),
    GameModeManualOn,
    GameModeManualOff,
    LibrarySaved,
    LibrarySelect,
    LibraryChanged,
    ProcessNameRequired,
    LayerSaved,
    RuleSaved,
    LayoutEmpty,
    /// Default name of a copy of the named item.
    CopyName(String),
    /// Menu actions run through the mapper, which is stopped.
    MapperRequired,
    CommandStarted(String),
    CommandDirectoryMissing,
    ScriptNotExecutable,
    ChooseCommandScript,
    ChooseCommandDirectory,
    ActionUnavailable,
    CommandsDisabled,
    SaveLayoutFirst,
    MenuIssue(lhc_core::profile::menus::MenuIssue),
    MacroIssue(lhc_core::profile::macros::MacroIssue),
    LayerNameRequired,
    KeyCodeRequired,
    ActionRequired,
    PickerSuppress,
    PickerInherit,
    PickerNative,
    PickerNoAction,
    PickerEmptyCell,
    PickerLayerOnly,
    TimeoutInvalid,
    HoldSecondsInvalid,
    TextEmpty,
    DelayRange,
    SystemActionRequired,
    PickerAction(String, u32),
    UnknownKind,
    InvalidAction(ActionIssue),
    Rule(RuleIssue),
    ActionFailed(String),
    /// Untranslated detail from the core or the OS.
    Error(String),
}

impl Msg {
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }

    pub fn to_ui(&self) -> Message {
        let empty = String::new;
        let (id, arg, count) = match self {
            Self::None => ("", empty(), 0),
            Self::Copied => ("copied", empty(), 0),
            Self::LayoutCreated(name) => ("layout-created", name.clone(), 0),
            Self::LayoutSaved(name) => ("layout-saved", name.clone(), 0),
            Self::LayoutDeleted(name) => ("layout-deleted", name.clone(), 0),
            Self::LayoutActivated(name) => ("layout-activated", name.clone(), 0),
            Self::LayoutDiscarded(name) => ("layout-discarded", name.clone(), 0),
            Self::MapperRunning(error) => ("mapper-running", error.clone().unwrap_or_default(), 0),
            Self::MapperStopped(error) => ("mapper-stopped", error.clone().unwrap_or_default(), 0),
            Self::MapperStarting => ("mapper-starting", empty(), 0),
            Self::ConfigLoaded(rules) => ("config-loaded", empty(), *rules),
            Self::ConfigSaved(rules) => ("config-saved", empty(), *rules),
            Self::ConfigReloaded(rules) => ("config-reloaded", empty(), *rules),
            Self::ConfigUnavailable(error) => ("config-unavailable", error.clone(), 0),
            Self::ConfigExternalChange => ("config-external-change", empty(), 0),
            Self::DeviceSaved(path) => ("device-saved", path.clone(), 0),
            Self::LoadConfigFirst => ("load-config-first", empty(), 0),
            Self::SelectInputDevice => ("select-input-device", empty(), 0),
            Self::WorkerUnavailable(error) => ("worker-unavailable", error.clone(), 0),
            Self::WorkerRestarting => ("worker-restarting", empty(), 0),
            Self::WorkerRestartLimit => ("worker-restart-limit", empty(), 0),
            Self::WorkerNotStarted => ("worker-not-started", empty(), 0),
            Self::WorkerError(error) => ("worker-error", error.clone(), 0),
            Self::SavedMapperNotUpdated(error) => ("saved-mapper-not-updated", error.clone(), 0),
            Self::ActionSaved => ("action-saved", empty(), 0),
            Self::LibrarySaved => ("library-saved", empty(), 0),
            Self::LibrarySelect => ("library-select", empty(), 0),
            Self::LibraryChanged => ("library-changed", empty(), 0),
            Self::CapabilityAvailable => ("capability-available", empty(), 0),
            Self::CapabilityUnavailable => ("capability-unavailable", empty(), 0),
            Self::CapabilityUnsupported => ("capability-unsupported", empty(), 0),
            Self::GameModeAutoDisabled => ("game-mode-auto-disabled", empty(), 0),
            Self::GameModeAutoUnavailable => ("game-mode-auto-unavailable", empty(), 0),
            Self::GameModeAutoInactive => ("game-mode-auto-inactive", empty(), 0),
            Self::GameModeDaemonActive => ("game-mode-daemon-active", empty(), 0),
            Self::GameModeProcessActive(name) => ("game-mode-process-active", name.clone(), 0),
            Self::GameModeFullscreenActive => ("game-mode-fullscreen-active", empty(), 0),
            Self::GameModeExcluded(name) => ("game-mode-excluded", name.clone(), 0),
            Self::GameModeManualOn => ("game-mode-manual-on", empty(), 0),
            Self::GameModeManualOff => ("game-mode-manual-off", empty(), 0),
            Self::SettingsSaved => ("settings-saved", empty(), 0),
            Self::ProcessNameRequired => ("process-name-required", empty(), 0),
            Self::MacroIssue(issue) => (
                match issue {
                    lhc_core::profile::macros::MacroIssue::InvalidId => "macro-invalid-id",
                    lhc_core::profile::macros::MacroIssue::DuplicateId => "macro-duplicate-id",
                    lhc_core::profile::macros::MacroIssue::DelayRange => "macro-delay-range",
                    lhc_core::profile::macros::MacroIssue::PauseRange => "macro-pause-range",
                    lhc_core::profile::macros::MacroIssue::UnknownKey => "macro-unknown-key",
                    lhc_core::profile::macros::MacroIssue::Cycle => "macro-cycle",
                },
                empty(),
                0,
            ),
            Self::MenuIssue(issue) => (
                match issue {
                    lhc_core::profile::menus::MenuIssue::EmojiCell => "menu-emoji-cell",
                    lhc_core::profile::menus::MenuIssue::UnknownKey => "macro-unknown-key",
                    lhc_core::profile::menus::MenuIssue::CommandId => "menu-command-id",
                    lhc_core::profile::menus::MenuIssue::DuplicateCommand => {
                        "menu-duplicate-command"
                    }
                    lhc_core::profile::menus::MenuIssue::EmptyCommand => "menu-empty-command",
                },
                empty(),
                0,
            ),
            Self::CommandStarted(name) => ("command-started", name.clone(), 0),
            Self::CommandDirectoryMissing => ("command-directory-missing", empty(), 0),
            Self::ScriptNotExecutable => ("script-not-executable", empty(), 0),
            Self::ChooseCommandScript => ("choose-command-script", empty(), 0),
            Self::ChooseCommandDirectory => ("choose-command-directory", empty(), 0),
            Self::ActionUnavailable => ("action-unavailable", empty(), 0),
            Self::SaveLayoutFirst => ("save-layout-first", empty(), 0),
            Self::CommandsDisabled => ("commands-disabled", empty(), 0),
            Self::LayerSaved => ("layer-saved", empty(), 0),
            Self::RuleSaved => ("rule-saved", empty(), 0),
            Self::LayoutEmpty => ("layout-empty", empty(), 0),
            Self::CopyName(name) => ("copy-name", name.clone(), 0),
            Self::MapperRequired => ("mapper-required", empty(), 0),
            Self::LayerNameRequired => ("layer-name-required", empty(), 0),
            Self::ActionRequired => ("action-required", empty(), 0),
            Self::PickerSuppress => ("picker-suppress", empty(), 0),
            Self::PickerInherit => ("picker-inherit", empty(), 0),
            Self::PickerNative => ("picker-native", empty(), 0),
            Self::PickerEmptyCell => ("picker-empty-cell", empty(), 0),
            Self::PickerNoAction => ("picker-no-action", empty(), 0),
            Self::PickerLayerOnly => ("picker-layer-only", empty(), 0),
            Self::KeyCodeRequired => ("key-code-required", empty(), 0),
            Self::HoldSecondsInvalid => ("hold-seconds-invalid", empty(), 0),
            Self::TimeoutInvalid => ("timeout-invalid", empty(), 0),
            Self::TextEmpty => ("text-empty", empty(), 0),
            Self::DelayRange => ("delay-range", empty(), 0),
            Self::PickerAction(id, n) => ("picker-action", id.clone(), *n as usize),
            Self::SystemActionRequired => ("system-action-required", empty(), 0),
            Self::UnknownKind => ("unknown-kind", empty(), 0),
            Self::InvalidAction(issue) => (
                match issue {
                    ActionIssue::InvalidSyntax => "action-invalid-syntax",
                    ActionIssue::PauseOutsideMacro => "action-pause-outside-macro",
                    ActionIssue::UnknownMacro => "action-unknown-macro",
                    ActionIssue::UnknownCommand => "action-unknown-command",
                    ActionIssue::UnknownSystemAction => "action-unknown-system",
                    ActionIssue::UnknownAppAction => "action-unknown-app",
                },
                empty(),
                0,
            ),
            Self::Rule(issue) => (
                match issue.code {
                    RuleIssueCode::MissingTrigger => "rule-missing-trigger",
                    RuleIssueCode::InvalidTrigger => "rule-invalid-trigger",
                    RuleIssueCode::DuplicateTrigger => "rule-duplicate-trigger",
                    RuleIssueCode::UnknownLayer => "rule-unknown-layer",
                    RuleIssueCode::InvalidTapAction => "rule-invalid-tap",
                    RuleIssueCode::InvalidHoldAction => "rule-invalid-hold",
                    RuleIssueCode::InvalidDoubleTapAction => "rule-invalid-double-tap",
                },
                issue.trigger.clone().unwrap_or_default(),
                0,
            ),
            Self::ActionFailed(error) => ("action-failed", error.clone(), 0),
            Self::Error(error) => ("error", error.clone(), 0),
        };
        Message {
            id: id.into(),
            arg: arg.into(),
            count: i32::try_from(count).unwrap_or(i32::MAX),
        }
    }
}

impl From<&ConfigError> for Msg {
    fn from(error: &ConfigError) -> Self {
        match error {
            ConfigError::ExternalChange => Self::ConfigExternalChange,
            ConfigError::Menu(issue) => Self::MenuIssue(*issue),
            ConfigError::Macro(issue) => Self::MacroIssue(*issue),
            ConfigError::InvalidAction(issue) => Self::InvalidAction(*issue),
            ConfigError::Rules(issues) if !issues.is_empty() => Self::Rule(issues[0].clone()),
            other => Self::Error(other.to_string()),
        }
    }
}

/// Tray menu entries, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayItem {
    Settings,
    Emoji,
    Quick,
    ToggleMapper,
    Quit,
}

impl TrayItem {
    pub const ALL: [TrayItem; 5] = [
        TrayItem::Settings,
        TrayItem::Emoji,
        TrayItem::Quick,
        TrayItem::ToggleMapper,
        TrayItem::Quit,
    ];

    pub fn label(self, language: Language) -> String {
        let source = match self {
            Self::Settings => "Settings",
            Self::Emoji => "Emoji",
            Self::Quick => "Quick actions",
            Self::ToggleMapper => "Mapper on / off",
            Self::Quit => "Quit",
        };
        tr(language, source, None)
    }

    pub fn command(self) -> crate::command::Command {
        use crate::command::{Command, Window};
        match self {
            Self::Settings => Command::Show(Window::Settings),
            Self::Emoji => Command::Show(Window::EMOJI),
            Self::Quick => Command::Show(Window::QUICK),
            Self::ToggleMapper => Command::ToggleMapper,
            Self::Quit => Command::Quit,
        }
    }
}

/// Game-mode choices of the tray submenu, in display order.
pub const TRAY_GAME_MODES: [crate::command::GameMode; 3] = [
    crate::command::GameMode::Auto,
    crate::command::GameMode::On,
    crate::command::GameMode::Off,
];

/// Title of the tray game-mode submenu: the effective state.
pub fn tray_game_title(language: Language, active: bool) -> String {
    tr(
        language,
        if active {
            "Game mode: on"
        } else {
            "Game mode: off"
        },
        None,
    )
}

/// Label of one tray game-mode choice.
pub fn tray_game_choice(language: Language, mode: crate::command::GameMode) -> String {
    use crate::command::GameMode;
    let source = match mode {
        GameMode::Auto => "Auto — detect games",
        GameMode::On => "On — always",
        GameMode::Off | GameMode::Toggle => "Off — never",
    };
    tr(language, source, None)
}

/// Index of `control` in [`TRAY_GAME_MODES`].
pub fn tray_game_index(control: lhc_core::gamemode::GameModeControl) -> usize {
    use lhc_core::gamemode::GameModeControl;
    match control {
        GameModeControl::Auto => 0,
        GameModeControl::On => 1,
        GameModeControl::Off => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_catalog_translates_ui_and_russian_plural_forms() {
        use tr::Translator;

        let translator = tr::MoTranslator::from_vec_u8(RUSSIAN_MO.to_vec()).unwrap();
        assert_eq!(translator.translate("Settings", None), "Настройки");
        for (count, suffix) in [
            (1, "правило"),
            (2, "правила"),
            (5, "правил"),
            (11, "правил"),
            (21, "правило"),
        ] {
            assert_eq!(
                translator.ntranslate(
                    count,
                    "Configuration loaded: {n} rule",
                    "Configuration loaded: {n} rules",
                    None
                ),
                format!("Конфигурация загружена: {{n}} {suffix}"),
            );
        }
    }

    #[test]
    fn po_catalog_translates_rust_side_text() {
        assert_eq!(TrayItem::Quit.label(Language::Russian), "Выход");
        assert_eq!(TrayItem::Quit.label(Language::English), "Quit");
        assert_eq!(
            tr(Language::Russian, "Switch to desktop {n}", Some(3)),
            "Переключиться на рабочий стол 3"
        );
        assert_eq!(
            tr(Language::Russian, "not in the catalog", None),
            "not in the catalog"
        );
    }

    #[test]
    fn po_parser_handles_continuations_and_escapes() {
        let po = "msgid \"\"\nmsgstr \"header\"\n\nmsgid \"a \\\"b\\\"\"\nmsgstr \"\"\n\"x\"\n\"y\"\n\nmsgid \"one\"\nmsgid_plural \"many\"\nmsgstr[0] \"одна\"\n";
        let map = parse_po(po);
        assert_eq!(map.get("a \"b\"").map(String::as_str), Some("xy"));
        assert!(!map.contains_key(""));
        assert!(!map.contains_key("one"));
    }

    #[test]
    fn contextual_game_mode_labels_do_not_replace_other_translations() {
        let po = "msgid \"On\"\nmsgstr \"Вкл\"\n\nmsgctxt \"Game mode control\"\nmsgid \"On\"\nmsgstr \"On\"\n";
        assert_eq!(parse_po(po).get("On").map(String::as_str), Some("Вкл"));
    }

    #[test]
    fn auto_language_follows_the_environment_only_for_auto() {
        assert_eq!(
            Language::resolve(LocalePreference::Russian),
            Language::Russian
        );
        assert_eq!(
            Language::resolve(LocalePreference::English),
            Language::English
        );
    }

    /// Source strings of `@tr("…")` in the Slint files (plural forms count
    /// by their singular).
    fn slint_strings() -> Vec<String> {
        let mut out = Vec::new();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "slint") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (index, _) in text.match_indices("@tr(\"") {
                let mut rest = &text[index + 5..];
                if let Some(quote) = rest.find('"')
                    && let Some(source) = rest[quote..].strip_prefix("\" => \"")
                {
                    rest = source;
                }
                let mut value = String::new();
                let mut chars = rest.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(next) = chars.next() {
                                value.push(if next == 'n' { '\n' } else { next });
                            }
                        }
                        c => value.push(c),
                    }
                }
                out.push(value);
            }
        }
        out
    }

    #[test]
    fn every_slint_string_has_a_russian_translation() {
        let catalog = parse_po(RUSSIAN_PO);
        // Plural entries are not in the parsed catalog; look for their msgid.
        let declared = |source: &str| {
            let escaped = source
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            RUSSIAN_PO.contains(&format!("msgid \"{escaped}\""))
        };
        let mut missing: Vec<String> = slint_strings()
            .into_iter()
            .filter(|source| !catalog.contains_key(source) && !declared(source))
            .collect();
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "missing in slint-shell.po: {missing:#?}"
        );
    }
}
