//! Game-mode state in the UI and the Auto/On/Off control shared by the
//! top bar, the tray, the CLI and key bindings.

use crate::{
    command::GameMode,
    document::{Document, View},
    i18n::Msg,
    ui::{AppState, GameModeControl, SettingsWindow},
};
use lhc_core::gamemode::{self, DetectorAvailability, GameModeMethod, GameModeStatus};
use slint::ComponentHandle;

/// Show the current state and apply the top-bar choice. The settings
/// process replaces the callback to also remember the choice and update
/// the tray.
pub fn bind(ui: &SettingsWindow) {
    refresh(ui, &gamemode::status());
    let weak = ui.as_weak();
    ui.global::<AppState>().on_set_game_control(move |control| {
        if let (Ok(status), Some(ui)) = (apply(from_ui(control), None), weak.upgrade()) {
            refresh(&ui, &status);
        }
    });
}

pub(crate) fn from_ui(control: GameModeControl) -> GameMode {
    match control {
        GameModeControl::Auto => GameMode::Auto,
        GameModeControl::On => GameMode::On,
        GameModeControl::Off => GameMode::Off,
    }
}

/// Apply `mode` and, when the user asked for it, remember the choice.
pub(crate) fn apply(mode: GameMode, document: Option<&Document>) -> Result<GameModeStatus, Msg> {
    let status = match mode {
        GameMode::Auto => gamemode::set_control(gamemode::GameModeControl::Auto),
        GameMode::On => gamemode::set_control(gamemode::GameModeControl::On),
        GameMode::Off => gamemode::set_control(gamemode::GameModeControl::Off),
        GameMode::Toggle => gamemode::toggle(),
    };
    let Some(document) = document else {
        return Ok(status);
    };
    let persist = {
        let config = document.read();
        let settings = &config.settings().game_mode;
        settings.remember_control && settings.control != status.control
    };
    if persist {
        document
            .edit(View::Shell, |config| {
                config.update_settings(|settings| settings.game_mode.control = status.control)
            })
            .map_err(|error| Msg::from(&error))?;
    }
    Ok(status)
}

pub(crate) fn refresh(ui: &SettingsWindow, status: &GameModeStatus) {
    let state = ui.global::<AppState>();
    state.set_game_active(status.active);
    state.set_game_state_available(status.state_available);
    state.set_game_control(match status.control {
        gamemode::GameModeControl::Auto => GameModeControl::Auto,
        gamemode::GameModeControl::On => GameModeControl::On,
        gamemode::GameModeControl::Off => GameModeControl::Off,
    });
    let automatic = automatic_message(status);
    state.set_game_auto_status(automatic.to_ui());
    state.set_game_status(
        match status.control {
            gamemode::GameModeControl::Auto => automatic,
            gamemode::GameModeControl::On => Msg::GameModeManualOn,
            gamemode::GameModeControl::Off => Msg::GameModeManualOff,
        }
        .to_ui(),
    );
    state.set_game_daemon_availability(availability_message(status.detectors.gamemoded).to_ui());
    state.set_game_fullscreen_availability(
        availability_message(status.detectors.fullscreen).to_ui(),
    );
    state.set_game_processes_availability(availability_message(status.detectors.processes).to_ui());
    state
        .set_game_window_availability(availability_message(status.detectors.active_window).to_ui());
    state.set_game_missing_tool(
        status
            .detectors
            .missing_window_tool
            .clone()
            .unwrap_or_default()
            .into(),
    );
}

fn availability_message(availability: DetectorAvailability) -> Msg {
    match availability {
        DetectorAvailability::Available => Msg::CapabilityAvailable,
        DetectorAvailability::Unavailable => Msg::CapabilityUnavailable,
        DetectorAvailability::Unsupported => Msg::CapabilityUnsupported,
    }
}

fn automatic_message(status: &GameModeStatus) -> Msg {
    if !status.detection_enabled {
        return Msg::GameModeAutoDisabled;
    }
    if let Some(name) = &status.excluded_by {
        return Msg::GameModeExcluded(name.clone());
    }
    match &status.automatic_method {
        Some(GameModeMethod::Daemon) => Msg::GameModeDaemonActive,
        Some(GameModeMethod::Fullscreen) => Msg::GameModeFullscreenActive,
        Some(GameModeMethod::Process(name)) => Msg::GameModeProcessActive(name.clone()),
        Some(GameModeMethod::Manual) | None if !status.automatic_available => {
            Msg::GameModeAutoUnavailable
        }
        Some(GameModeMethod::Manual) | None => Msg::GameModeAutoInactive,
    }
}
