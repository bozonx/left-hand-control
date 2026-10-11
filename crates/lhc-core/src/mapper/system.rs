// System functions: OS-level actions that each desktop environment
// implements its own way. Function names (`switchDesktop1`, …) stay
// cross-platform; the concrete action (DBus call or shell command) is
// picked per OS/DE by dispatching through `crate::platform::linux::detect()`.
//
// Only actions a key cannot express live here (switch to desktop / layout N,
// move window to desktop N); everything else is a standard key, a chord or
// a `kde:` global shortcut. See `dev_docs/removed-system-actions.md`.
//
// Two action shapes are supported:
//   * `SysAction::Dbus` — direct session-bus method call. Preferred when
//                         the target DE exposes a DBus service: avoids
//                         `fork+exec` of helper binaries (qdbus, gdbus)
//                         and gives sub-millisecond latency.
//   * `SysAction::Spawn` — fire-and-forget child process. Used for DEs
//                          whose control surface is a CLI tool
//                          (swaymsg, wmctrl, …).
//
// Current product support:
//   * Linux + KDE   — implemented.
//   * Linux + GNOME / Sway / X11 generic — intentional skeletons for now.
//   * Windows / macOS — intentional stubs; the mapper module itself is not
//                       shipped there yet, so this file stays Linux-only.

#![cfg(target_os = "linux")]

/// A concrete child-process invocation: `program` + positional args.
#[derive(Clone, Debug)]
pub struct SysCommand {
    pub program: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
}

/// Arguments for a DBus method call. Only the shapes we actually need
/// are listed; extend as new backends come online.
#[derive(Clone, Debug)]
#[allow(dead_code)] // I32/Bool/Str reserved for future backends (GNOME Shell.Eval, etc.)
pub enum DbusArg {
    U32(u32),
    I32(i32),
    Bool(bool),
    Str(String),
}

/// A direct session-bus method invocation.
#[derive(Clone, Debug)]
pub struct DbusCall {
    pub destination: String,
    pub path: String,
    pub interface: Option<String>,
    pub method: String,
    pub args: Vec<DbusArg>,
}

/// Resolved system-function: either a DBus call or a child-process spawn.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Spawn reserved for future Sway / wmctrl backends.
pub enum SysAction {
    Dbus(DbusCall),
    Spawn(SysCommand),
    AppEvent(String),
}

/// Resolve a system-function id (e.g. "switchDesktop3") into a concrete
/// action to run on this machine, or `None` if the function is not
/// available under the current desktop environment.
pub fn resolve(name: &str) -> Option<SysAction> {
    use crate::platform::linux::detect;
    resolve_for_desktop(name.trim(), &detect().desktop)
}

pub fn is_known(name: &str) -> bool {
    let name = name.trim();
    use crate::platform::linux::Desktop;
    // Check against the KDE catalog, which is the most complete backend.
    // This keeps config validation portable: a config created on KDE
    // (or using KDE-known ids) is not rejected when the mapper starts
    // on GNOME / Sway / X11 generic. At runtime `resolve()` will return
    // None there and the action becomes a no-op with a log line.
    resolve_for_desktop(name, &Desktop::Kde).is_some()
}

pub fn resolve_app(name: &str) -> Option<SysAction> {
    let name = name.trim();
    for page in 1..=5 {
        if name == format!("showQuickMenu{page}") {
            return Some(SysAction::AppEvent(format!("show_quick_menu_{page}")));
        }
        if name == format!("showEmojiMenu{page}") {
            return Some(SysAction::AppEvent(format!("show_emoji_menu_{page}")));
        }
        if name == format!("activateLayout{page}") {
            return Some(SysAction::AppEvent(format!("activate_layout_{page}")));
        }
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
    Some(SysAction::AppEvent(event.into()))
}

pub fn is_known_app(name: &str) -> bool {
    resolve_app(name).is_some()
}

/// Resolve a `kde:` id (`component/shortcut`) into a kglobalaccel call,
/// or `None` outside KDE or when the id is malformed.
pub fn resolve_kde_shortcut(id: &str) -> Option<SysAction> {
    use crate::platform::linux::{Desktop, detect};
    if !matches!(detect().desktop, Desktop::Kde) {
        return None;
    }
    kde_shortcut(id)
}

fn kde_shortcut(id: &str) -> Option<SysAction> {
    let (component, shortcut) = crate::profile::actions::kde_shortcut_parts(id.trim())?;
    Some(kde::invoke_shortcut(component, shortcut))
}

fn resolve_for_desktop(name: &str, desktop: &crate::platform::linux::Desktop) -> Option<SysAction> {
    let name = name.trim();

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

mod kde {
    use super::{DbusArg, DbusCall, SysAction};

    pub(super) fn invoke_shortcut(component: &str, shortcut: &str) -> SysAction {
        SysAction::Dbus(DbusCall {
            destination: "org.kde.kglobalaccel".into(),
            path: format!("/component/{component}"),
            interface: Some("org.kde.kglobalaccel.Component".into()),
            method: "invokeShortcut".into(),
            args: vec![DbusArg::Str(shortcut.into())],
        })
    }

    pub fn resolve(name: &str) -> Option<SysAction> {
        if let Some(rest) = name.strip_prefix("switchDesktop") {
            if let Ok(n) = rest.parse::<u32>() {
                if (1..=20).contains(&n) {
                    // org.kde.KWin /KWin setCurrentDesktop(i)
                    return Some(SysAction::Dbus(DbusCall {
                        destination: "org.kde.KWin".into(),
                        path: "/KWin".into(),
                        interface: Some("org.kde.KWin".into()),
                        method: "setCurrentDesktop".into(),
                        args: vec![DbusArg::I32(n as i32)],
                    }));
                }
            }
        }
        if let Some(rest) = name.strip_prefix("switchLayout") {
            if let Ok(n) = rest.parse::<u32>() {
                if (1..=10).contains(&n) {
                    return Some(SysAction::Dbus(DbusCall {
                        destination: "org.kde.keyboard".into(),
                        path: "/Layouts".into(),
                        interface: Some("org.kde.KeyboardLayouts".into()),
                        method: "setLayout".into(),
                        args: vec![DbusArg::U32(n - 1)],
                    }));
                }
            }
        }
        if let Some(rest) = name.strip_prefix("windowToDesktop") {
            if let Ok(n) = rest.parse::<u32>() {
                if (1..=20).contains(&n) {
                    return Some(invoke_shortcut("kwin", &format!("Window to Desktop {n}")));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{DbusArg, SysAction, kde_shortcut, resolve_app, resolve_for_desktop};
    use crate::platform::linux::Desktop;

    #[test]
    fn resolve_show_quick_menu_page() {
        let Some(SysAction::AppEvent(event)) = resolve_app("showQuickMenu3") else {
            panic!("showQuickMenu3 did not resolve to an AppEvent");
        };
        assert_eq!(event, "show_quick_menu_3");
        assert!(resolve_app("showQuickMenu").is_none());
        assert!(resolve_for_desktop("showQuickMenu3", &Desktop::Kde).is_none());
    }

    #[test]
    fn resolve_show_emoji_menu_page() {
        let Some(SysAction::AppEvent(event)) = resolve_app("showEmojiMenu5") else {
            panic!("showEmojiMenu5 did not resolve to an AppEvent");
        };
        assert_eq!(event, "show_emoji_menu_5");
        assert!(resolve_app("showEmojiMenu").is_none());
        assert!(resolve_for_desktop("showEmojiMenu5", &Desktop::Kde).is_none());
    }

    #[test]
    fn resolve_window_and_layout_actions() {
        for (name, event) in [
            ("showQuickMenuLast", "show_quick_menu_last"),
            ("showEmojiMenuLast", "show_emoji_menu_last"),
            ("showSettings", "show_settings"),
            ("activateLayout1", "activate_layout_1"),
            ("activateLayout5", "activate_layout_5"),
        ] {
            let Some(SysAction::AppEvent(resolved)) = resolve_app(name) else {
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
            let Some(SysAction::AppEvent(resolved)) = resolve_app(name) else {
                panic!("{name} did not resolve to an AppEvent");
            };
            assert_eq!(resolved, event);
            assert!(crate::profile::actions::app_action_name(name).is_some());
        }
    }

    #[test]
    fn switch_layout_uses_zero_based_kde_index() {
        let Some(SysAction::Dbus(call)) = resolve_for_desktop("switchLayout1", &Desktop::Kde)
        else {
            panic!("switchLayout1 did not resolve to a DBus action");
        };
        assert_eq!(call.method, "setLayout");
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(0)]));

        let Some(SysAction::Dbus(call)) = resolve_for_desktop("switchLayout3", &Desktop::Kde)
        else {
            panic!("switchLayout3 did not resolve to a DBus action");
        };
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(2)]));
    }

    #[test]
    fn window_to_desktop_and_kde_shortcuts_use_kglobalaccel() {
        let Some(SysAction::Dbus(call)) = resolve_for_desktop("windowToDesktop3", &Desktop::Kde)
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

        let Some(SysAction::Dbus(call)) = kde_shortcut("org_kde_powerdevil/Turn Off Screen") else {
            panic!("kde shortcut did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/org_kde_powerdevil");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Turn Off Screen"));
        assert!(kde_shortcut("Turn Off Screen").is_none());

        for removed in ["taskEntry1", "lockSession", "volumeUp", "screenOff"] {
            assert!(resolve_for_desktop(removed, &Desktop::Kde).is_none());
        }
    }
}

// --- GNOME (Mutter / Shell) -------------------------------------------------

mod gnome {
    use super::SysAction;

    pub fn resolve(name: &str) -> Option<SysAction> {
        // TODO: implement. Two approaches:
        //   1. Wayland + Shell extensions allowing Eval:
        //        org.gnome.Shell / /org/gnome/Shell / org.gnome.Shell.Eval
        //        ("global.workspace_manager \
        //           .get_workspace_by_index(N-1) \
        //           .activate(global.get_current_time())")
        //      On stock GNOME 41+ Eval is restricted in user mode.
        //   2. X11 sessions: spawn `wmctrl -s (N-1)`.
        log::debug!("[sys/gnome] {name:?}: not implemented yet");
        None
    }
}

// --- Sway / wlroots ---------------------------------------------------------

mod sway {
    use super::SysAction;

    pub fn resolve(name: &str) -> Option<SysAction> {
        // TODO: `swaymsg workspace number N` covers the common case.
        // Implementation sketch:
        //   if let Some(rest) = name.strip_prefix("switchDesktop") {
        //       if let Ok(n) = rest.parse::<u32>() {
        //           return Some(SysAction::Spawn(SysCommand {
        //               program: "swaymsg".into(),
        //               args: vec!["workspace".into(), "number".into(), n.to_string()],
        //           }));
        //       }
        //   }
        log::debug!("[sys/sway] {name:?}: not implemented yet");
        None
    }
}

// --- Generic X11 / EWMH -----------------------------------------------------

mod x11_generic {
    use super::SysAction;

    pub fn resolve(name: &str) -> Option<SysAction> {
        // TODO: spawn `wmctrl -s (N-1)` (EWMH-compliant WMs: i3, Openbox,
        // Fluxbox, Xfwm, Mutter-on-X11, Marco, Metacity, Kwin-on-X11).
        log::debug!("[sys/x11] {name:?}: not implemented yet");
        None
    }
}
