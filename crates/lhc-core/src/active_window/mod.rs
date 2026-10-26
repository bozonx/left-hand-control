//! Detection of the currently focused window.
//!
//! A watcher thread caches the latest result in `runtime_state` and emits
//! `CoreEvent::ActiveWindowChanged`; the mapper engine reads the cache to
//! evaluate per-rule app conditions. Where the platform reports focus
//! changes (KWin script, Hyprland and Sway IPC, wlr foreign-toplevel
//! events, the X11 root property, a Windows WinEvent hook) the watcher re-reads on every event and polls
//! only as a slow fallback.
//!
//! Window titles are read only while some condition needs them
//! (`title:` patterns, see `profile::app_match`): the app id and the
//! process are enough otherwise, and a title is the most private and the
//! least stable part of a window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(target_os = "linux")]
mod gnome;
#[cfg(target_os = "linux")]
mod kwin;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod terminal;
#[cfg(target_os = "linux")]
mod wlr;

pub use crate::runtime_state::ActiveWindow;

/// Poll interval without focus events.
const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Safety re-read while focus events arrive, e.g. for X11 titles.
const EVENT_FALLBACK_INTERVAL: Duration = Duration::from_secs(3);

/// The shell (rules, auto mode) needs window titles.
static TITLES_FOR_SHELL: AtomicBool = AtomicBool::new(false);
/// Game-mode process rules need window titles.
static TITLES_FOR_GAME_MODE: AtomicBool = AtomicBool::new(false);
/// Windows of this application do not count as the active window.
static IGNORE_OWN_WINDOWS: AtomicBool = AtomicBool::new(false);
static ACTUAL_WINDOW: Mutex<Option<ActiveWindow>> = Mutex::new(None);
#[cfg(target_os = "linux")]
static TEXT_WINDOW: Mutex<Option<ActiveWindow>> = Mutex::new(None);

#[cfg(target_os = "linux")]
fn is_text_popup(window: &ActiveWindow) -> bool {
    window.app_id == crate::storage::POPUP_APP_ID
}

#[cfg(target_os = "linux")]
fn remember_text_window(current: &Option<ActiveWindow>) {
    if let Ok(mut target) = TEXT_WINDOW.lock() {
        update_text_window(&mut target, current);
    }
}

#[cfg(target_os = "linux")]
fn update_text_window(target: &mut Option<ActiveWindow>, current: &Option<ActiveWindow>) {
    if !current.as_ref().is_some_and(is_text_popup) {
        *target = current.clone();
    }
}

/// Whether the window text input goes to is a terminal emulator.
#[cfg(target_os = "linux")]
pub(crate) fn text_window_is_terminal() -> bool {
    TEXT_WINDOW
        .lock()
        .ok()
        .is_some_and(|target| target.as_ref().is_some_and(terminal::is_terminal))
}

/// An application with an open window, for picking a condition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenWindow {
    pub app_id: String,
    pub process_name: Option<String>,
    pub title: String,
}

pub(crate) fn availability() -> crate::gamemode::DetectorAvailability {
    #[cfg(target_os = "linux")]
    {
        linux::availability()
    }
    // The system API always answers; an empty desktop is no failure.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        crate::gamemode::DetectorAvailability::Available
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        crate::gamemode::DetectorAvailability::Unsupported
    }
}

/// Tool window detection needs but cannot find, e.g. `kdotool`.
#[cfg(target_os = "linux")]
pub(crate) fn missing_tool() -> Option<&'static str> {
    linux::missing_tool()
}

/// Whether the shell's conditions need window titles. Off by default.
/// Skip this application's own windows (settings, popups, also those of
/// helper processes of the same executable): while one has the focus the
/// previous window stays active, so conditions and the application
/// picker see the application the user works in.
pub fn ignore_own_windows(ignore: bool) {
    IGNORE_OWN_WINDOWS.store(ignore, Ordering::SeqCst);
}

/// Whether `process` is this application's executable.
fn is_own_process(process: Option<&str>) -> bool {
    static OWN: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let own = OWN.get_or_init(|| {
        std::env::current_exe()
            .ok()?
            .file_name()?
            .to_str()
            .map(|name| crate::gamemode::process::without_deleted(name).to_lowercase())
    });
    matches!((own, process), (Some(own), Some(process)) if *own == process.to_lowercase())
}

fn is_ignored(window: &ActiveWindow) -> bool {
    IGNORE_OWN_WINDOWS.load(Ordering::SeqCst) && is_own_process(window.process_name.as_deref())
}

pub fn set_titles_needed(needed: bool) {
    if TITLES_FOR_SHELL.swap(needed, Ordering::SeqCst) != needed {
        wake();
    }
}

pub(crate) fn set_titles_needed_by_game_mode(needed: bool) {
    if TITLES_FOR_GAME_MODE.swap(needed, Ordering::SeqCst) != needed {
        wake();
    }
}

pub(crate) fn titles_needed() -> bool {
    TITLES_FOR_SHELL.load(Ordering::SeqCst) || TITLES_FOR_GAME_MODE.load(Ordering::SeqCst)
}

#[derive(Default)]
struct Control {
    stop: bool,
    refresh: bool,
}

struct Watcher {
    control: Arc<(Mutex<Control>, Condvar)>,
    thread: JoinHandle<()>,
}

static WATCHER: Mutex<Option<Watcher>> = Mutex::new(None);

pub fn cached_active_window() -> Option<ActiveWindow> {
    crate::runtime_state::active_window()
}

/// Re-read the focused window now; event sources call it.
pub(crate) fn wake() {
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
            log::error!("[active-window] watcher panicked: {error:?}");
        }
    }
}

pub fn start_watcher() {
    let Ok(mut slot) = WATCHER.lock() else {
        return;
    };
    if slot
        .as_ref()
        .is_some_and(|watcher| !watcher.thread.is_finished())
    {
        return;
    }
    let control = Arc::new((Mutex::new(Control::default()), Condvar::new()));
    let thread_control = control.clone();
    let thread = thread::Builder::new()
        .name("active-window-watcher".into())
        .spawn(move || {
            let events = start_events();
            let mut last: Option<ActiveWindow> = None;
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
                let current = detect_active_window();
                #[cfg(target_os = "linux")]
                remember_text_window(&current);
                if let Ok(mut actual) = ACTUAL_WINDOW.lock() {
                    *actual = current.clone();
                }
                if current != last && !current.as_ref().is_some_and(is_ignored) {
                    crate::runtime_state::set_active_window(current.clone());
                    crate::events::emit(crate::events::CoreEvent::ActiveWindowChanged(
                        current.clone(),
                    ));
                    // Fullscreen and foreground rules depend on the window.
                    crate::gamemode::wake();
                    last = current;
                }
                let interval = if events_active(events) {
                    EVENT_FALLBACK_INTERVAL
                } else {
                    POLL_INTERVAL
                };
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
            stop_events();
        });
    match thread {
        Ok(thread) => *slot = Some(Watcher { control, thread }),
        Err(error) => log::error!("[active-window] failed to spawn watcher: {error}"),
    }
}

/// Start the platform's focus-change events; `false` when there are none.
fn start_events() -> bool {
    #[cfg(target_os = "linux")]
    {
        linux::start_events()
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::start_focus_events()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

/// Whether the events started by [`start_events`] still arrive.
fn events_active(started: bool) -> bool {
    #[cfg(target_os = "linux")]
    {
        let _ = started;
        linux::events_active()
    }
    #[cfg(not(target_os = "linux"))]
    {
        started
    }
}

fn stop_events() {
    #[cfg(target_os = "linux")]
    linux::stop_events();
    #[cfg(target_os = "windows")]
    crate::platform::windows::stop_focus_events();
}

fn detect_active_window() -> Option<ActiveWindow> {
    detect_active_window_with_titles(titles_needed())
}

fn detect_active_window_with_titles(titles: bool) -> Option<ActiveWindow> {
    #[cfg(target_os = "linux")]
    let window = linux::detect(titles);
    #[cfg(target_os = "windows")]
    let window = crate::platform::windows::active_window(titles);
    #[cfg(target_os = "macos")]
    let window = crate::platform::macos::active_window();
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    let window: Option<ActiveWindow> = None;
    window.map(|mut window| {
        if !titles {
            window.title.clear();
        }
        window
    })
}

/// Detect synchronously and refresh the cache (used before showing popups).
/// An own window (see [`ignore_own_windows`]) leaves the cache as is.
pub fn detect_active_window_now() -> Option<ActiveWindow> {
    let current = detect_active_window();
    #[cfg(target_os = "linux")]
    remember_text_window(&current);
    if let Ok(mut actual) = ACTUAL_WINDOW.lock() {
        *actual = current.clone();
    }
    if current.as_ref().is_some_and(is_ignored) {
        return cached_active_window();
    }
    crate::runtime_state::set_active_window(current.clone());
    current
}

/// Applications with open windows, one entry per app id, process and title,
/// sorted by app id. Asked on demand when the user picks a condition, so
/// titles are included.
pub fn open_windows() -> Vec<OpenWindow> {
    #[cfg(target_os = "linux")]
    let mut windows = linux::open_windows();
    #[cfg(target_os = "windows")]
    let mut windows = crate::platform::windows::open_windows();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let mut windows: Vec<OpenWindow> = Vec::new();
    windows.retain(|window| {
        (!window.app_id.is_empty() || window.process_name.is_some())
            && !(IGNORE_OWN_WINDOWS.load(Ordering::SeqCst)
                && is_own_process(window.process_name.as_deref()))
    });
    windows.sort_by_key(|window| {
        (
            window.app_id.to_lowercase(),
            window
                .process_name
                .clone()
                .unwrap_or_default()
                .to_lowercase(),
            window.title.to_lowercase(),
        )
    });
    windows.dedup_by(|a, b| {
        a.app_id.eq_ignore_ascii_case(&b.app_id)
            && a.process_name.as_deref().map(str::to_lowercase)
                == b.process_name.as_deref().map(str::to_lowercase)
            && a.title == b.title
    });
    windows
}

/// A text destination, captured before a popup or an asynchronous backend starts.
#[cfg(target_os = "linux")]
#[derive(Clone)]
pub(crate) enum TextTarget {
    Kwin(String),
    Gnome(String),
    Cached(ActiveWindow),
    Unknown,
}

#[cfg(target_os = "linux")]
impl TextTarget {
    pub(crate) fn capture() -> Self {
        if kwin::active() {
            return kwin::text_target().map_or(Self::Unknown, Self::Kwin);
        }
        if matches!(
            crate::platform::linux::detect(),
            crate::platform::linux::Session {
                desktop: crate::platform::linux::Desktop::Gnome,
                session_type: crate::platform::linux::SessionType::Wayland,
                ..
            }
        ) {
            return gnome::text_target().map_or(Self::Unknown, Self::Gnome);
        }
        TEXT_WINDOW
            .lock()
            .ok()
            .and_then(|target| target.clone())
            .map_or(Self::Unknown, Self::Cached)
    }

    pub(crate) fn is_current(&self) -> bool {
        match self {
            Self::Kwin(id) => kwin::target_is_current(id),
            Self::Gnome(id) => gnome::target_is_current(id),
            Self::Cached(target) => ACTUAL_WINDOW
                .lock()
                .ok()
                .and_then(|current| current.clone())
                .is_some_and(|current| {
                    current.app_id == target.app_id
                        && current.process_name == target.process_name
                        && current.title == target.title
                }),
            Self::Unknown => false,
        }
    }
}

/// A text destination: the foreground window and its process.
#[cfg(target_os = "windows")]
#[derive(Clone)]
pub(crate) struct TextTarget(Option<(isize, u32)>);

#[cfg(target_os = "windows")]
impl TextTarget {
    pub(crate) fn capture() -> Self {
        Self(crate::platform::windows::foreground_window())
    }

    pub(crate) fn is_current(&self) -> bool {
        self.0.is_some() && crate::platform::windows::foreground_window() == self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn text_destination_is_independent_of_the_rule_cache() {
        let window = |id: &str| {
            Some(ActiveWindow {
                app_id: id.into(),
                ..ActiveWindow::default()
            })
        };
        let settings = window(crate::storage::APP_ID);
        let popup = window(crate::storage::POPUP_APP_ID);
        let mut target = window("org.kde.kate");
        update_text_window(&mut target, &settings);
        assert_eq!(target, settings);
        update_text_window(&mut target, &popup);
        assert_eq!(target, settings);
        update_text_window(&mut target, &None);
        assert!(target.is_none());
    }

    #[test]
    fn own_windows_are_ignored_only_when_asked() {
        let exe = std::env::current_exe().unwrap();
        let own = ActiveWindow {
            app_id: "slint-shell".into(),
            process_name: exe.file_name().unwrap().to_str().map(str::to_uppercase),
            ..ActiveWindow::default()
        };
        let other = ActiveWindow {
            process_name: Some("kate".into()),
            ..ActiveWindow::default()
        };
        assert!(!is_ignored(&own));
        ignore_own_windows(true);
        assert!(is_ignored(&own));
        assert!(!is_ignored(&other));
        ignore_own_windows(false);
    }
}
