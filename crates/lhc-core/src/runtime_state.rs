//! Process-wide cache of watcher results (active window, game mode, layout)
//! read by the engine.

use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindow {
    pub title: String,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<String>,
    /// Whether the window covers its screen; `None` when undetectable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fullscreen: Option<bool>,
}

/// Cache owned by a runtime. Independent instances let observers and their
/// consumers be exercised without changing process-wide state.
#[derive(Default)]
pub struct RuntimeState {
    active_window: Mutex<Option<ActiveWindow>>,
    game_mode: AtomicBool,
    layout: Mutex<Option<KeyboardLayoutSelection>>,
}

static CURRENT: RuntimeState = RuntimeState {
    active_window: Mutex::new(None),
    game_mode: AtomicBool::new(false),
    layout: Mutex::new(None),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardLayoutSelection {
    pub code: String,
    /// XKB variant; empty for the base layout or when unknown.
    pub variant: String,
    /// Zero-based index among the configured layouts (the XKB group).
    pub index: u32,
}

impl RuntimeState {
    pub fn active_window(&self) -> Option<ActiveWindow> {
        self.active_window
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }

    pub fn set_active_window(&self, value: Option<ActiveWindow>) {
        if let Ok(mut current) = self.active_window.lock() {
            *current = value;
        }
    }

    /// Effective game-mode state; undetectable counts as off.
    pub fn game_mode_active(&self) -> bool {
        self.game_mode.load(Ordering::SeqCst)
    }

    pub fn set_game_mode_active(&self, active: bool) {
        self.game_mode.store(active, Ordering::SeqCst);
    }

    pub fn keyboard_layout(&self) -> Option<KeyboardLayoutSelection> {
        self.layout.lock().ok().and_then(|value| value.clone())
    }

    pub fn set_keyboard_layout(&self, value: Option<KeyboardLayoutSelection>) {
        if let Ok(mut current) = self.layout.lock() {
            *current = value;
        }
    }
}

pub fn active_window() -> Option<ActiveWindow> {
    CURRENT.active_window()
}
pub fn set_active_window(value: Option<ActiveWindow>) {
    CURRENT.set_active_window(value);
}
/// Effective game-mode state; undetectable counts as off.
pub fn game_mode_active() -> bool {
    CURRENT.game_mode_active()
}
pub fn set_game_mode_active(active: bool) {
    CURRENT.set_game_mode_active(active);
}
pub fn keyboard_layout_code() -> Option<String> {
    keyboard_layout().map(|value| value.code)
}
pub fn keyboard_layout() -> Option<KeyboardLayoutSelection> {
    CURRENT.keyboard_layout()
}
pub fn set_keyboard_layout(value: Option<KeyboardLayoutSelection>) {
    CURRENT.set_keyboard_layout(value);
}

pub(crate) fn current() -> &'static RuntimeState {
    &CURRENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_caches_do_not_share_observations() {
        let first = RuntimeState::default();
        let second = RuntimeState::default();
        first.set_active_window(Some(ActiveWindow {
            app_id: "kate".into(),
            ..Default::default()
        }));
        first.set_game_mode_active(true);
        first.set_keyboard_layout(Some(KeyboardLayoutSelection {
            code: "us".into(),
            variant: "dvorak".into(),
            index: 1,
        }));
        assert!(second.active_window().is_none());
        assert!(!second.game_mode_active());
        assert!(second.keyboard_layout().is_none());
        assert_eq!(first.active_window().unwrap().app_id, "kate");
        assert!(first.game_mode_active());
        assert_eq!(first.keyboard_layout().unwrap().variant, "dvorak");
    }
}
