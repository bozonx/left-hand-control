//! Windows game-mode sources: running processes from a ToolHelp snapshot,
//! the game Steam runs (`RunningAppID`) and the executables Windows itself
//! recognised as games (the Game Bar list in `GameConfigStore`).
//! Fullscreen comes with the active window (`platform::windows`), which
//! also asks the shell whether a Direct3D game runs fullscreen.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::ERROR_MORE_DATA;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
    RegGetValueW, RegOpenKeyExW,
};
use windows::core::{PCWSTR, PWSTR, w};

use super::{DetectorAvailability, GameModeDetectors, GameModeSettings, Observation};

/// How long the Game Bar list is reused before it is read again.
const CATALOG_TTL: Duration = Duration::from_secs(60);

pub(super) fn observe(settings: &GameModeSettings) -> Observation {
    let need_running = super::needs_running_processes(settings) || settings.use_known_games;
    let running = need_running
        .then(crate::platform::windows::running_process_names)
        .flatten();
    let known_games = if settings.use_known_games {
        known_games(running.as_deref().unwrap_or_default())
    } else {
        Vec::new()
    };
    Observation {
        detectors: GameModeDetectors {
            gamemoded: DetectorAvailability::Unsupported,
            fullscreen: DetectorAvailability::Available,
            processes: if need_running && running.is_none() {
                DetectorAvailability::Unavailable
            } else {
                DetectorAvailability::Available
            },
            active_window: crate::active_window::availability(),
            known_games: DetectorAvailability::Available,
            missing_window_tool: None,
        },
        running: running.unwrap_or_default(),
        known_games,
        window_game: None,
        daemon: false,
    }
}

/// The game Steam runs, then running executables from the Game Bar list.
fn known_games(running: &[String]) -> Vec<String> {
    let mut games: Vec<String> = steam_running_app()
        .map(|id| format!("steam_app_{id}"))
        .into_iter()
        .collect();
    let catalog = catalog();
    games.extend(
        running
            .iter()
            .filter(|name| catalog.contains(&name.to_lowercase()))
            .cloned(),
    );
    games
}

/// The app id of the game Steam runs now; Steam keeps it in the registry.
fn steam_running_app() -> Option<u32> {
    let mut id = 0_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Valve\Steam"),
            w!("RunningAppID"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut id as *mut u32).cast()),
            Some(&mut size),
        )
    };
    (status.is_ok() && id != 0).then_some(id)
}

/// Lowercase executable names from the Game Bar list, cached for
/// [`CATALOG_TTL`].
fn catalog() -> HashSet<String> {
    static CACHE: Mutex<Option<(Instant, HashSet<String>)>> = Mutex::new(None);
    let Ok(mut cache) = CACHE.lock() else {
        return HashSet::new();
    };
    match cache.as_ref() {
        Some((at, names)) if at.elapsed() < CATALOG_TTL => names.clone(),
        _ => {
            let names = read_catalog();
            *cache = Some((Instant::now(), names.clone()));
            names
        }
    }
}

/// `HKCU\System\GameConfigStore\Children\<id>\MatchedExeFullPath`: the
/// executables Windows recognised as games.
fn read_catalog() -> HashSet<String> {
    let mut names = HashSet::new();
    let mut key = HKEY::default();
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"System\GameConfigStore\Children"),
            None,
            KEY_READ,
            &mut key,
        )
    };
    if opened.is_err() {
        return names;
    }
    for index in 0_u32.. {
        let mut child = [0_u16; 256];
        let mut length = child.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR(child.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if status == ERROR_MORE_DATA {
            continue;
        }
        if status.is_err() {
            break;
        }
        let mut path = [0_u16; 1024];
        let mut size = std::mem::size_of_val(&path) as u32;
        let status = unsafe {
            RegGetValueW(
                key,
                PCWSTR(child.as_ptr()),
                w!("MatchedExeFullPath"),
                RRF_RT_REG_SZ,
                None,
                Some(path.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            continue;
        }
        let length = path
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(path.len());
        let path = String::from_utf16_lossy(&path[..length]);
        let name = super::process::file_name(&path).trim().to_lowercase();
        if !name.is_empty() {
            names.insert(name);
        }
    }
    let _ = unsafe { RegCloseKey(key) };
    names
}
