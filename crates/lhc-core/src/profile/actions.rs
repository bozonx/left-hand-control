//! Action strings (`KeyA`, `Ctrl+KeyC`, `macro:id`, `builtin:id`, `cmd:id`, `sys:id`,
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
    /// User macro (`macro:<id>`).
    Macro(String),
    /// Built-in macro (`builtin:<id>`), independent of user macro IDs.
    BuiltinMacro(String),
    Command(String),
    System(String),
    App(String),
    /// KDE global shortcut, `component/shortcut` as named by kglobalaccel.
    KdeShortcut(String),
    Text(String),
    /// Macro-only delay in milliseconds.
    Pause(String),
    /// A `word:` prefix this build does not know, most likely an action
    /// type from a newer version. Kept verbatim so saving preserves it.
    Unknown(String),
}

impl Action {
    pub fn parse(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self::Swallow;
        };
        // Leading blanks are not part of the prefix; the text payload keeps
        // all of its whitespace.
        if let Some(text) = value.trim_start().strip_prefix("text:") {
            return Self::Text(text.into());
        }
        let value = value.trim();
        type Constructor = fn(String) -> Action;
        let prefixed: [(&str, Constructor); 7] = [
            ("macro:", Self::Macro),
            ("builtin:", Self::BuiltinMacro),
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
        } else if unknown_prefix(value) {
            Self::Unknown(value.into())
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
            Self::BuiltinMacro(id) => format!("builtin:{id}"),
            Self::Command(id) => format!("cmd:{id}"),
            Self::System(id) => format!("sys:{id}"),
            Self::App(id) => format!("app:{id}"),
            Self::KdeShortcut(id) => format!("kde:{id}"),
            Self::Text(text) => format!("text:{text}"),
            Self::Pause(ms) => format!("pause:{ms}"),
            Self::Unknown(value) => value.clone(),
        })
    }
}

/// `word:…` with a lowercase prefix, the shape of every action type. Key
/// names never contain `:`.
fn unknown_prefix(value: &str) -> bool {
    value.split_once(':').is_some_and(|(prefix, _)| {
        prefix.starts_with(|c: char| c.is_ascii_lowercase())
            && prefix.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// How an action should be presented: a translatable message id with an
/// optional number, or a user-provided label that is shown verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionName {
    /// Built-in system action; the id is the `sys:` id without its number.
    System { id: &'static str, n: u32 },
    /// Built-in app action (`showQuickMenu`, `activateLayout`, …).
    App { id: &'static str, n: u32 },
    /// Built-in macro; its id is translated by the shell.
    BuiltinMacro { id: &'static str },
    /// Name chosen by the user.
    Verbatim(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub action: Action,
    pub name: ActionName,
}

const NUMBERED_SYSTEM_ACTIONS: [&str; 3] = ["switchDesktop", "switchLayout", "windowToDesktop"];

/// System actions without a number. Each has a documented counterpart on
/// Linux desktops and Windows, except `windowToNextDesktop`: like
/// `windowToDesktopN`, Windows has no public API to move another app's
/// window between virtual desktops. See [`unsupported_on_macos`] for macOS.
pub const PLAIN_SYSTEM_ACTIONS: [&str; 10] = [
    "windowClose",
    "windowMinimize",
    "windowMaximize",
    "windowMaximizeVertical",
    "windowKeepAbove",
    "windowToNextDesktop",
    "showDesktop",
    "overview",
    "appLauncher",
    "screenOff",
];

/// `sys:` ids (numbered ones without their number) that macOS has no
/// public API for. Running one there is ignored with a notice.
const MACOS_UNSUPPORTED_SYSTEM_ACTIONS: [&str; 7] = [
    "switchDesktop",
    "windowToDesktop",
    "windowToNextDesktop",
    "windowMaximizeVertical",
    "windowKeepAbove",
    "showDesktop",
    "appLauncher",
];

/// Numbered actions have ids 1…10 (system), 1… without an upper bound
/// (menu pages) and 1…5 (layouts in library order).
///
/// Menu pages and layouts are addressed by position on purpose, not by id:
/// the user binds keys to slots once and changes what a key opens by
/// reordering pages or library layouts, which the UI shows. Reordering
/// therefore does not rewrite these actions. Do not switch them to ids.
/// Highest N of numbered system actions (`switchLayoutN`, …) the profile accepts.
pub const SYSTEM_NUMBERED_COUNT: u32 = 10;
/// Menu pages offered in the action picker; any page number is valid.
pub const MENU_PAGE_PICKER_COUNT: u32 = 10;
/// How deep macros may reference other macros.
pub const MAX_MACRO_DEPTH: usize = 10;
/// Most steps a macro may expand to, nested macros included.
pub const MAX_MACRO_STEPS: usize = 1000;
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
        .or_else(|| {
            PLAIN_SYSTEM_ACTIONS
                .iter()
                .find(|known| **known == id)
                .map(|known| (*known, 0))
        })
        .map(|(name, n)| ActionName::System { id: name, n })
}

/// Whether the `sys:` action `id` cannot run on macOS.
pub fn unsupported_on_macos(id: &str) -> bool {
    matches!(system_action_name(id), Some(ActionName::System { id, .. })
        if MACOS_UNSUPPORTED_SYSTEM_ACTIONS.contains(&id))
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

/// Actions of the built-in popup hotkeys Ctrl+Alt+F11 and Ctrl+Alt+F12.
pub const SHOW_EMOJI_MENU_LAST: &str = "app:showEmojiMenuLast";
pub const SHOW_QUICK_MENU_LAST: &str = "app:showQuickMenuLast";

pub fn app_action_name(id: &str) -> Option<ActionName> {
    numbered(id, &["showQuickMenu", "showEmojiMenu"], u32::MAX)
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
    PLAIN_SYSTEM_ACTIONS
        .into_iter()
        .map(|id| CatalogEntry {
            action: Action::System(id.into()),
            name: ActionName::System { id, n: 0 },
        })
        .chain(NUMBERED_SYSTEM_ACTIONS.into_iter().flat_map(|name| {
            (1..=SYSTEM_NUMBERED_COUNT).map(move |n| CatalogEntry {
                action: Action::System(format!("{name}{n}")),
                name: ActionName::System { id: name, n },
            })
        }))
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
            (1..=MENU_PAGE_PICKER_COUNT)
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
    out.extend(BUILTIN_MACROS.iter().map(|builtin| CatalogEntry {
        action: Action::BuiltinMacro(builtin.id.into()),
        name: ActionName::BuiltinMacro { id: builtin.id },
    }));
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
    /// An action type this build does not know (see [`Action::Unknown`]).
    UnknownActionType,
}

/// Check an action outside a macro. Key names are checked by the mapper
/// when it starts; here only the chord shape is validated.
pub fn validate(action: &Action, config: &AppConfig) -> Option<ActionIssue> {
    match action {
        Action::Native | Action::Swallow | Action::Text(_) => None,
        Action::Keys(keys) => (!valid_held_key(keys)).then_some(ActionIssue::InvalidSyntax),
        Action::Pause(_) => Some(ActionIssue::PauseOutsideMacro),
        Action::Unknown(_) => Some(ActionIssue::UnknownActionType),
        Action::Macro(id) => {
            let known = config.macros.iter().any(|item| &item.id == id);
            (!known).then_some(ActionIssue::UnknownMacro)
        }
        Action::BuiltinMacro(id) => {
            (!BUILTIN_MACROS.iter().any(|item| item.id == id)).then_some(ActionIssue::UnknownMacro)
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
        if depth > MAX_MACRO_DEPTH {
            return Some(ExecutionIssue::Unavailable);
        }
        match action {
            Action::Command(id) => {
                if !crate::mapper::runtime::SUPPORTED {
                    Some(ExecutionIssue::Unavailable)
                } else if !config.settings.commands_enabled {
                    Some(ExecutionIssue::CommandsDisabled)
                } else if config
                    .commands
                    .iter()
                    .find(|command| &command.id == id)
                    .is_some_and(|command| command.script().trim().is_empty())
                {
                    // Only a script for another OS: the mapper skips it here.
                    Some(ExecutionIssue::Unavailable)
                } else {
                    None
                }
            }
            Action::System(id) => {
                #[cfg(any(target_os = "linux", target_os = "windows"))]
                if crate::mapper::system::resolve(id).is_some() {
                    return None;
                }
                #[cfg(not(any(target_os = "linux", target_os = "windows")))]
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
            Action::Macro(_) | Action::BuiltinMacro(_) => {
                let steps: Vec<&str> = match action {
                    Action::Macro(id) => config
                        .macros
                        .iter()
                        .find(|item| item.id == *id)
                        .map(|item| item.steps.iter().map(|step| step.action.as_str()).collect())
                        .unwrap_or_default(),
                    Action::BuiltinMacro(id) => BUILTIN_MACROS
                        .iter()
                        .find(|item| item.id == id)
                        .map(|item| item.steps.to_vec())
                        .unwrap_or_default(),
                    _ => unreachable!(),
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
            "macro:custom",
            "builtin:copyLine",
            "cmd:music",
            "sys:switchDesktop2",
            "app:showQuickMenu1",
            "kde:kwin/Window Quick Tile Top",
            "text:  keep spaces ",
            "text:   ",
            "text:first\nsecond\n",
            "pause:100",
            "future:thing",
            "",
        ] {
            assert_eq!(Action::parse(Some(raw)).format().as_deref(), Some(raw));
        }
        assert_eq!(Action::parse(None), Action::Swallow);
        assert_eq!(Action::Swallow.format(), None);
        // Same reading as the mapper: blanks before the prefix are ignored,
        // the payload keeps its whitespace.
        assert_eq!(
            Action::parse(Some("  text: hi ")),
            Action::Text(" hi ".into())
        );
    }

    #[test]
    fn unknown_action_types_are_kept_and_reported() {
        let config = AppConfig::from_parts(AppSettings::default(), LayoutPreset::default(), None);
        assert_eq!(
            Action::parse(Some(" newKind:some value ")),
            Action::Unknown("newKind:some value".into())
        );
        assert_eq!(
            validate(&Action::parse(Some("newKind:x")), &config),
            Some(ActionIssue::UnknownActionType)
        );
        // Not the shape of an action type: still a (bad) key chord.
        for raw in ["Ctrl+a:b", "Key:A", ":x"] {
            assert_eq!(Action::parse(Some(raw)), Action::Keys(raw.into()));
        }
    }

    #[test]
    fn catalogs_preserve_builtin_action_ids() {
        assert_eq!(system_actions().len(), 40);
        assert_eq!(app_actions().len(), 32);
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
            system_action_name("screenOff"),
            Some(ActionName::System {
                id: "screenOff",
                n: 0
            })
        );
        assert!(unsupported_on_macos("switchDesktop3"));
        assert!(unsupported_on_macos("windowKeepAbove"));
        assert!(unsupported_on_macos("windowMaximizeVertical"));
        assert!(unsupported_on_macos("windowToNextDesktop"));
        assert!(!unsupported_on_macos("switchLayout2"));
        assert!(!unsupported_on_macos("windowClose"));
        assert!(!unsupported_on_macos("lockSession"));
        assert_eq!(
            kde_shortcut_parts("org_kde_powerdevil/Turn Off Screen"),
            Some(("org_kde_powerdevil", "Turn Off Screen"))
        );
        assert_eq!(kde_shortcut_parts("kwin"), None);
        assert_eq!(kde_shortcut_parts("kwin/ "), None);
        assert_eq!(kde_shortcut_parts("org.kde/Overview"), None);
        assert_eq!(menu_page("showEmojiMenu3"), Some((true, 3)));
        assert_eq!(menu_page("showQuickMenu42"), Some((false, 42)));
        assert_eq!(menu_page("showQuickMenu0"), None);
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
    fn builtin_macro_labels_are_typed_and_user_names_stay_verbatim() {
        let mut config =
            AppConfig::from_parts(AppSettings::default(), LayoutPreset::initial(), None);
        config.macros.push(super::super::model::Macro {
            id: "copyLine".into(),
            name: "Copy line".into(),
            steps: vec![],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        let entries = catalog(&config);
        for builtin in BUILTIN_MACROS {
            assert!(entries.iter().any(|entry| entry.action
                == Action::BuiltinMacro(builtin.id.into())
                && entry.name == ActionName::BuiltinMacro { id: builtin.id }));
        }
        assert!(
            entries
                .iter()
                .any(|entry| entry.action == Action::Macro("copyLine".into())
                    && entry.name == ActionName::Verbatim("Copy line".into()))
        );
    }

    #[test]
    fn execution_issues_follow_what_this_os_runs() {
        let mut config =
            AppConfig::from_parts(AppSettings::default(), LayoutPreset::default(), None);
        let other_os_only = if cfg!(target_os = "windows") {
            ("true", "")
        } else {
            ("", "dir")
        };
        for (id, (linux, windows)) in [("both", ("true", "dir")), ("foreign", other_os_only)] {
            config.commands.push(crate::profile::model::Command {
                id: id.into(),
                name: String::new(),
                linux: linux.into(),
                windows: windows.into(),
                working_directory: None,
            });
        }
        let issue =
            |config: &AppConfig, raw: &str| execution_issue(&Action::parse(Some(raw)), config);

        config.settings.commands_enabled = false;
        let disabled = if crate::mapper::runtime::SUPPORTED {
            Some(ExecutionIssue::CommandsDisabled)
        } else {
            Some(ExecutionIssue::Unavailable)
        };
        assert_eq!(issue(&config, "cmd:both"), disabled);

        config.settings.commands_enabled = true;
        let runnable = (!crate::mapper::runtime::SUPPORTED).then_some(ExecutionIssue::Unavailable);
        assert_eq!(issue(&config, "cmd:both"), runnable);
        assert_eq!(
            issue(&config, "cmd:foreign"),
            Some(ExecutionIssue::Unavailable)
        );
        // Windows runs system actions through its own backend.
        if cfg!(target_os = "windows") {
            assert_eq!(issue(&config, "sys:windowClose"), None);
        }
        if !crate::mapper::runtime::SUPPORTED {
            assert_eq!(
                issue(&config, "sys:windowClose"),
                Some(ExecutionIssue::Unavailable)
            );
        }
        assert_eq!(
            issue(&config, "sys:switchDesktop11"),
            Some(ExecutionIssue::Unavailable)
        );
    }

    #[test]
    fn validation_checks_references() {
        let config = AppConfig::from_parts(AppSettings::default(), LayoutPreset::initial(), None);
        let check = |raw: &str| validate(&Action::parse(Some(raw)), &config);
        assert_eq!(check("Ctrl+KeyC"), None);
        assert_eq!(check("Ctrl+"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("NotAKey"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("KeyA+KeyB"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("builtin:copyLine"), None);
        assert_eq!(check("macro:copyLine"), Some(ActionIssue::UnknownMacro));
        assert_eq!(check("builtin:nope"), Some(ActionIssue::UnknownMacro));
        assert_eq!(check("macro:nope"), Some(ActionIssue::UnknownMacro));
        assert_eq!(check("cmd:nope"), Some(ActionIssue::UnknownCommand));
        assert_eq!(check("sys:windowToDesktop2"), None);
        assert_eq!(
            check("sys:lockSession"),
            Some(ActionIssue::UnknownSystemAction)
        );
        assert_eq!(check("kde:ksmserver/Lock Session"), None);
        assert_eq!(check("kde:Lock Session"), Some(ActionIssue::InvalidSyntax));
        assert_eq!(check("app:showEmojiMenu42"), None);
        assert_eq!(
            check("app:showEmojiMenu0"),
            Some(ActionIssue::UnknownAppAction)
        );
        assert_eq!(check("pause:10"), Some(ActionIssue::PauseOutsideMacro));
    }
}
