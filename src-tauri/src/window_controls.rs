//! Caption-button layout for the app's custom title bar.
//!
//! The window runs with `decorations: false` (see `tauri.conf.json`), so the
//! frontend draws its own title bar. To match what the user configured in
//! their desktop, the button set + side is read from whatever the surrounding
//! environment exposes.
//!
//! Architecture: a small ordered list of [`WindowControlsSource`]s is tried in
//! turn; the first one that yields a layout wins, otherwise a sane per-OS
//! default is used. Adding support for another environment is a matter of
//! implementing the trait and pushing the source into [`sources`] — nothing
//! else changes.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// A caption button the custom title bar knows how to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

/// Resolved title-bar layout handed to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct WindowControlsLayout {
    /// Buttons docked to the left, in visual (left-to-right) order.
    pub left: Vec<WindowButton>,
    /// Buttons docked to the right, in visual (left-to-right) order.
    pub right: Vec<WindowButton>,
    /// Which source produced this layout (`"kde"`, `"gnome"`, `"default"`, …).
    /// Purely informational — handy when debugging on a new desktop.
    pub source: &'static str,
}

/// A place a window-button layout can be read from.
///
/// Implementations must be cheap and free of side effects. Returning `None`
/// means "not applicable here" (e.g. the backing config file is absent) — the
/// next source is then consulted.
trait WindowControlsSource {
    /// Stable identifier, surfaced in [`WindowControlsLayout::source`].
    fn id(&self) -> &'static str;
    /// `config_dir` is the user's XDG config directory (`~/.config`), passed
    /// in so the logic stays unit-testable. Sources that don't need it ignore
    /// the argument.
    fn read(&self, config_dir: &Path) -> Option<(Vec<WindowButton>, Vec<WindowButton>)>;
}

/// Ordered list of sources to consult on the current OS.
fn sources() -> Vec<Box<dyn WindowControlsSource>> {
    #[cfg(target_os = "linux")]
    {
        vec![Box::new(linux::KdeKwinrc), Box::new(linux::GnomeGsettings)]
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

fn platform_default() -> WindowControlsLayout {
    #[cfg(target_os = "macos")]
    {
        WindowControlsLayout {
            left: vec![
                WindowButton::Close,
                WindowButton::Minimize,
                WindowButton::Maximize,
            ],
            right: Vec::new(),
            source: "default",
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        WindowControlsLayout {
            left: Vec::new(),
            right: vec![
                WindowButton::Minimize,
                WindowButton::Maximize,
                WindowButton::Close,
            ],
            source: "default",
        }
    }
}

/// Resolve the title-bar layout, given the user's config directory.
pub fn detect(config_dir: &Path) -> WindowControlsLayout {
    for source in sources() {
        if let Some((left, right)) = source.read(config_dir) {
            // A source that recognises no buttons at all is treated as
            // "not applicable" rather than "the user wants an empty bar".
            if left.is_empty() && right.is_empty() {
                continue;
            }
            return WindowControlsLayout {
                left,
                right,
                source: source.id(),
            };
        }
    }
    platform_default()
}

fn config_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        let dir = PathBuf::from(dir);
        if dir.is_absolute() {
            return Some(dir);
        }
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
}

#[tauri::command]
pub fn get_window_controls_layout() -> WindowControlsLayout {
    match config_home() {
        Some(dir) => detect(&dir),
        None => platform_default(),
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{WindowButton, WindowControlsSource};
    use std::path::Path;

    /// KDE Plasma — `~/.config/kwinrc`, group `[org.kde.kdecoration2]`, keys
    /// `ButtonsOnLeft` / `ButtonsOnRight`. Button codes: `I` minimize, `A`
    /// maximize, `X` close; menu / help / sticky / keep-above / keep-below /
    /// shade / spacer (`M H S F B L _ N` …) are not drawn and ignored. When
    /// the keys are absent KWin uses `MS` / `HIAX`, mirrored here.
    pub struct KdeKwinrc;

    impl KdeKwinrc {
        fn parse_codes(spec: &str) -> Vec<WindowButton> {
            spec.chars()
                .filter_map(|c| match c {
                    'I' => Some(WindowButton::Minimize),
                    'A' => Some(WindowButton::Maximize),
                    'X' => Some(WindowButton::Close),
                    _ => None,
                })
                .collect()
        }

        fn parse_kwinrc(contents: &str) -> (Vec<WindowButton>, Vec<WindowButton>) {
            let mut in_group = false;
            let mut left: Option<String> = None;
            let mut right: Option<String> = None;
            for raw in contents.lines() {
                let line = raw.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    in_group = line == "[org.kde.kdecoration2]";
                    continue;
                }
                if !in_group {
                    continue;
                }
                if let Some(v) = line.strip_prefix("ButtonsOnLeft=") {
                    left = Some(v.to_string());
                } else if let Some(v) = line.strip_prefix("ButtonsOnRight=") {
                    right = Some(v.to_string());
                }
            }
            (
                Self::parse_codes(left.as_deref().unwrap_or("MS")),
                Self::parse_codes(right.as_deref().unwrap_or("HIAX")),
            )
        }
    }

    impl WindowControlsSource for KdeKwinrc {
        fn id(&self) -> &'static str {
            "kde"
        }

        fn read(&self, config_dir: &Path) -> Option<(Vec<WindowButton>, Vec<WindowButton>)> {
            // `kwinrc` is written by KWin — its presence is what marks this as
            // a Plasma session for our purposes.
            let contents = std::fs::read_to_string(config_dir.join("kwinrc")).ok()?;
            Some(Self::parse_kwinrc(&contents))
        }
    }

    /// GNOME and other GTK desktops — `gsettings get
    /// org.gnome.desktop.wm.preferences button-layout`, e.g.
    /// `'appmenu:minimize,maximize,close'`. Tokens left of `:` dock left,
    /// the rest dock right; `appmenu` / `menu` / `icon` / `spacer` are not
    /// drawn and ignored.
    pub struct GnomeGsettings;

    impl GnomeGsettings {
        fn parse_side(spec: &str) -> Vec<WindowButton> {
            spec.split(',')
                .filter_map(|token| match token.trim() {
                    "minimize" => Some(WindowButton::Minimize),
                    "maximize" => Some(WindowButton::Maximize),
                    "close" => Some(WindowButton::Close),
                    _ => None,
                })
                .collect()
        }

        fn parse_layout(value: &str) -> (Vec<WindowButton>, Vec<WindowButton>) {
            let value = value.trim().trim_matches(|c| c == '\'' || c == '"');
            match value.split_once(':') {
                Some((left, right)) => (Self::parse_side(left), Self::parse_side(right)),
                None => (Vec::new(), Self::parse_side(value)),
            }
        }
    }

    impl WindowControlsSource for GnomeGsettings {
        fn id(&self) -> &'static str {
            "gnome"
        }

        fn read(&self, _config_dir: &Path) -> Option<(Vec<WindowButton>, Vec<WindowButton>)> {
            let output = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.wm.preferences", "button-layout"])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let value = String::from_utf8_lossy(&output.stdout);
            Some(Self::parse_layout(&value))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::WindowButton::{Close, Maximize, Minimize};
        use super::{GnomeGsettings, KdeKwinrc};

        #[test]
        fn kwinrc_reads_explicit_buttons() {
            let cfg = "[General]\nfoo=bar\n\n[org.kde.kdecoration2]\nButtonsOnLeft=XAI\nButtonsOnRight=M\n";
            assert_eq!(
                KdeKwinrc::parse_kwinrc(cfg),
                (vec![Close, Maximize, Minimize], vec![])
            );
        }

        #[test]
        fn kwinrc_falls_back_to_kwin_defaults_when_keys_absent() {
            let cfg = "[org.kde.kdecoration2]\nBorderSize=Normal\n";
            assert_eq!(
                KdeKwinrc::parse_kwinrc(cfg),
                (vec![], vec![Minimize, Maximize, Close])
            );
        }

        #[test]
        fn gsettings_parses_quoted_layout() {
            assert_eq!(
                GnomeGsettings::parse_layout("'appmenu:minimize,maximize,close'\n"),
                (vec![], vec![Minimize, Maximize, Close])
            );
            assert_eq!(
                GnomeGsettings::parse_layout("'close,minimize:'"),
                (vec![Close, Minimize], vec![])
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WindowButton::{Close, Maximize, Minimize};
    use super::platform_default;

    #[test]
    fn platform_default_exposes_the_three_standard_buttons() {
        let layout = platform_default();
        let mut all = layout.left.clone();
        all.extend(layout.right.clone());
        all.sort_by_key(|b| match b {
            Minimize => 0,
            Maximize => 1,
            Close => 2,
        });
        assert_eq!(all, vec![Minimize, Maximize, Close]);
        assert_eq!(layout.source, "default");
    }
}
