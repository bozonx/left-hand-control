//! Action strings (`KeyA`, `Ctrl+KeyC`, `macro:id`, `cmd:id`, `sys:id`,
//! `app:id`, `text:…`, `pause:ms`) and the catalogs of built-in actions.

use super::model::AppConfig;
use crate::mapper::system_macros::SYSTEM_MACROS;

/// A bindable action, parsed from its canonical string form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Pass the physical key through (`""` / absent field).
    Native,
    /// Suppress the key (`null`).
    Swallow,
    /// Key or chord, e.g. `Ctrl+KeyC`.
    Keys(String),
    Macro(String),
    Command(String),
    System(String),
    App(String),
    Text(String),
    /// Macro-only delay in milliseconds.
    Pause(String),
}

impl Action {
    pub fn parse(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self::Swallow;
        };
        if let Some(text) = value.strip_prefix("text:") {
            return Self::Text(text.into());
        }
        let value = value.trim();
        type Constructor = fn(String) -> Action;
        let prefixed: [(&str, Constructor); 5] = [
            ("macro:", Self::Macro),
            ("cmd:", Self::Command),
            ("sys:", Self::System),
            ("app:", Self::App),
            ("pause:", Self::Pause),
        ];
        for (prefix, build) in prefixed {
            if let Some(id) = value.strip_prefix(prefix) {
                return build(id.trim().into());
            }
        }
        if value.is_empty() {
            Self::Native
        } else {
            Self::Keys(value.into())
        }
    }

    /// Canonical string; `None` for [`Action::Swallow`].
    pub fn format(&self) -> Option<String> {
        Some(match self {
            Self::Native => String::new(),
            Self::Swallow => return None,
            Self::Keys(keys) => keys.clone(),
            Self::Macro(id) => format!("macro:{id}"),
            Self::Command(id) => format!("cmd:{id}"),
            Self::System(id) => format!("sys:{id}"),
            Self::App(id) => format!("app:{id}"),
            Self::Text(text) => format!("text:{text}"),
            Self::Pause(ms) => format!("pause:{ms}"),
        })
    }
}

/// How an action should be presented: a translatable message id with an
/// optional number, or a user-provided label that is shown verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionName {
    /// Built-in system action; the id is the `sys:` id without its number.
    System { id: &'static str, n: u32 },
    /// Built-in app action (`showQuickMenu` / `showEmojiMenu`).
    App { id: &'static str, n: u32 },
    /// Name chosen by the user or an untranslated built-in name.
    Verbatim(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub action: Action,
    pub name: ActionName,
}

const NUMBERED_SYSTEM_ACTIONS: [&str; 3] = ["switchDesktop", "switchLayout", "taskEntry"];

/// `sys:` ids of `utils/systemActions.ts`, apart from the numbered ones.
pub const SYSTEM_ACTION_IDS: [&str; 52] = [
    "walkThroughWindowsAlternative",
    "walkThroughWindowsCurrentApp",
    "showClipboardHistory",
    "volumeDown",
    "volumeUp",
    "muteAudio",
    "brightnessDown",
    "brightnessUp",
    "windowClose",
    "windowToNextDesktop",
    "windowToPreviousDesktop",
    "windowKeepAbove",
    "windowMaximizeVertical",
    "windowMaximizeHorizontal",
    "screenOff",
    "launchKrunner",
    "launchSystemMonitor",
    "manageActivities",
    "nextActivity",
    "previousActivity",
    "muteMicrophone",
    "showDisplayConfig",
    "toggleTouchpad",
    "lockSession",
    "logout",
    "logoutWithoutConfirmation",
    "increaseKeyboardBrightness",
    "decreaseKeyboardBrightness",
    "toggleKeyboardBacklight",
    "activateApplicationLauncher",
    "showDesktop",
    "maximizeWindow",
    "minimizeWindow",
    "moveWindow",
    "windowToNextScreen",
    "windowToPreviousScreen",
    "quickTileWindowTop",
    "quickTileWindowBottom",
    "quickTileWindowLeft",
    "quickTileWindowRight",
    "toggleNightColor",
    "toggleGridView",
    "toggleOverview",
    "togglePresentWindowsAllDesktops",
    "togglePresentWindowsCurrentDesktop",
    "windowMenu",
    "zoomIn",
    "zoomOut",
    "zoomActualSize",
    "killWindow",
    "windowFullscreen",
    "windowOnAllDesktops",
];

/// Numbered actions have ids 1…10 (system) and 1…5 (app menus).
const SYSTEM_NUMBERED_COUNT: u32 = 10;
pub const MENU_PAGE_COUNT: u32 = 5;

/// Split `switchDesktop3` into its catalog name and number.
fn numbered(id: &str, names: &[&'static str], max: u32) -> Option<(&'static str, u32)> {
    names.iter().find_map(|name| {
        let n: u32 = id.strip_prefix(name)?.parse().ok()?;
        (1..=max).contains(&n).then_some((*name, n))
    })
}

pub fn system_action_name(id: &str) -> Option<ActionName> {
    if let Some((name, n)) = numbered(id, &NUMBERED_SYSTEM_ACTIONS, SYSTEM_NUMBERED_COUNT) {
        return Some(ActionName::System { id: name, n });
    }
    SYSTEM_ACTION_IDS
        .iter()
        .find(|known| **known == id)
        .map(|known| ActionName::System { id: known, n: 0 })
}

pub fn app_action_name(id: &str) -> Option<ActionName> {
    numbered(id, &["showQuickMenu", "showEmojiMenu"], MENU_PAGE_COUNT)
        .map(|(id, n)| ActionName::App { id, n })
}

/// Page of an `app:` menu action, e.g. `showEmojiMenu2` → (Emoji, 2).
pub fn menu_page(app_id: &str) -> Option<(bool, u32)> {
    match app_action_name(app_id)? {
        ActionName::App { id, n } => Some((id == "showEmojiMenu", n)),
        _ => None,
    }
}

pub fn system_actions() -> Vec<CatalogEntry> {
    let mut out = Vec::new();
    for name in NUMBERED_SYSTEM_ACTIONS {
        for n in 1..=SYSTEM_NUMBERED_COUNT {
            out.push(CatalogEntry {
                action: Action::System(format!("{name}{n}")),
                name: ActionName::System { id: name, n },
            });
        }
    }
    for id in SYSTEM_ACTION_IDS {
        out.push(CatalogEntry {
            action: Action::System(id.into()),
            name: ActionName::System { id, n: 0 },
        });
    }
    out
}

pub fn app_actions() -> Vec<CatalogEntry> {
    (1..=MENU_PAGE_COUNT)
        .flat_map(|n| {
            ["showQuickMenu", "showEmojiMenu"].map(|id| CatalogEntry {
                action: Action::App(format!("{id}{n}")),
                name: ActionName::App { id, n },
            })
        })
        .collect()
}

/// Every action that can be picked for `config`: user macros and
/// commands, system macros, system and app actions.
pub fn catalog(config: &AppConfig) -> Vec<CatalogEntry> {
    let mut out: Vec<CatalogEntry> = config
        .macros
        .iter()
        .map(|item| CatalogEntry {
            action: Action::Macro(item.id.clone()),
            name: ActionName::Verbatim(item.name.clone()),
        })
        .collect();
    out.extend(
        SYSTEM_MACROS
            .iter()
            .filter(|system| !config.macros.iter().any(|user| user.id == system.id))
            .map(|system| CatalogEntry {
                action: Action::Macro(system.id.into()),
                name: ActionName::Verbatim(system.name.into()),
            }),
    );
    out.extend(config.commands.iter().map(|command| CatalogEntry {
        action: Action::Command(command.id.clone()),
        name: ActionName::Verbatim(command.name.clone()),
    }));
    out.extend(system_actions());
    out.extend(app_actions());
    out
}

/// Why an action cannot be used, following `validateActionValue()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionIssue {
    InvalidSyntax,
    PauseOutsideMacro,
    UnknownMacro,
    UnknownCommand,
    UnknownSystemAction,
    UnknownAppAction,
}

/// Check an action outside a macro. Key names are checked by the mapper
/// when it starts; here only the chord shape is validated.
pub fn validate(action: &Action, config: &AppConfig) -> Option<ActionIssue> {
    match action {
        Action::Native | Action::Swallow | Action::Text(_) => None,
        Action::Keys(keys) => (!valid_held_key(keys)).then_some(ActionIssue::InvalidSyntax),
        Action::Pause(_) => Some(ActionIssue::PauseOutsideMacro),
        Action::Macro(id) => {
            let known = config.macros.iter().any(|item| &item.id == id)
                || SYSTEM_MACROS.iter().any(|item| item.id == id);
            (!known).then_some(ActionIssue::UnknownMacro)
        }
        Action::Command(id) => (!config.commands.iter().any(|command| &command.id == id))
            .then_some(ActionIssue::UnknownCommand),
        Action::System(id) => system_action_name(id)
            .is_none()
            .then_some(ActionIssue::UnknownSystemAction),
        Action::App(id) => app_action_name(id)
            .is_none()
            .then_some(ActionIssue::UnknownAppAction),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionIssue {
    ApprovalRequired,
    Unavailable,
}

pub fn execution_issue(
    action: &Action,
    config: &AppConfig,
    trusted: bool,
) -> Option<ExecutionIssue> {
    fn check(
        action: &Action,
        config: &AppConfig,
        trusted: bool,
        depth: usize,
    ) -> Option<ExecutionIssue> {
        if depth > 10 {
            return Some(ExecutionIssue::Unavailable);
        }
        match action {
            Action::Command(_) => {
                if !cfg!(target_os = "linux") {
                    Some(ExecutionIssue::Unavailable)
                } else if !trusted {
                    Some(ExecutionIssue::ApprovalRequired)
                } else {
                    None
                }
            }
            Action::System(id) => {
                #[cfg(target_os = "linux")]
                if crate::mapper::system::resolve(id).is_some() {
                    return None;
                }
                #[cfg(not(target_os = "linux"))]
                let _ = id;
                Some(ExecutionIssue::Unavailable)
            }
            Action::Macro(id) => {
                let steps: Vec<&str> =
                    if let Some(item) = config.macros.iter().find(|item| item.id == *id) {
                        item.steps.iter().map(|step| step.action.as_str()).collect()
                    } else {
                        SYSTEM_MACROS
                            .iter()
                            .find(|item| item.id == id)
                            .map(|item| item.steps.to_vec())
                            .unwrap_or_default()
                    };
                steps.into_iter().find_map(|value| {
                    check(&Action::parse(Some(value)), config, trusted, depth + 1)
                })
            }
            _ => None,
        }
    }
    check(action, config, trusted, 0)
}

pub fn valid_held_key(value: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::mapper::action::parse_action(value).is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut tokens = value.split('+').map(str::trim).collect::<Vec<_>>();
        let Some(key) = tokens.pop() else {
            return false;
        };
        super::key_catalog::CATEGORIES
            .iter()
            .any(|keys| keys.contains(&key))
            && tokens.iter().all(|modifier| {
                matches!(
                    *modifier,
                    "Ctrl"
                        | "ControlLeft"
                        | "ControlRight"
                        | "Shift"
                        | "ShiftLeft"
                        | "ShiftRight"
                        | "Alt"
                        | "AltLeft"
                        | "AltRight"
                        | "AltGr"
                        | "Meta"
                        | "MetaLeft"
                        | "MetaRight"
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::model::{AppSettings, LayoutPreset};

    #[test]
    fn parse_and_format_round_trip() {
        for raw in [
            "KeyA",
            "Ctrl+KeyC",
            "macro:copyLine",
            "cmd:music",
            "sys:switchDesktop2",
            "app:showQuickMenu1",
            "text:  keep spaces ",
            "pause:100",
            "",
        ] {
            assert_eq!(Action::parse(Some(raw)).format().as_deref(), Some(raw));
        }
        assert_eq!(Action::parse(None), Action::Swallow);
        assert_eq!(Action::Swallow.format(), None);
    }

    #[test]
    fn catalogs_match_the_frontend() {
        assert_eq!(system_actions().len(), 30 + 52);
        assert_eq!(app_actions().len(), 10);
        assert_eq!(
            system_action_name("switchDesktop10"),
            Some(ActionName::System {
                id: "switchDesktop",
                n: 10
            })
        );
        assert_eq!(system_action_name("switchDesktop11"), None);
        assert_eq!(menu_page("showEmojiMenu3"), Some((true, 3)));
        assert_eq!(menu_page("showQuickMenu6"), None);
    }

    #[test]
    fn validation_checks_references() {
        let config = AppConfig::from_parts(AppSettings::default(), LayoutPreset::initial(), None);
        let check = |raw: &str| validate(&Action::parse(Some(raw)), &config);
        assert_eq!(check("Ctrl+KeyC"), None);
        assert_eq!(check("Ctrl+"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("NotAKey"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("KeyA+KeyB"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("macro:copyLine"), None);
        assert_eq!(check("macro:nope"), Some(ActionIssue::UnknownMacro));
        assert_eq!(check("cmd:nope"), Some(ActionIssue::UnknownCommand));
        assert_eq!(check("sys:lockSession"), None);
        assert_eq!(
            check("app:showEmojiMenu9"),
            Some(ActionIssue::UnknownAppAction)
        );
        assert_eq!(check("pause:10"), Some(ActionIssue::PauseOutsideMacro));
    }
}
