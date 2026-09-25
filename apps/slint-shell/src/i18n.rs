//! User-visible messages produced by Rust.
//!
//! Rust never formats UI text itself: it sends a message id with arguments
//! and `Locale.text()` in `ui/i18n.slint` translates it through `@tr`.
//! The tray menu lives outside Slint, so its labels are kept here.

use crate::ui::Message;
use lhc_core::config_document::ConfigError;

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
    DeviceSaved(String),
    LoadConfigFirst,
    SelectInputDevice,
    WorkerUnavailable(String),
    WorkerRestarting,
    WorkerRestartLimit,
    WorkerNotStarted,
    WorkerError(String),
    ActionSaved,
    SavedMapperNotUpdated(String),
    TextEmpty,
    DelayRange,
    SystemActionRequired,
    ShortcutRequired,
    UnknownKind,
    /// Untranslated detail from the core or the OS.
    Error(String),
}

impl Msg {
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }

    pub fn to_ui(&self) -> Message {
        let (id, arg, count) = match self {
            Self::None => ("", String::new(), 0),
            Self::MapperRunning(error) => ("mapper-running", error.clone().unwrap_or_default(), 0),
            Self::MapperStopped(error) => ("mapper-stopped", error.clone().unwrap_or_default(), 0),
            Self::MapperStarting => ("mapper-starting", String::new(), 0),
            Self::ConfigLoaded(rules) => ("config-loaded", String::new(), *rules),
            Self::ConfigSaved(rules) => ("config-saved", String::new(), *rules),
            Self::ConfigReloaded(rules) => ("config-reloaded", String::new(), *rules),
            Self::ConfigUnavailable(error) => ("config-unavailable", error.clone(), 0),
            Self::DeviceSaved(path) => ("device-saved", path.clone(), 0),
            Self::LoadConfigFirst => ("load-config-first", String::new(), 0),
            Self::SelectInputDevice => ("select-input-device", String::new(), 0),
            Self::WorkerUnavailable(error) => ("worker-unavailable", error.clone(), 0),
            Self::WorkerRestarting => ("worker-restarting", String::new(), 0),
            Self::WorkerRestartLimit => ("worker-restart-limit", String::new(), 0),
            Self::WorkerNotStarted => ("worker-not-started", String::new(), 0),
            Self::WorkerError(error) => ("worker-error", error.clone(), 0),
            Self::ActionSaved => ("action-saved", String::new(), 0),
            Self::SavedMapperNotUpdated(error) => ("saved-mapper-not-updated", error.clone(), 0),
            Self::TextEmpty => ("text-empty", String::new(), 0),
            Self::DelayRange => ("delay-range", String::new(), 0),
            Self::SystemActionRequired => ("system-action-required", String::new(), 0),
            Self::ShortcutRequired => ("shortcut-required", String::new(), 0),
            Self::UnknownKind => ("unknown-kind", String::new(), 0),
            Self::Error(error) => ("error", error.clone(), 0),
        };
        Message {
            id: id.into(),
            arg: arg.into(),
            count: i32::try_from(count).unwrap_or(i32::MAX),
        }
    }
}

impl From<&ConfigError> for Message {
    fn from(error: &ConfigError) -> Self {
        let (id, arg) = match error {
            ConfigError::ExternalChange => ("config-external-change", String::new()),
            ConfigError::ConditionalRules { key } => ("config-conditional", key.clone()),
            other => ("error", other.to_string()),
        };
        Message {
            id: id.into(),
            arg: arg.into(),
            count: 0,
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

    pub fn label(self, english: bool) -> &'static str {
        match (self, english) {
            (Self::Settings, false) => "Настройки",
            (Self::Settings, true) => "Settings",
            (Self::Emoji, false) => "Эмодзи",
            (Self::Emoji, true) => "Emoji",
            (Self::Quick, false) => "Быстрые действия",
            (Self::Quick, true) => "Quick actions",
            (Self::ToggleMapper, false) => "Mapper вкл / выкл",
            (Self::ToggleMapper, true) => "Mapper on / off",
            (Self::Quit, false) => "Выход",
            (Self::Quit, true) => "Quit",
        }
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
