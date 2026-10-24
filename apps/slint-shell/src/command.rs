//! Typed shell commands and their IPC wire format.
//!
//! The wire format stays plain text (`show emoji`, `preferences dark ru`, …)
//! because the CLI, benchmark scripts and the Spell worker all speak it.

use crate::i18n::Language;
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
    /// Explicit mapper actions and IPC remain usable while gaming.
    pub(crate) fn popup_blocked(self, game_active: bool) -> bool {
        game_active && matches!(self, Self::Hotkey | Self::Evdev)
    }

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

/// Theme of every window; `System` follows the desktop color scheme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
    EInk,
}

impl ThemeMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::EInk => "eink",
        }
    }
}

/// Game-mode control from the tray, the CLI or a key binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Auto,
    On,
    Off,
    /// Override to the opposite of the current effective state.
    Toggle,
}

impl GameMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
        }
    }
}

/// Resolved appearance shared with the Spell worker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub theme: ThemeMode,
    pub language: Language,
    pub high_contrast: bool,
    pub reduce_motion: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Show(Window),
    ShowPage(Popup, u32),
    PopupLayout(Option<String>),
    PopupContents(String),
    /// Show the keys of the held layer in the Spell worker; `None` hides them.
    LayerHint(Option<crate::layer_hint::LayerHint>),
    Hide,
    ToggleSettings,
    ToggleMapper,
    GameMode(GameMode),
    Quit,
    Ping,
    Preferences(Preferences),
    /// Run an action (`text:…`, `Ctrl+KeyC`, `macro:id`, …) in the focused
    /// window; the Spell worker sends this after a popup selection.
    Execute(String),
}

pub const USAGE: &str = "usage: left-hand-control [--version | --help | show emoji|quick [page] | show settings | hide | toggle-mapper | \
game auto|on|off|toggle | preferences system|light|dark en|ru | execute <action> | ping | quit]";

impl Command {
    pub fn from_app_action(name: &str) -> Option<Self> {
        match name {
            // Without a page the popups keep the page they showed last.
            "show_quick_menu_last" => return Some(Self::Show(Window::QUICK)),
            "show_emoji_menu_last" => return Some(Self::Show(Window::EMOJI)),
            "show_settings" => return Some(Self::Show(Window::Settings)),
            _ => {}
        }
        let game = match name {
            "game_mode_toggle" => Some(GameMode::Toggle),
            "game_mode_auto" => Some(GameMode::Auto),
            "game_mode_on" => Some(GameMode::On),
            "game_mode_off" => Some(GameMode::Off),
            _ => None,
        };
        if let Some(game) = game {
            return Some(Self::GameMode(game));
        }
        let (popup, suffix) = if let Some(suffix) = name.strip_prefix("show_emoji_menu_") {
            (Popup::Emoji, suffix)
        } else {
            (Popup::Quick, name.strip_prefix("show_quick_menu_")?)
        };
        let page = suffix.parse::<u32>().ok().filter(|page| *page >= 1)?;
        Some(Self::ShowPage(popup, page))
    }

    /// Parse command-line arguments. `execute` takes the rest of the line
    /// verbatim, so an action may contain spaces.
    pub fn parse_args(args: &[String]) -> Result<Self, String> {
        match args {
            [first, rest @ ..] if first == "execute" && !rest.is_empty() => {
                Ok(Self::Execute(rest.join(" ")))
            }
            _ => Self::parse(&args.join(" ")),
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        if let Some(contents) = value.strip_prefix("popup-contents ") {
            return serde_json::from_str(contents)
                .map(Self::PopupContents)
                .map_err(|_| USAGE.into());
        }
        if let Some(layout) = value.strip_prefix("popup-layout ") {
            return serde_json::from_str(layout)
                .map(Self::PopupLayout)
                .map_err(|_| USAGE.into());
        }
        if let Some(hint) = value.strip_prefix("layer-hint ") {
            return serde_json::from_str(hint)
                .map(Self::LayerHint)
                .map_err(|_| USAGE.into());
        }
        if let Some(action) = value.strip_prefix("execute ") {
            return Ok(Self::Execute(action.into()));
        }
        let words: Vec<&str> = value.split_whitespace().collect();
        let command = match words.as_slice() {
            ["show", popup @ ("emoji" | "quick"), page] => {
                let page = page
                    .parse::<u32>()
                    .ok()
                    .filter(|page| *page >= 1)
                    .ok_or(USAGE)?;
                Self::ShowPage(
                    if *popup == "emoji" {
                        Popup::Emoji
                    } else {
                        Popup::Quick
                    },
                    page,
                )
            }
            ["show", "emoji"] => Self::Show(Window::EMOJI),
            ["show", "quick"] => Self::Show(Window::QUICK),
            ["show", "settings"] => Self::Show(Window::Settings),
            ["hide"] => Self::Hide,
            ["toggle-settings"] => Self::ToggleSettings,
            ["toggle-mapper"] => Self::ToggleMapper,
            ["game", mode] => Self::GameMode(match *mode {
                "auto" => GameMode::Auto,
                "on" => GameMode::On,
                "off" => GameMode::Off,
                "toggle" => GameMode::Toggle,
                _ => return Err(USAGE.into()),
            }),
            ["quit"] => Self::Quit,
            ["ping"] => Self::Ping,
            ["preferences", theme, language, flags @ ..] => {
                if flags
                    .iter()
                    .any(|flag| !matches!(*flag, "high-contrast" | "reduce-motion"))
                {
                    return Err(USAGE.into());
                }
                Self::Preferences(Preferences {
                    theme: match *theme {
                        "system" => ThemeMode::System,
                        "light" => ThemeMode::Light,
                        "dark" => ThemeMode::Dark,
                        "eink" => ThemeMode::EInk,
                        _ => return Err(USAGE.into()),
                    },
                    language: Language::from_code(language).ok_or(USAGE)?,
                    high_contrast: flags.contains(&"high-contrast"),
                    reduce_motion: flags.contains(&"reduce-motion"),
                })
            }
            _ => return Err(USAGE.into()),
        };
        Ok(command)
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Show(window) => write!(f, "show {}", window.name()),
            Self::ShowPage(popup, page) => write!(f, "show {} {page}", popup.name()),
            Self::PopupLayout(layout) => write!(
                f,
                "popup-layout {}",
                serde_json::to_string(layout).map_err(|_| fmt::Error)?
            ),
            Self::PopupContents(contents) => write!(
                f,
                "popup-contents {}",
                serde_json::to_string(contents).map_err(|_| fmt::Error)?
            ),
            Self::LayerHint(hint) => write!(
                f,
                "layer-hint {}",
                serde_json::to_string(hint).map_err(|_| fmt::Error)?
            ),
            Self::Hide => f.write_str("hide"),
            Self::ToggleSettings => f.write_str("toggle-settings"),
            Self::ToggleMapper => f.write_str("toggle-mapper"),
            Self::GameMode(mode) => write!(f, "game {}", mode.name()),
            Self::Quit => f.write_str("quit"),
            Self::Ping => f.write_str("ping"),
            Self::Preferences(preferences) => {
                write!(
                    f,
                    "preferences {} {}",
                    preferences.theme.name(),
                    preferences.language.code()
                )?;
                if preferences.high_contrast {
                    f.write_str(" high-contrast")?;
                }
                if preferences.reduce_motion {
                    f.write_str(" reduce-motion")?;
                }
                Ok(())
            }
            Self::Execute(action) => write!(f, "execute {action}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_mode_blocks_shortcuts_but_preserves_explicit_popup_actions() {
        for source in [Source::Hotkey, Source::Evdev] {
            assert!(!source.popup_blocked(false));
            assert!(source.popup_blocked(true));
        }
        for source in [Source::Mapper, Source::Ipc, Source::Tray, Source::Button] {
            assert!(!source.popup_blocked(true));
        }
    }

    #[test]
    fn wire_format_round_trips() {
        let commands = [
            Command::Show(Window::EMOJI),
            Command::Show(Window::QUICK),
            Command::Show(Window::Settings),
            Command::PopupLayout(Some("user:Привет".into())),
            Command::PopupLayout(None),
            Command::PopupContents(r#"{"rules": [], "description": "Привет"}"#.into()),
            Command::LayerHint(None),
            Command::LayerHint(Some(crate::layer_hint::LayerHint {
                name: "Навигация".into(),
                left: vec![
                    None,
                    Some(crate::layer_hint::HintKey {
                        label: "Q".into(),
                        kind: crate::layer_hint::HintKind::Action,
                        action: "text: 👋".into(),
                        emoji: false,
                    }),
                ],
                right: vec![Some(crate::layer_hint::HintKey {
                    label: "H".into(),
                    kind: crate::layer_hint::HintKind::Suppress,
                    action: String::new(),
                    emoji: false,
                })],
            })),
            Command::ShowPage(Popup::Quick, 3),
            Command::ShowPage(Popup::Emoji, 5),
            Command::Hide,
            Command::ToggleSettings,
            Command::ToggleMapper,
            Command::GameMode(GameMode::Auto),
            Command::GameMode(GameMode::On),
            Command::GameMode(GameMode::Off),
            Command::GameMode(GameMode::Toggle),
            Command::Quit,
            Command::Ping,
            Command::Preferences(Preferences {
                theme: ThemeMode::Light,
                language: Language::English,
                ..Preferences::default()
            }),
            Command::Preferences(Preferences {
                theme: ThemeMode::EInk,
                language: Language::Russian,
                ..Preferences::default()
            }),
            Command::Preferences(Preferences {
                theme: ThemeMode::Dark,
                language: Language::English,
                high_contrast: true,
                reduce_motion: true,
            }),
            Command::Preferences(Preferences::default()),
            Command::Execute("text:  Привет 👋 ".into()),
            Command::Execute("Ctrl+KeyC".into()),
        ];
        for command in commands {
            assert_eq!(Command::parse(&command.to_string()), Ok(command));
        }
    }

    #[test]
    fn mapper_actions_preserve_pages_and_reject_invalid_actions() {
        for page in [1, 5, 10, 42] {
            assert_eq!(
                Command::from_app_action(&format!("show_quick_menu_{page}")),
                Some(Command::ShowPage(Popup::Quick, page))
            );
            assert_eq!(
                Command::from_app_action(&format!("show_emoji_menu_{page}")),
                Some(Command::ShowPage(Popup::Emoji, page))
            );
        }
        assert_eq!(
            Command::from_app_action("game_mode_toggle"),
            Some(Command::GameMode(GameMode::Toggle))
        );
        assert_eq!(
            Command::from_app_action("show_emoji_menu_last"),
            Some(Command::Show(Window::EMOJI))
        );
        assert_eq!(
            Command::from_app_action("show_settings"),
            Some(Command::Show(Window::Settings))
        );
        for invalid in [
            "show_quick_menu_0",
            "show_emoji_menu_-1",
            "show_emoji_menu_bad",
            "activate_layout_1",
            "quit",
        ] {
            assert!(Command::from_app_action(invalid).is_none());
        }
    }

    #[test]
    fn wire_strings_are_validated() {
        assert_eq!(
            Command::parse("preferences light ru"),
            Ok(Command::Preferences(Preferences {
                theme: ThemeMode::Light,
                language: Language::Russian,
                ..Preferences::default()
            }))
        );
        assert!(Command::parse("preferences light ru bold").is_err());
        assert!(Command::parse("show nothing").is_err());
        assert!(Command::parse("game maybe").is_err());
        assert!(Command::parse("preferences dim ru").is_err());
    }

    #[test]
    fn command_line_arguments() {
        let args = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<Vec<_>>();
        assert_eq!(
            Command::parse_args(&args(&["execute", "text:a  b"])),
            Ok(Command::Execute("text:a  b".into()))
        );
        assert_eq!(
            Command::parse_args(&args(&["show", "quick", "2"])),
            Ok(Command::ShowPage(Popup::Quick, 2))
        );
        assert!(Command::parse_args(&args(&["execute"])).is_err());
    }

    #[test]
    fn unknown_sources_are_external_ipc() {
        assert_eq!(Source::from_wire(Some("tray")), Source::Tray);
        assert_eq!(Source::from_wire(Some("script")), Source::Ipc);
        assert_eq!(Source::from_wire(None), Source::Ipc);
    }
}
