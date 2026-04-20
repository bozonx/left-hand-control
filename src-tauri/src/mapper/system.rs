// System functions: OS-level actions that each desktop environment
// implements its own way. Function names (`switchDesktop1`, …) stay
// cross-platform; the concrete action (DBus call or shell command) is
// picked per OS/DE by dispatching through `crate::platform::linux::detect()`.
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
// Current coverage:
//   * Linux + KDE   — DBus (`org.kde.KWin` → `setCurrentDesktop(i)`).
//   * Linux + GNOME — skeleton (TODO: DBus `org.gnome.Shell.Eval` or
//                     wmctrl fallback on X11 sessions).
//   * Linux + Sway  — skeleton (TODO: spawn `swaymsg workspace number N`).
//   * Linux + X11   — skeleton (TODO: spawn `wmctrl -s N`).
//   * Windows / macOS — not applicable yet; the mapper module itself is
//                       a stub there so this file is Linux-only.

#![cfg(target_os = "linux")]

/// A concrete child-process invocation: `program` + positional args.
#[derive(Clone, Debug)]
pub struct SysCommand {
    pub program: String,
    pub args: Vec<String>,
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
}

/// Resolve a system-function id (e.g. "switchDesktop3") into a concrete
/// action to run on this machine, or `None` if the function is not
/// available under the current desktop environment.
pub fn resolve(name: &str) -> Option<SysAction> {
    let name = name.trim();
    use crate::platform::linux::{detect, Desktop};
    match detect().desktop {
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
    use super::{DbusArg, DbusCall, SysAction, SysCommand};

    fn invoke_shortcut(component: &str, shortcut: &str) -> SysAction {
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
        if let Some(rest) = name.strip_prefix("taskEntry") {
            if let Ok(n) = rest.parse::<u32>() {
                if (1..=10).contains(&n) {
                    return Some(invoke_shortcut(
                        "plasmashell",
                        &format!("activate task manager entry {n}"),
                    ));
                }
            }
        }
        match name {
            "walkThroughWindowsAlternative" => {
                return Some(invoke_shortcut("kwin", "Walk Through Windows Alternative"));
            }
            "walkThroughWindowsCurrentApp" => {
                return Some(invoke_shortcut(
                    "kwin",
                    "Walk Through Windows of Current Application",
                ));
            }
            "volumeDown" => return Some(invoke_shortcut("kmix", "decrease_volume")),
            "volumeUp" => return Some(invoke_shortcut("kmix", "increase_volume")),
            "muteAudio" => return Some(invoke_shortcut("kmix", "mute")),
            "brightnessDown" => {
                return Some(invoke_shortcut(
                    "org_kde_powerdevil",
                    "Decrease Screen Brightness",
                ));
            }
            "brightnessUp" => {
                return Some(invoke_shortcut(
                    "org_kde_powerdevil",
                    "Increase Screen Brightness",
                ));
            }
            "windowClose" => return Some(invoke_shortcut("kwin", "Window Close")),
            "windowToNextDesktop" => {
                return Some(invoke_shortcut("kwin", "Window to Next Desktop"));
            }
            "windowKeepAbove" => {
                return Some(invoke_shortcut("kwin", "Window Above Other Windows"));
            }
            "windowMaximizeVertical" => {
                return Some(invoke_shortcut("kwin", "Window Maximize Vertical"));
            }
            "windowMaximizeHorizontal" => {
                return Some(invoke_shortcut("kwin", "Window Maximize Horizontal"));
            }
            "screenshot" => {
                return Some(SysAction::Spawn(SysCommand {
                    program: "spectacle".into(),
                    args: vec!["-r".into()],
                }));
            }
            "screenOff" => {
                return Some(invoke_shortcut("org_kde_powerdevil", "Turn Off Screen"));
            }
            _ => {}
        }
        if name == "showClipboardHistory" {
            return Some(SysAction::Dbus(DbusCall {
                destination: "org.kde.plasmashell".into(),
                path: "/klipper".into(),
                interface: Some("org.kde.klipper.klipper".into()),
                method: "showKlipperPopupMenu".into(),
                args: vec![],
            }));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve, DbusArg, SysAction};

    #[test]
    fn switch_layout_uses_zero_based_kde_index() {
        let Some(SysAction::Dbus(call)) = resolve("switchLayout1") else {
            panic!("switchLayout1 did not resolve to a DBus action");
        };
        assert_eq!(call.method, "setLayout");
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(0)]));

        let Some(SysAction::Dbus(call)) = resolve("switchLayout3") else {
            panic!("switchLayout3 did not resolve to a DBus action");
        };
        assert!(matches!(call.args.as_slice(), [DbusArg::U32(2)]));
    }

    #[test]
    fn volume_and_window_actions_resolve_via_kglobalaccel() {
        let Some(SysAction::Dbus(call)) = resolve("walkThroughWindowsAlternative") else {
            panic!("walkThroughWindowsAlternative did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/kwin");
        assert!(matches!(
            call.args.as_slice(),
            [DbusArg::Str(s)] if s == "Walk Through Windows Alternative"
        ));

        let Some(SysAction::Dbus(call)) = resolve("walkThroughWindowsCurrentApp") else {
            panic!("walkThroughWindowsCurrentApp did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/kwin");
        assert!(matches!(
            call.args.as_slice(),
            [DbusArg::Str(s)] if s == "Walk Through Windows of Current Application"
        ));

        let Some(SysAction::Dbus(call)) = resolve("volumeUp") else {
            panic!("volumeUp did not resolve to a DBus action");
        };
        assert_eq!(call.destination, "org.kde.kglobalaccel");
        assert_eq!(call.path, "/component/kmix");
        assert_eq!(call.interface.as_deref(), Some("org.kde.kglobalaccel.Component"));
        assert_eq!(call.method, "invokeShortcut");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "increase_volume"));

        let Some(SysAction::Dbus(call)) = resolve("windowMaximizeVertical") else {
            panic!("windowMaximizeVertical did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/kwin");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Window Maximize Vertical"));

        let Some(SysAction::Dbus(call)) = resolve("windowToNextDesktop") else {
            panic!("windowToNextDesktop did not resolve to a DBus action");
        };
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Window to Next Desktop"));

        let Some(SysAction::Dbus(call)) = resolve("windowKeepAbove") else {
            panic!("windowKeepAbove did not resolve to a DBus action");
        };
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "Window Above Other Windows"));

        let Some(SysAction::Dbus(call)) = resolve("taskEntry3") else {
            panic!("taskEntry3 did not resolve to a DBus action");
        };
        assert_eq!(call.path, "/component/plasmashell");
        assert!(matches!(call.args.as_slice(), [DbusArg::Str(s)] if s == "activate task manager entry 3"));
    }

    #[test]
    fn screenshot_resolves_to_spectacle_spawn() {
        let Some(SysAction::Spawn(cmd)) = resolve("screenshot") else {
            panic!("screenshot did not resolve to a spawned command");
        };
        assert_eq!(cmd.program, "spectacle");
        assert_eq!(cmd.args, vec!["-r"]);
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
        eprintln!("[sys/gnome] {name:?}: not implemented yet");
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
        eprintln!("[sys/sway] {name:?}: not implemented yet");
        None
    }
}

// --- Generic X11 / EWMH -----------------------------------------------------

mod x11_generic {
    use super::SysAction;

    pub fn resolve(name: &str) -> Option<SysAction> {
        // TODO: spawn `wmctrl -s (N-1)` (EWMH-compliant WMs: i3, Openbox,
        // Fluxbox, Xfwm, Mutter-on-X11, Marco, Metacity, Kwin-on-X11).
        eprintln!("[sys/x11] {name:?}: not implemented yet");
        None
    }
}
