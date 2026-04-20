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
    use super::{DbusArg, DbusCall, SysAction};

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
                        args: vec![DbusArg::U32(n)],
                    }));
                }
            }
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
