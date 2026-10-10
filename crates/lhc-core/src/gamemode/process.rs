//! Process and window names as game-mode rules see them.

use crate::profile::model::GameModeProcessMatcher;
use crate::runtime_state::ActiveWindow;

/// Applications that go fullscreen without being games: browsers, video
/// players and presentations. Matched as substrings of the app id or
/// process name.
const FULLSCREEN_NON_GAMES: [&str; 24] = [
    "firefox",
    "librewolf",
    "waterfox",
    "chrom",
    "brave",
    "vivaldi",
    "opera",
    "msedge",
    "microsoft-edge",
    "yandex",
    "zen-browser",
    "falkon",
    "epiphany",
    "mpv",
    "vlc",
    "celluloid",
    "totem",
    "smplayer",
    "haruna",
    "mplayer",
    "kodi",
    "libreoffice",
    "soffice",
    "okular",
];

/// Whether a fullscreen `window` most likely shows a video or a page,
/// not a game.
pub(crate) fn fullscreen_non_game(window: &ActiveWindow) -> bool {
    let app_id = window.app_id.to_lowercase();
    let process = window.process_name.as_deref().unwrap_or("").to_lowercase();
    FULLSCREEN_NON_GAMES
        .iter()
        .any(|name| app_id.contains(name) || process.contains(name))
}

/// Last component of a Unix or Windows path: Wine and Proton games show
/// up as `Z:\games\Game.exe` in `/proc/<pid>/cmdline`.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Wine loaders that run Windows executables; the game name is then in
/// the command line, not in the executable path.
fn is_wine_loader(name: &str) -> bool {
    matches!(name, "wine" | "wine64") || (name.starts_with("wine") && name.ends_with("preloader"))
}

/// Names a `/proc/<pid>` entry is known by: `comm` (truncated to 15
/// bytes by the kernel), the executable and the first command-line word.
#[cfg(target_os = "linux")]
pub(crate) fn proc_names(dir: &std::path::Path) -> Vec<String> {
    let mut names = Vec::with_capacity(3);
    if let Ok(comm) = std::fs::read_to_string(dir.join("comm")) {
        names.push(comm.trim().to_owned());
    }
    if let Ok(exe) = std::fs::read_link(dir.join("exe"))
        && let Some(name) = exe.file_name().and_then(|name| name.to_str())
    {
        names.push(name.to_owned());
    }
    if let Some(argv0) = proc_argv0(dir) {
        names.push(file_name(&argv0).to_owned());
    }
    names.retain(|name| !name.is_empty());
    names
}

#[cfg(target_os = "linux")]
fn proc_argv0(dir: &std::path::Path) -> Option<String> {
    let cmdline = std::fs::read(dir.join("cmdline")).ok()?;
    let argv0 = cmdline.split(|byte| *byte == 0).next()?;
    (!argv0.is_empty()).then(|| String::from_utf8_lossy(argv0).into_owned())
}

/// The single most useful name of a process for display and matching:
/// the executable, or for Wine/Proton the Windows executable it runs.
#[cfg(target_os = "linux")]
pub(crate) fn proc_display_name(pid: u64) -> Option<String> {
    let dir = std::path::PathBuf::from(format!("/proc/{pid}"));
    let exe = std::fs::read_link(dir.join("exe"))
        .ok()
        .and_then(|exe| exe.file_name()?.to_str().map(str::to_owned));
    match exe {
        Some(exe) if !is_wine_loader(&exe) => Some(exe),
        _ => proc_argv0(&dir)
            .map(|argv0| file_name(&argv0).to_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                std::fs::read_to_string(dir.join("comm"))
                    .ok()
                    .map(|comm| comm.trim().to_owned())
                    .filter(|comm| !comm.is_empty())
            }),
    }
}

pub(super) fn matches(matcher: &GameModeProcessMatcher, candidate: &str) -> bool {
    let needle = matcher.name.trim().to_lowercase();
    let candidate = candidate.trim().to_lowercase();
    if needle.is_empty() || candidate.is_empty() {
        return false;
    }
    if needle.contains('*') || needle.contains('?') {
        wildcard_matches(&needle, &candidate)
    } else {
        candidate == needle
            || candidate.strip_suffix(".exe") == Some(needle.as_str())
            || needle.strip_suffix(".exe") == Some(candidate.as_str())
    }
}

fn wildcard_matches(pattern: &str, candidate: &str) -> bool {
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

    fn matcher(name: &str) -> GameModeProcessMatcher {
        GameModeProcessMatcher {
            id: String::new(),
            name: name.into(),
            only_active_window: false,
            is_blacklist: false,
        }
    }

    #[test]
    fn exact_names_do_not_match_unrelated_processes() {
        assert!(matches(&matcher(" GAME "), "game"));
        assert!(matches(&matcher("game"), "Game.exe"));
        assert!(!matches(&matcher("game"), "game-launcher"));
        assert!(!matches(&matcher("steam"), "steamwebhelper"));
        assert!(!matches(&matcher(""), "game"));
    }

    #[test]
    fn explicit_wildcards_match_the_entire_name() {
        assert!(matches(&matcher("steam*"), "steamwebhelper"));
        assert!(matches(&matcher("game?.exe"), "game1.exe"));
        assert!(!matches(&matcher("game?.exe"), "game12.exe"));
        assert!(matches(&matcher("*game*"), "my-game-launcher"));
        assert!(matches(&matcher("*игра?"), "моя-игра1"));
        assert!(!matches(&matcher("game*"), "other-game"));
        assert!(matches(&matcher("*a*b"), "zaaab"));
    }

    #[test]
    fn wine_paths_and_loaders() {
        assert_eq!(
            file_name(r"Z:\games\Cyberpunk2077.exe"),
            "Cyberpunk2077.exe"
        );
        assert_eq!(file_name("/usr/bin/steam"), "steam");
        assert_eq!(file_name("game"), "game");
        assert!(is_wine_loader("wine64-preloader"));
        assert!(is_wine_loader("wine"));
        assert!(!is_wine_loader("winecfg"));
    }

    #[test]
    fn browsers_and_players_are_not_fullscreen_games() {
        let window = |app_id: &str, process: Option<&str>| ActiveWindow {
            app_id: app_id.into(),
            process_name: process.map(str::to_owned),
            ..ActiveWindow::default()
        };
        assert!(fullscreen_non_game(&window("org.mozilla.firefox", None)));
        assert!(fullscreen_non_game(&window("", Some("chrome.exe"))));
        assert!(fullscreen_non_game(&window("mpv", None)));
        assert!(!fullscreen_non_game(&window(
            "steam_app_1091500",
            Some("Cyberpunk2077.exe")
        )));
    }
}
