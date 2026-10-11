//! Action strings (`KeyA`, `Ctrl+KeyC`, `macro:id`, `cmd:id`, `sys:id`,
//! `app:id`, `kde:component/shortcut`, `text:…`, `pause:ms`) and the
//! catalogs of built-in actions.

use super::model::AppConfig;
use crate::mapper::builtin_macros::BUILTIN_MACROS;

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
    /// KDE global shortcut, `component/shortcut` as named by kglobalaccel.
    KdeShortcut(String),
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
        let prefixed: [(&str, Constructor); 6] = [
            ("macro:", Self::Macro),
            ("cmd:", Self::Command),
            ("sys:", Self::System),
            ("app:", Self::App),
            ("kde:", Self::KdeShortcut),
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
            Self::KdeShortcut(id) => format!("kde:{id}"),
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
    /// Built-in app action (`showQuickMenu`, `activateLayout`, …).
    App { id: &'static str, n: u32 },
    /// Name chosen by the user or an untranslated built-in name.
    Verbatim(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub action: Action,
    pub name: ActionName,
}

const NUMBERED_SYSTEM_ACTIONS: [&str; 3] = ["switchDesktop", "switchLayout", "windowToDesktop"];

/// Numbered actions have ids 1…10 (system), 1…5 (app menus) and 1…5
/// (layouts in library order).
const SYSTEM_NUMBERED_COUNT: u32 = 10;
pub const MENU_PAGE_COUNT: u32 = 5;
pub const LAYOUT_SLOT_COUNT: u32 = 5;

/// Split `switchDesktop3` into its catalog name and number.
fn numbered(id: &str, names: &[&'static str], max: u32) -> Option<(&'static str, u32)> {
    names.iter().find_map(|name| {
        let n: u32 = id.strip_prefix(name)?.parse().ok()?;
        (1..=max).contains(&n).then_some((*name, n))
    })
}

pub fn system_action_name(id: &str) -> Option<ActionName> {
    numbered(id, &NUMBERED_SYSTEM_ACTIONS, SYSTEM_NUMBERED_COUNT)
        .map(|(name, n)| ActionName::System { id: name, n })
}

/// Split a `kde:` id into its kglobalaccel component and shortcut name.
/// The component is the last segment of its D-Bus object path, so it may
/// only hold the characters such a segment allows.
pub fn kde_shortcut_parts(id: &str) -> Option<(&str, &str)> {
    let (component, shortcut) = id.split_once('/')?;
    let valid_component = !component.is_empty()
        && component
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    (valid_component && !shortcut.trim().is_empty()).then_some((component, shortcut))
}

/// Game-mode control actions: flip the effective state, or set the mode.
pub const GAME_MODE_APP_ACTIONS: [&str; 4] = [
    "gameModeToggle",
    "gameModeAuto",
    "gameModeOn",
    "gameModeOff",
];

/// App actions without a number: the menus on their last used page and
/// the settings window.
pub const WINDOW_APP_ACTIONS: [&str; 3] =
    ["showQuickMenuLast", "showEmojiMenuLast", "showSettings"];

pub fn app_action_name(id: &str) -> Option<ActionName> {
    numbered(id, &["showQuickMenu", "showEmojiMenu"], MENU_PAGE_COUNT)
        .or_else(|| numbered(id, &["activateLayout"], LAYOUT_SLOT_COUNT))
        .map(|(id, n)| ActionName::App { id, n })
        .or_else(|| {
            WINDOW_APP_ACTIONS
                .iter()
                .chain(&GAME_MODE_APP_ACTIONS)
                .find(|known| **known == id)
                .map(|known| ActionName::App { id: known, n: 0 })
        })
}

/// Page of an `app:` menu action, e.g. `showEmojiMenu2` → (Emoji, 2).
pub fn menu_page(app_id: &str) -> Option<(bool, u32)> {
    match app_action_name(app_id)? {
        ActionName::App {
            id: id @ ("showQuickMenu" | "showEmojiMenu"),
            n,
        } => Some((id == "showEmojiMenu", n)),
        _ => None,
    }
}

pub fn system_actions() -> Vec<CatalogEntry> {
    NUMBERED_SYSTEM_ACTIONS
        .into_iter()
        .flat_map(|name| {
            (1..=SYSTEM_NUMBERED_COUNT).map(move |n| CatalogEntry {
                action: Action::System(format!("{name}{n}")),
                name: ActionName::System { id: name, n },
            })
        })
        .collect()
}

pub fn app_actions() -> Vec<CatalogEntry> {
    let plain = |id: &'static str| CatalogEntry {
        action: Action::App(id.into()),
        name: ActionName::App { id, n: 0 },
    };
    let numbered = |id: &'static str, n: u32| CatalogEntry {
        action: Action::App(format!("{id}{n}")),
        name: ActionName::App { id, n },
    };
    WINDOW_APP_ACTIONS
        .into_iter()
        .map(plain)
        .chain(
            (1..=MENU_PAGE_COUNT)
                .flat_map(|n| ["showQuickMenu", "showEmojiMenu"].map(|id| numbered(id, n))),
        )
        .chain((1..=LAYOUT_SLOT_COUNT).map(|n| numbered("activateLayout", n)))
        .chain(GAME_MODE_APP_ACTIONS.map(plain))
        .collect()
}

/// Every action that can be picked for `config`: user macros and
/// commands, built-in macros, system and app actions.
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
        BUILTIN_MACROS
            .iter()
            .filter(|builtin| !config.macros.iter().any(|user| user.id == builtin.id))
            .map(|builtin| CatalogEntry {
                action: Action::Macro(builtin.id.into()),
                name: ActionName::Verbatim(builtin.name.into()),
            }),
    );
    out.extend(config.commands.iter().map(|command| CatalogEntry {
        action: Action::Command(command.id.clone()),
        name: ActionName::Verbatim(if command.name.is_empty() {
            command.linux.clone()
        } else {
            command.name.clone()
        }),
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
                || BUILTIN_MACROS.iter().any(|item| item.id == id);
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
        Action::KdeShortcut(id) => kde_shortcut_parts(id)
            .is_none()
            .then_some(ActionIssue::InvalidSyntax),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionIssue {
    CommandsDisabled,
    Unavailable,
}

pub fn execution_issue(action: &Action, config: &AppConfig) -> Option<ExecutionIssue> {
    fn check(action: &Action, config: &AppConfig, depth: usize) -> Option<ExecutionIssue> {
        if depth > 10 {
            return Some(ExecutionIssue::Unavailable);
        }
        match action {
            Action::Command(_) => {
                if !cfg!(target_os = "linux") {
                    Some(ExecutionIssue::Unavailable)
                } else if !config.settings.commands_enabled {
                    Some(ExecutionIssue::CommandsDisabled)
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
            Action::KdeShortcut(id) => {
                #[cfg(target_os = "linux")]
                if crate::mapper::system::resolve_kde_shortcut(id).is_some() {
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
                        BUILTIN_MACROS
                            .iter()
                            .find(|item| item.id == id)
                            .map(|item| item.steps.to_vec())
                            .unwrap_or_default()
                    };
                steps
                    .into_iter()
                    .find_map(|value| check(&Action::parse(Some(value)), config, depth + 1))
            }
            _ => None,
        }
    }
    check(action, config, 0)
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
        super::key_catalog::valid_key(key)
            && tokens
                .iter()
                .all(|modifier| super::key_catalog::modifier_key(modifier).is_some())
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
            "kde:kwin/Window Quick Tile Top",
            "text:  keep spaces ",
            "text:   ",
            "text:first\nsecond\n",
            "pause:100",
            "",
        ] {
            assert_eq!(Action::parse(Some(raw)).format().as_deref(), Some(raw));
        }
        assert_eq!(Action::parse(None), Action::Swallow);
        assert_eq!(Action::Swallow.format(), None);
    }

    #[test]
    fn catalogs_preserve_builtin_action_ids() {
        assert_eq!(system_actions().len(), 30);
        assert_eq!(app_actions().len(), 22);
        assert_eq!(
            system_action_name("switchDesktop10"),
            Some(ActionName::System {
                id: "switchDesktop",
                n: 10
            })
        );
        assert_eq!(system_action_name("switchDesktop11"), None);
        assert_eq!(system_action_name("lockSession"), None);
        assert_eq!(
            kde_shortcut_parts("org_kde_powerdevil/Turn Off Screen"),
            Some(("org_kde_powerdevil", "Turn Off Screen"))
        );
        assert_eq!(kde_shortcut_parts("kwin"), None);
        assert_eq!(kde_shortcut_parts("kwin/ "), None);
        assert_eq!(kde_shortcut_parts("org.kde/Overview"), None);
        assert_eq!(menu_page("showEmojiMenu3"), Some((true, 3)));
        assert_eq!(menu_page("showQuickMenu6"), None);
        assert_eq!(menu_page("gameModeToggle"), None);
        assert_eq!(menu_page("activateLayout2"), None);
        assert_eq!(menu_page("showEmojiMenuLast"), None);
        assert_eq!(
            app_action_name("gameModeOff"),
            Some(ActionName::App {
                id: "gameModeOff",
                n: 0
            })
        );
        assert_eq!(
            app_action_name("activateLayout5"),
            Some(ActionName::App {
                id: "activateLayout",
                n: 5
            })
        );
        assert_eq!(app_action_name("activateLayout6"), None);
        assert!(app_action_name("showQuickMenuLast").is_some());
        assert!(app_action_name("showSettings").is_some());
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
        assert_eq!(check("sys:windowToDesktop2"), None);
        assert_eq!(
            check("sys:lockSession"),
            Some(ActionIssue::UnknownSystemAction)
        );
        assert_eq!(check("kde:ksmserver/Lock Session"), None);
        assert_eq!(check("kde:Lock Session"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(
            check("app:showEmojiMenu9"),
            Some(ActionIssue::UnknownAppAction)
        );
        assert_eq!(check("pause:10"), Some(ActionIssue::PauseOutsideMacro));
    }
}
