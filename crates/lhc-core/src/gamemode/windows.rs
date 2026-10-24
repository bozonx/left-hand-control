//! Windows game-mode sources: running processes from a ToolHelp snapshot,
//! the game Steam runs (`RunningAppID`) and the executables Windows itself
//! recognised as games (the Game Bar list in `GameConfigStore`).
//! Fullscreen comes with the active window (`platform::windows`), which
//! also asks the shell whether a Direct3D game runs fullscreen.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
    RegGetValueW, RegOpenKeyExW,
};
use windows::core::{PCWSTR, PWSTR, w};

use super::{DetectorAvailability, GameModeDetectors, GameModeSettings, Observation};

/// Pause before a failed catalog read is tried again.
const CATALOG_RETRY: Duration = Duration::from_secs(3);

/// How long the Game Bar list is reused before it is read again.
const CATALOG_TTL: Duration = Duration::from_secs(60);

pub(super) fn observe(settings: &GameModeSettings) -> Observation {
    let running = crate::platform::windows::running_process_names();
    let steam = steam_running_app();
    let catalog = catalog();
    let mut known_games = Vec::new();
    if settings.use_steam
        && let Ok(Some(id)) = steam
    {
        known_games.push(format!("steam_app_{id}"));
    }
    if settings.use_game_bar
        && let (Some(running), Ok(catalog)) = (&running, &catalog)
    {
        known_games.extend(
            running
                .iter()
                .filter(|name| {
                    catalog.contains(&name.to_lowercase())
                        && !super::process::launcher_process(name)
                })
                .cloned(),
        );
    }
    let window = crate::active_window::cached_active_window();
    Observation {
        detectors: GameModeDetectors {
            gamemoded: DetectorAvailability::Unsupported,
            fullscreen: availability(window.and_then(|window| window.fullscreen).is_some()),
            processes: availability(running.is_some()),
            active_window: crate::active_window::availability(),
            steam: Some(availability(steam.is_ok())),
            game_bar: Some(availability(catalog.is_ok() && running.is_some())),
            missing_window_tool: None,
        },
        running: running.unwrap_or_default(),
        known_games,
        window_game: None,
        daemon: false,
    }
}

fn availability(available: bool) -> DetectorAvailability {
    if available {
        DetectorAvailability::Available
    } else {
        DetectorAvailability::Unavailable
    }
}

struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// The app id of the game Steam runs now; Steam keeps it in the registry.
fn steam_running_app() -> Result<Option<u32>, ()> {
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Valve\Steam"),
            None,
            KEY_READ,
            &mut key,
        )
    }
    .ok()
    .map_err(|_| ())?;
    let key = RegistryKey(key);
    let mut id = 0_u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            key.0,
            None,
            w!("RunningAppID"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut id as *mut u32).cast()),
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    status.ok().map_err(|_| ())?;
    Ok((id != 0).then_some(id))
}

/// Lowercase executable names from the Game Bar list, cached for
/// [`CATALOG_TTL`].
fn catalog() -> Result<HashSet<String>, ()> {
    type CatalogResult = Result<HashSet<String>, ()>;
    static CACHE: Mutex<Option<(Instant, CatalogResult)>> = Mutex::new(None);
    let Ok(mut cache) = CACHE.lock() else {
        return Err(());
    };
    match cache.as_ref() {
        Some((at, names))
            if at.elapsed()
                < if names.is_ok() {
                    CATALOG_TTL
                } else {
                    CATALOG_RETRY
                } =>
        {
            names.clone()
        }
        _ => {
            let names = read_catalog();
            *cache = Some((Instant::now(), names.clone()));
            names
        }
    }
}

/// `HKCU\System\GameConfigStore\Children\<id>\MatchedExeFullPath`: the
/// executables Windows recognised as games.
fn read_catalog() -> Result<HashSet<String>, ()> {
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
    if opened == ERROR_FILE_NOT_FOUND || opened == ERROR_PATH_NOT_FOUND {
        return Ok(names);
    }
    opened.ok().map_err(|_| ())?;
    let key = RegistryKey(key);
    for index in 0_u32.. {
        let mut child = [0_u16; 256];
        let mut length = child.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                key.0,
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
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        status.ok().map_err(|_| ())?;
        let mut path = vec![0_u16; 32768];
        let mut size = (path.len() * std::mem::size_of::<u16>()) as u32;
        let status = unsafe {
            RegGetValueW(
                key.0,
                PCWSTR(child.as_ptr()),
                w!("MatchedExeFullPath"),
                RRF_RT_REG_SZ,
                None,
                Some(path.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
            continue;
        }
        status.ok().map_err(|_| ())?;
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
    Ok(names)
}
