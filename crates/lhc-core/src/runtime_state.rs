use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindow {
    pub title: String,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<String>,
}

static ACTIVE_WINDOW: Mutex<Option<ActiveWindow>> = Mutex::new(None);
static GAME_MODE: AtomicU8 = AtomicU8::new(0);
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
    game_mode().0
}

pub fn game_mode() -> (bool, bool) {
    let state = GAME_MODE.load(Ordering::SeqCst);
    (state & 1 != 0, state & 2 != 0)
}

pub fn set_game_mode(active: bool, state_available: bool) {
    GAME_MODE.store(u8::from(active) | (u8::from(state_available) << 1), Ordering::SeqCst);
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
