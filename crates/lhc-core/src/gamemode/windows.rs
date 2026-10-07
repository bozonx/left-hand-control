use crate::mapper_config::GameModeSettings;
use crate::platform::windows;

use super::{Detection, DetectorAvailability, GameModeDetectors};

pub(super) fn detect(settings: &GameModeSettings) -> Detection {
    let need_running = settings
        .process_matchers
        .iter()
        .any(|matcher| !matcher.only_active_window && !matcher.name.trim().is_empty());
    let running = need_running.then(windows::running_process_names).flatten();
    let fullscreen = settings
        .use_fullscreen
        .then(windows::fullscreen_active)
        .flatten();
    let detectors = GameModeDetectors {
        gamemoded: DetectorAvailability::Unsupported,
        fullscreen: if settings.use_fullscreen && fullscreen.is_none() {
            DetectorAvailability::Unavailable
        } else {
            DetectorAvailability::Available
        },
        processes: if need_running && running.is_none() {
            DetectorAvailability::Unavailable
        } else {
            DetectorAvailability::Available
        },
        active_window: crate::active_window::availability(),
    };
    super::evaluate(
        settings,
        detectors,
        running.as_deref().unwrap_or_default(),
        crate::active_window::cached_active_window().as_ref(),
        false,
        fullscreen.unwrap_or(false),
    )
}
