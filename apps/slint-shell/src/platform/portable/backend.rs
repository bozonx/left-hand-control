//! Popup backend on Windows and macOS: always winit windows.

/// Placeholder for the Linux Spell worker, which does not exist here.
pub struct Worker;

impl Worker {
    pub fn is_alive(&mut self) -> bool {
        false
    }

    pub fn send(
        &self,
        _: &crate::command::Command,
        _: crate::command::Source,
        _: std::time::Instant,
        _: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("Spell worker is unavailable on this platform".into())
    }
}

pub fn spell_requested() -> Result<bool, Box<dyn std::error::Error>> {
    match super::super::PopupMode::from_env()? {
        super::super::PopupMode::Spell => {
            Err("Spell popups are available only on Linux Wayland".into())
        }
        super::super::PopupMode::Auto | super::super::PopupMode::Winit => Ok(false),
    }
}

/// The layer hint needs layer-shell, which exists only on Linux.
pub fn layer_shell_supported() -> bool {
    false
}

pub fn spawn() -> Result<Worker, Box<dyn std::error::Error>> {
    Err("Spell worker is unavailable on this platform".into())
}
