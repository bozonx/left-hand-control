// System functions: OS-level actions that each desktop environment
// implements its own way. Function names (`switchDesktop1`, ...) are
// cross-platform; the actual shell command is picked per OS/DE.
//
// Currently supported: KDE Plasma on Linux (via `qdbus`).

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

    // KDE virtual-desktop switching: switchDesktop1 .. switchDesktop20.
    if let Some(rest) = name.strip_prefix("switchDesktop") {
        if let Ok(n) = rest.parse::<u32>() {
            if (1..=20).contains(&n) && is_kde() {
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

fn is_kde() -> bool {
    if let Ok(v) = std::env::var("XDG_CURRENT_DESKTOP") {
        if v.to_ascii_uppercase().split(':').any(|s| s == "KDE") {
            return true;
        }
    }
    std::env::var("KDE_FULL_SESSION").is_ok()
}
