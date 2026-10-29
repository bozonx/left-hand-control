//! System functions: OS-level actions that each desktop environment
//! implements its own way. Function names (`switchDesktop1`, …) stay
//! cross-platform; the concrete action is picked per OS, and on Linux per
//! DE by dispatching through `crate::platform::linux::detect()`.
//!
//! ```text
//! Only actions a key cannot express live here: switch to desktop / layout N,
//! move window to desktop N, and window/shell actions that every supported
//! platform documents (close, minimize, maximize, keep above, show desktop,
//! overview, application launcher, screen off). Everything else is a
//! standard key, a chord or a `kde:` global shortcut. See
//! `docs/design/removed-system-actions.md`.
//!
//! Action shapes:
//!   * `SystemAction::Dbus`    — Linux session-bus method call. Preferred when
//!                            the target DE exposes a DBus service: avoids
//!                            `fork+exec` of helper binaries (qdbus, gdbus)
//!                            and gives sub-millisecond latency.
//!   * `SystemAction::Windows` — a Win32 call (`mapper::windows::system`).
//!   * `SystemAction::AppEvent` — handled by the shell on every OS.
//!
//! Current product support:
//!   * Linux + KDE   — implemented.
//!   * Linux + GNOME — optional Shell extension, fixed action allowlist.
//!   * Linux + Sway / X11 generic — intentional skeletons for now.
//!   * Windows — every id with a documented Win32 API or fixed system
//!     shortcut; virtual-desktop ids have no public API there.
//! ```

/// A concrete child-process invocation: `program` + positional args.
#[derive(Clone, Debug)]
pub struct ProcessInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
}

/// Arguments for a DBus method call. Only the shapes we actually need
/// are listed; extend as new backends come online.
#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
pub enum DbusArg {
    U32(u32),
    I32(i32),
    Str(String),
}

/// A direct session-bus method invocation.
#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
pub struct DbusCall {
    pub destination: String,
    pub path: String,
    pub interface: Option<String>,
    pub method: String,
    pub args: Vec<DbusArg>,
}

#[cfg(target_os = "linux")]
impl DbusCall {
    /// Whether the call switches the KDE keyboard layout.
    pub fn switches_layout(&self) -> bool {
        use crate::platform::dbus;
        self.destination == dbus::KEYBOARD_SERVICE && self.method == dbus::KEYBOARD_SET_LAYOUT
    }
}

/// Resolved system-function.
#[derive(Clone, Debug)]
pub enum SystemAction {
    #[cfg(target_os = "linux")]
    Dbus(DbusCall),
    #[cfg(target_os = "windows")]
    Windows(super::windows::system::WindowsAction),
    AppEvent(String),
}

impl SystemAction {
    /// Whether the shell answers this action by showing a popup that takes
    /// the keyboard focus.
    pub fn opens_popup(&self) -> bool {
        matches!(self, Self::AppEvent(event)
            if event.starts_with("show_quick_menu_") || event.starts_with("show_emoji_menu_"))
    }
}

/// Resolve a system-function id (e.g. "switchDesktop3") into a concrete
/// action to run on this machine, or `None` if the function is not
/// available under the current OS or desktop environment.
pub fn resolve(name: &str) -> Option<SystemAction> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::detect;
        resolve_for_desktop(name.trim(), &detect().desktop)
    }
    #[cfg(target_os = "windows")]
    {
        super::windows::system::resolve(name.trim()).map(SystemAction::Windows)
    }
}

/// Whether `name` is a system-function id on any platform. Config
/// validation stays portable: a config created on KDE is not rejected on
/// another desktop or OS; `resolve()` decides what runs there.
pub fn is_known(name: &str) -> bool {
    // One catalog for the mapper, the editor and the action picker.
    crate::profile::actions::system_action_name(name.trim()).is_some()
}

pub fn resolve_app(name: &str) -> Option<SystemAction> {
    use crate::profile::actions::{ActionName, app_action_name};
    let name = name.trim();
    if let Some(ActionName::App { id, n }) = app_action_name(name)
        && n > 0
    {
        let event = match id {
            "showQuickMenu" => "show_quick_menu",
            "showEmojiMenu" => "show_emoji_menu",
            "activateLayout" => "activate_layout",
            _ => return None,
        };
        return Some(SystemAction::AppEvent(format!("{event}_{n}")));
    }
    let event = match name {
        "showQuickMenuLast" => "show_quick_menu_last",
        "showEmojiMenuLast" => "show_emoji_menu_last",
        "showSettings" => "show_settings",
        "gameModeToggle" => "game_mode_toggle",
        "gameModeAuto" => "game_mode_auto",
        "gameModeOn" => "game_mode_on",
        "gameModeOff" => "game_mode_off",
        _ => return None,
    };
    Some(SystemAction::AppEvent(event.into()))
}

pub fn is_known_app(name: &str) -> bool {
    resolve_app(name).is_some()
}

/// Resolve a `kde:` id (`component/shortcut`) into a kglobalaccel call,
/// or `None` outside KDE or when the id is malformed.
pub fn resolve_kde_shortcut(id: &str) -> Option<SystemAction> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{Desktop, detect};
        if !matches!(detect().desktop, Desktop::Kde) {
            return None;
        }
        kde_shortcut(id)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = id;
        None
    }
}

#[cfg(target_os = "linux")]
fn kde_shortcut(id: &str) -> Option<SystemAction> {
    let (component, shortcut) = crate::profile::actions::kde_shortcut_parts(id.trim())?;
    Some(kde::invoke_shortcut(component, shortcut))
}

#[cfg(target_os = "linux")]
fn resolve_for_desktop(
    name: &str,
    desktop: &crate::platform::linux::Desktop,
) -> Option<SystemAction> {
    let name = name.trim();
    if !is_known(name) {
        return None;
    }

    use crate::platform::linux::Desktop;
    match desktop {
        Desktop::Kde => kde::resolve(name),
        Desktop::Gnome => gnome::resolve(name),
        Desktop::Sway => sway::resolve(name),
        // Hyprland, Xfce, Cinnamon, MATE, Unknown, … all try the generic
        // X11/EWMH path. If `wmctrl` is missing or the WM is not EWMH the
        // command will fail at exec time — we log and move on.
        _ => x11_generic::resolve(name),
    }
}

// --- KDE Plasma -------------------------------------------------------------

#[cfg(target_os = "linux")]
mod kde {
    use super::{DbusArg, DbusCall, SystemAction};
    use crate::platform::dbus;
    use crate::profile::actions::SYSTEM_NUMBERED_COUNT;

    /// Most virtual desktops KWin can have.
    const KWIN_MAX_DESKTOPS: u32 = 20;

    pub(super) fn invoke_shortcut(component: &str, shortcut: &str) -> SystemAction {
        SystemAction::Dbus(DbusCall {
            destination: dbus::KGLOBALACCEL_SERVICE.into(),
            path: format!("/component/{component}"),
            interface: Some(dbus::KGLOBALACCEL_COMPONENT_INTERFACE.into()),
            method: "invokeShortcut".into(),
            args: vec![DbusArg::Str(shortcut.into())],
        })
    }

    pub fn resolve(name: &str) -> Option<SystemAction> {
        if let Some(rest) = name.strip_prefix("switchDesktop")
            && let Ok(n) = rest.parse::<u32>()
            && (1..=KWIN_MAX_DESKTOPS).contains(&n)
        {
            // org.kde.KWin /KWin setCurrentDesktop(i)
            return Some(SystemAction::Dbus(DbusCall {
                destination: dbus::KWIN_SERVICE.into(),
                path: dbus::KWIN_PATH.into(),
                interface: Some(dbus::KWIN_INTERFACE.into()),
                method: "setCurrentDesktop".into(),
                args: vec![DbusArg::I32(n as i32)],
            }));
        }
        if let Some(rest) = name.strip_prefix("switchLayout")
            && let Ok(n) = rest.parse::<u32>()
            && (1..=SYSTEM_NUMBERED_COUNT).contains(&n)
        {
            return Some(SystemAction::Dbus(DbusCall {
                destination: dbus::KEYBOARD_SERVICE.into(),
                path: dbus::KEYBOARD_LAYOUTS_PATH.into(),
                interface: Some(dbus::KEYBOARD_LAYOUTS_INTERFACE.into()),
                method: dbus::KEYBOARD_SET_LAYOUT.into(),
                args: vec![DbusArg::U32(n - 1)],
            }));
        }
        if let Some(rest) = name.strip_prefix("windowToDesktop")
            && let Ok(n) = rest.parse::<u32>()
            && (1..=KWIN_MAX_DESKTOPS).contains(&n)
        {
            return Some(invoke_shortcut("kwin", &format!("Window to Desktop {n}")));
        }
        let (component, shortcut) = match name {
            "windowClose" => ("kwin", "Window Close"),
            "windowMinimize" => ("kwin", "Window Minimize"),
            "windowMaximize" => ("kwin", "Window Maximize"),
            "windowMaximizeVertical" => ("kwin", "Window Maximize Vertical"),
            "windowKeepAbove" => ("kwin", "Window Above Other Windows"),
            "windowToNextDesktop" => ("kwin", "Window to Next Desktop"),
            "showDesktop" => ("kwin", "Show Desktop"),
            "overview" => ("kwin", "Overview"),
            "appLauncher" => ("plasmashell", "activate application launcher"),
            "screenOff" => ("org_kde_powerdevil", "Turn Off Screen"),
            _ => return None,
        };
        Some(invoke_shortcut(component, shortcut))
    }
}

#[cfg(test)]
mod tests {
    use super::{SystemAction, is_known, resolve_app};
    #[test]
    fn resolve_window_and_layout_actions() {
        for (name, event) in [
            ("showQuickMenuLast", "show_quick_menu_last"),
            ("showEmojiMenuLast", "show_emoji_menu_last"),
            ("showSettings", "show_settings"),
            ("activateLayout1", "activate_layout_1"),
            ("activateLayout5", "activate_layout_5"),
        ] {
            let Some(SystemAction::AppEvent(resolved)) = resolve_app(name) else {
                panic!("{name} did not resolve to an AppEvent");
            };
            assert_eq!(resolved, event);
            assert!(crate::profile::actions::app_action_name(name).is_some());
        }
        assert!(resolve_app("activateLayout6").is_none());
    }

    #[test]
    fn resolve_game_mode_actions() {
        for (name, event) in [
            ("gameModeToggle", "game_mode_toggle"),
            ("gameModeAuto", "game_mode_auto"),
            ("gameModeOn", "game_mode_on"),
            ("gameModeOff", "game_mode_off"),
        ] {
            let Some(SystemAction::AppEvent(resolved)) = resolve_app(name) else {
                panic!("{name} did not resolve to an AppEvent");
            };
            assert_eq!(resolved, event);
            assert!(crate::profile::actions::app_action_name(name).is_some());
        }
    }

    #[test]
    fn resolve_show_quick_menu_page() {
        let Some(SystemAction::AppEvent(event)) = resolve_app("showQuickMenu3") else {
            panic!("showQuickMenu3 did not resolve to an AppEvent");
        };
        assert_eq!(event, "show_quick_menu_3");
        assert!(resolve_app("showQuickMenu").is_none());
    }

    #[test]
    fn resolve_show_emoji_menu_page() {
        let Some(SystemAction::AppEvent(event)) = resolve_app("showEmojiMenu12") else {
            panic!("showEmojiMenu12 did not resolve to an AppEvent");
        };
        assert_eq!(event, "show_emoji_menu_12");
        assert!(resolve_app("showEmojiMenu").is_none());
    }

    #[test]
    fn app_ids_are_not_system_ids() {
        for id in [
            "showQuickMenu3",
            "showEmojiMenu5",
            "taskEntry1",
            "lockSession",
            "volumeUp",
        ] {
            assert!(!is_known(id), "{id}");
        }
        assert!(is_known("switchDesktop10") && is_known("screenOff"));
        // The same numbered range as the action picker.
        assert!(!is_known("switchDesktop11") && !is_known("windowToDesktop11"));
        for entry in crate::profile::actions::system_actions() {
            let crate::profile::actions::Action::System(id) = entry.action else {
                panic!("system catalog holds only sys: actions");
            };
            assert!(is_known(&id), "{id}");
        }
        assert!(matches!(
            resolve_app("showSettings"),
            Some(SystemAction::AppEvent(_))
        ));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::{DbusArg, SystemAction, is_known, kde_shortcut, resolve_for_desktop};
    use crate::platform::linux::Desktop;

    #[test]
    fn known_ids_match_the_kde_catalog() {
        for prefix in [
            "switchDesktop",
            "windowToDesktop",
            "switchLayout",
            "windowClose",
        ] {
            for n in 0..=25 {
                let id = if prefix == "windowClose" {
                    prefix.to_string()
                } else {
                    format!("{prefix}{n}")
                };
                assert_eq!(
                    is_known(&id),
                    resolve_for_desktop(&id, &Desktop::Kde).is_some(),
                    "{id}"
                );
            }
        }
    }
    #[test]
    fn gnome_actions_use_only_the_fixed_bridge_api() {
        for name in [
            "switchLayout1",
            "switchDesktop2",
            "windowToDesktop3",
            "windowClose",
            "windowMaximizeVertical",
            "windowToNextDesktop",
            "overview",
        ] {
            let Some(SystemAction::Dbus(call)) = resolve_for_desktop(name, &Desktop::Gnome) else {
                panic!("{name} did not resolve");
            };
            assert_eq!(call.destination, "org.gnome.Shell");
            assert_eq!(call.path, "/dev/bozonx/LeftHandControl/Gnome");
            assert_eq!(call.method, "Action");
            assert!(matches!(call.args.as_slice(), [DbusArg::Str(value)] if value == name));
        }
        for name in [
            "showDesktop",
            "screenOff",
            "switchLayout0",
            "switchLayout11",
            "switchDesktop21",
            "Eval",
            "global.display.focus_window.delete(0)",
        ] {
            assert!(resolve_for_desktop(name, &Desktop::Gnome).is_none());
        }
    }

    #[test]
    fn switch_layout_uses_zero_based_kde_index() {
        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("switchLayout1", &Desktop::Kde)
        else {
            panic!("switchLayout1 did not resolve to a DBus action");
        };
        assert_eq!(call.method, "setLayout");
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(0)]));

        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("switchLayout3", &Desktop::Kde)
        else {
            panic!("switchLayout3 did not resolve to a DBus action");
        };
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(2)]));
    }

    #[test]
    fn window_to_desktop_and_kde_shortcuts_use_kglobalaccel() {
        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("windowToDesktop3", &Desktop::Kde)
        else {
            panic!("windowToDesktop3 did not resolve to a DBus action");
        };
        assert_eq!(call.destination, "org.kde.kglobalaccel");
        assert_eq!(call.path, "/component/kwin");
        assert_eq!(
            call.interface.as_deref(),
            Some("org.kde.kglobalaccel.Component")
        );
        assert_eq!(call.method, "invokeShortcut");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Window to Desktop 3"));

        let Some(SystemAction::Dbus(call)) = kde_shortcut("org_kde_powerdevil/Turn Off Screen")
        else {
            panic!("kde shortcut did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/org_kde_powerdevil");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Turn Off Screen"));
        assert!(kde_shortcut("Turn Off Screen").is_none());

        for removed in ["taskEntry1", "lockSession", "volumeUp", "toggleOverview"] {
            assert!(resolve_for_desktop(removed, &Desktop::Kde).is_none());
        }
    }

    #[test]
    fn plain_system_actions_resolve_on_kde() {
        for id in crate::profile::actions::PLAIN_SYSTEM_ACTIONS {
            assert!(
                matches!(
                    resolve_for_desktop(id, &Desktop::Kde),
                    Some(SystemAction::Dbus(_))
                ),
                "{id} has no KDE implementation"
            );
        }
        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("screenOff", &Desktop::Kde) else {
            panic!("screenOff did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/org_kde_powerdevil");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Turn Off Screen"));
    }
}

// --- GNOME (Mutter / Shell) -------------------------------------------------

#[cfg(target_os = "linux")]
mod gnome {
    use super::{DbusArg, DbusCall, SystemAction};

    pub fn resolve(name: &str) -> Option<SystemAction> {
        if !super::is_known(name) || matches!(name, "showDesktop" | "screenOff") {
            return None;
        }
        Some(SystemAction::Dbus(DbusCall {
            destination: crate::platform::gnome::SERVICE.into(),
            path: crate::platform::gnome::PATH.into(),
            interface: Some(crate::platform::gnome::INTERFACE.into()),
            method: "Action".into(),
            args: vec![DbusArg::Str(name.into())],
        }))
    }
}

// --- Sway / wlroots ---------------------------------------------------------

#[cfg(target_os = "linux")]
mod sway {
    use super::SystemAction;

    pub fn resolve(name: &str) -> Option<SystemAction> {
        log::debug!("[sys/sway] {name:?}: not implemented yet");
        None
    }
}

// --- Generic X11 / EWMH -----------------------------------------------------

#[cfg(target_os = "linux")]
mod x11_generic {
    use super::SystemAction;

    pub fn resolve(name: &str) -> Option<SystemAction> {
        // TODO: spawn `wmctrl -s (N-1)` (EWMH-compliant WMs: i3, Openbox,
        // Fluxbox, Xfwm, Mutter-on-X11, Marco, Metacity, Kwin-on-X11).
        log::debug!("[sys/x11] {name:?}: not implemented yet");
        None
    }
}
