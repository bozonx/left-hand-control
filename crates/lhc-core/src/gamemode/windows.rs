//! Windows game-mode sources: running processes from a ToolHelp snapshot.
//! Fullscreen comes with the active window (`platform::windows`), which
//! also asks the shell whether a Direct3D game runs fullscreen.

use super::{DetectorAvailability, GameModeDetectors, GameModeSettings, Observation};

pub(super) fn observe(settings: &GameModeSettings) -> Observation {
    let need_running = super::needs_running_processes(settings);
    let running = need_running
        .then(crate::platform::windows::running_process_names)
        .flatten();
    Observation {
        detectors: GameModeDetectors {
            gamemoded: DetectorAvailability::Unsupported,
            fullscreen: DetectorAvailability::Available,
            processes: if need_running && running.is_none() {
                DetectorAvailability::Unavailable
            } else {
                DetectorAvailability::Available
            },
            active_window: crate::active_window::availability(),
            missing_window_tool: None,
        },
        running: running.unwrap_or_default(),
        daemon: false,
    }
}
