//! Typed shell commands and their IPC wire format.
//!
//! The wire format stays plain text (`show emoji`, `preferences dark ru`, …)
//! because the CLI, benchmark scripts and the Spell worker all speak it.

use std::{fmt, sync::Arc, time::Instant};

/// Thread-safe entry point that forwards a command to the UI thread.
pub type Dispatch = Arc<dyn Fn(Command, Source, Instant, Option<String>) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Popup {
    Emoji,
    Quick,
}

impl Popup {
    pub const ALL: [Popup; 2] = [Popup::Emoji, Popup::Quick];

    pub fn name(self) -> &'static str {
        match self {
            Self::Emoji => "emoji",
            Self::Quick => "quick",
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::Emoji => Self::Quick,
            Self::Quick => Self::Emoji,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Window {
    Settings,
    Popup(Popup),
}

impl Window {
    pub const EMOJI: Window = Window::Popup(Popup::Emoji);
    pub const QUICK: Window = Window::Popup(Popup::Quick);
    pub const ALL: [Window; 3] = [Window::Settings, Window::EMOJI, Window::QUICK];

    /// Stable name used on the wire and as the metrics window label.
    pub fn name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Popup(popup) => popup.name(),
        }
    }

    pub fn popup(self) -> Option<Popup> {
        match self {
            Self::Settings => None,
            Self::Popup(popup) => Some(popup),
        }
    }
}

/// What triggered a command; recorded in metrics and used for focus rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Button,
    Tray,
    Evdev,
    Hotkey,
    Ipc,
    Mapper,
    Diagnostic,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Tray => "tray",
            Self::Evdev => "evdev",
            Self::Hotkey => "hotkey",
            Self::Ipc => "ipc",
            Self::Mapper => "mapper",
            Self::Diagnostic => "diagnostic",
        }
    }

    /// Sources forwarded over IPC; anything unknown counts as external IPC.
    pub fn from_wire(value: Option<&str>) -> Self {
        match value {
            Some("button") => Self::Button,
            Some("tray") => Self::Tray,
            Some("evdev") => Self::Evdev,
            Some("hotkey") => Self::Hotkey,
            Some("mapper") => Self::Mapper,
            _ => Self::Ipc,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub dark: bool,
    pub english: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            dark: true,
            english: false,
        }
    }
}

impl Preferences {
    pub fn language(self) -> &'static str {
        if self.english { "en" } else { "ru" }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Show(Window),
    Hide,
    ToggleSettings,
    ToggleMapper,
    Quit,
    Ping,
    Preferences(Preferences),
}

pub const USAGE: &str =
    "usage: slint-shell [show emoji|quick|settings | hide | toggle-mapper | ping | quit]";

impl Command {
    pub fn parse(value: &str) -> Result<Self, String> {
        let words: Vec<&str> = value.split_whitespace().collect();
        let command = match words.as_slice() {
            ["show", "emoji"] => Self::Show(Window::EMOJI),
            ["show", "quick"] => Self::Show(Window::QUICK),
            ["show", "settings"] => Self::Show(Window::Settings),
            ["hide"] => Self::Hide,
            ["toggle-settings"] => Self::ToggleSettings,
            ["toggle-mapper"] => Self::ToggleMapper,
            ["quit"] => Self::Quit,
            ["ping"] => Self::Ping,
            [
                "preferences",
                theme @ ("dark" | "light"),
                language @ ("ru" | "en"),
            ] => Self::Preferences(Preferences {
                dark: *theme == "dark",
                english: *language == "en",
            }),
            _ => return Err(USAGE.into()),
        };
        Ok(command)
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Show(window) => write!(f, "show {}", window.name()),
            Self::Hide => f.write_str("hide"),
            Self::ToggleSettings => f.write_str("toggle-settings"),
            Self::ToggleMapper => f.write_str("toggle-mapper"),
            Self::Quit => f.write_str("quit"),
            Self::Ping => f.write_str("ping"),
            Self::Preferences(preferences) => write!(
                f,
                "preferences {} {}",
                if preferences.dark { "dark" } else { "light" },
                preferences.language()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_round_trips() {
        let commands = [
            Command::Show(Window::EMOJI),
            Command::Show(Window::QUICK),
            Command::Show(Window::Settings),
            Command::Hide,
            Command::ToggleSettings,
            Command::ToggleMapper,
            Command::Quit,
            Command::Ping,
            Command::Preferences(Preferences {
                dark: false,
                english: true,
            }),
            Command::Preferences(Preferences::default()),
        ];
        for command in commands {
            assert_eq!(Command::parse(&command.to_string()), Ok(command));
        }
    }

    #[test]
    fn legacy_wire_strings_stay_supported() {
        assert_eq!(
            Command::parse("preferences light ru"),
            Ok(Command::Preferences(Preferences {
                dark: false,
                english: false
            }))
        );
        assert!(Command::parse("show nothing").is_err());
        assert!(Command::parse("preferences dim ru").is_err());
    }

    #[test]
    fn unknown_sources_are_external_ipc() {
        assert_eq!(Source::from_wire(Some("tray")), Source::Tray);
        assert_eq!(Source::from_wire(Some("script")), Source::Ipc);
        assert_eq!(Source::from_wire(None), Source::Ipc);
    }
}
