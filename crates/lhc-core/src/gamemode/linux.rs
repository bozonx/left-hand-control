//! Linux game-mode sources: the Feral GameMode daemon over D-Bus, the
//! user's processes from `/proc` (with games that Steam and Lutris run)
//! and the application category of the active window. Fullscreen comes
//! with the active window (`active_window::linux`), which already asks
//! the compositor.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, MessageIterator, Proxy, fdo::DBusProxy};
use zbus::names::BusName;
use zbus::proxy::CacheProperties;
use zbus::{MatchRule, message};

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
    let need_running = super::needs_polling(settings);
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
        return Some(state);
    }
    if let Some(old) = listener.take() {
        let _ = old.connection.close();
    }
    let started = listen()?;
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
            .sender("org.freedesktop.DBus")
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
    /// Zero for another user's process, which is never read.
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

/// Processes of the previous scan by pid, other users' ones included.
#[derive(Default)]
struct ProcessCache {
    processes: HashMap<u32, ProcInfo>,
    scanned: Option<Instant>,
}

/// Within this time the kernel does not hand out a pid again (it would
/// have to cycle through all of `pid_max`), so a pid seen in the previous
/// scan is the same process. Older caches are checked again.
const PID_REUSE_GUARD: Duration = Duration::from_secs(5);

static PROCESSES: Mutex<Option<ProcessCache>> = Mutex::new(None);

/// Scan the current user's processes. Only processes started since the
/// previous scan are read, so frequent polling costs a directory listing.
pub(super) fn scan_processes() -> Option<Scan> {
    let entries = std::fs::read_dir("/proc").ok()?;
    let uid = unsafe { libc::geteuid() };
    let mut cache = PROCESSES.lock().ok()?;
    let previous = cache.take().unwrap_or_default();
    let trust = previous
        .scanned
        .is_some_and(|at| at.elapsed() < PID_REUSE_GUARD);
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
        let known = previous.remove(&pid);
        let info = match known {
            Some(info) if trust => info,
            known => {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                if metadata.uid() != uid {
                    current.insert(pid, ProcInfo::foreign());
                    continue;
                }
                let dir = entry.path();
                let Some(start) = start_time(&dir) else {
                    continue;
                };
                match known {
                    Some(info) if info.start == start => info,
                    _ => read_process(&dir, start),
                }
            }
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
    *cache = Some(ProcessCache {
        processes: current,
        scanned: Some(Instant::now()),
    });
    games.sort();
    games.dedup();
    Some(Scan {
        names: names.into_iter().collect(),
        games,
    })
}

impl ProcInfo {
    fn foreign() -> Self {
        ProcInfo {
            start: 0,
            names: Vec::new(),
            game: None,
        }
    }
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
