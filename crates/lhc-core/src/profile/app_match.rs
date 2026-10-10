//! Matching the active window or a running process against the patterns
//! of rule, auto-mode and game-mode conditions.
//!
//! A pattern is a plain string:
//! * `org.telegram.desktop`, `Game.exe` — the application: its app id
//!   (Wayland app id, X11 `WM_CLASS`, macOS bundle id) or its process
//!   name; whole name, case-insensitive, `.exe` optional.
//! * `steam_app_*` — the same with `*` / `?` wildcards over the whole name.
//! * `title:YouTube` — the window title contains the text; with `*` / `?`
//!   the wildcards cover the whole title.
//!
//! The title is the least stable property of a window, so it is read only
//! when some condition uses a `title:` pattern (see [`uses_title`]).

use crate::runtime_state::ActiveWindow;

const TITLE_PREFIX: &str = "title:";

/// A parsed pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMatcher {
    /// App id or process name.
    App(String),
    /// Window title.
    Title(String),
}

impl AppMatcher {
    /// `None` for a blank pattern.
    pub fn parse(pattern: &str) -> Option<Self> {
        let pattern = pattern.trim();
        let matcher = match strip_prefix_ignore_case(pattern, TITLE_PREFIX) {
            Some(title) => Self::Title(title.trim().to_lowercase()),
            None => Self::App(pattern.to_lowercase()),
        };
        (!matcher.text().is_empty()).then_some(matcher)
    }

    fn text(&self) -> &str {
        match self {
            Self::App(text) | Self::Title(text) => text,
        }
    }

    /// Whether the focused `window` matches.
    pub fn matches_window(&self, window: &ActiveWindow) -> bool {
        match self {
            Self::App(pattern) => {
                name_matches(pattern, &window.app_id)
                    || window
                        .process_name
                        .as_deref()
                        .is_some_and(|name| name_matches(pattern, name))
            }
            Self::Title(pattern) => {
                let title = window.title.to_lowercase();
                if has_wildcards(pattern) {
                    wildcard_matches(pattern, &title)
                } else {
                    title.contains(pattern.as_str())
                }
            }
        }
    }

    /// Whether a running process called `name` matches; title patterns
    /// never match a process.
    pub fn matches_process(&self, name: &str) -> bool {
        match self {
            Self::App(pattern) => name_matches(pattern, name),
            Self::Title(_) => false,
        }
    }
}

/// Pattern matching window titles that contain `text`.
pub fn title_pattern(text: &str) -> String {
    format!("{TITLE_PREFIX}{}", text.trim())
}

/// Text of a title pattern as written, `None` for app patterns.
pub fn title_text(pattern: &str) -> Option<&str> {
    strip_prefix_ignore_case(pattern.trim(), TITLE_PREFIX).map(str::trim)
}

/// Whether any of `patterns` matches `window`.
pub fn any_matches_window(patterns: &[String], window: &ActiveWindow) -> bool {
    patterns
        .iter()
        .filter_map(|pattern| AppMatcher::parse(pattern))
        .any(|matcher| matcher.matches_window(window))
}

/// Whether some of `patterns` needs the window title.
pub fn uses_title<'a>(patterns: impl IntoIterator<Item = &'a String>) -> bool {
    patterns
        .into_iter()
        .any(|pattern| matches!(AppMatcher::parse(pattern), Some(AppMatcher::Title(_))))
}

/// Patterns equivalent to a legacy substring needle, which matched the
/// title or the app id: `*needle*` and `title:needle`.
pub fn from_legacy(needle: &str) -> Vec<String> {
    let needle = needle.trim();
    if needle.is_empty() {
        return Vec::new();
    }
    vec![format!("*{needle}*"), format!("{TITLE_PREFIX}{needle}")]
}

/// [`from_legacy`] over a whole list.
pub fn list_from_legacy(needles: &[String]) -> Vec<String> {
    needles
        .iter()
        .flat_map(|needle| from_legacy(needle))
        .collect()
}

/// Whether `outer` matches every window `inner` matches. Conservative:
/// `false` when it cannot tell.
pub fn covers(outer: &str, inner: &str) -> bool {
    match (AppMatcher::parse(outer), AppMatcher::parse(inner)) {
        (Some(AppMatcher::App(outer)), Some(AppMatcher::App(inner)))
        | (Some(AppMatcher::Title(outer)), Some(AppMatcher::Title(inner)))
            if outer == inner =>
        {
            true
        }
        (Some(AppMatcher::App(outer)), Some(AppMatcher::App(inner))) => {
            has_wildcards(&outer) && !has_wildcards(&inner) && wildcard_matches(&outer, &inner)
        }
        (Some(AppMatcher::Title(outer)), Some(AppMatcher::Title(inner))) => {
            // `*text*` is the same as plain `text`; a title containing
            // `inner` also contains its substrings.
            let outer = outer
                .strip_prefix('*')
                .and_then(|text| text.strip_suffix('*'))
                .unwrap_or(&outer);
            !has_wildcards(outer) && !has_wildcards(&inner) && inner.contains(outer)
        }
        _ => false,
    }
}

/// Whether a lowercase `pattern` matches the whole `name`.
fn name_matches(pattern: &str, name: &str) -> bool {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return false;
    }
    if has_wildcards(pattern) {
        wildcard_matches(pattern, &name)
    } else {
        name == pattern
            || name.strip_suffix(".exe") == Some(pattern)
            || pattern.strip_suffix(".exe") == Some(name.as_str())
    }
}

fn has_wildcards(pattern: &str) -> bool {
    pattern.contains(['*', '?'])
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// `*` matches any run of characters, `?` one character; the pattern
/// covers the whole candidate.
pub(crate) fn wildcard_matches(pattern: &str, candidate: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let candidate: Vec<char> = candidate.chars().collect();
    let (mut p, mut c) = (0, 0);
    let (mut star, mut retry) = (None, 0);
    while c < candidate.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == candidate[c]) {
            p += 1;
            c += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            retry = c;
        } else if let Some(index) = star {
            p = index + 1;
            retry += 1;
            c = retry;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(title: &str, app_id: &str, process: Option<&str>) -> ActiveWindow {
        ActiveWindow {
            title: title.into(),
            app_id: app_id.into(),
            process_name: process.map(str::to_owned),
            fullscreen: None,
        }
    }

    fn matches(pattern: &str, window: &ActiveWindow) -> bool {
        AppMatcher::parse(pattern).is_some_and(|matcher| matcher.matches_window(window))
    }

    #[test]
    fn parses_kinds() {
        assert_eq!(AppMatcher::parse("  "), None);
        assert_eq!(AppMatcher::parse("title:  "), None);
        assert_eq!(
            AppMatcher::parse(" Firefox "),
            Some(AppMatcher::App("firefox".into()))
        );
        assert_eq!(
            AppMatcher::parse("Title: YouTube"),
            Some(AppMatcher::Title("youtube".into()))
        );
    }

    #[test]
    fn app_patterns_match_whole_names() {
        let telegram = window("Chat", "org.telegram.desktop", Some("Telegram"));
        assert!(matches("org.telegram.desktop", &telegram));
        assert!(matches("TELEGRAM", &telegram));
        assert!(!matches("telegram.desktop", &telegram));
        assert!(matches("*telegram*", &telegram));
        let game = window("", "steam_app_1091500", Some("Cyberpunk2077.exe"));
        assert!(matches("cyberpunk2077", &game));
        assert!(matches("steam_app_*", &game));
        assert!(!matches("steam", &game));
        assert!(!matches("Chat", &telegram));
    }

    #[test]
    fn title_patterns_match_substrings_or_wildcards() {
        let browser = window("Rust — YouTube — Mozilla Firefox", "firefox", None);
        assert!(matches("title:youtube", &browser));
        assert!(matches("title:*firefox", &browser));
        assert!(!matches("title:youtube*", &browser));
        assert!(!matches("title:firefox", &window("", "firefox", None)));
    }

    #[test]
    fn processes_match_app_patterns_only() {
        let matcher = AppMatcher::parse("game").unwrap();
        assert!(matcher.matches_process("Game.exe"));
        assert!(!matcher.matches_process("game-launcher"));
        assert!(
            !AppMatcher::parse("title:game")
                .unwrap()
                .matches_process("game")
        );
    }

    #[test]
    fn wildcards() {
        assert!(wildcard_matches("steam*", "steamwebhelper"));
        assert!(wildcard_matches("game?.exe", "game1.exe"));
        assert!(!wildcard_matches("game?.exe", "game12.exe"));
        assert!(wildcard_matches("*игра?", "моя-игра1"));
        assert!(!wildcard_matches("game*", "other-game"));
        assert!(wildcard_matches("*a*b", "zaaab"));
    }

    #[test]
    fn legacy_needles_keep_matching_title_or_app_id() {
        let patterns = list_from_legacy(&["tele".into(), " ".into(), "YouTube".into()]);
        assert_eq!(
            patterns,
            ["*tele*", "title:tele", "*YouTube*", "title:YouTube"]
        );
        assert!(any_matches_window(
            &patterns,
            &window("", "org.telegram.desktop", None)
        ));
        assert!(any_matches_window(
            &patterns,
            &window("Video - YouTube", "firefox", None)
        ));
        assert!(!any_matches_window(
            &patterns,
            &window("Kate", "kate", None)
        ));
    }

    #[test]
    fn title_patterns_are_built_and_read_back() {
        assert_eq!(title_pattern(" YouTube "), "title:YouTube");
        assert_eq!(title_text("TITLE: YouTube"), Some("YouTube"));
        assert_eq!(title_text("firefox"), None);
    }

    #[test]
    fn title_use_is_detected() {
        assert!(!uses_title(&["firefox".to_string()]));
        assert!(uses_title(&["firefox".to_string(), "title:x".to_string()]));
    }

    #[test]
    fn coverage() {
        assert!(covers("firefox", "Firefox"));
        assert!(covers("steam_app_*", "steam_app_1"));
        assert!(!covers("steam_app_1", "steam_app_*"));
        assert!(!covers("firefox", "title:firefox"));
        assert!(covers("title:tube", "title:youtube"));
        assert!(!covers("title:youtube", "title:tube"));
        assert!(covers("title:*tube*", "title:youtube"));
        assert!(!covers("title:*tube", "title:tube"));
    }
}
