use crate::{
    i18n::Msg,
    ui::{AppState, GameModeControl, SettingsWindow},
};
use lhc_core::gamemode::{self, DetectorAvailability, GameModeStatus};
use slint::ComponentHandle;

pub fn bind(ui: &SettingsWindow) {
    refresh(ui, &gamemode::status());
    let weak = ui.as_weak();
    ui.global::<AppState>().on_set_game_control(move |control| {
        let control = match control {
            GameModeControl::Auto => gamemode::GameModeControl::Auto,
            GameModeControl::On => gamemode::GameModeControl::On,
            GameModeControl::Off => gamemode::GameModeControl::Off,
        };
        let status = gamemode::set_control(control);
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &status);
        }
    });
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
    match status.automatic_method.as_deref() {
        Some("gamemoded") => Msg::GameModeDaemonActive,
        Some("fullscreen") => Msg::GameModeFullscreenActive,
        Some(method) if method.starts_with("process:") => {
            Msg::GameModeProcessActive(method[8..].into())
        }
        _ if !status.automatic_available => Msg::GameModeAutoUnavailable,
        _ => Msg::GameModeAutoInactive,
    }
}
