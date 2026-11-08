//! System functions: OS-level actions that each desktop environment
//! implements its own way. Function names (`switchToDesktop1`, …) stay
//! cross-platform; the concrete action is picked per OS, and on Linux per
//! DE by dispatching through `crate::platform::linux::detect()`.
//!
//! ```text
//! Only actions a key cannot express live here: switch to desktop / layout N,
//! move window to desktop N, and window/shell actions that at least two
//! platforms document (close, minimize, maximize, keep above, next/previous
//! desktop, next screen, show desktop, overview, application launcher,
//! screen off, lock). Everything else is a standard key, a chord, a `kde:`
//! global shortcut or an `xdg:` application action. See
//! `docs/design/removed-system-actions.md`.
//!
//! Action shapes:
//!   * `SystemAction::Dbus`    — Linux session-bus method call. Preferred when
//!                            the target DE exposes a DBus service: avoids
//!                            `fork+exec` of helper binaries (qdbus, gdbus)
//!                            and gives sub-millisecond latency.
//!   * `SystemAction::Process` — a Linux child process (`xdg:` actions of
//!                            applications that are not D-Bus activatable).
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
    /// An empty `av`.
    EmptyVariants,
    /// An empty `a{sv}`.
    EmptyVariantDict,
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
    #[cfg(target_os = "linux")]
    Process(ProcessInvocation),
    #[cfg(target_os = "windows")]
    Windows(super::windows::system::WindowsAction),
    AppEvent(crate::config::app_action::AppAction),
}

impl SystemAction {
    /// Whether the shell answers this action by showing a popup that takes
    /// the keyboard focus.
    pub fn opens_popup(&self) -> bool {
        matches!(self, Self::AppEvent(crate::config::app_action::AppAction::ShowMenu { .. }))
    }
}

/// Resolve a system-function id (e.g. "switchToDesktop3") into a concrete
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
    crate::config::actions::system_action_name(name.trim()).is_some()
}

pub fn resolve_app(name: &str) -> Option<SystemAction> {
    crate::config::app_action::AppAction::parse(name).map(SystemAction::AppEvent)
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

/// Resolve an `xdg:` id (`desktop-id/action`) into a D-Bus activation or
/// a process, or `None` off Linux or when the application or action is
/// not installed.
pub fn resolve_desktop_entry(id: &str) -> Option<SystemAction> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::desktop_entries::{self, Launch};
        let (app, action) = crate::config::actions::desktop_entry_parts(id.trim())?;
        Some(match desktop_entries::resolve(app, action)? {
            Launch::Activate {
                bus_name,
                path,
                action,
            } => SystemAction::Dbus(DbusCall {
                destination: bus_name,
                path,
                interface: Some("org.freedesktop.Application".into()),
                method: "ActivateAction".into(),
                args: vec![
                    DbusArg::Str(action),
                    DbusArg::EmptyVariants,
                    DbusArg::EmptyVariantDict,
                ],
            }),
            Launch::Exec(mut argv) => {
                if argv.is_empty() {
                    return None;
                }
                let program = argv.remove(0);
                SystemAction::Process(ProcessInvocation {
                    program,
                    args: argv,
                    working_directory: None,
                })
            }
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = id;
        None
    }
}

/// Whether the KDE global shortcut `component/name` is what a plain `sys:`
/// action already runs on KDE; the `kde:` list hides these duplicates.
#[cfg(target_os = "linux")]
pub(crate) fn kde_runs_as_system_action(component: &str, name: &str) -> bool {
    kde::SHORTCUT_ACTIONS
        .iter()
        .any(|(_, c, n)| *c == component && *n == name)
        || (component == "ksmserver" && name == "Lock Session")
}

#[cfg(target_os = "linux")]
fn kde_shortcut(id: &str) -> Option<SystemAction> {
    let (component, shortcut) = crate::config::actions::kde_shortcut_parts(id.trim())?;
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
    use crate::config::actions::SYSTEM_NUMBERED_COUNT;

    /// Plain `sys:` actions KWin, Plasma and PowerDevil register as
    /// kglobalaccel shortcuts: (id, component, shortcut name).
    pub(super) const SHORTCUT_ACTIONS: [(&str, &str, &str); 15] = [
        ("closeWindow", "kwin", "Window Close"),
        ("minimizeWindow", "kwin", "Window Minimize"),
        ("maximizeWindow", "kwin", "Window Maximize"),
        ("maximizeWindowVertically", "kwin", "Window Maximize Vertical"),
        ("toggleWindowKeepAbove", "kwin", "Window Above Other Windows"),
        ("moveWindowToNextDesktop", "kwin", "Window to Next Desktop"),
        ("moveWindowToPreviousDesktop", "kwin", "Window to Previous Desktop"),
        ("moveWindowToNextScreen", "kwin", "Window to Next Screen"),
        ("moveWindowToPreviousScreen", "kwin", "Window to Previous Screen"),
        ("switchToNextDesktop", "kwin", "Switch to Next Desktop"),
        ("switchToPreviousDesktop", "kwin", "Switch to Previous Desktop"),
        ("showDesktop", "kwin", "Show Desktop"),
        ("showOverview", "kwin", "Overview"),
        ("showAppLauncher", "plasmashell", "activate application launcher"),
        ("turnOffScreen", "org_kde_powerdevil", "Turn Off Screen"),
    ];

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
        let numbered = |prefix: &str| {
            name.strip_prefix(prefix)
                .and_then(|rest| rest.parse::<u32>().ok())
                .filter(|n| (1..=SYSTEM_NUMBERED_COUNT).contains(n))
        };
        // KWin registers both families as global shortcuts; the same path
        // as every other window action, instead of the legacy
        // org.kde.KWin.setCurrentDesktop.
        if let Some(n) = numbered("switchToDesktop") {
            return Some(invoke_shortcut("kwin", &format!("Switch to Desktop {n}")));
        }
        if let Some(n) = numbered("moveWindowToDesktop") {
            return Some(invoke_shortcut("kwin", &format!("Window to Desktop {n}")));
        }
        if let Some(n) = numbered("switchKeyboardLayout") {
            return Some(SystemAction::Dbus(DbusCall {
                destination: dbus::KEYBOARD_SERVICE.into(),
                path: dbus::KEYBOARD_LAYOUTS_PATH.into(),
                interface: Some(dbus::KEYBOARD_LAYOUTS_INTERFACE.into()),
                method: dbus::KEYBOARD_SET_LAYOUT.into(),
                args: vec![DbusArg::U32(n - 1)],
            }));
        }
        if name == "lockSession" {
            // The freedesktop screen saver API, served by KScreenLocker.
            return Some(SystemAction::Dbus(DbusCall {
                destination: "org.freedesktop.ScreenSaver".into(),
                path: "/ScreenSaver".into(),
                interface: Some("org.freedesktop.ScreenSaver".into()),
                method: "Lock".into(),
                args: vec![],
            }));
        }
        SHORTCUT_ACTIONS
            .iter()
            .find(|(id, _, _)| *id == name)
            .map(|(_, component, shortcut)| invoke_shortcut(component, shortcut))
    }
}

#[cfg(test)]
mod tests {
    use super::{SystemAction, is_known, resolve_app};
    use crate::config::app_action::{AppAction, GameModeRequest, Menu};
    use std::num::NonZeroU32;
    #[test]
    fn resolve_window_and_profile_actions() {
        for (name, event) in [
            ("showQuickMenuRecent", AppAction::ShowMenu { menu: Menu::Quick, page: None }),
            ("showEmojiMenuRecent", AppAction::ShowMenu { menu: Menu::Emoji, page: None }),
            ("showSettings", AppAction::ShowSettings),
            ("activateProfile1", AppAction::ActivateProfile(NonZeroU32::new(1).unwrap())),
            ("activateProfile5", AppAction::ActivateProfile(NonZeroU32::new(5).unwrap())),
        ] {
            let Some(SystemAction::AppEvent(resolved)) = resolve_app(name) else {
                panic!("{name} did not resolve to an AppEvent");
            };
            assert_eq!(resolved, event);
            assert!(crate::config::actions::app_action_name(name).is_some());
        }
        assert!(resolve_app("activateProfile6").is_none());
    }

    #[test]
    fn resolve_game_mode_actions() {
        for (name, event) in [
            ("toggleGameMode", AppAction::GameMode(GameModeRequest::Toggle)),
            ("setGameModeAuto", AppAction::GameMode(GameModeRequest::Auto)),
            ("setGameModeOn", AppAction::GameMode(GameModeRequest::On)),
            ("setGameModeOff", AppAction::GameMode(GameModeRequest::Off)),
        ] {
            let Some(SystemAction::AppEvent(resolved)) = resolve_app(name) else {
                panic!("{name} did not resolve to an AppEvent");
            };
            assert_eq!(resolved, event);
            assert!(crate::config::actions::app_action_name(name).is_some());
        }
    }

    #[test]
    fn resolve_show_quick_menu_page() {
        let Some(SystemAction::AppEvent(event)) = resolve_app("showQuickMenu3") else {
            panic!("showQuickMenu3 did not resolve to an AppEvent");
        };
        assert_eq!(event, AppAction::ShowMenu { menu: Menu::Quick, page: NonZeroU32::new(3) });
        assert!(resolve_app("showQuickMenu").is_none());
    }

    #[test]
    fn resolve_show_emoji_menu_page() {
        let Some(SystemAction::AppEvent(event)) = resolve_app("showEmojiMenu12") else {
            panic!("showEmojiMenu12 did not resolve to an AppEvent");
        };
        assert_eq!(event, AppAction::ShowMenu { menu: Menu::Emoji, page: NonZeroU32::new(12) });
        assert!(resolve_app("showEmojiMenu").is_none());
    }

    #[test]
    fn app_ids_are_not_system_ids() {
        for id in [
            "showQuickMenu3",
            "showEmojiMenu5",
            "taskEntry1",
            "lockScreen",
            "volumeUp",
        ] {
            assert!(!is_known(id), "{id}");
        }
        assert!(is_known("switchToDesktop10") && is_known("turnOffScreen"));
        // The same numbered range as the action picker.
        assert!(!is_known("switchToDesktop11") && !is_known("moveWindowToDesktop11"));
        for entry in crate::config::actions::system_actions() {
            let crate::config::actions::Action::System(id) = entry.action else {
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
            "switchToDesktop",
            "moveWindowToDesktop",
            "switchKeyboardLayout",
            "closeWindow",
        ] {
            for n in 0..=25 {
                let id = if prefix == "closeWindow" {
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
            "switchKeyboardLayout1",
            "switchToDesktop2",
            "moveWindowToDesktop3",
            "closeWindow",
            "maximizeWindowVertically",
            "moveWindowToNextDesktop",
            "moveWindowToPreviousDesktop",
            "moveWindowToNextScreen",
            "moveWindowToPreviousScreen",
            "switchToNextDesktop",
            "showOverview",
            "turnOffScreen",
            "lockSession",
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
            "switchKeyboardLayout0",
            "switchKeyboardLayout11",
            "switchToDesktop21",
            "Eval",
            "global.display.focus_window.delete(0)",
        ] {
            assert!(resolve_for_desktop(name, &Desktop::Gnome).is_none());
        }
    }

    #[test]
    fn switch_layout_uses_zero_based_kde_index() {
        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("switchKeyboardLayout1", &Desktop::Kde)
        else {
            panic!("switchKeyboardLayout1 did not resolve to a DBus action");
        };
        assert_eq!(call.method, "setLayout");
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(0)]));

        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("switchKeyboardLayout3", &Desktop::Kde)
        else {
            panic!("switchKeyboardLayout3 did not resolve to a DBus action");
        };
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(2)]));
    }

    #[test]
    fn window_to_desktop_and_kde_shortcuts_use_kglobalaccel() {
        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("moveWindowToDesktop3", &Desktop::Kde)
        else {
            panic!("moveWindowToDesktop3 did not resolve to a DBus action");
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

        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("switchToDesktop2", &Desktop::Kde)
        else {
            panic!("switchToDesktop2 did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/kwin");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Switch to Desktop 2"));

        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("lockSession", &Desktop::Kde)
        else {
            panic!("lockSession did not resolve to a DBus action");
        };
        assert_eq!(call.destination, "org.freedesktop.ScreenSaver");
        assert_eq!(call.method, "Lock");

        for removed in ["taskEntry1", "volumeUp", "toggleOverview"] {
            assert!(resolve_for_desktop(removed, &Desktop::Kde).is_none());
        }
    }

    #[test]
    fn plain_system_actions_resolve_on_kde() {
        for id in crate::config::actions::PLAIN_SYSTEM_ACTIONS {
            assert!(
                matches!(
                    resolve_for_desktop(id, &Desktop::Kde),
                    Some(SystemAction::Dbus(_))
                ),
                "{id} has no KDE implementation"
            );
        }
        let Some(SystemAction::Dbus(call)) = resolve_for_desktop("turnOffScreen", &Desktop::Kde) else {
            panic!("turnOffScreen did not resolve to a DBus action");
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
        // GNOME has no "show desktop" mode.
        if !super::is_known(name) || name == "showDesktop" {
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
