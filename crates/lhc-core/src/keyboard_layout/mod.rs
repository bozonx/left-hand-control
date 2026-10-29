//! Current keyboard layout detection.
//!
//! ```text
//! Current product support is Linux/KDE. The facade keeps the other backends
//! visible as explicit stubs/skeletons so unsupported platforms and desktop
//! environments fail predictably instead of silently diverging.
//!
//! Backends:
//!   * Linux + KDE Plasma     — DBus `org.kde.keyboard`            (implemented)
//!   * Linux + GNOME          — optional Shell extension over D-Bus
//!   * Linux + Sway / wlroots — `swaymsg -t get_inputs`             (skeleton)
//!   * Linux + generic X11    — `setxkbmap -query`                  (skeleton,
//!                                                                   also used
//!                                                                   as fallback
//!                                                                   for unknown
//!                                                                   DEs)
//!   * Windows                — GetKeyboardLayout (foreground)      (implemented)
//!   * macOS                  — TISCopyCurrentKeyboardInputSource   (stub)
//!
//! Shell contract:
//!   * `current()` / `available_layouts()` / `set()` for one-shot calls
//!   * `CoreEvent::LayoutChanged(KeyboardLayoutInfo)` from the watcher
//! ```

use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
mod linux_gnome;
#[cfg(target_os = "linux")]
mod linux_kde;
#[cfg(target_os = "linux")]
mod linux_sway;
#[cfg(target_os = "linux")]
mod linux_x11;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod signal_watcher;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KeyboardLayoutInfo {
    /// Short code, e.g. "us", "ru".
    #[serde(rename = "short")]
    pub code: String,
    /// Optional user-visible label, e.g. "lat"; not an XKB variant.
    #[serde(rename = "display")]
    pub display_label: String,
    /// Long human-readable name, e.g. "English (US)".
    #[serde(rename = "long")]
    pub name: String,
    /// XKB variant, e.g. "dvorak"; empty for the base layout or when unknown.
    pub variant: String,
    /// Zero-based index among the configured layouts.
    pub index: u32,
    /// Which backend produced this info.
    pub backend: &'static str,
}

static WATCHER_STOP: AtomicBool = AtomicBool::new(false);
static WATCHERS: Mutex<Vec<std::thread::JoinHandle<()>>> = Mutex::new(Vec::new());
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
static LAST_PUBLISHED: Mutex<LayoutPublisher> = Mutex::new(LayoutPublisher { last: None });

pub fn stop_watcher() {
    WATCHER_STOP.store(true, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    {
        linux_kde::interrupt_signal_watcher();
        linux_gnome::interrupt_signal_watcher();
    }
    if let Ok(mut watchers) = WATCHERS.lock() {
        for watcher in watchers.drain(..) {
            if let Err(error) = watcher.join() {
                log::error!("[layout] watcher panicked: {error:?}");
            }
        }
    }
    LAST_PUBLISHED
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .last = None;
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn register_watcher(watcher: std::io::Result<std::thread::JoinHandle<()>>) {
    match watcher {
        Ok(watcher) => {
            if let Ok(mut watchers) = WATCHERS.lock() {
                watchers.push(watcher);
            }
        }
        Err(error) => log::error!("[layout] failed to spawn watcher: {error}"),
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn watcher_stop_requested() -> bool {
    WATCHER_STOP.load(Ordering::SeqCst)
}

#[derive(Default)]
struct LayoutPublisher {
    last: Option<KeyboardLayoutInfo>,
}

impl LayoutPublisher {
    fn publish(
        &mut self,
        info: &KeyboardLayoutInfo,
        cache: &crate::runtime_state::RuntimeState,
        emit: impl FnOnce(crate::events::CoreEvent),
    ) {
        cache.set_keyboard_layout(Some(crate::runtime_state::KeyboardLayoutSelection {
            code: info.code.clone(),
            variant: info.variant.clone(),
            index: info.index,
        }));
        if self.last.as_ref() != Some(info) {
            self.last = Some(info.clone());
            emit(crate::events::CoreEvent::LayoutChanged(info.clone()));
        }
    }
}

/// Cache each observation and publish changes, including variants and group
/// indices: two configured groups may use the same code and display label.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn publish(info: &KeyboardLayoutInfo) {
    LAST_PUBLISHED
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .publish(info, crate::runtime_state::current(), crate::events::emit);
}

pub fn current() -> Result<Option<KeyboardLayoutInfo>, String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{Desktop, detect};
        match detect().desktop {
            Desktop::Kde => linux_kde::current(),
            Desktop::Gnome => linux_gnome::current(),
            Desktop::Sway => linux_sway::current(),
            // Everything else — Hyprland, Xfce, MATE, Unknown, … — falls
            // through to the generic X11 backend. It is still a skeleton
            // but is the correct catch-all once implemented.
            _ => linux_x11::current(),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::current()
    }
    #[cfg(target_os = "macos")]
    {
        macos::current()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Ok(None)
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn refresh_cache() -> Result<Option<KeyboardLayoutInfo>, String> {
    use crate::platform::linux::{Desktop, detect};
    match detect().desktop {
        Desktop::Kde => linux_kde::refresh_cache(),
        _ => current(),
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn refresh_cache() -> Result<Option<KeyboardLayoutInfo>, String> {
    windows::refresh_cache()
}

pub fn available_layouts() -> Result<Vec<KeyboardLayoutInfo>, String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{Desktop, detect};
        match detect().desktop {
            Desktop::Kde => linux_kde::available_layouts(),
            Desktop::Gnome => linux_gnome::available_layouts(),
            _ => Ok(vec![]),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::available_layouts()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Ok(vec![])
    }
}

/// Switch the active OS keyboard layout to the given zero-based index.
/// Currently implemented for KDE Plasma, GNOME (with its extension) and Windows; other desktops/OSes
/// return an explicit error so the shell can disable the control.
pub fn set(index: u32) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{Desktop, detect};
        match detect().desktop {
            Desktop::Kde => linux_kde::set_layout(index),
            Desktop::Gnome => linux_gnome::set_layout(index),
            d => Err(format!(
                "Switching layout is not implemented for desktop '{}'",
                d.label()
            )),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::set_layout(index)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = index;
        Err("Switching layout is not implemented on this OS".to_string())
    }
}

/// Start a background watcher that emits `CoreEvent::LayoutChanged`.
/// Safe to call once at app startup. No-op if no backend is available.
pub fn start_watcher() {
    WATCHER_STOP.store(false, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{Desktop, detect};
        let s = detect();
        log::debug!(
            "[layout] linux session: desktop={} session_type={} xdg={:?}",
            s.desktop.label(),
            s.session_type.label(),
            s.xdg_current_desktop,
        );
        match s.desktop {
            Desktop::Kde => linux_kde::start_watcher(),
            Desktop::Gnome => linux_gnome::start_watcher(),
            Desktop::Sway => linux_sway::start_watcher(),
            _ => linux_x11::start_watcher(),
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows::start_watcher();
    }
    #[cfg(target_os = "macos")]
    {
        macos::start_watcher();
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn xkb_options() -> String {
    if matches!(
        crate::platform::linux::detect().desktop,
        crate::platform::linux::Desktop::Kde
    ) {
        linux_kde::xkb_options()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(code: &str, display_label: &str) -> KeyboardLayoutInfo {
        KeyboardLayoutInfo {
            code: code.into(),
            display_label: display_label.into(),
            name: String::new(),
            variant: String::new(),
            index: 0,
            backend: "test",
        }
    }

    #[test]
    fn publishes_variant_index_and_label_changes_but_not_repeated_observations() {
        let cache = crate::runtime_state::RuntimeState::default();
        let mut publisher = LayoutPublisher::default();
        let mut events = Vec::new();
        let mut observe =
            |info: &KeyboardLayoutInfo| publisher.publish(info, &cache, |event| events.push(event));
        let mut layout = info("us", "");
        observe(&layout);
        observe(&layout);
        layout.variant = "dvorak".into();
        observe(&layout);
        observe(&layout);
        layout.index = 1;
        observe(&layout);
        layout.display_label = "custom".into();
        observe(&layout);
        assert_eq!(events.len(), 4);
        assert_eq!(cache.keyboard_layout().unwrap().variant, "dvorak");
        assert_eq!(cache.keyboard_layout().unwrap().index, 1);
        // A new watcher session must announce its initial observation.
        publisher.last = None;
        publisher.publish(&layout, &cache, |event| events.push(event));
        assert_eq!(events.len(), 5);
    }
}
