//! Process and window names as game-mode rules see them.

use crate::profile::app_match::AppMatcher;
use crate::profile::model::GameModeProcessMatcher;
use crate::runtime_state::ActiveWindow;

/// Applications that go fullscreen without being games: browsers, video
/// players and presentations. Compared with the app id, its last dotted
/// part and the process name, whole or followed by `-` (`brave-browser`).
const FULLSCREEN_NON_GAMES: [&str; 27] = [
    "firefox",
    "librewolf",
    "waterfox",
    "chrome",
    "chromium",
    "google-chrome",
    "brave",
    "vivaldi",
    "opera",
    "msedge",
    "microsoft-edge",
    "yandex",
    "yandex-browser",
    "zen",
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
    "soffice.bin",
    "okular",
];

/// Whether a fullscreen `window` most likely shows a video or a page,
/// not a game.
pub(crate) fn fullscreen_non_game(window: &ActiveWindow) -> bool {
    let app_id = window.app_id.trim().to_lowercase();
    let process = window
        .process_name
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let candidates = [
        app_id.as_str(),
        app_id.rsplit('.').next().unwrap_or(""),
        process.strip_suffix(".exe").unwrap_or(&process),
    ];
    FULLSCREEN_NON_GAMES.iter().any(|name| {
        candidates.iter().any(|candidate| {
            candidate
                .strip_prefix(name)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
        })
    })
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

/// Whether `matcher` looks at the active window only: by choice, or
/// because a title pattern has no process to match.
pub(super) fn window_only(matcher: &GameModeProcessMatcher) -> bool {
    matcher.only_active_window
        || matches!(AppMatcher::parse(&matcher.name), Some(AppMatcher::Title(_)))
}

pub(super) fn matches_window(matcher: &GameModeProcessMatcher, window: &ActiveWindow) -> bool {
    AppMatcher::parse(&matcher.name).is_some_and(|parsed| parsed.matches_window(window))
}

pub(super) fn matches_process(matcher: &GameModeProcessMatcher, name: &str) -> bool {
    AppMatcher::parse(&matcher.name).is_some_and(|parsed| parsed.matches_process(name))
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
        assert!(matches_process(&matcher(" GAME "), "game"));
        assert!(matches_process(&matcher("game"), "Game.exe"));
        assert!(!matches_process(&matcher("game"), "game-launcher"));
        assert!(!matches_process(&matcher("steam"), "steamwebhelper"));
        assert!(matches_process(&matcher("steam*"), "steamwebhelper"));
        assert!(!matches_process(&matcher(""), "game"));
        assert!(!matches_process(&matcher("title:game"), "game"));
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
        assert!(fullscreen_non_game(&window("brave-browser", None)));
        assert!(fullscreen_non_game(&window("org.kde.haruna", None)));
        assert!(!fullscreen_non_game(&window(
            "",
            Some("OperationFlashpoint.exe")
        )));
        assert!(!fullscreen_non_game(&window("zenless", None)));
        assert!(!fullscreen_non_game(&window(
            "steam_app_1091500",
            Some("Cyberpunk2077.exe")
        )));
    }
}
