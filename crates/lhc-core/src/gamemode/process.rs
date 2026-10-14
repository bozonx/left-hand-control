//! Process and window names as game-mode rules see them.

use super::GameSource;
use crate::profile::app_match::AppMatcher;
use crate::profile::model::GameModeProcessMatcher;
use crate::runtime_state::ActiveWindow;

/// Applications that go fullscreen without being games: browsers, video
/// players, presentations and the like. Compared with the app id, its last
/// dotted part and the process name, whole or followed by `-`
/// (`brave-browser`). The settings page shows this list.
pub const FULLSCREEN_NON_GAMES: &[&str] = &[
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
    "freetube",
    "mpv",
    "vlc",
    "celluloid",
    "totem",
    "smplayer",
    "haruna",
    "mplayer",
    "mpc-hc",
    "mpc-hc64",
    "mpc-be",
    "mpc-be64",
    "potplayer",
    "potplayermini",
    "potplayermini64",
    "kodi",
    "obs",
    "obs64",
    "zoom",
    "teams",
    "ms-teams",
    "discord",
    "libreoffice",
    "soffice.bin",
    "okular",
    "explorer",
];

/// Launchers and game tools: they are tagged as games or start them, but
/// their own windows and processes are not games. Compared like
/// [`FULLSCREEN_NON_GAMES`]. The settings page shows this list.
pub const GAME_LAUNCHERS: &[&str] = &[
    "steam",
    "steamwebhelper",
    "lutris",
    "heroic",
    "com.heroicgameslauncher.hgl",
    "bottles",
    "itch",
    "minigalaxy",
    "gamehub",
    "playonlinux",
    "net.davidotek.pupgui2",
    "protonplus",
    "epicgameslauncher",
    "galaxyclient",
    "battle.net",
    "eadesktop",
    "ealauncher",
    "upc",
    "playnite.desktopapp",
    "playnite.fullscreenapp",
    "cartridges",
    "rare",
    "faugus-launcher",
    "steamtinkerlaunch",
    "goverlay",
];

/// Whether `window` most likely shows a video or a page, not a game.
pub(crate) fn fullscreen_non_game(window: &ActiveWindow) -> bool {
    window_in(FULLSCREEN_NON_GAMES, window)
}

/// The name a window is shown by: its app id, else its process name.
pub(crate) fn window_name(window: &ActiveWindow) -> String {
    let app_id = window.app_id.trim();
    if app_id.is_empty() {
        window
            .process_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_owned()
    } else {
        app_id.to_owned()
    }
}

/// Whether `window` belongs to a launcher, not to a game.
pub(crate) fn launcher_window(window: &ActiveWindow) -> bool {
    window_in(GAME_LAUNCHERS, window)
}

/// Whether the process `name` is a launcher, not a game.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn launcher_process(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    in_list(
        GAME_LAUNCHERS,
        &[name.strip_suffix(".exe").unwrap_or(&name)],
    )
}

fn window_in(list: &[&str], window: &ActiveWindow) -> bool {
    let app_id = window.app_id.trim().to_lowercase();
    let process = window
        .process_name
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    in_list(
        list,
        &[
            app_id.as_str(),
            app_id.rsplit('.').next().unwrap_or(""),
            process.strip_suffix(".exe").unwrap_or(&process),
        ],
    )
}

fn in_list(list: &[&str], candidates: &[&str]) -> bool {
    list.iter().any(|name| {
        candidates.iter().any(|candidate| {
            candidate
                .strip_prefix(name)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('-'))
        })
    })
}

/// A game recognised by its window alone: a Steam game (`steam_app_<id>`,
/// X11 and XWayland) or a nested gamescope session.
pub(crate) fn known_game_window(window: &ActiveWindow) -> Option<(GameSource, String)> {
    let app_id = window.app_id.trim().to_lowercase();
    let steam = app_id
        .strip_prefix("steam_app_")
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()));
    if steam {
        Some((GameSource::Steam, app_id))
    } else {
        (app_id == "gamescope").then_some((GameSource::Gamescope, app_id))
    }
}

/// Last component of a Unix or Windows path: Wine and Proton games show
/// up as `Z:\games\Game.exe` in `/proc/<pid>/cmdline`.
#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Wine loaders that run Windows executables; the game name is then in
/// the command line, not in the executable path.
#[cfg(any(target_os = "linux", test))]
fn is_wine_loader(name: &str) -> bool {
    matches!(name, "wine" | "wine64") || (name.starts_with("wine") && name.ends_with("preloader"))
}

/// Names a `/proc/<pid>` entry is known by: `comm` (truncated to 15
/// bytes by the kernel), the executable and the first command-line word.
#[cfg(target_os = "linux")]
pub(crate) fn proc_names(dir: &std::path::Path, args: &[String]) -> Vec<String> {
    let mut names = Vec::with_capacity(3);
    if let Ok(comm) = std::fs::read_to_string(dir.join("comm")) {
        names.push(comm.trim().to_owned());
    }
    if let Some(name) = exe_name(dir) {
        names.push(name);
    }
    if let Some(argv0) = args.first() {
        names.push(file_name(argv0).to_owned());
    }
    names.retain(|name| !name.is_empty());
    names
}

/// File name of the executable of a `/proc/<pid>` entry. The kernel adds
/// ` (deleted)` to the link once the file is replaced, as by a rebuild.
#[cfg(target_os = "linux")]
fn exe_name(dir: &std::path::Path) -> Option<String> {
    let exe = std::fs::read_link(dir.join("exe")).ok()?;
    let name = exe.file_name()?.to_str()?;
    Some(without_deleted(name).to_owned())
}

/// `name` without the ` (deleted)` the kernel adds to a replaced executable.
pub(crate) fn without_deleted(name: &str) -> &str {
    name.strip_suffix(" (deleted)").unwrap_or(name)
}

/// The command line of a `/proc/<pid>` entry, one string per argument.
#[cfg(target_os = "linux")]
pub(crate) fn proc_args(dir: &std::path::Path) -> Vec<String> {
    std::fs::read(dir.join("cmdline"))
        .map(|cmdline| split_cmdline(&cmdline))
        .unwrap_or_default()
}

/// Arguments of a `cmdline` file. Processes that rewrite their title
/// (Electron, setproctitle) leave one space-separated string; it is split
/// at spaces when it holds options, so the program name stays short.
/// A Windows path with spaces has no options and stays whole.
#[cfg(any(target_os = "linux", test))]
fn split_cmdline(cmdline: &[u8]) -> Vec<String> {
    let cmdline = cmdline.strip_suffix(&[0]).unwrap_or(cmdline);
    if cmdline.is_empty() {
        return Vec::new();
    }
    let args: Vec<String> = cmdline
        .split(|byte| *byte == 0)
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();
    match args.as_slice() {
        [title]
            if title
                .split_whitespace()
                .skip(1)
                .any(|word| word.len() > 1 && word.starts_with('-')) =>
        {
            title.split_whitespace().map(str::to_owned).collect()
        }
        _ => args,
    }
}

#[cfg(target_os = "linux")]
fn proc_argv0(dir: &std::path::Path) -> Option<String> {
    proc_args(dir)
        .into_iter()
        .next()
        .filter(|argv0| !argv0.is_empty())
}

/// A game a launcher runs, from one process's command line: Steam starts
/// every game under `reaper SteamLaunch AppId=<id>`, Lutris under
/// `lutris-wrapper <title>`. Steam games are named `steam_app_<id>`, like
/// their windows; a Lutris game without a title is `lutris`.
#[cfg(target_os = "linux")]
pub(crate) fn launched_game(args: &[String]) -> Option<(GameSource, String)> {
    let program = |arg: &String| file_name(arg).to_owned();
    if args.first().map(program).as_deref() == Some("reaper")
        && args.iter().any(|arg| arg == "SteamLaunch")
    {
        return args
            .iter()
            .find_map(|arg| arg.strip_prefix("AppId="))
            .filter(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
            .map(|id| (GameSource::Steam, format!("steam_app_{id}")));
    }
    // A Python script: the interpreter may come first.
    let interpreter = args
        .first()
        .is_some_and(|arg| program(arg).starts_with("python"));
    let wrapper = usize::from(interpreter);
    if args.get(wrapper).map(program).as_deref() != Some("lutris-wrapper") {
        return None;
    }
    Some((
        GameSource::Lutris,
        args.get(wrapper + 1)
            .map(|title| title.trim())
            .filter(|title| !title.is_empty())
            .unwrap_or("lutris")
            .to_owned(),
    ))
}

/// The single most useful name of a process for display and matching:
/// the executable, or for Wine/Proton the Windows executable it runs.
#[cfg(target_os = "linux")]
pub(crate) fn proc_display_name(pid: u64) -> Option<String> {
    let dir = std::path::PathBuf::from(format!("/proc/{pid}"));
    match exe_name(&dir) {
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
    fn replaced_executables_keep_their_name() {
        assert_eq!(without_deleted("slint-shell (deleted)"), "slint-shell");
        assert_eq!(without_deleted("kate"), "kate");
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
    fn rewritten_titles_are_split_but_windows_paths_are_not() {
        assert_eq!(
            split_cmdline(b"app.asar --type=renderer --lang=en\0"),
            ["app.asar", "--type=renderer", "--lang=en"]
        );
        assert_eq!(
            split_cmdline(b"C:\\Program Files\\Game\\Game.exe\0"),
            [r"C:\Program Files\Game\Game.exe"]
        );
        assert_eq!(
            split_cmdline(b"Z:\\Games\\Some - Game\\Game.exe\0"),
            [r"Z:\Games\Some - Game\Game.exe"]
        );
        assert_eq!(split_cmdline(b"reaper\0SteamLaunch\0AppId=1\0").len(), 3);
        assert!(split_cmdline(b"").is_empty());
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

    #[cfg(target_os = "linux")]
    #[test]
    fn games_started_by_steam_and_lutris() {
        let args = |line: &str| line.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            launched_game(&args(
                "/home/u/.steam/ubuntu12_32/reaper SteamLaunch AppId=1091500 -- /x/proton waitforexitandrun"
            )),
            Some((GameSource::Steam, "steam_app_1091500".into()))
        );
        assert_eq!(launched_game(&args("reaper SteamLaunch AppId=abc")), None);
        assert_eq!(launched_game(&args("/usr/bin/reaper --help")), None);
        assert_eq!(
            launched_game(&args(
                "/usr/bin/python3 /usr/bin/lutris-wrapper Celeste 0 0 /x/wine"
            )),
            Some((GameSource::Lutris, "Celeste".into()))
        );
        assert_eq!(
            launched_game(&args("/usr/bin/lutris-wrapper")),
            Some((GameSource::Lutris, "lutris".into()))
        );
        assert_eq!(launched_game(&args("vim lutris-wrapper")), None);
        assert_eq!(launched_game(&args("/usr/bin/vim notes.txt")), None);
    }

    #[test]
    fn windows_known_as_games_and_launchers() {
        let window = |app_id: &str, process: Option<&str>| ActiveWindow {
            app_id: app_id.into(),
            process_name: process.map(str::to_owned),
            ..ActiveWindow::default()
        };
        assert_eq!(
            known_game_window(&window("steam_app_42", None)),
            Some((GameSource::Steam, "steam_app_42".into()))
        );
        assert_eq!(
            known_game_window(&window("gamescope", None)),
            Some((GameSource::Gamescope, "gamescope".into()))
        );
        assert_eq!(known_game_window(&window("steam_app_", None)), None);
        assert_eq!(known_game_window(&window("steam", None)), None);
        assert!(launcher_window(&window("com.valvesoftware.Steam", None)));
        assert!(launcher_window(&window("", Some("EpicGamesLauncher.exe"))));
        assert!(launcher_window(&window("net.lutris.Lutris", None)));
        assert!(!launcher_window(&window("steam_app_42", None)));
        assert!(launcher_process("steamwebhelper"));
        assert!(!launcher_process("Cyberpunk2077.exe"));
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
