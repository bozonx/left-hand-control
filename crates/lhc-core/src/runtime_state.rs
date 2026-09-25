use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindow {
    pub title: String,
    pub app_id: String,
}

static ACTIVE_WINDOW: Mutex<Option<ActiveWindow>> = Mutex::new(None);
static GAME_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
static GAME_MODE_DETECTION_ENABLED: AtomicBool = AtomicBool::new(true);
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

pub fn game_mode_active() -> bool {
    GAME_MODE_ACTIVE.load(Ordering::SeqCst)
}

pub fn game_mode_detection_enabled() -> bool {
    GAME_MODE_DETECTION_ENABLED.load(Ordering::SeqCst)
}

pub fn set_game_mode(active: bool, detection_enabled: bool) {
    GAME_MODE_ACTIVE.store(active, Ordering::SeqCst);
    GAME_MODE_DETECTION_ENABLED.store(detection_enabled, Ordering::SeqCst);
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
