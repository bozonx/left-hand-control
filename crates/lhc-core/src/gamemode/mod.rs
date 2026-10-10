//! Game mode: a global on/off state that rule and layout conditions use.
//!
//! The user picks Auto, On or Off. In Auto a watcher thread combines the
//! enabled sources (GameMode daemon, process rules, fullscreen window) and
//! keeps game mode on for [`OFF_DELAY`] after the last match, so Alt+Tab
//! or a loading screen does not flip layouts back and forth. A state that
//! cannot be detected counts as off.

use serde::{Serialize, Serializer};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use crate::profile::model::{GameModeControl, GameModeSettings};
use crate::runtime_state::ActiveWindow;
use crate::storage::StoragePaths;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(crate) mod process;
#[cfg(target_os = "windows")]
mod windows;

const WATCH_INTERVAL: Duration = Duration::from_secs(1);
/// How long Auto keeps game mode on after the last detection.
pub const OFF_DELAY: Duration = Duration::from_secs(3);

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
    /// Command-line tool the window sources need but which is missing,
    /// e.g. `kdotool` on KDE Wayland.
    pub missing_window_tool: Option<String>,
}

/// What turned game mode on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameModeMethod {
    Manual,
    Daemon,
    Fullscreen,
    /// A process rule; the payload is the rule's pattern.
    Process(String),
}

impl Serialize for GameModeMethod {
    /// Plain strings (`gamemoded`, `process:<name>`, …) as the legacy
    /// frontend expects them.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Manual => serializer.serialize_str("manual"),
            Self::Daemon => serializer.serialize_str("gamemoded"),
            Self::Fullscreen => serializer.serialize_str("fullscreen"),
            Self::Process(name) => serializer.serialize_str(&format!("process:{name}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameModeStatus {
    pub revision: u64,
    /// Effective state that conditions see.
    pub active: bool,
    pub method: Option<GameModeMethod>,
    /// At least one automatic source is enabled.
    pub detection_enabled: bool,
    /// The effective state is known: manual, or Auto with a working
    /// source. An unknown state counts as off.
    pub state_available: bool,
    pub control: GameModeControl,
    pub automatic_active: bool,
    pub automatic_available: bool,
    pub automatic_method: Option<GameModeMethod>,
    pub excluded_by: Option<String>,
    pub detectors: GameModeDetectors,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Detection {
    active: bool,
    method: Option<GameModeMethod>,
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
                Some(GameModeMethod::Manual)
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

    /// Apply `change`, publish the new status if it differs and return it.
    fn update(&mut self, change: impl FnOnce(&mut Self)) -> (GameModeStatus, bool) {
        let before = self.status();
        change(self);
        let mut status = self.status();
        let changed = before != status;
        if changed {
            self.revision += 1;
            status.revision = self.revision;
        }
        crate::runtime_state::set_game_mode_active(status.active);
        (status, changed)
    }
}

#[derive(Default)]
struct SettingsCache {
    settings: Option<GameModeSettings>,
    revision: u64,
    /// The remembered Auto/On/Off choice was applied once at startup.
    control_restored: bool,
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
    control_restored: false,
});
static WATCHER: Mutex<Option<Watcher>> = Mutex::new(None);

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}

fn publish((status, changed): (GameModeStatus, bool)) -> GameModeStatus {
    if changed {
        log::debug!("[gamemode] state changed: {status:?}");
        crate::events::emit(crate::events::CoreEvent::GameModeChanged(status.clone()));
    }
    status
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
    let result = state.update(|state| state.control = control);
    drop(state);
    publish(result)
}

/// Flip the effective state: a manual override to the opposite of what
/// is active now.
pub fn toggle() -> GameModeStatus {
    set_control(if status().active {
        GameModeControl::Off
    } else {
        GameModeControl::On
    })
}

/// Use `settings` for detection from now on. The first call also restores
/// a remembered Auto/On/Off choice.
pub fn set_settings(settings: &GameModeSettings) {
    let restore = {
        let Ok(mut cache) = CACHED_SETTINGS.lock() else {
            return;
        };
        cache.settings = Some(settings.clone());
        cache.revision += 1;
        !std::mem::replace(&mut cache.control_restored, true) && settings.remember_control
    };
    if restore {
        set_control(settings.control);
    }
    crate::active_window::set_titles_needed_by_game_mode(crate::profile::app_match::uses_title(
        settings
            .process_matchers
            .iter()
            .map(|matcher| &matcher.name),
    ));
    wake();
}

/// Legacy entry point: detection settings from the raw `config.json` text.
pub fn update_settings_from_config_json(raw: &str) {
    if let Some(settings) = parse_game_mode_settings(raw) {
        set_settings(&settings);
    }
}

/// Re-run detection now, e.g. after the focused window changed.
pub(crate) fn wake() {
    if let Ok(watcher) = WATCHER.lock()
        && let Some(watcher) = watcher.as_ref()
        && let Ok(mut control) = watcher.control.0.lock()
    {
        control.refresh = true;
        watcher.control.1.notify_one();
    }
}

/// Names of the user's running processes, sorted, for picking a rule.
pub fn running_processes() -> Vec<String> {
    #[cfg(target_os = "linux")]
    let names = linux::running_process_names();
    #[cfg(target_os = "windows")]
    let names = crate::platform::windows::running_process_names();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let names: Option<Vec<String>> = None;
    let mut names = names.unwrap_or_default();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
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
            let mut last_match: Option<Instant> = None;
            let mut last_revision = None;
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
                let window = crate::active_window::cached_active_window();
                let detection = evaluate(&settings, &observe(&settings), window.as_ref());
                let now = Instant::now();
                if detection.active {
                    last_match = Some(now);
                }
                // A settings change applies at once; detection gaps wait.
                let settings_changed = last_revision.replace(revision) != Some(revision);
                if let Ok(cache) = CACHED_SETTINGS.lock()
                    && cache.revision == revision
                    && let Ok(control) = thread_control.0.lock()
                    && !control.stop
                    && let Ok(mut state) = state().lock()
                {
                    let detection = if settings_changed {
                        detection
                    } else {
                        hold_active(&state.detection, detection, last_match, now)
                    };
                    let result = state.update(|state| state.detection = detection);
                    drop(state);
                    drop(control);
                    drop(cache);
                    publish(result);
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

/// Keep a recent automatic match alive for [`OFF_DELAY`]; an exclusion
/// ends it at once.
fn hold_active(
    previous: &Detection,
    next: Detection,
    last_match: Option<Instant>,
    now: Instant,
) -> Detection {
    let recent = last_match.is_some_and(|at| now.duration_since(at) < OFF_DELAY);
    if !next.active && next.enabled && next.excluded_by.is_none() && previous.active && recent {
        Detection {
            active: true,
            method: previous.method.clone(),
            ..next
        }
    } else {
        next
    }
}

/// What the platform sources report right now.
#[derive(Debug, Clone, Default)]
struct Observation {
    detectors: GameModeDetectors,
    running: Vec<String>,
    daemon: bool,
}

fn observe(settings: &GameModeSettings) -> Observation {
    #[cfg(target_os = "linux")]
    {
        linux::observe(settings)
    }
    #[cfg(target_os = "windows")]
    {
        windows::observe(settings)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = settings;
        Observation::default()
    }
}

fn needs_running_processes(settings: &GameModeSettings) -> bool {
    settings
        .process_matchers
        .iter()
        .any(|matcher| !process::window_only(matcher) && !matcher.name.trim().is_empty())
}

fn evaluate(
    settings: &GameModeSettings,
    observation: &Observation,
    window: Option<&ActiveWindow>,
) -> Detection {
    let detectors = &observation.detectors;
    let source_available = |matcher: &crate::profile::model::GameModeProcessMatcher| {
        if process::window_only(matcher) {
            detectors.active_window == DetectorAvailability::Available
        } else {
            detectors.processes == DetectorAvailability::Available
        }
    };
    let matches = |matcher: &crate::profile::model::GameModeProcessMatcher| {
        source_available(matcher)
            && if process::window_only(matcher) {
                window.is_some_and(|window| process::matches_window(matcher, window))
            } else {
                observation
                    .running
                    .iter()
                    .any(|name| process::matches_process(matcher, name))
            }
    };
    let whitelist: Vec<_> = settings
        .process_matchers
        .iter()
        .filter(|matcher| !matcher.is_blacklist && !matcher.name.trim().is_empty())
        .collect();
    let enabled = settings.use_gamemoded || settings.use_fullscreen || !whitelist.is_empty();
    let mut detection = Detection {
        enabled,
        // Nothing to detect is a known "off".
        available: !enabled
            || (settings.use_gamemoded && detectors.gamemoded == DetectorAvailability::Available)
            || (settings.use_fullscreen && detectors.fullscreen == DetectorAvailability::Available)
            || whitelist.iter().any(|matcher| source_available(matcher)),
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
    let fullscreen_game = window.is_some_and(|window| {
        window.fullscreen == Some(true) && !process::fullscreen_non_game(window)
    });
    detection.method = if settings.use_gamemoded
        && observation.daemon
        && detectors.gamemoded == DetectorAvailability::Available
    {
        Some(GameModeMethod::Daemon)
    } else if let Some(matcher) = whitelist.into_iter().find(|matcher| matches(matcher)) {
        Some(GameModeMethod::Process(matcher.name.trim().into()))
    } else if settings.use_fullscreen
        && fullscreen_game
        && detectors.fullscreen == DetectorAvailability::Available
    {
        Some(GameModeMethod::Fullscreen)
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
        .unwrap_or_else(|| crate::profile::model::AppSettings::default().game_mode);
    if let Ok(mut cache) = CACHED_SETTINGS.lock() {
        let settings = cache.settings.get_or_insert(settings).clone();
        return (settings, cache.revision);
    }
    (settings, 0)
}

/// The same lenient parsing the settings page uses.
fn parse_game_mode_settings(raw: &str) -> Option<GameModeSettings> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    Some(crate::profile::settings::from_value(value.get("settings")).game_mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::auto_switch::{AutoSwitchContext, matches_condition_set};
    use crate::profile::model::{GameModeProcessMatcher, LayoutConditionSet};

    fn observation(running: &[&str], daemon: bool) -> Observation {
        Observation {
            detectors: GameModeDetectors {
                gamemoded: DetectorAvailability::Available,
                fullscreen: DetectorAvailability::Available,
                processes: DetectorAvailability::Available,
                active_window: DetectorAvailability::Available,
                missing_window_tool: None,
            },
            running: running.iter().map(|name| name.to_string()).collect(),
            daemon,
        }
    }

    fn matcher(name: &str, only_active_window: bool, is_blacklist: bool) -> GameModeProcessMatcher {
        GameModeProcessMatcher {
            id: String::new(),
            name: name.into(),
            only_active_window,
            is_blacklist,
        }
    }

    fn settings(matchers: Vec<GameModeProcessMatcher>) -> GameModeSettings {
        GameModeSettings {
            use_gamemoded: false,
            use_fullscreen: false,
            process_matchers: matchers,
            remember_control: false,
            control: GameModeControl::Auto,
            block_popups: false,
        }
    }

    fn fullscreen(app_id: &str) -> ActiveWindow {
        ActiveWindow {
            app_id: app_id.into(),
            fullscreen: Some(true),
            ..ActiveWindow::default()
        }
    }

    #[test]
    fn blacklist_wins_in_auto_but_manual_override_wins_over_blacklist() {
        let mut settings = settings(vec![matcher("editor", false, true)]);
        settings.use_gamemoded = true;
        settings.use_fullscreen = true;
        let detection = evaluate(
            &settings,
            &observation(&["editor"], true),
            Some(&fullscreen("game")),
        );
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
        let settings = settings(vec![matcher("game", true, false)]);
        let mut window = ActiveWindow {
            title: "game - download in Firefox".into(),
            app_id: "org.mozilla.firefox".into(),
            process_name: Some("firefox".into()),
            fullscreen: None,
        };
        assert!(!evaluate(&settings, &observation(&[], false), Some(&window)).active);
        window.process_name = Some("game.exe".into());
        assert!(evaluate(&settings, &observation(&[], false), Some(&window)).active);
        let mut unavailable = observation(&[], false);
        unavailable.detectors.active_window = DetectorAvailability::Unsupported;
        let detection = evaluate(&settings, &unavailable, Some(&window));
        assert!(!detection.active);
        assert!(!detection.available);
    }

    #[test]
    fn unavailable_source_does_not_prevent_other_sources_from_working() {
        let mut settings = settings(vec![matcher("game", false, false)]);
        settings.use_gamemoded = true;
        let mut observed = observation(&["game.exe"], false);
        observed.detectors.gamemoded = DetectorAvailability::Unsupported;
        let detection = evaluate(&settings, &observed, None);
        assert!(detection.active);
        assert!(detection.available);
        assert_eq!(
            detection.method,
            Some(GameModeMethod::Process("game".into()))
        );
    }

    #[test]
    fn fullscreen_ignores_browsers_and_video_players() {
        let mut settings = settings(Vec::new());
        settings.use_fullscreen = true;
        let observed = observation(&[], false);
        assert!(evaluate(&settings, &observed, Some(&fullscreen("steam_app_42"))).active);
        assert!(!evaluate(&settings, &observed, Some(&fullscreen("firefox"))).active);
        let mut windowed = fullscreen("steam_app_42");
        windowed.fullscreen = Some(false);
        assert!(!evaluate(&settings, &observed, Some(&windowed)).active);
    }

    #[test]
    fn disabled_detection_is_a_known_off_state() {
        let detection = evaluate(&settings(Vec::new()), &observation(&[], true), None);
        assert!(!detection.enabled);
        assert!(detection.available);
        assert!(!detection.active);
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
            assert_eq!(status.method, Some(GameModeMethod::Manual));
            assert!(matches_condition_set(
                &LayoutConditionSet {
                    game_mode: Some(if active { "on" } else { "off" }.into()),
                    layouts: vec![],
                    apps: vec![],
                },
                &AutoSwitchContext {
                    game_mode_active: status.active,
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
                active: true,
                method: Some(GameModeMethod::Process("game".into())),
                ..Detection::default()
            },
            ..State::default()
        };
        assert!(!state.status().active);
        assert!(state.status().automatic_active);
        state.control = GameModeControl::Auto;
        assert!(state.status().active);
        assert_eq!(
            state.status().method,
            Some(GameModeMethod::Process("game".into()))
        );
    }

    #[test]
    fn short_detection_gaps_keep_game_mode_on() {
        let now = Instant::now();
        let active = Detection {
            enabled: true,
            available: true,
            active: true,
            method: Some(GameModeMethod::Fullscreen),
            ..Detection::default()
        };
        let gap = Detection {
            enabled: true,
            available: true,
            ..Detection::default()
        };
        let held = hold_active(
            &active,
            gap.clone(),
            Some(now),
            now + Duration::from_secs(1),
        );
        assert!(held.active);
        assert_eq!(held.method, Some(GameModeMethod::Fullscreen));
        assert!(!hold_active(&active, gap.clone(), Some(now), now + OFF_DELAY).active);
        let excluded = Detection {
            excluded_by: Some("editor".into()),
            ..gap
        };
        assert!(!hold_active(&active, excluded, Some(now), now).active);
    }

    #[test]
    fn method_serializes_as_legacy_strings() {
        assert_eq!(
            serde_json::to_string(&GameModeMethod::Process("steam".into())).unwrap(),
            "\"process:steam\""
        );
        assert_eq!(
            serde_json::to_string(&GameModeMethod::Daemon).unwrap(),
            "\"gamemoded\""
        );
    }

    #[test]
    fn parses_settings_like_the_settings_page() {
        let settings = parse_game_mode_settings(
            r#"{"settings":{"gameMode":{"useFullscreen":true,"processMatchers":[{"name":"steam"},3]}}}"#,
        )
        .unwrap();
        // Missing fields take the page's defaults, a bad entry is skipped.
        assert!(settings.use_gamemoded);
        assert!(settings.use_fullscreen);
        assert_eq!(settings.process_matchers.len(), 1);
        assert!(settings.process_matchers[0].only_active_window);
        let settings = parse_game_mode_settings(r#"{"settings":{}}"#).unwrap();
        assert!(settings.use_gamemoded);
    }
}
