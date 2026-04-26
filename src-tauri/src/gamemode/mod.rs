use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::process::Command;
use tauri::{AppHandle, Emitter};

use crate::mapper::config::GameModeSettings;

#[cfg(target_os = "linux")]
mod linux;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameModeStatus {
    pub active: bool,
    pub method: Option<String>,
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

pub fn cached_status_active() -> bool {
    CACHED_GAMEMODE_ACTIVE.load(Ordering::SeqCst)
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
            let mut last_active = false;
            
            while !watcher_stop_requested() {
                let status = check_gamemode(&app);
                
                if status.active != last_active {
                    CACHED_GAMEMODE_ACTIVE.store(status.active, Ordering::SeqCst);
                    let time_str = get_current_time();
                    let trigger = status.method.as_deref().unwrap_or("none");
                    println!(
                        "[gamemode debug] [{}] State changed: {} (Trigger: {})",
                        time_str,
                        if status.active { "ACTIVE" } else { "INACTIVE" },
                        trigger
                    );

                    if let Err(e) = app.emit("game-mode-changed", status.clone()) {
                        eprintln!("[gamemode] emit error: {e}");
                    }
                    last_active = status.active;
                }
                
                thread::sleep(Duration::from_secs(2));
            }
        });
}

fn check_gamemode(app: &AppHandle) -> GameModeStatus {
    let settings = load_game_mode_settings(app);

    #[cfg(target_os = "linux")]
    {
        if settings.use_gamemoded && linux::is_gamemoded_active() {
            return GameModeStatus {
                active: true,
                method: Some("gamemoded".into()),
            };
        }

        if settings.use_fullscreen && linux::is_fullscreen_active() {
            return GameModeStatus {
                active: true,
                method: Some("fullscreen".into()),
            };
        }
    }

    GameModeStatus {
        active: false,
        method: None,
    }
}

#[tauri::command]
pub fn get_gamemode_status(app: AppHandle) -> Result<GameModeStatus, String> {
    Ok(check_gamemode(&app))
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
