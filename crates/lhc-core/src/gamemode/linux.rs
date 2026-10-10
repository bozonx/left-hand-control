//! Linux game-mode sources: the Feral GameMode daemon over D-Bus and the
//! user's processes from `/proc`. Fullscreen comes with the active window
//! (`active_window::linux`), which already asks the compositor.

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::sync::OnceLock;

use zbus::blocking::{Connection, Proxy, fdo::DBusProxy};
use zbus::names::BusName;

use crate::platform::linux::command_available;

use super::{DetectorAvailability, GameModeDetectors, GameModeSettings, Observation};

const GAMEMODE_NAME: &str = "com.feralinteractive.GameMode";
const GAMEMODE_PATH: &str = "/com/feralinteractive/GameMode";

pub(super) fn observe(settings: &GameModeSettings) -> Observation {
    let daemon = if settings.use_gamemoded {
        gamemoded_active()
    } else {
        daemon_installed().then_some(false)
    };
    let need_running = super::needs_running_processes(settings);
    let running = need_running.then(running_process_names).flatten();
    let window = crate::active_window::availability();
    Observation {
        detectors: GameModeDetectors {
            gamemoded: availability(daemon.is_some()),
            fullscreen: window,
            processes: availability(if need_running {
                running.is_some()
            } else {
                std::fs::read_dir("/proc").is_ok()
            }),
            active_window: window,
            missing_window_tool: crate::active_window::missing_tool().map(str::to_owned),
        },
        running: running.unwrap_or_default(),
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

fn daemon_installed() -> bool {
    command_available("gamemoded")
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

/// Names of the current user's processes as `comm`, executable and the
/// first command-line word (the `.exe` for Wine and Proton games).
pub(super) fn running_process_names() -> Option<Vec<String>> {
    let mut names = HashSet::new();
    let uid = unsafe { libc::geteuid() };
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        if !entry.metadata().is_ok_and(|metadata| metadata.uid() == uid) {
            continue;
        }
        names.extend(super::process::proc_names(&entry.path()));
    }
    Some(names.into_iter().collect())
}
