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
use std::sync::OnceLock;

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
    LibrarySaved,
    LibrarySelect,
    LibraryChanged,
    ProcessNameRequired,
    LayerSaved,
    MenuSaveFirst,
    MenuSaved,
    MenuIssue(lhc_core::profile::menus::MenuIssue),
    MacroSaved,
    MacroIssue(lhc_core::profile::macros::MacroIssue),
    MacroDraftChanged,
    LayerNameRequired,
    KeyCodeRequired,
    TimeoutInvalid,
    TextEmpty,
    DelayRange,
    SystemActionRequired,
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
            Self::MenuSaveFirst => ("menu-save-first", empty(), 0),
            Self::MenuSaved => ("menu-saved", String::new(), 0),
            Self::MacroSaved => ("macro-saved", empty(), 0),
            Self::MacroDraftChanged => ("macro-draft-changed", empty(), 0),
            Self::LayerSaved => ("layer-saved", empty(), 0),
            Self::LayerNameRequired => ("layer-name-required", empty(), 0),
            Self::KeyCodeRequired => ("key-code-required", empty(), 0),
            Self::TimeoutInvalid => ("timeout-invalid", empty(), 0),
            Self::TextEmpty => ("text-empty", empty(), 0),
            Self::DelayRange => ("delay-range", empty(), 0),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
