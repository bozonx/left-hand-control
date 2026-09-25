use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct KeyboardDevice {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    pub path: String,
    pub name: String,
    pub is_keyboard: bool,
    pub is_mouse: bool,
}
