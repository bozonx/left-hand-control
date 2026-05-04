use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
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

pub fn cached_status_active() -> bool {
    CACHED_GAMEMODE_ACTIVE.load(Ordering::SeqCst)
}

pub fn cached_detection_enabled() -> bool {
    CACHED_GAMEMODE_DETECTION_ENABLED.load(Ordering::SeqCst)
}

fn store_cached_status(status: &GameModeStatus) {
    CACHED_GAMEMODE_ACTIVE.store(status.active, Ordering::SeqCst);
    CACHED_GAMEMODE_DETECTION_ENABLED.store(status.detection_enabled, Ordering::SeqCst);
}

pub fn stop_watcher() {
    WATCHER_STOP.store(true, Ordering::SeqCst);
}

fn watcher_stop_requested() -> bool {
    WATCHER_STOP.load(Ordering::SeqCst)
}

fn get_current_time() -> String {
    let output = Command::new("date").arg("+%Y-%m-%d %H:%M:%S").output().ok();
    output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown time".to_string())
}

pub fn start_watcher(app: AppHandle) {
    WATCHER_STOP.store(false, Ordering::SeqCst);

    let _ = thread::Builder::new()
        .name("gamemode-watcher".into())
        .spawn(move || {
            let mut last_status = GameModeStatus::default();

            while !watcher_stop_requested() {
                let status = check_gamemode(&app);
                store_cached_status(&status);

                if status != last_status {
                    let time_str = get_current_time();
                    let trigger = status.method.as_deref().unwrap_or("none");
                    eprintln!(
                        "[gamemode debug] [{}] State changed: {} (Trigger: {})",
                        time_str,
                        if status.active { "ACTIVE" } else { "INACTIVE" },
                        trigger
                    );

                    if let Err(e) = app.emit("game-mode-changed", status.clone()) {
                        eprintln!("[gamemode] emit error: {e}");
                    }
                    last_status = status;
                }

                thread::sleep(Duration::from_secs(2));
            }
        });
}

fn check_gamemode(app: &AppHandle) -> GameModeStatus {
    let settings = load_game_mode_settings(app);
    let detection_enabled = settings.use_gamemoded
        || settings.use_fullscreen
        || settings
            .process_matchers
            .iter()
            .any(|matcher| !matcher.name.trim().is_empty());

    if !detection_enabled {
        return GameModeStatus {
            active: false,
            method: None,
            detection_enabled: false,
        };
    }

    #[cfg(target_os = "linux")]
    {
        if settings.use_gamemoded && linux::is_gamemoded_active() {
            return GameModeStatus {
                active: true,
                method: Some("gamemoded".into()),
                detection_enabled,
            };
        }

        if let Some(name) = linux::active_process_match(&settings.process_matchers) {
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
pub fn get_gamemode_status(app: AppHandle) -> Result<GameModeStatus, String> {
    let status = check_gamemode(&app);
    store_cached_status(&status);
    Ok(status)
}

fn load_game_mode_settings(app: &AppHandle) -> GameModeSettings {
    let Ok(storage) = crate::storage::resolve_storage_paths(app) else {
        return GameModeSettings::default();
    };
    let Ok(raw) = storage.load_config() else {
        return GameModeSettings::default();
    };
    if raw.trim().is_empty() {
        return GameModeSettings::default();
    }
    serde_json::from_str::<PersistedConfig>(&raw)
        .map(|config| config.settings.game_mode)
        .unwrap_or_default()
}
