//! Linux game-mode sources: the Feral GameMode daemon over D-Bus, the
//! user's processes from `/proc` (with games that Steam and Lutris run)
//! and the application category of the active window. Fullscreen comes
//! with the active window (`active_window::linux`), which already asks
//! the compositor.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy, fdo::DBusProxy};
use zbus::names::BusName;

use super::{
    DetectorAvailability, GameModeDetectors, GameModeSettings, Observation, desktop_entries,
};
use crate::runtime_state::ActiveWindow;

const GAMEMODE_NAME: &str = "com.feralinteractive.GameMode";
const GAMEMODE_PATH: &str = "/com/feralinteractive/GameMode";
/// How long the answer to "is GameMode installed" is reused.
const INSTALLED_TTL: Duration = Duration::from_secs(60);

pub(super) fn observe(settings: &GameModeSettings, window: Option<&ActiveWindow>) -> Observation {
    let daemon = if settings.use_gamemoded {
        gamemoded_active()
    } else {
        daemon_installed().then_some(false)
    };
    let need_running = super::needs_running_processes(settings) || settings.use_known_games;
    let scan = need_running.then(scan_processes).flatten();
    let processes_available = if need_running {
        scan.is_some()
    } else {
        std::fs::read_dir("/proc").is_ok()
    };
    let window_game = settings
        .use_known_games
        .then(|| window.and_then(desktop_entries::game_window))
        .flatten();
    let active_window = crate::active_window::availability();
    let (running, known_games) = scan
        .map(|scan| (scan.names, scan.games))
        .unwrap_or_default();
    Observation {
        detectors: GameModeDetectors {
            gamemoded: availability(daemon.is_some()),
            fullscreen: active_window,
            processes: availability(processes_available),
            active_window,
            known_games: availability(processes_available),
            missing_window_tool: crate::active_window::missing_tool().map(str::to_owned),
        },
        running,
        known_games,
        window_game,
        daemon: daemon.unwrap_or(false),
    }
}

fn availability(available: bool) -> DetectorAvailability {
    if available {
        DetectorAvailability::Available
    } else {
        DetectorAvailability::Unavailable
    }
}

fn session_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| {
        Connection::session()
            .inspect_err(|error| log::debug!("[gamemode] session bus: {error}"))
            .ok()
    })
    .as_ref()
}

/// Whether the session bus can start GameMode, i.e. its D-Bus service is
/// installed. The answer is cached for [`INSTALLED_TTL`].
fn daemon_installed() -> bool {
    static CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    if let Ok(cache) = CACHE.lock()
        && let Some((at, installed)) = *cache
        && at.elapsed() < INSTALLED_TTL
    {
        return installed;
    }
    let installed = session_bus()
        .and_then(|bus| DBusProxy::new(bus).ok()?.list_activatable_names().ok())
        .is_some_and(|names| names.iter().any(|name| name.as_str() == GAMEMODE_NAME));
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((Instant::now(), installed));
    }
    installed
}

/// Whether a game registered with GameMode. Asks the bus who owns the
/// name first, so polling never D-Bus-activates the daemon; an absent
/// daemon that is installed simply means "no game". `None` when GameMode
/// is not installed or the bus does not answer.
fn gamemoded_active() -> Option<bool> {
    let bus = session_bus()?;
    let name = BusName::try_from(GAMEMODE_NAME).ok()?;
    let running = DBusProxy::new(bus).ok()?.name_has_owner(name).ok()?;
    if !running {
        return daemon_installed().then_some(false);
    }
    let proxy = Proxy::new(bus, GAMEMODE_NAME, GAMEMODE_PATH, GAMEMODE_NAME).ok()?;
    let clients: i32 = proxy.get_property("ClientCount").ok()?;
    Some(clients > 0)
}

/// What one process is known by; read once per process.
struct ProcInfo {
    /// Start time in clock ticks: tells a reused pid from the old process.
    start: u64,
    names: Vec<String>,
    game: Option<String>,
}

/// The current user's processes.
#[derive(Default)]
pub(super) struct Scan {
    /// Every name a process is known by, see [`super::process::proc_names`],
    /// plus `steam_app_<id>` for running Steam games.
    pub names: Vec<String>,
    /// Games that a launcher runs, sorted.
    pub games: Vec<String>,
}

/// Processes of the previous scan by pid.
static PROCESSES: Mutex<Option<HashMap<u32, ProcInfo>>> = Mutex::new(None);

/// Scan the current user's processes. Only processes started since the
/// previous scan are read in full, so polling costs one `stat` per process.
pub(super) fn scan_processes() -> Option<Scan> {
    let entries = std::fs::read_dir("/proc").ok()?;
    let uid = unsafe { libc::geteuid() };
    let mut cache = PROCESSES.lock().ok()?;
    let mut previous = cache.take().unwrap_or_default();
    let mut current = HashMap::with_capacity(previous.len());
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if !entry.metadata().is_ok_and(|metadata| metadata.uid() == uid) {
            continue;
        }
        let dir = entry.path();
        let Some(start) = start_time(&dir) else {
            continue;
        };
        let info = match previous.remove(&pid) {
            Some(info) if info.start == start => info,
            _ => read_process(&dir, start),
        };
        current.insert(pid, info);
    }
    let mut names = HashSet::new();
    let mut games = Vec::new();
    for info in current.values() {
        names.extend(info.names.iter().cloned());
        if let Some(game) = &info.game {
            if game.starts_with("steam_app_") {
                names.insert(game.clone());
            }
            games.push(game.clone());
        }
    }
    *cache = Some(current);
    games.sort();
    games.dedup();
    Some(Scan {
        names: names.into_iter().collect(),
        games,
    })
}

fn read_process(dir: &Path, start: u64) -> ProcInfo {
    let args = super::process::proc_args(dir);
    ProcInfo {
        start,
        names: super::process::proc_names(dir, &args),
        game: super::process::launched_game(&args),
    }
}

/// Field 22 of `/proc/<pid>/stat`. The command name before it may hold
/// spaces and parentheses, so fields are counted after the last `)`.
fn start_time(dir: &Path) -> Option<u64> {
    let stat = std::fs::read_to_string(dir.join("stat")).ok()?;
    parse_start_time(&stat)
}

fn parse_start_time(stat: &str) -> Option<u64> {
    let (_, fields) = stat.rsplit_once(')')?;
    // Fields after the name start with field 3 (state).
    fields.split_whitespace().nth(22 - 3)?.parse().ok()
}

/// Names of the current user's processes, for picking a rule.
pub(super) fn running_process_names() -> Option<Vec<String>> {
    scan_processes().map(|scan| scan.names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_time_skips_odd_command_names() {
        let stat =
            "4242 (Web (Content) x) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20";
        assert_eq!(parse_start_time(stat), Some(987654));
        assert_eq!(parse_start_time("4242 (short) S 1"), None);
        let own = std::fs::read_to_string("/proc/self/stat").unwrap();
        let expected = own.split_whitespace().nth(21).unwrap().parse().ok();
        assert_eq!(parse_start_time(&own), expected);
    }

    #[test]
    fn own_processes_are_scanned_and_cached() {
        let first = scan_processes().unwrap();
        let own = std::env::current_exe().unwrap();
        let own = own.file_name().unwrap().to_str().unwrap();
        assert!(first.names.iter().any(|name| name == own));
        let second = scan_processes().unwrap();
        assert!(second.names.iter().any(|name| name == own));
    }
}
