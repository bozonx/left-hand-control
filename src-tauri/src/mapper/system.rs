// System functions: OS-level actions that each desktop environment
// implements its own way. Function names (`switchDesktop1`, …) stay
// cross-platform; the actual shell command is picked per OS/DE by
// dispatching through `crate::platform::linux::detect()`.
//
// Current coverage:
//   * Linux + KDE   — implemented via `qdbus org.kde.KWin`.
//   * Linux + GNOME — skeleton (TODO: gdbus to org.gnome.Shell or
//                     wmctrl fallback on X11 sessions).
//   * Linux + Sway  — skeleton (TODO: `swaymsg workspace number N`).
//   * Linux + X11   — skeleton (TODO: `wmctrl -s N`).
//   * Windows / macOS — not applicable yet; the mapper module itself is
//                       a stub there so this file is Linux-only.

#![cfg(target_os = "linux")]

/// A concrete child-process invocation: `program` + positional args.
#[derive(Clone, Debug)]
pub struct SysCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// Resolve a system-function id (e.g. "switchDesktop3") into a concrete
/// command to run on this machine, or `None` if the function is not
/// available under the current desktop environment.
pub fn resolve(name: &str) -> Option<SysCommand> {
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
    use super::SysCommand;

    pub fn resolve(name: &str) -> Option<SysCommand> {
        if let Some(rest) = name.strip_prefix("switchDesktop") {
            if let Ok(n) = rest.parse::<u32>() {
                if (1..=20).contains(&n) {
                    return Some(SysCommand {
                        program: "qdbus".into(),
                        args: vec![
                            "org.kde.KWin".into(),
                            "/KWin".into(),
                            "setCurrentDesktop".into(),
                            n.to_string(),
                        ],
                    });
                }
            }
        }
        None
    }
}

// --- GNOME (Mutter / Shell) -------------------------------------------------

mod gnome {
    use super::SysCommand;

    pub fn resolve(name: &str) -> Option<SysCommand> {
        // TODO: implement. Two approaches:
        //   1. Wayland + Shell extensions allowing Eval:
        //        gdbus call --session --dest org.gnome.Shell \
        //          --object-path /org/gnome/Shell \
        //          --method org.gnome.Shell.Eval \
        //          'global.workspace_manager \
        //             .get_workspace_by_index(N-1) \
        //             .activate(global.get_current_time())'
        //      On stock GNOME 41+ Eval is restricted in user mode.
        //   2. X11 sessions: wmctrl -s (N-1). Requires wmctrl + a
        //      non-minimised number of workspaces.
        eprintln!("[sys/gnome] {name:?}: not implemented yet");
        None
    }
}

// --- Sway / wlroots ---------------------------------------------------------

mod sway {
    use super::SysCommand;

    pub fn resolve(name: &str) -> Option<SysCommand> {
        // TODO: `swaymsg workspace number N` covers the common case.
        // Implementation sketch:
        //   if let Some(rest) = name.strip_prefix("switchDesktop") {
        //       if let Ok(n) = rest.parse::<u32>() {
        //           return Some(SysCommand {
        //               program: "swaymsg".into(),
        //               args: vec!["workspace".into(), "number".into(), n.to_string()],
        //           });
        //       }
        //   }
        eprintln!("[sys/sway] {name:?}: not implemented yet");
        None
    }
}

// --- Generic X11 / EWMH -----------------------------------------------------

mod x11_generic {
    use super::SysCommand;

    pub fn resolve(name: &str) -> Option<SysCommand> {
        // TODO: `wmctrl -s (N-1)` (EWMH-compliant WMs: i3, Openbox, Fluxbox,
        // Xfwm, Mutter-on-X11, Marco, Metacity, Kwin-on-X11).
        eprintln!("[sys/x11] {name:?}: not implemented yet");
        None
    }
}
