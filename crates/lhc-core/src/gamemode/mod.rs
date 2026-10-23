//! Game mode: a global on/off state that rule and layout conditions use.
//!
//! The user picks Auto, On or Off. In Auto a watcher thread combines the
//! enabled sources (GameMode daemon, games known to a launcher or to the
//! system, process rules, fullscreen window) and
//! keeps game mode on for [`OFF_DELAY`] after the last match, so Alt+Tab
//! or a loading screen does not flip layouts back and forth. A state that
//! cannot be detected counts as off.

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub use crate::profile::model::{GameModeControl, GameModeSettings};
use crate::runtime_state::ActiveWindow;
use crate::storage::StoragePaths;

#[cfg(target_os = "linux")]
mod desktop_entries;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
pub(crate) mod process;
#[cfg(target_os = "windows")]
mod windows;

pub use process::{FULLSCREEN_NON_GAMES, GAME_LAUNCHERS};

/// Poll interval while running processes are watched or game mode is on.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
/// Safety re-check otherwise: window and daemon changes wake the watcher.
const IDLE_INTERVAL: Duration = Duration::from_secs(30);
/// How long Auto keeps game mode on after the last detection.
pub const OFF_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DetectorAvailability {
    Available,
    Unavailable,
    #[default]
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameModeDetectors {
    pub gamemoded: DetectorAvailability,
    pub fullscreen: DetectorAvailability,
    pub processes: DetectorAvailability,
    pub active_window: DetectorAvailability,
    /// Registry sources have separate health from process enumeration.
    pub steam: Option<DetectorAvailability>,
    pub game_bar: Option<DetectorAvailability>,
    /// Command-line tool the window sources need but which is missing,
    /// e.g. `kdotool` on KDE Wayland.
    pub missing_window_tool: Option<String>,
}

/// A way to recognise a running game without a rule. Each platform
/// supports some of them, see [`GameSource::SUPPORTED`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameSource {
    /// Linux: games Steam starts (`reaper SteamLaunch AppId=<id>`) and
    /// `steam_app_<id>` windows. Windows: the game Steam reports running.
    Steam,
    /// Linux: games Lutris starts through `lutris-wrapper`.
    Lutris,
    /// Linux: games Heroic starts; they inherit `HEROIC_APP_NAME`.
    Heroic,
    /// Linux: games run through `umu-run`, which Heroic, Lutris, Faugus
    /// and other launchers use for Proton.
    Umu,
    /// Linux: a nested gamescope session in the active window.
    Gamescope,
    /// The active window's application category: `.desktop` files on
    /// Linux, `Info.plist` on macOS. Launchers are skipped
    /// ([`GAME_LAUNCHERS`]).
    AppCategory,
    /// Windows: running executables from the Game Bar list. Launchers
    /// are skipped ([`GAME_LAUNCHERS`]).
    GameBar,
}

impl GameSource {
    #[cfg(target_os = "linux")]
    pub const SUPPORTED: &[Self] = &[
        Self::Steam,
        Self::Lutris,
        Self::Heroic,
        Self::Umu,
        Self::Gamescope,
        Self::AppCategory,
    ];
    #[cfg(target_os = "windows")]
    pub const SUPPORTED: &[Self] = &[Self::Steam, Self::GameBar];
    #[cfg(target_os = "macos")]
    pub const SUPPORTED: &[Self] = &[Self::AppCategory];
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    pub const SUPPORTED: &[Self] = &[];

    /// Whether the user turned this source on.
    pub fn enabled(self, settings: &GameModeSettings) -> bool {
        match self {
            Self::Steam => settings.use_steam,
            Self::Lutris => settings.use_lutris,
            Self::Heroic => settings.use_heroic,
            Self::Umu => settings.use_umu,
            Self::Gamescope => settings.use_gamescope,
            Self::AppCategory => settings.use_app_category,
            Self::GameBar => settings.use_game_bar,
        }
    }

    /// Whether the source can be read now. `Unsupported` means this OS has
    /// no such source; one the current session cannot read is
    /// `Unavailable`.
    pub fn availability(self, detectors: &GameModeDetectors) -> DetectorAvailability {
        if !Self::SUPPORTED.contains(&self) {
            return DetectorAvailability::Unsupported;
        }
        let detector = match self {
            Self::Gamescope | Self::AppCategory => detectors.active_window,
            // Steam keeps the running game in the registry.
            Self::Steam if cfg!(target_os = "windows") => {
                detectors.steam.unwrap_or(DetectorAvailability::Unavailable)
            }
            Self::GameBar if cfg!(target_os = "windows") => detectors
                .game_bar
                .unwrap_or(DetectorAvailability::Unavailable),
            Self::Steam | Self::Lutris | Self::Heroic | Self::Umu | Self::GameBar => {
                detectors.processes
            }
        };
        match detector {
            DetectorAvailability::Unsupported => DetectorAvailability::Unavailable,
            available => available,
        }
    }

    /// Sources that have no change events and are polled.
    fn polled(self) -> bool {
        matches!(
            self,
            Self::Steam | Self::Lutris | Self::Heroic | Self::Umu | Self::GameBar
        )
    }
}

/// The supported sources the user turned on.
fn enabled_sources(settings: &GameModeSettings) -> impl Iterator<Item = GameSource> + '_ {
    GameSource::SUPPORTED
        .iter()
        .copied()
        .filter(|source| source.enabled(settings))
}

/// What turned game mode on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameModeMethod {
    Manual,
    Daemon,
    /// A game a launcher runs or the system lists; the payload names it,
    /// e.g. `steam_app_1091500` or `Game.exe`.
    KnownGame(String),
    Fullscreen,
    /// A process rule; the payload is the rule's pattern.
    Process(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// The active window would look like a game to a heuristic source but
    /// is on its built-in list (a launcher, a browser, a player); it names
    /// the window's application.
    pub skipped_window: Option<String>,
    pub detectors: GameModeDetectors,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Detection {
    active: bool,
    method: Option<GameModeMethod>,
    enabled: bool,
    available: bool,
    excluded_by: Option<String>,
    skipped_window: Option<String>,
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
            skipped_window: self.detection.skipped_window.clone(),
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
    control_restored: bool,
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
    control_restored: false,
    settings: None,
    revision: 0,
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

impl SettingsCache {
    /// Control changes do not invalidate detection or its grace period.
    fn update(&mut self, settings: &GameModeSettings) -> (bool, bool) {
        let apply_control = !self.control_restored
            || self
                .settings
                .as_ref()
                .is_none_or(|old| old.control != settings.control);
        self.control_restored = true;
        let mut detection_settings = settings.clone();
        detection_settings.control = GameModeControl::Auto;
        let changed = self.settings.as_ref().is_none_or(|old| {
            let mut old = old.clone();
            old.control = GameModeControl::Auto;
            old != detection_settings
        });
        if changed {
            self.revision += 1;
        }
        self.settings = Some(settings.clone());
        (apply_control, changed)
    }
}

/// Apply detection settings and a changed persisted Auto/On/Off choice.
pub fn set_settings(settings: &GameModeSettings) {
    let (result, changed) = {
        let Ok(mut cache) = CACHED_SETTINGS.lock() else {
            return;
        };
        let (apply_control, changed) = cache.update(settings);
        // Serialize persisted choices with cache updates, including startup.
        let result = apply_control
            .then(|| {
                state()
                    .lock()
                    .ok()
                    .map(|mut state| state.update(|state| state.control = settings.control))
            })
            .flatten();
        (result, changed)
    };
    if let Some(result) = result {
        publish(result);
    }
    if changed {
        crate::active_window::set_titles_needed_by_game_mode(
            crate::profile::app_match::uses_title(
                settings
                    .process_matchers
                    .iter()
                    .map(|matcher| &matcher.name),
            ),
        );
        wake();
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
    #[cfg(target_os = "macos")]
    let names = macos::running_process_names();
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
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
    #[cfg(target_os = "linux")]
    linux::stop();
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
                let detection = evaluate(
                    &settings,
                    &observe(&settings, window.as_ref()),
                    window.as_ref(),
                );
                let now = Instant::now();
                if detection.active {
                    last_match = Some(now);
                }
                // A settings change applies at once; detection gaps wait.
                let settings_changed = last_revision.replace(revision) != Some(revision);
                // Processes have no change events, and a held state must
                // expire on time; everything else wakes the watcher.
                let mut interval = if needs_polling(&settings) {
                    WATCH_INTERVAL
                } else {
                    IDLE_INTERVAL
                };
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
                    if state.detection.active {
                        interval = WATCH_INTERVAL;
                    }
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
                        .wait_timeout_while(control, interval, |control| {
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
    /// Running games that a launcher or the system knows, from enabled
    /// sources only.
    known_games: Vec<String>,
    /// The active window's application when the system lists it as a game.
    window_game: Option<String>,
    daemon: bool,
}

fn observe(settings: &GameModeSettings, window: Option<&ActiveWindow>) -> Observation {
    #[cfg(target_os = "linux")]
    {
        linux::observe(settings, window)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = window;
        windows::observe(settings)
    }
    #[cfg(target_os = "macos")]
    {
        macos::observe(settings, window)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (settings, window);
        Observation::default()
    }
}

/// The first known game from an enabled source: running under a launcher
/// or listed by the system, or shown in the active window. What a
/// launcher reports always counts; the application category skips
/// launchers ([`GAME_LAUNCHERS`]), which are tagged as games too.
fn known_game(
    settings: &GameModeSettings,
    observation: &Observation,
    window: Option<&ActiveWindow>,
) -> Option<String> {
    observation
        .known_games
        .first()
        .cloned()
        .or_else(|| {
            window
                .and_then(process::known_game_window)
                .filter(|(source, _)| {
                    GameSource::SUPPORTED.contains(source) && source.enabled(settings)
                })
                .map(|(_, name)| name)
        })
        .or_else(|| {
            window
                .filter(|window| !process::launcher_window(window))
                .and(observation.window_game.clone())
        })
}

/// The active window's application when a heuristic source would
/// consider it but its built-in list skips it.
fn skipped_window(
    settings: &GameModeSettings,
    detectors: &GameModeDetectors,
    window: &ActiveWindow,
) -> Option<String> {
    let category = settings.use_app_category
        && GameSource::AppCategory.availability(detectors) == DetectorAvailability::Available
        && process::launcher_window(window);
    let fullscreen = settings.use_fullscreen
        && detectors.fullscreen == DetectorAvailability::Available
        && window.fullscreen == Some(true)
        && process::fullscreen_non_game(window);
    (category || fullscreen)
        .then(|| process::window_name(window))
        .filter(|name| !name.is_empty())
}

/// Whether detection reads sources that report no changes: running
/// processes, and on Windows the game Steam runs.
fn needs_polling(settings: &GameModeSettings) -> bool {
    needs_running_processes(settings) || enabled_sources(settings).any(GameSource::polled)
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
    let known_games = enabled_sources(settings).next().is_some();
    let enabled = (settings.use_gamemoded
        && detectors.gamemoded != DetectorAvailability::Unsupported)
        || settings.use_fullscreen
        || known_games
        || !whitelist.is_empty();
    let mut detection = Detection {
        enabled,
        // Nothing to detect is a known "off".
        available: !enabled
            || (settings.use_gamemoded && detectors.gamemoded == DetectorAvailability::Available)
            || (settings.use_fullscreen && detectors.fullscreen == DetectorAvailability::Available)
            || enabled_sources(settings)
                .any(|source| source.availability(detectors) == DetectorAvailability::Available)
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
    } else if let Some(name) = known_games
        .then(|| known_game(settings, observation, window))
        .flatten()
    {
        Some(GameModeMethod::KnownGame(name))
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
    if !detection.active {
        detection.skipped_window =
            window.and_then(|window| skipped_window(settings, detectors, window));
    }
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
        if let Some(settings) = &cache.settings {
            return (settings.clone(), cache.revision);
        }
        cache.update(&settings);
        let revision = cache.revision;
        let result = state()
            .lock()
            .ok()
            .map(|mut state| state.update(|state| state.control = settings.control));
        drop(cache);
        if let Some(result) = result {
            publish(result);
        }
        crate::active_window::set_titles_needed_by_game_mode(
            crate::profile::app_match::uses_title(
                settings
                    .process_matchers
                    .iter()
                    .map(|matcher| &matcher.name),
            ),
        );
        return (settings, revision);
    }
    (settings, 0)
}

/// Game-mode settings of `config.json`; `None` when the file is invalid.
fn parse_game_mode_settings(raw: &str) -> Option<GameModeSettings> {
    Some(crate::profile::settings::parse(raw).ok()?.game_mode)
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
                ..GameModeDetectors::default()
            },
            running: running.iter().map(|name| name.to_string()).collect(),
            known_games: Vec::new(),
            window_game: None,
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
            use_steam: false,
            use_lutris: false,
            use_heroic: false,
            use_umu: false,
            use_gamescope: false,
            use_app_category: false,
            use_game_bar: false,
            process_matchers: matchers,
            control: GameModeControl::Auto,
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
    fn persisted_control_changes_do_not_invalidate_detection() {
        let mut cache = SettingsCache::default();
        let mut settings = settings(vec![]);
        assert_eq!(cache.update(&settings), (true, true));
        let revision = cache.revision;
        assert_eq!(cache.update(&settings), (false, false));
        settings.control = GameModeControl::On;
        assert_eq!(cache.update(&settings), (true, false));
        assert_eq!(cache.revision, revision);
        settings.control = GameModeControl::Off;
        assert_eq!(cache.update(&settings), (true, false));
        settings.use_fullscreen = true;
        assert_eq!(cache.update(&settings), (false, true));
        assert_eq!(cache.revision, revision + 1);
    }

    #[test]
    fn watcher_preloaded_settings_still_restore_control() {
        let mut settings = settings(vec![]);
        settings.control = GameModeControl::On;
        let mut cache = SettingsCache {
            settings: Some(settings.clone()),
            ..SettingsCache::default()
        };
        assert!(cache.update(&settings).0);
        assert!(!cache.update(&settings).0);
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
        let browser = evaluate(&settings, &observed, Some(&fullscreen("firefox")));
        assert!(!browser.active);
        assert_eq!(browser.skipped_window.as_deref(), Some("firefox"));
        let mut windowed = fullscreen("steam_app_42");
        windowed.fullscreen = Some(false);
        assert!(!evaluate(&settings, &observed, Some(&windowed)).active);
    }

    #[test]
    fn unsupported_window_sources_do_not_leak_into_other_platforms() {
        let mut settings = settings(Vec::new());
        settings.use_steam = true;
        settings.use_gamescope = true;
        settings.use_app_category = true;
        let observed = observation(&[], false);
        for (source, app_id) in [
            (GameSource::Steam, "steam_app_42"),
            (GameSource::Gamescope, "gamescope"),
        ] {
            assert_eq!(
                evaluate(&settings, &observed, Some(&fullscreen(app_id))).active,
                GameSource::SUPPORTED.contains(&source)
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn known_games_come_from_launchers_windows_and_categories() {
        let mut settings = settings(Vec::new());
        settings.use_steam = true;
        settings.use_app_category = true;
        let mut observed = observation(&[], false);
        assert!(!evaluate(&settings, &observed, None).active);
        // A Steam game in the foreground, even windowed.
        let mut window = fullscreen("steam_app_42");
        window.fullscreen = Some(false);
        assert_eq!(
            evaluate(&settings, &observed, Some(&window)).method,
            Some(GameModeMethod::KnownGame("steam_app_42".into()))
        );
        // A running game wins over the window.
        observed.known_games = vec!["steam_app_7".into()];
        assert_eq!(
            evaluate(&settings, &observed, Some(&window)).method,
            Some(GameModeMethod::KnownGame("steam_app_7".into()))
        );
        // What a launcher runs counts even when it is a launcher itself,
        // like Battle.net installed in Lutris, and in front of a launcher.
        observed.known_games = vec!["Battle.net".into()];
        assert_eq!(
            evaluate(
                &settings,
                &observed,
                Some(&fullscreen("com.valvesoftware.Steam"))
            )
            .method,
            Some(GameModeMethod::KnownGame("Battle.net".into()))
        );
        observed.known_games.clear();
        observed.window_game = Some("org.example.puzzle".into());
        assert_eq!(
            evaluate(
                &settings,
                &observed,
                Some(&fullscreen("org.example.puzzle"))
            )
            .method,
            Some(GameModeMethod::KnownGame("org.example.puzzle".into()))
        );
        let launcher = evaluate(
            &settings,
            &observed,
            Some(&fullscreen("com.valvesoftware.Steam")),
        );
        assert!(!launcher.active);
        assert_eq!(
            launcher.skipped_window.as_deref(),
            Some("com.valvesoftware.Steam")
        );
        // An exclusion still wins, and the source can be turned off.
        observed.running = vec!["obs".into()];
        let mut excluded = settings.clone();
        excluded.process_matchers = vec![matcher("obs", false, true)];
        assert!(!evaluate(&excluded, &observed, Some(&window)).active);
        settings.use_steam = false;
        settings.use_app_category = false;
        assert!(!evaluate(&settings, &observed, Some(&window)).active);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn window_sources_turn_off_separately() {
        let mut settings = settings(Vec::new());
        settings.use_steam = true;
        let observed = observation(&[], false);
        assert!(evaluate(&settings, &observed, Some(&fullscreen("steam_app_42"))).active);
        assert!(!evaluate(&settings, &observed, Some(&fullscreen("gamescope"))).active);
        settings.use_gamescope = true;
        assert_eq!(
            evaluate(&settings, &observed, Some(&fullscreen("gamescope"))).method,
            Some(GameModeMethod::KnownGame("gamescope".into()))
        );
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
    fn parses_settings_like_the_settings_page() {
        let settings = parse_game_mode_settings(
            r#"{"version":1,"settings":{"gameMode":{"detectors":{"fullscreen":true},"processes":[{"name":"steam"}]}},"state":{"gameMode":"on"}}"#,
        )
        .unwrap();
        // Missing fields take the page's defaults.
        assert_eq!(settings.use_gamemoded, cfg!(target_os = "linux"));
        assert!(settings.use_fullscreen);
        assert_eq!(settings.process_matchers.len(), 1);
        assert!(settings.process_matchers[0].only_active_window);
        assert_eq!(settings.control, GameModeControl::On);
        let settings = parse_game_mode_settings(r#"{"version":1}"#).unwrap();
        assert_eq!(settings.use_gamemoded, cfg!(target_os = "linux"));
        assert!(
            parse_game_mode_settings(r#"{"version":1,"settings":{"gameMode":{"x":1}}}"#).is_none()
        );
    }
}
