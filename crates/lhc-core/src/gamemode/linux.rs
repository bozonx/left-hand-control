//! Linux game-mode sources: the Feral GameMode daemon over D-Bus, the
//! user's processes from `/proc` (with games that Steam, Lutris, Heroic
//! and umu run)
//! and the application category of the active window. Fullscreen comes
//! with the active window (`active_window::linux`), which already asks
//! the compositor.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator, Proxy, fdo::DBusProxy};
use zbus::names::BusName;
use zbus::proxy::CacheProperties;
use zbus::{MatchRule, message};

use super::{
    DetectorAvailability, GameModeDetectors, GameModeSettings, GameSource, KnownGame, Observation,
    desktop_entries,
};
use crate::runtime_state::ActiveWindow;

const GAMEMODE_NAME: &str = "com.feralinteractive.GameMode";
const GAMEMODE_PATH: &str = "/com/feralinteractive/GameMode";
/// How long the answer to "is GameMode installed" is reused.
const INSTALLED_TTL: Duration = Duration::from_secs(60);
/// Pause before a failed GameMode listener is started again.
const LISTEN_RETRY: Duration = Duration::from_secs(10);

pub(super) fn observe(settings: &GameModeSettings, window: Option<&ActiveWindow>) -> Observation {
    let daemon = if settings.use_gamemoded {
        gamemoded_active()
    } else {
        daemon_installed().then_some(false)
    };
    let need_running = super::needs_polling(settings);
    let scan = need_running.then(scan_processes).flatten();
    let processes_available = if need_running {
        scan.is_some()
    } else {
        std::fs::read_dir("/proc").is_ok()
    };
    let window_game = settings
        .use_app_category
        .then(|| window.and_then(desktop_entries::game_window))
        .flatten();
    let active_window = crate::active_window::availability();
    let (running, games) = scan
        .map(|scan| (scan.names, scan.games))
        .unwrap_or_default();
    let known_games = games
        .into_iter()
        .filter(|game| game.source.enabled(settings))
        .map(|game| KnownGame {
            name: game.name,
            processes: Some(game.processes),
        })
        .collect();
    Observation {
        detectors: GameModeDetectors {
            gamemoded: availability(daemon.is_some()),
            fullscreen: active_window,
            processes: availability(processes_available),
            active_window,
            missing_window_tool: crate::active_window::missing_tool().map(str::to_owned),
            ..GameModeDetectors::default()
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

fn session_bus() -> Option<Connection> {
    Connection::session()
        .inspect_err(|error| log::debug!("[gamemode] session bus: {error}"))
        .ok()
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
        .and_then(|bus| DBusProxy::new(&bus).ok()?.list_activatable_names().ok())
        .is_some_and(|names| names.iter().any(|name| name.as_str() == GAMEMODE_NAME));
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((Instant::now(), installed));
    }
    installed
}

/// GameMode as the signal listener last read it.
#[derive(Debug, Clone, Copy)]
enum Daemon {
    Stopped,
    Running {
        clients: bool,
    },
    /// Running, but its client count could not be read.
    Unknown,
}

/// A connection subscribed to GameMode's signals and the state it keeps.
struct Listener {
    connection: Connection,
    /// `None` once the listener's thread ended.
    state: Arc<Mutex<Option<Daemon>>>,
}

static LISTENER: Mutex<Option<Listener>> = Mutex::new(None);

/// Whether a game registered with GameMode. A listener follows the
/// daemon's signals, so polling costs no D-Bus calls and never
/// D-Bus-activates it; an absent daemon that is installed simply means
/// "no game". `None` when GameMode is not installed or the bus does not
/// answer.
fn gamemoded_active() -> Option<bool> {
    match daemon_state()? {
        Daemon::Stopped => daemon_installed().then_some(false),
        Daemon::Running { clients } => Some(clients),
        Daemon::Unknown => None,
    }
}

/// The daemon state, (re)starting the listener when none runs.
fn daemon_state() -> Option<Daemon> {
    let mut listener = LISTENER.lock().ok()?;
    if let Some(state) = listener
        .as_ref()
        .and_then(|listener| *listener.state.lock().ok()?)
    {
        if matches!(state, Daemon::Unknown) {
            let listener = listener.as_ref()?;
            let state = read_daemon(&listener.connection);
            *listener.state.lock().ok()? = Some(state);
            return Some(state);
        }
        return Some(state);
    }
    if let Some(old) = listener.take() {
        let _ = old.connection.close();
    }
    // Without a session bus every poll would try again.
    static FAILED: Mutex<Option<Instant>> = Mutex::new(None);
    let mut failed = FAILED.lock().ok()?;
    if failed.is_some_and(|at| at.elapsed() < LISTEN_RETRY) {
        return None;
    }
    let Some(started) = listen() else {
        *failed = Some(Instant::now());
        return None;
    };
    *failed = None;
    let state = *started.state.lock().ok()?;
    *listener = Some(started);
    state
}

/// Stop the listener; the next detection starts a new one.
pub(super) fn stop() {
    let listener = LISTENER.lock().ok().and_then(|mut slot| slot.take());
    if let Some(listener) = listener
        && let Err(error) = listener.connection.close()
    {
        log::debug!("[gamemode] close listener connection: {error}");
    }
}

/// Subscribe to GameMode's owner changes and signals (`PropertiesChanged`
/// of `ClientCount`, `GameRegistered`, `GameUnregistered`) and re-read
/// the state on each, waking the watcher.
fn listen() -> Option<Listener> {
    let connection = Connection::session()
        .inspect_err(|error| log::debug!("[gamemode] listener bus: {error}"))
        .ok()?;
    // Created before subscribing, so no signal is missed.
    let messages = MessageIterator::from(&connection);
    let dbus = DBusProxy::new(&connection).ok()?;
    let rules = [
        MatchRule::builder()
            .msg_type(message::Type::Signal)
            .sender(crate::platform::dbus::BUS_SERVICE)
            .and_then(|rule| rule.member("NameOwnerChanged"))
            .and_then(|rule| rule.arg(0, GAMEMODE_NAME))
            .map(|rule| rule.build()),
        MatchRule::builder()
            .msg_type(message::Type::Signal)
            .path(GAMEMODE_PATH)
            .map(|rule| rule.build()),
    ];
    for rule in rules {
        dbus.add_match_rule(rule.ok()?)
            .inspect_err(|error| log::debug!("[gamemode] subscribe: {error}"))
            .ok()?;
    }
    let state = Arc::new(Mutex::new(Some(read_daemon(&connection))));
    let thread_state = state.clone();
    let thread_connection = connection.clone();
    std::thread::Builder::new()
        .name("gamemode-dbus".into())
        .spawn(move || {
            for message in messages {
                let Ok(message) = message else {
                    break;
                };
                // Replies to the reads below arrive here too.
                if message.message_type() != message::Type::Signal {
                    continue;
                }
                let daemon = read_daemon(&thread_connection);
                if let Ok(mut state) = thread_state.lock() {
                    *state = Some(daemon);
                }
                super::wake();
            }
            if let Ok(mut state) = thread_state.lock() {
                *state = None;
            }
        })
        .inspect_err(|error| log::debug!("[gamemode] listener thread: {error}"))
        .ok()?;
    Some(Listener { connection, state })
}

fn read_daemon(connection: &Connection) -> Daemon {
    let Ok(name) = BusName::try_from(GAMEMODE_NAME) else {
        return Daemon::Unknown;
    };
    let running = DBusProxy::new(connection)
        .ok()
        .and_then(|dbus| dbus.name_has_owner(name).ok());
    match running {
        Some(false) => Daemon::Stopped,
        Some(true) => client_count(connection).map_or(Daemon::Unknown, |clients| Daemon::Running {
            clients: clients > 0,
        }),
        None => Daemon::Unknown,
    }
}

fn client_count(connection: &Connection) -> zbus::Result<i32> {
    // Without a property cache the proxy costs no bus traffic of its own.
    zbus::blocking::proxy::Builder::<Proxy>::new(connection)
        .destination(GAMEMODE_NAME)?
        .path(GAMEMODE_PATH)?
        .interface(GAMEMODE_NAME)?
        .cache_properties(CacheProperties::No)
        .build()?
        .get_property("ClientCount")
}

/// What one process is known by; read once per process.
struct ProcInfo {
    /// Start time in clock ticks: tells a reused pid from the old process.
    start: u64,
    parent: u32,
    executable: Option<PathBuf>,
    refreshed: Instant,
    names: Vec<String>,
    games: Vec<(GameSource, String)>,
}

/// The current user's processes.
#[derive(Default)]
pub(super) struct Scan {
    /// Every name a process is known by, see [`super::process::proc_names`],
    /// plus `steam_app_<id>` for running Steam games.
    pub names: Vec<String>,
    /// Games that a launcher runs, sorted.
    pub games: Vec<ScannedGame>,
}

/// A game a launcher runs and the names of its processes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ScannedGame {
    pub source: GameSource,
    pub name: String,
    /// Lowercase names of the launching process and its descendants.
    pub processes: Vec<String>,
}

/// How far up the process tree a game's launcher is looked for.
const MAX_TREE_DEPTH: usize = 64;

/// Processes of the previous scan by pid.
#[derive(Default)]
struct ProcessCache {
    processes: HashMap<u32, ProcInfo>,
}

/// Also refresh argv/environ: applications may rewrite them without exec.
const PROCESS_REFRESH: Duration = Duration::from_secs(5);

static PROCESSES: Mutex<Option<ProcessCache>> = Mutex::new(None);

/// Scan the current user's processes, detecting exec and periodically
/// refreshing mutable argv/environ metadata.
pub(super) fn scan_processes() -> Option<Scan> {
    let entries = std::fs::read_dir("/proc").ok()?;
    let uid = unsafe { libc::geteuid() };
    let mut cache = PROCESSES.lock().ok()?;
    let previous = cache.take().unwrap_or_default();
    let now = Instant::now();
    let mut previous = previous.processes;
    let mut current = HashMap::with_capacity(previous.len());
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let dir = entry.path();
        let Some(info) = cached_process(&dir, uid, previous.remove(&pid), now) else {
            continue;
        };
        current.insert(pid, info);
    }
    let mut names = HashSet::new();
    for info in current.values() {
        names.extend(info.names.iter().cloned());
        for (source, game) in &info.games {
            if *source == GameSource::Steam {
                names.insert(game.clone());
            }
        }
    }
    let games = group_games(&current);
    *cache = Some(ProcessCache { processes: current });
    Some(Scan {
        names: names.into_iter().collect(),
        games,
    })
}

/// Running games with the names of their processes, sorted.
fn group_games(current: &HashMap<u32, ProcInfo>) -> Vec<ScannedGame> {
    let mut games: std::collections::BTreeMap<(GameSource, String), HashSet<String>> =
        std::collections::BTreeMap::new();
    for info in current.values() {
        // The process belongs to its own games and to those of its
        // ancestors: the game runs below `reaper` or `lutris-wrapper`.
        let lower: Vec<String> = info.names.iter().map(|name| name.to_lowercase()).collect();
        let mut owner = Some(info);
        for _ in 0..MAX_TREE_DEPTH {
            let Some(process) = owner else {
                break;
            };
            for game in &process.games {
                games
                    .entry(game.clone())
                    .or_default()
                    .extend(lower.iter().cloned());
            }
            owner = (process.parent > 1)
                .then(|| current.get(&process.parent))
                .flatten();
        }
    }
    let mut games: Vec<ScannedGame> = games
        .into_iter()
        .map(|((source, name), processes)| {
            let mut processes: Vec<String> = processes.into_iter().collect();
            processes.sort();
            ScannedGame {
                source,
                name,
                processes,
            }
        })
        .collect();
    games.sort();
    games
}

fn cached_process(dir: &Path, uid: u32, known: Option<ProcInfo>, now: Instant) -> Option<ProcInfo> {
    if std::fs::metadata(dir).ok()?.uid() != uid {
        return None;
    }
    let (start, parent) = start_time(dir)?;
    let executable = std::fs::read_link(dir.join("exe")).ok();
    if let Some(mut info) = known
        && info.start == start
        && info.executable == executable
        && now.duration_since(info.refreshed) < PROCESS_REFRESH
    {
        // Orphans move to a new parent without exec.
        info.parent = parent;
        return Some(info);
    }
    let args = super::process::proc_args(dir);
    Some(ProcInfo {
        start,
        parent,
        executable,
        refreshed: now,
        names: super::process::proc_names(dir, &args),
        games: super::process::launched_game(&args)
            .into_iter()
            .chain(
                std::fs::read(dir.join("environ"))
                    .ok()
                    .and_then(|environ| super::process::heroic_game(&environ)),
            )
            .collect(),
    })
}

/// Start time (field 22) and parent pid (field 4) of `/proc/<pid>/stat`.
fn start_time(dir: &Path) -> Option<(u64, u32)> {
    let stat = std::fs::read_to_string(dir.join("stat")).ok()?;
    Some((parse_start_time(&stat)?, stat_field(&stat, PARENT)?))
}

// Field numbers from proc_pid_stat(5); those after the name start with 3 (state).
const FIRST_AFTER_NAME: usize = 3;
const PARENT: usize = 4;
const START_TIME: usize = 22;

fn parse_start_time(stat: &str) -> Option<u64> {
    stat_field(stat, START_TIME)
}

/// A numeric field of `stat`. The command name may hold spaces and
/// parentheses, so fields are counted after the last `)`.
fn stat_field<T: std::str::FromStr>(stat: &str, field: usize) -> Option<T> {
    let (_, fields) = stat.rsplit_once(')')?;
    fields
        .split_whitespace()
        .nth(field - FIRST_AFTER_NAME)?
        .parse()
        .ok()
}

/// Names of the current user's processes, for picking a rule.
pub(super) fn running_process_names() -> Option<Vec<String>> {
    scan_processes().map(|scan| scan.names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_cache_detects_exec_and_refreshes_mutable_metadata() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let stat = "42 (game) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 999 20";
        std::fs::write(root.join("stat"), stat).unwrap();
        std::fs::write(root.join("cmdline"), b"/bin/sh\0").unwrap();
        std::fs::write(root.join("environ"), b"").unwrap();
        symlink("/bin/sh", root.join("exe")).unwrap();
        let uid = unsafe { libc::geteuid() };
        let now = Instant::now();
        let first = cached_process(root, uid, None, now).unwrap();
        std::fs::remove_file(root.join("exe")).unwrap();
        symlink("/games/Game.exe", root.join("exe")).unwrap();
        std::fs::write(root.join("cmdline"), b"/games/Game.exe\0").unwrap();
        let next = cached_process(root, uid, Some(first), now).unwrap();
        assert!(next.names.iter().any(|name| name == "Game.exe"));
        std::fs::write(root.join("environ"), b"HEROIC_APP_NAME=NewGame\0").unwrap();
        let next = cached_process(root, uid, Some(next), now + PROCESS_REFRESH).unwrap();
        assert_eq!(next.games, vec![(GameSource::Heroic, "NewGame".into())]);
        // Heroic and umu are independent toggles even on the same process.
        std::fs::write(root.join("cmdline"), b"umu-run\0Game.exe\0").unwrap();
        let next = cached_process(root, uid, Some(next), now + PROCESS_REFRESH * 2).unwrap();
        assert_eq!(
            next.games,
            vec![
                (GameSource::Umu, "Game.exe".into()),
                (GameSource::Heroic, "NewGame".into())
            ]
        );
        assert!(cached_process(root, uid.wrapping_add(1), Some(next), now).is_none());
    }

    #[test]
    fn games_own_the_processes_below_their_launcher() {
        let info = |parent, names: &[&str], games: Vec<(GameSource, String)>| ProcInfo {
            start: 0,
            parent,
            executable: None,
            refreshed: Instant::now(),
            names: names.iter().map(|name| name.to_string()).collect(),
            games,
        };
        let current = HashMap::from([
            (
                10,
                info(
                    1,
                    &["reaper"],
                    vec![(GameSource::Steam, "steam_app_7".into())],
                ),
            ),
            (11, info(10, &["wine64-preload", "Game.exe"], Vec::new())),
            (12, info(1, &["firefox"], Vec::new())),
        ]);
        assert_eq!(
            group_games(&current),
            vec![ScannedGame {
                source: GameSource::Steam,
                name: "steam_app_7".into(),
                processes: vec!["game.exe".into(), "reaper".into(), "wine64-preload".into()],
            }]
        );
    }

    #[test]
    fn start_time_skips_odd_command_names() {
        let stat =
            "4242 (Web (Content) x) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654 20";
        assert_eq!(parse_start_time(stat), Some(987654));
        assert_eq!(stat_field::<u32>(stat, PARENT), Some(1));
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
