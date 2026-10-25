//! Terminal emulators: Ctrl+V does not paste there, and Shift+Insert often
//! pastes the mouse selection, so text input uses a separate paste shortcut.

use super::ActiveWindow;

/// Last dot-separated segments of terminal app ids and process names.
/// Names containing "terminal" (GNOME Terminal, xfce4-terminal, …) match too.
const TERMINALS: &[&str] = &[
    "konsole",
    "yakuake",
    "alacritty",
    "kitty",
    "foot",
    "footclient",
    "wezterm",
    "wezterm-gui",
    "xterm",
    "uxterm",
    "urxvt",
    "rxvt",
    "terminator",
    "tilix",
    "ghostty",
    "ptyxis",
    "kgx",
    "console",
    "contour",
    "rio",
    "st",
    "st-256color",
    "terminology",
    "cool-retro-term",
    "qterminal",
    "blackbox",
    "guake",
    "tilda",
    "sakura",
    "cosmicterm",
    "cosmic-term",
    "warp",
];

pub(super) fn is_terminal(window: &ActiveWindow) -> bool {
    std::iter::once(window.app_id.as_str())
        .chain(window.process_name.as_deref())
        .any(is_terminal_name)
}

fn is_terminal_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    let name = name.strip_suffix(".desktop").unwrap_or(&name);
    let last = name.rsplit('.').next().unwrap_or_default();
    TERMINALS.contains(&last) || name.contains("terminal")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(app_id: &str, process: Option<&str>) -> ActiveWindow {
        ActiveWindow {
            app_id: app_id.into(),
            process_name: process.map(Into::into),
            ..ActiveWindow::default()
        }
    }

    #[test]
    fn terminals_are_recognised_by_app_id_or_process() {
        for app_id in [
            "org.kde.konsole",
            "Alacritty",
            "kitty",
            "org.gnome.Terminal",
            "org.gnome.Console",
            "com.mitchellh.ghostty",
            "org.wezfurlong.wezterm",
            "xfce4-terminal",
            "org.kde.yakuake.desktop",
        ] {
            assert!(is_terminal(&window(app_id, None)), "{app_id}");
        }
        assert!(is_terminal(&window("", Some("gnome-terminal-server"))));
        assert!(is_terminal(&window("unknown", Some("foot"))));
    }

    #[test]
    fn other_apps_are_not_terminals() {
        for app_id in ["org.kde.kate", "firefox", "org.kde.dolphin", "code", ""] {
            assert!(!is_terminal(&window(app_id, Some("kate"))), "{app_id}");
        }
    }
}
