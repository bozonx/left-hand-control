use serde::{Deserialize, Serialize};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::mapper_config::GameModeSettings;
use crate::storage::StoragePaths;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
mod process;
#[cfg(target_os = "windows")]
mod windows;

const WATCH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameModeControl {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetectorAvailability {
    Available,
    Unavailable,
    #[default]
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameModeDetectors {
    pub gamemoded: DetectorAvailability,
    pub fullscreen: DetectorAvailability,
    pub processes: DetectorAvailability,
    pub active_window: DetectorAvailability,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameModeStatus {
    pub revision: u64,
    pub active: bool,
    pub method: Option<String>,
    pub detection_enabled: bool,
    pub state_available: bool,
    pub control: GameModeControl,
    pub automatic_active: bool,
    pub automatic_available: bool,
    pub automatic_method: Option<String>,
    pub excluded_by: Option<String>,
    pub detectors: GameModeDetectors,
}

#[derive(Debug, Clone, Default)]
struct Detection {
    active: bool,
    method: Option<String>,
    enabled: bool,
    available: bool,
    excluded_by: Option<String>,
    detectors: GameModeDetectors,
}

#[derive(Default)]
struct State {
    revision: u64,
    control: GameModeControl,
    detection: Detection,
}

impl State {
    fn status(&self) -> GameModeStatus {
        let manual = self.control != GameModeControl::Auto;
        GameModeStatus {
            revision: self.revision,
            active: match self.control {
                GameModeControl::Auto => self.detection.active,
                GameModeControl::On => true,
                GameModeControl::Off => false,
            },
            method: if manual {
                Some("manual".into())
            } else {
                self.detection.method.clone()
            },
            detection_enabled: self.detection.enabled,
            state_available: manual || self.detection.available,
            control: self.control,
            automatic_active: self.detection.active,
            automatic_available: self.detection.available,
            automatic_method: self.detection.method.clone(),
            excluded_by: self.detection.excluded_by.clone(),
            detectors: self.detection.detectors.clone(),
        }
    }
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

#[derive(Default)]
struct SettingsCache {
    settings: Option<GameModeSettings>,
    revision: u64,
}

#[derive(Default)]
struct WatchControl {
    stop: bool,
    refresh: bool,
}

struct Watcher {
    control: Arc<(Mutex<WatchControl>, Condvar)>,
    thread: JoinHandle<()>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static CACHED_SETTINGS: Mutex<SettingsCache> = Mutex::new(SettingsCache {
    settings: None,
    revision: 0,
});
static WATCHER: Mutex<Option<Watcher>> = Mutex::new(None);

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}

pub fn status() -> GameModeStatus {
    state()
        .lock()
        .map(|state| state.status())
        .unwrap_or_default()
}

pub fn set_control(control: GameModeControl) -> GameModeStatus {
    let Ok(mut state) = state().lock() else {
        return GameModeStatus::default();
    };
    let before = state.status();
    state.control = control;
    if before != state.status() {
        state.revision += 1;
    }
    let status = state.status();
    crate::runtime_state::set_game_mode(status.active, status.state_available);
    drop(state);
    if before != status {
        crate::events::emit(crate::events::CoreEvent::GameModeChanged(status.clone()));
    }
    status
}

pub fn update_settings_from_config_json(raw: &str) {
    let Some(settings) = parse_game_mode_settings(raw) else {
        return;
    };
    if let Ok(mut cache) = CACHED_SETTINGS.lock() {
        cache.settings = Some(settings);
        cache.revision += 1;
    }
    if let Ok(watcher) = WATCHER.lock()
        && let Some(watcher) = watcher.as_ref()
        && let Ok(mut control) = watcher.control.0.lock()
    {
        control.refresh = true;
        watcher.control.1.notify_one();
    }
}

pub fn stop_watcher() {
    let watcher = WATCHER.lock().ok().and_then(|mut watcher| watcher.take());
    if let Some(watcher) = watcher {
        if let Ok(mut control) = watcher.control.0.lock() {
            control.stop = true;
            watcher.control.1.notify_one();
        }
        if let Err(error) = watcher.thread.join() {
            log::error!("[gamemode] watcher panicked: {error:?}");
        }
    }
}

pub fn start_watcher(storage: Option<StoragePaths>) {
    let Ok(mut watcher) = WATCHER.lock() else {
        return;
    };
    if watcher
        .as_ref()
        .is_some_and(|watcher| !watcher.thread.is_finished())
    {
        return;
    }
    let control = Arc::new((Mutex::new(WatchControl::default()), Condvar::new()));
    let thread_control = control.clone();
    let thread = thread::Builder::new()
        .name("gamemode-watcher".into())
        .spawn(move || {
            loop {
                {
                    let Ok(mut control) = thread_control.0.lock() else {
                        break;
                    };
                    if control.stop {
                        break;
                    }
                    control.refresh = false;
                }
                let (settings, revision) = load_game_mode_settings(storage.as_ref());
                let detection = detect(&settings);
                if let Ok(cache) = CACHED_SETTINGS.lock()
                    && cache.revision == revision
                    && let Ok(control) = thread_control.0.lock()
                    && !control.stop
                    && let Ok(mut state) = state().lock()
                {
                    let before = state.status();
                    state.detection = detection;
                    if before != state.status() {
                        state.revision += 1;
                    }
                    let status = state.status();
                    crate::runtime_state::set_game_mode(status.active, status.state_available);
                    drop(state);
                    drop(control);
                    drop(cache);
                    if before != status {
                        log::debug!("[gamemode] state changed: {status:?}");
                        crate::events::emit(crate::events::CoreEvent::GameModeChanged(status));
                    }
                }
                let Ok(control) = thread_control.0.lock() else {
                    break;
                };
                let Ok((control, _)) =
                    thread_control
                        .1
                        .wait_timeout_while(control, WATCH_INTERVAL, |control| {
                            !control.stop && !control.refresh
                        })
                else {
                    break;
                };
                if control.stop {
                    break;
                }
            }
        });
    match thread {
        Ok(thread) => *watcher = Some(Watcher { control, thread }),
        Err(error) => log::error!("[gamemode] watcher thread spawn failed: {error}"),
    }
}

fn detect(settings: &GameModeSettings) -> Detection {
    #[cfg(target_os = "linux")]
    {
        linux::detect(settings)
    }
    #[cfg(target_os = "windows")]
    {
        windows::detect(settings)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = settings;
        Detection::default()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn evaluate(
    settings: &GameModeSettings,
    detectors: GameModeDetectors,
    running_names: &[String],
    active_window: Option<&crate::runtime_state::ActiveWindow>,
    daemon: bool,
    fullscreen: bool,
) -> Detection {
    let source_available = |matcher: &crate::mapper_config::GameModeProcessMatcher| {
        if matcher.only_active_window {
            detectors.active_window == DetectorAvailability::Available
        } else {
            detectors.processes == DetectorAvailability::Available
        }
    };
    let matches = |matcher: &crate::mapper_config::GameModeProcessMatcher| {
        source_available(matcher)
            && if matcher.only_active_window {
                active_window.is_some_and(|window| {
                    process::matches(matcher, &window.app_id)
                        || window
                            .process_name
                            .as_deref()
                            .is_some_and(|name| process::matches(matcher, name))
                })
            } else {
                running_names
                    .iter()
                    .any(|name| process::matches(matcher, name))
            }
    };
    let whitelist: Vec<_> = settings
        .process_matchers
        .iter()
        .filter(|matcher| !matcher.is_blacklist && !matcher.name.trim().is_empty())
        .collect();
    let enabled = settings.use_gamemoded || settings.use_fullscreen || !whitelist.is_empty();
    let available = (settings.use_gamemoded
        && detectors.gamemoded == DetectorAvailability::Available)
        || (settings.use_fullscreen && detectors.fullscreen == DetectorAvailability::Available)
        || whitelist.iter().any(|matcher| source_available(matcher));
    let mut detection = Detection {
        enabled,
        available,
        detectors: detectors.clone(),
        ..Detection::default()
    };
    if !enabled {
        return detection;
    }
    if let Some(matcher) = settings
        .process_matchers
        .iter()
        .find(|matcher| matcher.is_blacklist && matches(matcher))
    {
        detection.excluded_by = Some(matcher.name.trim().into());
        detection.available = true;
        return detection;
    }
    detection.method = if settings.use_gamemoded
        && daemon
        && detectors.gamemoded == DetectorAvailability::Available
    {
        Some("gamemoded".into())
    } else if let Some(matcher) = whitelist.into_iter().find(|matcher| matches(matcher)) {
        Some(format!("process:{}", matcher.name.trim()))
    } else if settings.use_fullscreen
        && fullscreen
        && detectors.fullscreen == DetectorAvailability::Available
    {
        Some("fullscreen".into())
    } else {
        None
    };
    detection.active = detection.method.is_some();
    detection
}

fn load_game_mode_settings(storage: Option<&StoragePaths>) -> (GameModeSettings, u64) {
    if let Ok(cache) = CACHED_SETTINGS.lock()
        && let Some(settings) = &cache.settings
    {
        return (settings.clone(), cache.revision);
    }
    let settings = storage
        .and_then(|storage| storage.load_config().ok())
        .and_then(|raw| parse_game_mode_settings(&raw))
        .unwrap_or_default();
    if let Ok(mut cache) = CACHED_SETTINGS.lock() {
        let settings = cache.settings.get_or_insert(settings).clone();
        return (settings, cache.revision);
    }
    (settings, 0)
}

fn parse_game_mode_settings(raw: &str) -> Option<GameModeSettings> {
    serde_json::from_str::<PersistedConfig>(raw)
        .map(|config| config.settings.game_mode)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::auto_switch::{AutoSwitchContext, matches_condition_set};
    use crate::profile::model::LayoutConditionSet;

    fn detectors() -> GameModeDetectors {
        GameModeDetectors {
            gamemoded: DetectorAvailability::Available,
            fullscreen: DetectorAvailability::Available,
            processes: DetectorAvailability::Available,
            active_window: DetectorAvailability::Available,
        }
    }

    #[test]
    fn blacklist_wins_in_auto_but_manual_override_wins_over_blacklist() {
        let settings = GameModeSettings {
            use_gamemoded: true,
            use_fullscreen: true,
            process_matchers: vec![crate::mapper_config::GameModeProcessMatcher {
                name: "editor".into(),
                only_active_window: false,
                is_blacklist: true,
            }],
        };
        let detection = evaluate(&settings, detectors(), &["editor".into()], None, true, true);
        assert!(!detection.active);
        assert_eq!(detection.excluded_by.as_deref(), Some("editor"));
        let mut state = State {
            detection,
            ..State::default()
        };
        assert!(!state.status().active);
        state.control = GameModeControl::On;
        assert!(state.status().active);
        state.control = GameModeControl::Auto;
        assert!(!state.status().active);
    }

    #[test]
    fn foreground_process_rule_does_not_match_a_browser_title() {
        let settings = GameModeSettings {
            process_matchers: vec![crate::mapper_config::GameModeProcessMatcher {
                name: "game".into(),
                only_active_window: true,
                is_blacklist: false,
            }],
            ..GameModeSettings::default()
        };
        let mut window = crate::runtime_state::ActiveWindow {
            title: "game - download in Firefox".into(),
            app_id: "org.mozilla.firefox".into(),
            process_name: Some("firefox".into()),
        };
        assert!(!evaluate(&settings, detectors(), &[], Some(&window), false, false).active);
        window.process_name = Some("game.exe".into());
        assert!(evaluate(&settings, detectors(), &[], Some(&window), false, false).active);
        let mut unavailable = detectors();
        unavailable.active_window = DetectorAvailability::Unsupported;
        let detection = evaluate(&settings, unavailable, &[], Some(&window), false, false);
        assert!(!detection.active);
        assert!(!detection.available);
    }

    #[test]
    fn unavailable_source_does_not_prevent_other_sources_from_working() {
        let settings = GameModeSettings {
            use_gamemoded: true,
            process_matchers: vec![crate::mapper_config::GameModeProcessMatcher {
                name: "game".into(),
                only_active_window: false,
                is_blacklist: false,
            }],
            ..GameModeSettings::default()
        };
        let mut capabilities = detectors();
        capabilities.gamemoded = DetectorAvailability::Unsupported;
        let detection = evaluate(
            &settings,
            capabilities,
            &["game.exe".into()],
            None,
            false,
            false,
        );
        assert!(detection.active);
        assert!(detection.available);
        assert_eq!(detection.method.as_deref(), Some("process:game"));
    }

    #[test]
    fn manual_override_is_available_without_any_detectors() {
        for (control, active) in [(GameModeControl::On, true), (GameModeControl::Off, false)] {
            let state = State {
                control,
                ..State::default()
            };
            let status = state.status();
            assert_eq!(status.active, active);
            assert!(status.state_available);
            assert!(!status.detection_enabled);
            assert_eq!(status.method.as_deref(), Some("manual"));
            assert!(matches_condition_set(
                &LayoutConditionSet {
                    game_mode: Some(if active { "on" } else { "off" }.into()),
                    layouts: vec![],
                    apps: vec![],
                },
                &AutoSwitchContext {
                    game_mode_active: status.active,
                    game_mode_detection_enabled: status.state_available,
                    ..AutoSwitchContext::default()
                },
            ));
        }
    }

    #[test]
    fn automatic_detection_keeps_updating_during_manual_override() {
        let mut state = State {
            control: GameModeControl::Off,
            detection: Detection {
                enabled: true,
                available: true,
                ..Detection::default()
            },
            ..State::default()
        };
        state.detection.active = true;
        state.detection.method = Some("process:game".into());
        assert!(!state.status().active);
        assert!(state.status().automatic_active);
        state.control = GameModeControl::Auto;
        assert!(state.status().active);
        assert_eq!(state.status().method.as_deref(), Some("process:game"));
    }

    #[test]
    fn enabled_but_unavailable_detection_is_not_a_known_off_state() {
        let state = State {
            detection: Detection {
                enabled: true,
                ..Detection::default()
            },
            ..State::default()
        };
        assert!(state.status().detection_enabled);
        assert!(!state.status().state_available);
        assert!(!matches_condition_set(
            &LayoutConditionSet {
                game_mode: Some("off".into()),
                layouts: vec![],
                apps: vec![]
            },
            &AutoSwitchContext {
                game_mode_detection_enabled: false,
                ..AutoSwitchContext::default()
            },
        ));
    }

    #[test]
    fn parses_cached_settings_from_persisted_config() {
        let settings = parse_game_mode_settings(
            r#"{"settings":{"gameMode":{"useGamemoded":false,"useFullscreen":true,"processMatchers":[{"name":"steam","onlyActiveWindow":false}]}}}"#,
        ).unwrap();
        assert!(!settings.use_gamemoded);
        assert!(settings.use_fullscreen);
        assert_eq!(settings.process_matchers[0].name, "steam");
    }
}
