use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::mapper::config::GameModeSettings;

#[cfg(target_os = "linux")]
mod linux;

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameModeStatus {
    pub active: bool,
    pub method: Option<String>,
    pub detection_enabled: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedSettings {
    #[serde(default)]
    game_mode: GameModeSettings,
}

#[derive(Debug, Deserialize, Default)]
struct PersistedConfig {
    #[serde(default)]
    settings: PersistedSettings,
}

static WATCHER_STOP: AtomicBool = AtomicBool::new(false);
static CACHED_GAMEMODE_ACTIVE: AtomicBool = AtomicBool::new(false);
static CACHED_GAMEMODE_DETECTION_ENABLED: AtomicBool = AtomicBool::new(true);
static CACHED_SETTINGS: Mutex<Option<GameModeSettings>> = Mutex::new(None);
// Full status (including method string) cached for get_gamemode_status.
static CACHED_STATUS_FULL: OnceLock<Mutex<GameModeStatus>> = OnceLock::new();

pub fn cached_status_active() -> bool {
    CACHED_GAMEMODE_ACTIVE.load(Ordering::SeqCst)
}

pub fn cached_detection_enabled() -> bool {
    CACHED_GAMEMODE_DETECTION_ENABLED.load(Ordering::SeqCst)
}

#[cfg(test)]
pub fn set_cached_for_test(active: bool, detection_enabled: bool) {
    CACHED_GAMEMODE_ACTIVE.store(active, Ordering::SeqCst);
    CACHED_GAMEMODE_DETECTION_ENABLED.store(detection_enabled, Ordering::SeqCst);
}

fn store_cached_status(status: &GameModeStatus) {
    CACHED_GAMEMODE_ACTIVE.store(status.active, Ordering::SeqCst);
    CACHED_GAMEMODE_DETECTION_ENABLED.store(status.detection_enabled, Ordering::SeqCst);
    let mutex = CACHED_STATUS_FULL.get_or_init(|| Mutex::new(GameModeStatus::default()));
    if let Ok(mut guard) = mutex.lock() {
        *guard = status.clone();
    }
}

pub fn stop_watcher() {
    WATCHER_STOP.store(true, Ordering::SeqCst);
}

fn cached_status_full() -> GameModeStatus {
    CACHED_STATUS_FULL
        .get_or_init(|| Mutex::new(GameModeStatus::default()))
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

pub fn update_settings_from_config_json(raw: &str) {
    let settings = parse_game_mode_settings(raw);
    match CACHED_SETTINGS.lock() {
        Ok(mut guard) => *guard = settings,
        Err(e) => eprintln!("[gamemode] settings cache lock poisoned, update skipped: {e}"),
    }
}

fn watcher_stop_requested() -> bool {
    WATCHER_STOP.load(Ordering::SeqCst)
}

pub fn start_watcher(app: AppHandle) {
    WATCHER_STOP.store(false, Ordering::SeqCst);

    if let Err(e) = thread::Builder::new()
        .name("gamemode-watcher".into())
        .spawn(move || {
            let mut last_status = GameModeStatus::default();

            while !watcher_stop_requested() {
                let status = check_gamemode(&app);
                store_cached_status(&status);

                if status != last_status {
                    eprintln!(
                        "[gamemode] state changed: {} (method: {})",
                        if status.active { "active" } else { "inactive" },
                        status.method.as_deref().unwrap_or("none"),
                    );

                    if let Err(e) = app.emit("game-mode-changed", status.clone()) {
                        eprintln!("[gamemode] emit error: {e}");
                    }
                    last_status = status;
                }

                thread::sleep(Duration::from_millis(200));
            }
        })
    {
        eprintln!("[gamemode] watcher thread spawn failed: {e}");
    }
}

fn check_gamemode(app: &AppHandle) -> GameModeStatus {
    let settings = load_game_mode_settings(app);
    let detection_enabled = settings.use_gamemoded
        || settings.use_fullscreen
        || settings
            .process_matchers
            .iter()
            .any(|matcher| !matcher.is_blacklist && !matcher.name.trim().is_empty());

    if !detection_enabled {
        return GameModeStatus {
            active: false,
            method: None,
            detection_enabled: false,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let blacklist: Vec<_> = settings
            .process_matchers
            .iter()
            .filter(|m| m.is_blacklist)
            .cloned()
            .collect();

        if !blacklist.is_empty() {
            if let Some(_name) = linux::active_process_match(&blacklist) {
                return GameModeStatus {
                    active: false,
                    method: None,
                    detection_enabled,
                };
            }
        }

        if settings.use_gamemoded && linux::is_gamemoded_active() {
            return GameModeStatus {
                active: true,
                method: Some("gamemoded".into()),
                detection_enabled,
            };
        }

        let whitelist: Vec<_> = settings
            .process_matchers
            .iter()
            .filter(|m| !m.is_blacklist)
            .cloned()
            .collect();

        if let Some(name) = linux::active_process_match(&whitelist) {
            return GameModeStatus {
                active: true,
                method: Some(format!("process:{name}")),
                detection_enabled,
            };
        }

        if settings.use_fullscreen && linux::is_fullscreen_active() {
            return GameModeStatus {
                active: true,
                method: Some("fullscreen".into()),
                detection_enabled,
            };
        }
    }

    GameModeStatus {
        active: false,
        method: None,
        detection_enabled,
    }
}

#[tauri::command]
pub fn get_gamemode_status() -> Result<GameModeStatus, String> {
    // Return the watcher's cached status immediately; the watcher thread
    // keeps it fresh on its 200ms cycle without blocking this call.
    Ok(cached_status_full())
}

fn load_game_mode_settings(app: &AppHandle) -> GameModeSettings {
    if let Ok(guard) = CACHED_SETTINGS.lock() {
        if let Some(settings) = guard.clone() {
            return settings;
        }
    }
    let Ok(storage) = crate::storage::resolve_storage_paths(app) else {
        return GameModeSettings::default();
    };
    let Ok(raw) = storage.load_config() else {
        return GameModeSettings::default();
    };
    if raw.trim().is_empty() {
        return GameModeSettings::default();
    }
    let settings = parse_game_mode_settings(&raw).unwrap_or_default();
    // Populate the cache so subsequent watcher cycles skip the disk read.
    if let Ok(mut guard) = CACHED_SETTINGS.lock() {
        *guard = Some(settings.clone());
    }
    settings
}

fn parse_game_mode_settings(raw: &str) -> Option<GameModeSettings> {
    serde_json::from_str::<PersistedConfig>(raw)
        .map(|config| config.settings.game_mode)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::parse_game_mode_settings;

    #[test]
    fn parses_cached_settings_from_persisted_config() {
        let settings = parse_game_mode_settings(
            r#"{
                "settings": {
                    "gameMode": {
                        "useGamemoded": false,
                        "useFullscreen": true,
                        "processMatchers": [
                            {
                                "id": "steam",
                                "name": "steam",
                                "matchMode": "exact",
                                "onlyActiveWindow": false
                            }
                        ]
                    }
                }
            }"#,
        )
        .expect("parse settings");

        assert!(!settings.use_gamemoded);
        assert!(settings.use_fullscreen);
        assert_eq!(settings.process_matchers.len(), 1);
        assert_eq!(settings.process_matchers[0].name, "steam");
    }
}
