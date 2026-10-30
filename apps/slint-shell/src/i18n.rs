//! User-visible text produced by Rust.
//!
//! Status and error messages are sent to Slint as ids with arguments and
//! translated by `Locale.text()` in `ui/i18n.slint` through `@tr`, so they
//! follow language switches without Rust re-sending them.
//!
//! Text shown outside Slint uses a typed catalog generated at build time
//! from the same PO catalogs. Missing translations fail the build.

use crate::ui::Message;
use lhc_core::config_document::ConfigError;
use lhc_core::platform::InputAccessIssue;
use lhc_core::profile::actions::ActionIssue;
use lhc_core::profile::diagnostics::{RuleIssue, RuleIssueCode};
use lhc_core::profile::model::LocalePreference;
use lhc_core::profile::names::{NameIssue, NameKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
    #[default]
    English,
    Russian,
    German,
    French,
    Spanish,
    Italian,
    Portuguese,
    Polish,
    Turkish,
    Ukrainian,
    Czech,
    Hungarian,
    Romanian,
    Greek,
    Croatian,
    Georgian,
    Kazakh,
    ChineseSimplified,
    ChineseTraditional,
    Japanese,
    Korean,
}

/// Every language with its translation code and settings choice.
const LANGUAGES: [(Language, &str, LocalePreference); 21] = [
    (Language::English, "en", LocalePreference::English),
    (Language::Russian, "ru", LocalePreference::Russian),
    (Language::German, "de", LocalePreference::German),
    (Language::French, "fr", LocalePreference::French),
    (Language::Spanish, "es", LocalePreference::Spanish),
    (Language::Italian, "it", LocalePreference::Italian),
    (Language::Portuguese, "pt", LocalePreference::Portuguese),
    (Language::Polish, "pl", LocalePreference::Polish),
    (Language::Turkish, "tr", LocalePreference::Turkish),
    (Language::Ukrainian, "uk", LocalePreference::Ukrainian),
    (Language::Czech, "cs", LocalePreference::Czech),
    (Language::Hungarian, "hu", LocalePreference::Hungarian),
    (Language::Romanian, "ro", LocalePreference::Romanian),
    (Language::Greek, "el", LocalePreference::Greek),
    (Language::Croatian, "hr", LocalePreference::Croatian),
    (Language::Georgian, "ka", LocalePreference::Georgian),
    (Language::Kazakh, "kk", LocalePreference::Kazakh),
    (
        Language::ChineseSimplified,
        "zh-Hans",
        LocalePreference::ChineseSimplified,
    ),
    (
        Language::ChineseTraditional,
        "zh-Hant",
        LocalePreference::ChineseTraditional,
    ),
    (Language::Japanese, "ja", LocalePreference::Japanese),
    (Language::Korean, "ko", LocalePreference::Korean),
];

impl Language {
    /// Code of the bundled Slint translation.
    pub fn code(self) -> &'static str {
        LANGUAGES
            .iter()
            .find(|(language, ..)| *language == self)
            .map_or("en", |(_, code, _)| code)
    }

    pub fn select_bundled(self) {
        if let Err(error) = select_ui_language(self.code()) {
            log::error!("select translation: {error}");
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        LANGUAGES
            .iter()
            .find(|(_, known, _)| *known == code)
            .map(|(language, ..)| *language)
    }

    /// Resolve an OS locale, including Chinese script and region variants.
    fn from_os_locale(locale: &str) -> Option<Self> {
        let normalized = locale
            .split(['.', '@'])
            .next()
            .unwrap_or(locale)
            .replace('_', "-")
            .to_ascii_lowercase();
        let parts: Vec<_> = normalized.split('-').collect();
        if parts[0] == "zh" {
            return Some(
                if !parts.contains(&"hans")
                    && parts
                        .iter()
                        .any(|part| matches!(*part, "hant" | "tw" | "hk" | "mo"))
                {
                    Self::ChineseTraditional
                } else {
                    Self::ChineseSimplified
                },
            );
        }
        Self::from_code(parts[0])
    }

    /// Resolve the configured preference; `auto` follows the OS language
    /// and falls back to English.
    pub fn resolve(preference: LocalePreference) -> Self {
        if let Some((language, ..)) = LANGUAGES.iter().find(|(.., choice)| *choice == preference) {
            return *language;
        }
        match preference {
            LocalePreference::Auto => {
                Self::from_languages(&lhc_core::platform::locale::message_languages())
            }
            _ => Self::English,
        }
    }

    fn from_languages(languages: &[String]) -> Self {
        languages
            .iter()
            .find_map(|locale| Self::from_os_locale(locale))
            .unwrap_or_default()
    }
}

pub fn select_ui_language(code: &str) -> Result<(), String> {
    let language =
        Language::from_code(code).ok_or_else(|| format!("unknown UI language: {code}"))?;
    let translator: Option<Box<dyn i_slint_core::translations::Translator>> =
        (language != Language::English).then(|| {
            Box::new(CatalogTranslator(language)) as Box<dyn i_slint_core::translations::Translator>
        });
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

type CatalogEntry = (
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static [&'static str],
);

include!(concat!(env!("OUT_DIR"), "/rust_translations.rs"));

/// Lookups use the checked PO data directly. The MO parser in rspolib 0.1.2
/// reverses context and source, so it cannot preserve contextual translations.
struct CatalogTranslator(Language);

impl CatalogTranslator {
    fn entry(&self, context: Option<&str>, source: &str) -> Option<&'static CatalogEntry> {
        let key = (context.unwrap_or_default(), source);
        let entries = catalog(self.0);
        entries
            .binary_search_by_key(&key, |entry| (entry.0, entry.1))
            .ok()
            .map(|index| &entries[index])
    }
}

impl tr::Translator for CatalogTranslator {
    fn translate<'a>(
        &'a self,
        source: &'a str,
        context: Option<&'a str>,
    ) -> std::borrow::Cow<'a, str> {
        if let Some(entry) = self.entry(context, source)
            && entry.2.is_none()
        {
            return entry.3[0].into();
        }
        log::error!("missing singular translation: {context:?} {source:?}");
        source.into()
    }

    fn ntranslate<'a>(
        &'a self,
        n: u64,
        singular: &'a str,
        plural: &'a str,
        context: Option<&'a str>,
    ) -> std::borrow::Cow<'a, str> {
        if let Some(entry) = self.entry(context, singular)
            && entry.2 == Some(plural)
        {
            let form = plural_form(self.0, n);
            return entry.3[form].into();
        }
        log::error!("missing plural translation: {context:?} {singular:?} {plural:?}");
        if n == 1 { singular } else { plural }.into()
    }
}

/// Integer plural rules matching each bundled catalog's Plural-Forms header.
fn plural_form(language: Language, n: u64) -> usize {
    use Language::*;
    match language {
        ChineseSimplified | ChineseTraditional | Japanese | Korean => 0,
        Russian | Ukrainian | Croatian => match (n % 10, n % 100) {
            (1, hundred) if hundred != 11 => 0,
            (2..=4, hundred) if !(12..=14).contains(&hundred) => 1,
            _ => 2,
        },
        Polish => {
            if n == 1 {
                0
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }
        Czech => {
            if n == 1 {
                0
            } else if (2..=4).contains(&n) {
                1
            } else {
                2
            }
        }
        Romanian => {
            if n == 1 {
                0
            } else if n == 0 || (1..20).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }
        French | Turkish => usize::from(n > 1),
        _ => usize::from(n != 1),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    None,
    Copied,
    ScrollHorizontal,
    ScrollVertical,
    LayoutCreated(String),
    LayoutSaved(String),
    LayoutDeleted(String),
    AutoRuleDeleted,
    LayoutActivated(String),
    /// `activateLayoutN` found no N-th layout in the library.
    LayoutSlotEmpty(u32),
    LayoutDiscarded(String),
    MapperWaiting,
    MapperRunning(Option<String>),
    MapperStopped(Option<String>),
    MapperStarting,
    ConfigLoaded(usize),
    ConfigSaved(usize),
    ConfigUnavailable(String),
    ConfigEditConflict,
    ConfigIo,
    ConfigParse,
    ConfigInvalid,
    MapperFailed(String),
    DeviceSaved(String),
    LoadConfigFirst,
    SelectInputDevice,
    InputDiscoveryFailed(String),
    MapperUnsupported,
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
    /// Why the mapper cannot use the input devices.
    InputAccess(InputAccessIssue),
    GameModeAutoDisabled,
    GameModeAutoUnavailable,
    GameModeAutoInactive,
    GameModeDaemonActive,
    GameModeKnownGameActive(String),
    GameModeProcessActive(String),
    GameModeFullscreenActive,
    GameModeExcluded(String),
    GameModeWindowSkipped(String),
    GameModeManualOn,
    GameModeManualOff,
    LibrarySaved,
    LibrarySelect,
    LibraryChanged,
    LayerSaved,
    RuleSaved,
    LayoutEmpty,
    LayoutLoadFailed(String),
    /// Default name of a copy of the named item.
    CopyName(String),
    /// Menu actions run through the mapper, which is stopped.
    MapperRequired,
    CommandStarted(String),
    CommandHelpLinux,
    CommandHelpWindows,
    CommandHelpMacos,

    CommandDirectoryMissing,
    ScriptNotExecutable,
    ChooseCommandScript,
    ChooseCommandDirectory,
    ActionUnavailable,
    ActionUnavailableNamed(String),
    /// A `sys:` action macOS has no API for was ignored.
    ActionUnsupportedMacos,
    CommandsDisabled,
    SaveLayoutFirst,
    MenuIssue(lhc_core::profile::menus::MenuIssue),
    MacroIssue(lhc_core::profile::macros::MacroIssue),
    Name(NameKind, NameIssue),
    KeyCodeRequired,
    ActionRequired,
    PickerSuppress,
    PickerInherit,
    PickerNative,
    PickerNoAction,
    PickerEmptyCell,
    PickerLayerOnly,
    TimeoutInvalid,
    SettingsTimingInvalid,
    LongHoldInvalid,
    TextEmpty,
    DelayRange,
    SystemActionRequired,
    PickerAction(String, u32),
    BuiltinMacro(String),
    UnknownKind,
    InvalidAction(ActionIssue),
    Rule(RuleIssue),
    ActionFailed(String),
    /// A `text:` action was not typed.
    TextInputFailed(lhc_core::events::TextInputFailure),
    /// An operation failed; technical details are logged, never displayed.
    Error(String),
}

impl Msg {
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }

    fn diagnostic_key(&self) -> String {
        let detail = match self {
            Self::Error(detail)
            | Self::ActionFailed(detail)
            | Self::MapperFailed(detail)
            | Self::ConfigUnavailable(detail)
            | Self::InputDiscoveryFailed(detail)
            | Self::WorkerUnavailable(detail)
            | Self::WorkerError(detail)
            | Self::SavedMapperNotUpdated(detail)
            | Self::LayoutLoadFailed(detail)
            | Self::MapperRunning(Some(detail))
            | Self::MapperStopped(Some(detail)) => detail,
            Self::TextInputFailed(
                lhc_core::events::TextInputFailure::BackendUnavailable(detail)
                | lhc_core::events::TextInputFailure::Clipboard(detail)
                | lhc_core::events::TextInputFailure::Tool(detail),
            ) => detail,
            _ => return String::new(),
        };
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        detail.hash(&mut hash);
        format!("{:x}", hash.finish())
    }

    pub fn to_ui(&self) -> Message {
        let empty = String::new;
        let (id, arg, count) = match self {
            Self::None => ("", empty(), 0),
            Self::Copied => ("copied", empty(), 0),
            Self::ScrollHorizontal => ("scroll-horizontal", empty(), 0),
            Self::ScrollVertical => ("scroll-vertical", empty(), 0),
            Self::LayoutCreated(name) => ("layout-created", name.clone(), 0),
            Self::LayoutSaved(name) => ("layout-saved", name.clone(), 0),
            Self::AutoRuleDeleted => ("auto-rule-deleted", String::new(), 0),
            Self::LayoutDeleted(name) => ("layout-deleted", name.clone(), 0),
            Self::LayoutActivated(name) => ("layout-activated", name.clone(), 0),
            Self::LayoutSlotEmpty(n) => ("layout-slot-empty", empty(), *n as usize),
            Self::LayoutDiscarded(name) => ("layout-discarded", name.clone(), 0),
            Self::MapperWaiting => ("mapper-waiting", empty(), 0),
            Self::MapperRunning(error) => {
                if let Some(error) = error {
                    diagnostic("mapper-running-error", error)
                } else {
                    ("mapper-running", empty(), 0)
                }
            }
            Self::MapperStopped(error) => {
                if let Some(error) = error {
                    diagnostic("mapper-stopped-error", error)
                } else {
                    ("mapper-stopped", empty(), 0)
                }
            }
            Self::MapperStarting => ("mapper-starting", empty(), 0),
            Self::ConfigLoaded(rules) => ("config-loaded", empty(), *rules),
            Self::ConfigSaved(rules) => ("config-saved", empty(), *rules),
            Self::ConfigUnavailable(error) => diagnostic("config-unavailable", error),
            Self::ConfigIo => ("config-io", empty(), 0),
            Self::ConfigParse => ("config-parse", empty(), 0),
            Self::ConfigInvalid => ("config-invalid", empty(), 0),
            Self::MapperFailed(error) => diagnostic("mapper-failed", error),
            Self::ConfigEditConflict => ("config-edit-conflict", empty(), 0),
            Self::DeviceSaved(path) => ("device-saved", path.clone(), 0),
            Self::LoadConfigFirst => ("load-config-first", empty(), 0),
            Self::SelectInputDevice => ("select-input-device", empty(), 0),
            Self::InputDiscoveryFailed(error) => diagnostic("input-discovery-failed", error),
            Self::MapperUnsupported => ("mapper-unsupported", empty(), 0),
            Self::WorkerUnavailable(error) => diagnostic("worker-unavailable", error),
            Self::WorkerRestarting => ("worker-restarting", empty(), 0),
            Self::WorkerRestartLimit => ("worker-restart-limit", empty(), 0),
            Self::WorkerNotStarted => ("worker-not-started", empty(), 0),
            Self::WorkerError(error) => diagnostic("worker-error", error),
            Self::SavedMapperNotUpdated(error) => diagnostic(SAVED_MAPPER_NOT_UPDATED, error),
            Self::ActionSaved => ("action-saved", empty(), 0),
            Self::LibrarySaved => ("library-saved", empty(), 0),
            Self::LibrarySelect => ("library-select", empty(), 0),
            Self::LibraryChanged => ("library-changed", empty(), 0),
            Self::CapabilityAvailable => ("capability-available", empty(), 0),
            Self::CapabilityUnavailable => ("capability-unavailable", empty(), 0),
            Self::CapabilityUnsupported => ("capability-unsupported", empty(), 0),
            Self::InputAccess(issue) => (
                match issue {
                    InputAccessIssue::NoInputDevices => "input-access-no-devices",
                    InputAccessIssue::KeyboardMissing => "input-access-keyboard-missing",
                    InputAccessIssue::InputDenied => "input-access-input-denied",
                    InputAccessIssue::UinputMissing => "input-access-uinput-missing",
                    InputAccessIssue::UinputDenied => "input-access-uinput-denied",
                },
                empty(),
                0,
            ),
            Self::GameModeAutoDisabled => ("game-mode-auto-disabled", empty(), 0),
            Self::GameModeAutoUnavailable => ("game-mode-auto-unavailable", empty(), 0),
            Self::GameModeAutoInactive => ("game-mode-auto-inactive", empty(), 0),
            Self::GameModeDaemonActive => ("game-mode-daemon-active", empty(), 0),
            Self::GameModeKnownGameActive(name) => ("game-mode-known-game-active", name.clone(), 0),
            Self::GameModeProcessActive(name) => ("game-mode-process-active", name.clone(), 0),
            Self::GameModeFullscreenActive => ("game-mode-fullscreen-active", empty(), 0),
            Self::GameModeExcluded(name) => ("game-mode-excluded", name.clone(), 0),
            Self::GameModeWindowSkipped(name) => ("game-mode-window-skipped", name.clone(), 0),
            Self::GameModeManualOn => ("game-mode-manual-on", empty(), 0),
            Self::GameModeManualOff => ("game-mode-manual-off", empty(), 0),
            Self::SettingsSaved => ("settings-saved", empty(), 0),
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
            Self::CommandHelpLinux => ("command-help-linux", empty(), 0),
            Self::CommandHelpWindows => ("command-help-windows", empty(), 0),
            Self::CommandHelpMacos => ("command-help-macos", empty(), 0),
            Self::CommandStarted(name) => ("command-started", name.clone(), 0),
            Self::CommandDirectoryMissing => ("command-directory-missing", empty(), 0),
            Self::ScriptNotExecutable => ("script-not-executable", empty(), 0),
            Self::ChooseCommandScript => ("choose-command-script", empty(), 0),
            Self::ChooseCommandDirectory => ("choose-command-directory", empty(), 0),
            Self::ActionUnavailableNamed(action) => ("action-unavailable-named", action.clone(), 0),
            Self::ActionUnavailable => ("action-unavailable", empty(), 0),
            Self::ActionUnsupportedMacos => ("action-unsupported-macos", empty(), 0),
            Self::SaveLayoutFirst => ("save-layout-first", empty(), 0),
            Self::CommandsDisabled => ("commands-disabled", empty(), 0),
            Self::LayerSaved => ("layer-saved", empty(), 0),
            Self::RuleSaved => ("rule-saved", empty(), 0),
            Self::LayoutEmpty => ("layout-empty", empty(), 0),
            Self::LayoutLoadFailed(error) => diagnostic("layout-load-failed", error),
            Self::CopyName(name) => ("copy-name", name.clone(), 0),
            Self::MapperRequired => ("mapper-required", empty(), 0),
            Self::Name(kind, issue) => (
                match (kind, issue) {
                    (NameKind::Layer, NameIssue::Empty) => "layer-name-required",
                    (NameKind::Layer, NameIssue::Taken) => "layer-name-taken",
                    (NameKind::Layout, NameIssue::Empty) => "layout-name-required",
                    (NameKind::Layout, NameIssue::Taken) => "layout-name-taken",
                    (_, NameIssue::Invalid) => "name-invalid",
                },
                empty(),
                0,
            ),
            Self::ActionRequired => ("action-required", empty(), 0),
            Self::PickerSuppress => ("picker-suppress", empty(), 0),
            Self::PickerInherit => ("picker-inherit", empty(), 0),
            Self::PickerNative => ("picker-native", empty(), 0),
            Self::PickerEmptyCell => ("picker-empty-cell", empty(), 0),
            Self::PickerNoAction => ("picker-no-action", empty(), 0),
            Self::PickerLayerOnly => ("picker-layer-only", empty(), 0),
            Self::KeyCodeRequired => ("key-code-required", empty(), 0),
            Self::LongHoldInvalid => ("long-hold-invalid", empty(), 0),
            Self::SettingsTimingInvalid => ("settings-timing-invalid", empty(), 0),
            Self::TimeoutInvalid => ("timeout-invalid", empty(), 0),
            Self::TextEmpty => ("text-empty", empty(), 0),
            Self::DelayRange => ("delay-range", empty(), 0),
            Self::BuiltinMacro(id) => ("builtin-macro", id.clone(), 0),
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
                    ActionIssue::UnknownActionType => "action-unknown-type",
                },
                empty(),
                0,
            ),
            Self::Rule(issue) => (
                match issue.code {
                    RuleIssueCode::InvalidLayerKey => "config-invalid-layer-key",
                    RuleIssueCode::InvalidLayerAction => "config-invalid-layer-action",
                    RuleIssueCode::InvalidMacroStep => "config-invalid-macro-step",
                    RuleIssueCode::MacroCycle => "config-macro-cycle",
                    RuleIssueCode::InvalidMacro => "config-invalid-macro",
                    RuleIssueCode::InvalidMenuAction => "config-invalid-menu-action",
                    RuleIssueCode::InvalidTiming => "config-invalid-timing",
                    RuleIssueCode::InvalidCondition => "config-invalid-condition",
                    RuleIssueCode::InvalidRuleConfiguration => "config-invalid-hold",
                    RuleIssueCode::MissingTrigger => "rule-missing-trigger",
                    RuleIssueCode::InvalidTrigger => "rule-invalid-trigger",
                    RuleIssueCode::DuplicateTrigger => "rule-duplicate-trigger",
                    RuleIssueCode::ShadowedTrigger => "rule-shadowed-trigger",
                    RuleIssueCode::UnknownLayer => "rule-unknown-layer",
                    RuleIssueCode::InvalidTapAction => "rule-invalid-tap",
                    RuleIssueCode::InvalidHoldAction => "rule-invalid-hold",
                    RuleIssueCode::InvalidDoubleTapAction => "rule-invalid-double-tap",
                },
                issue.trigger.clone().unwrap_or_default(),
                0,
            ),
            Self::ActionFailed(error) => diagnostic("action-failed", error),
            Self::TextInputFailed(failure) => {
                use lhc_core::events::TextInputFailure as F;
                match failure {
                    F::MissingCharacters => ("text-missing-characters", empty(), 0),
                    F::DestinationChanged => ("text-destination-changed", empty(), 0),
                    F::LayoutUnknown => ("text-layout-unknown", empty(), 0),
                    F::BackendUnavailable(detail) => diagnostic("text-backend-unavailable", detail),
                    F::Clipboard(detail) => diagnostic("text-clipboard-failed", detail),
                    F::Tool(detail) => diagnostic("text-tool-failed", detail),
                }
            }
            Self::Error(error) => diagnostic("error", error),
        };
        Message {
            id: id.into(),
            arg: arg.into(),
            count: i32::try_from(count).unwrap_or(i32::MAX),
            diagnostic_key: self.diagnostic_key().into(),
        }
    }
}

/// Id of the warning cleared once the mapper picks up the saved config.
pub const SAVED_MAPPER_NOT_UPDATED: &str = "saved-mapper-not-updated";

fn diagnostic(id: &'static str, detail: &str) -> (&'static str, String, usize) {
    log::error!("{id}: {detail}");
    (id, String::new(), 0)
}

impl From<&ConfigError> for Msg {
    fn from(error: &ConfigError) -> Self {
        match error {
            ConfigError::EditConflict => Self::ConfigEditConflict,
            ConfigError::Menu(issue) => Self::MenuIssue(*issue),
            ConfigError::Macro(issue) => Self::MacroIssue(*issue),
            ConfigError::Name(kind, issue) => Self::Name(*kind, *issue),
            ConfigError::InvalidAction(issue) => Self::InvalidAction(*issue),
            ConfigError::Rules(issues) if !issues.is_empty() => Self::Rule(issues[0].clone()),
            ConfigError::Io(detail) => {
                log::error!("configuration I/O: {detail}");
                Self::ConfigIo
            }
            ConfigError::Parse(detail) => {
                log::error!("configuration parsing: {detail}");
                Self::ConfigParse
            }
            ConfigError::Invalid(detail) => {
                log::error!("configuration validation: {detail}");
                Self::ConfigInvalid
            }
            ConfigError::Rules(_) => Self::ConfigInvalid,
        }
    }
}

/// Tray menu entries, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayItem {
    Settings,
    Quick,
    ToggleMapper,
    Quit,
}

impl TrayItem {
    pub const ALL: [TrayItem; 4] = [
        TrayItem::Settings,
        TrayItem::Quick,
        TrayItem::ToggleMapper,
        TrayItem::Quit,
    ];

    pub fn label(self, language: Language) -> String {
        match self {
            Self::Settings => RustText::Settings,
            Self::Quick => RustText::Quick,
            Self::ToggleMapper => RustText::ToggleMapper,
            Self::Quit => RustText::Quit,
        }
        .text(language)
        .to_owned()
    }

    pub fn command(self) -> crate::command::Command {
        use crate::command::{Command, Window};
        match self {
            Self::Settings => Command::Show(Window::Settings),
            Self::Quick => Command::Show(Window::QUICK),
            Self::ToggleMapper => Command::ToggleMapper,
            Self::Quit => Command::Quit,
        }
    }
}

/// Application title shown by native tray APIs.
pub fn application_name(language: Language) -> String {
    RustText::ApplicationName.text(language).to_owned()
}

/// Game-mode choices of the tray submenu, in display order.
pub const TRAY_GAME_MODES: [crate::command::GameMode; 3] = [
    crate::command::GameMode::Auto,
    crate::command::GameMode::On,
    crate::command::GameMode::Off,
];

/// Title of the tray game-mode submenu: the effective state.
pub fn tray_game_title(language: Language, active: bool) -> String {
    if active {
        RustText::GameOn
    } else {
        RustText::GameOff
    }
    .text(language)
    .to_owned()
}

/// Label of one tray game-mode choice.
pub fn tray_game_choice(language: Language, mode: crate::command::GameMode) -> String {
    use crate::command::GameMode;
    match mode {
        GameMode::Auto => RustText::GameAuto,
        GameMode::On => RustText::GameAlways,
        GameMode::Off | GameMode::Toggle => RustText::GameNever,
    }
    .text(language)
    .to_owned()
}

/// On-screen text announcing the active layout; `None` is no layout.
pub fn layout_osd(language: Language, name: Option<&str>) -> String {
    match name {
        Some(name) => {
            let template = RustText::Layout.text(language);
            template.replace(if template.contains("{}") { "{}" } else { "{0}" }, name)
        }
        None => RustText::NoLayout.text(language).to_owned(),
    }
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

        let translator = CatalogTranslator(Language::Russian);
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
    fn runtime_catalog_contains_every_translation() {
        use tr::Translator;
        let translator = CatalogTranslator(Language::Russian);
        for &(context, source, plural, translated) in catalog(Language::Russian) {
            let context = (!context.is_empty()).then_some(context);
            if let Some(plural) = plural {
                for (count, form) in [(0, 2), (1, 0), (2, 1), (5, 2), (11, 2), (21, 0)] {
                    assert_eq!(
                        translator.ntranslate(count, source, plural, context),
                        translated[form],
                        "{source} ({count})"
                    );
                }
            } else {
                assert_eq!(
                    translator.translate(source, context),
                    translated[0],
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn contextual_translations_and_missing_keys_use_readable_fallbacks() {
        use tr::Translator;
        let translator = CatalogTranslator(Language::Russian);
        assert_eq!(
            translator.translate("Auto", Some("Game mode control")),
            "Авто"
        );
        assert_eq!(translator.translate("On", Some("Game mode control")), "Вкл");
        assert_eq!(
            translator.translate("not in the catalog", None),
            "not in the catalog"
        );
        assert_eq!(
            translator.translate("Auto", Some("unknown context")),
            "Auto"
        );
        for (n, expected) in [(0, "unknowns"), (1, "unknown"), (2, "unknowns")] {
            assert_eq!(
                translator.ntranslate(n, "unknown", "unknowns", None),
                expected
            );
        }
    }

    #[test]
    fn explicit_language_preferences_and_codes_are_exact() {
        assert_eq!(
            Language::resolve(LocalePreference::Russian),
            Language::Russian
        );
        assert_eq!(
            Language::resolve(LocalePreference::English),
            Language::English
        );
        for language in [Language::Russian, Language::English] {
            assert_eq!(Language::from_code(language.code()), Some(language));
        }
        assert_eq!(Language::from_code("de"), Some(Language::German));
        assert_eq!(Language::from_code("xx"), None);
    }

    #[test]
    fn os_locales_resolve_regions_scripts_and_fallbacks() {
        for (locale, expected) in [
            ("de_DE.UTF-8", Language::German),
            ("es_AR.UTF-8", Language::Spanish),
            ("pt_BR", Language::Portuguese),
            ("uk_UA", Language::Ukrainian),
            ("ka_GE", Language::Georgian),
            ("kk_KZ", Language::Kazakh),
            ("zh_CN.UTF-8", Language::ChineseSimplified),
            ("zh_SG", Language::ChineseSimplified),
            ("zh-Hans", Language::ChineseSimplified),
            ("zh-Hans-TW", Language::ChineseSimplified),
            ("zh_TW", Language::ChineseTraditional),
            ("zh_HK.UTF-8", Language::ChineseTraditional),
            ("zh-Hant", Language::ChineseTraditional),
            ("ja_JP", Language::Japanese),
            ("ko_KR", Language::Korean),
            ("C.UTF-8", Language::English),
            ("unknown", Language::English),
        ] {
            assert_eq!(
                Language::from_os_locale(locale).unwrap_or_default(),
                expected,
                "{locale}"
            );
        }
    }

    #[test]
    fn language_preferences_skip_unsupported_languages_and_respect_english() {
        for (locales, expected) in [
            (vec!["xx", "ru", "en"], Language::Russian),
            (vec!["xx", "en", "ru"], Language::English),
            (vec!["", "zh_HK.UTF-8"], Language::ChineseTraditional),
            (vec!["xx"], Language::English),
            (vec![], Language::English),
        ] {
            assert_eq!(
                Language::from_languages(
                    &locales.into_iter().map(String::from).collect::<Vec<_>>()
                ),
                expected
            );
        }
    }

    #[test]
    fn plural_rules_cover_language_specific_boundaries() {
        for (language, cases) in [
            (
                Language::Polish,
                &[(0, 2), (1, 0), (2, 1), (5, 2), (12, 2), (21, 2), (22, 1)][..],
            ),
            (
                Language::Czech,
                &[(0, 2), (1, 0), (2, 1), (4, 1), (5, 2), (21, 2)][..],
            ),
            (
                Language::Romanian,
                &[(0, 1), (1, 0), (2, 1), (19, 1), (20, 2), (100, 2), (101, 1)][..],
            ),
            (
                Language::Ukrainian,
                &[(0, 2), (1, 0), (2, 1), (11, 2), (21, 0), (22, 1)][..],
            ),
            (Language::Croatian, &[(1, 0), (3, 1), (13, 2), (21, 0)][..]),
            (Language::French, &[(0, 0), (1, 0), (2, 1)][..]),
            (Language::Portuguese, &[(0, 1), (1, 0), (2, 1)][..]),
            (Language::ChineseTraditional, &[(0, 0), (1, 0), (2, 0)][..]),
        ] {
            for &(n, expected) in cases {
                assert_eq!(
                    plural_form(language, n),
                    expected,
                    "{}: {n}",
                    language.code()
                );
            }
        }
    }

    #[test]
    fn all_languages_roundtrip_preferences_ipc_and_plural_catalogs() {
        use tr::Translator;
        for preference in LocalePreference::ALL.into_iter().skip(1) {
            let serialized = serde_json::to_string(&preference).unwrap();
            assert_eq!(
                serde_json::from_str::<LocalePreference>(&serialized).unwrap(),
                preference
            );
            let language = Language::resolve(preference);
            assert_eq!(Language::from_code(language.code()), Some(language));
            let wire = format!("preferences dark {}", language.code());
            assert_eq!(
                crate::command::Command::parse(&wire).unwrap().to_string(),
                wire
            );
            if language == Language::English {
                continue;
            }
            let translator = CatalogTranslator(language);
            assert_eq!(catalog(language).len(), catalog(Language::Russian).len());
            for &(context, source, plural, translated) in catalog(language) {
                let context = (!context.is_empty()).then_some(context);
                if let Some(plural) = plural {
                    for n in [0, 1, 2, 3, 4, 5, 11, 12, 14, 21, 22, 100, 101, u64::MAX] {
                        let form = plural_form(language, n);
                        assert!(
                            form < translated.len(),
                            "{}: {source} ({n})",
                            language.code()
                        );
                        assert_eq!(
                            translator.ntranslate(n, source, plural, context),
                            translated[form]
                        );
                    }
                } else {
                    assert_eq!(translator.translate(source, context), translated[0]);
                }
            }
        }
    }

    #[test]
    fn rust_text_uses_the_catalog_in_both_languages() {
        assert_eq!(TrayItem::Quit.label(Language::Russian), "Выход");
        assert_eq!(TrayItem::Quit.label(Language::English), "Quit");
        assert_eq!(
            layout_osd(Language::Russian, Some("Main")),
            "Раскладка: Main"
        );
        assert_eq!(layout_osd(Language::English, Some("Main")), "Layout: Main");
        assert_eq!(layout_osd(Language::Russian, None), "Без раскладки");
    }

    #[test]
    fn technical_errors_are_never_exposed_as_ui_arguments() {
        for message in [
            Msg::Error("untranslated error".into()),
            Msg::ActionFailed("untranslated error".into()),
            Msg::MapperFailed("untranslated error".into()),
            Msg::MapperStopped(Some("untranslated error".into())),
            Msg::TextInputFailed(lhc_core::events::TextInputFailure::Tool(
                "untranslated error".into(),
            )),
        ] {
            assert!(message.to_ui().arg.is_empty());
        }
        assert_eq!(
            Msg::from(&ConfigError::Parse("details".into())),
            Msg::ConfigParse
        );
    }
}
