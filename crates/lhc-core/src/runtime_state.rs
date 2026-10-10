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

static ACTIVE_WINDOW: Mutex<Option<ActiveWindow>> = Mutex::new(None);
static GAME_MODE: AtomicBool = AtomicBool::new(false);
static LAYOUT: Mutex<Option<LayoutSelection>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSelection {
    pub short: String,
    pub variant: String,
}

pub fn active_window() -> Option<ActiveWindow> {
    ACTIVE_WINDOW.lock().ok().and_then(|value| value.clone())
}

pub fn set_active_window(value: Option<ActiveWindow>) {
    if let Ok(mut current) = ACTIVE_WINDOW.lock() {
        *current = value;
    }
}

/// Effective game-mode state; undetectable counts as off.
pub fn game_mode_active() -> bool {
    GAME_MODE.load(Ordering::SeqCst)
}

pub fn set_game_mode_active(active: bool) {
    GAME_MODE.store(active, Ordering::SeqCst);
}

pub fn layout_short() -> Option<String> {
    layout().map(|value| value.short)
}

pub fn layout() -> Option<LayoutSelection> {
    LAYOUT.lock().ok().and_then(|value| value.clone())
}

pub fn set_layout(value: Option<LayoutSelection>) {
    if let Ok(mut current) = LAYOUT.lock() {
        *current = value;
    }
}
