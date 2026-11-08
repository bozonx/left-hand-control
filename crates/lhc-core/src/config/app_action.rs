//! Typed application requests emitted by mapper actions.

use super::actions::{ActionName, app_action_name};
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Menu {
    Quick,
    Emoji,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameModeRequest {
    Auto,
    On,
    Off,
    Toggle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAction {
    ShowMenu {
        menu: Menu,
        page: Option<NonZeroU32>,
    },
    ShowSettings,
    ActivateProfile(NonZeroU32),
    GameMode(GameModeRequest),
}

impl AppAction {
    /// Decode a persisted `lhc:` id. Wire/config spelling stays at this boundary.
    pub fn parse(id: &str) -> Option<Self> {
        let ActionName::App { id, n } = app_action_name(id.trim())? else {
            return None;
        };
        Some(match id {
            "showQuickMenu" => Self::ShowMenu {
                menu: Menu::Quick,
                page: Some(NonZeroU32::new(n)?),
            },
            "showEmojiMenu" => Self::ShowMenu {
                menu: Menu::Emoji,
                page: Some(NonZeroU32::new(n)?),
            },
            "showQuickMenuLast" => Self::ShowMenu {
                menu: Menu::Quick,
                page: None,
            },
            "showEmojiMenuLast" => Self::ShowMenu {
                menu: Menu::Emoji,
                page: None,
            },
            "showSettings" => Self::ShowSettings,
            "activateProfile" => Self::ActivateProfile(NonZeroU32::new(n)?),
            "setGameModeAuto" => Self::GameMode(GameModeRequest::Auto),
            "setGameModeOn" => Self::GameMode(GameModeRequest::On),
            "setGameModeOff" => Self::GameMode(GameModeRequest::Off),
            "toggleGameMode" => Self::GameMode(GameModeRequest::Toggle),
            _ => return None,
        })
    }
}

impl GameModeRequest {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_actions_have_typed_requests() {
        for entry in super::super::actions::app_actions() {
            let super::super::actions::Action::App(id) = entry.action else {
                panic!("app catalog")
            };
            assert!(AppAction::parse(&id).is_some(), "{id}");
        }
        for id in [
            "showQuickMenu0",
            "showEmojiMenu-1",
            "activateProfile0",
            "quit",
        ] {
            assert!(AppAction::parse(id).is_none());
        }
    }
}
