//! macOS game-mode sources: running applications from `NSWorkspace` and
//! the frontmost application's category from its `Info.plist`. Fullscreen
//! comes with the active window (`platform::macos`).

use super::{DetectorAvailability, GameModeDetectors, GameModeSettings, Observation};
use crate::runtime_state::ActiveWindow;

pub(super) fn observe(settings: &GameModeSettings, window: Option<&ActiveWindow>) -> Observation {
    let need_running = super::needs_running_processes(settings);
    let running = need_running.then(running_process_names).flatten();
    let window_game = settings
        .use_app_category
        .then(|| window.and_then(crate::platform::macos::game_window))
        .flatten();
    Observation {
        detectors: GameModeDetectors {
            gamemoded: DetectorAvailability::Unsupported,
            // NSWorkspace always answers; no window is simply "not fullscreen".
            fullscreen: DetectorAvailability::Available,
            processes: if need_running && running.is_none() {
                DetectorAvailability::Unavailable
            } else {
                DetectorAvailability::Available
            },
            active_window: crate::active_window::availability(),
            missing_window_tool: None,
            ..GameModeDetectors::default()
        },
        running: running.unwrap_or_default(),
        known_games: Vec::new(),
        window_game,
        daemon: false,
    }
}

pub(super) fn running_process_names() -> Option<Vec<String>> {
    crate::platform::macos::running_process_names()
}
