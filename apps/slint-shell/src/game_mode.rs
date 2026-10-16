//! Game-mode state in the UI and the Auto/On/Off control shared by the
//! top bar, the tray, the CLI and key bindings.

use crate::{
    command::GameMode,
    document::{Document, View},
    i18n::Msg,
    ui::{AppState, GameModeControl, SettingsWindow},
};
use lhc_core::gamemode::{self, DetectorAvailability, GameModeMethod, GameModeStatus, GameSource};
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
    let control = match mode {
        GameMode::Auto => gamemode::GameModeControl::Auto,
        GameMode::On => gamemode::GameModeControl::On,
        GameMode::Off => gamemode::GameModeControl::Off,
        GameMode::Toggle if gamemode::status().active => gamemode::GameModeControl::Off,
        GameMode::Toggle => gamemode::GameModeControl::On,
    };
    if let Some(document) = document {
        let persist = document.read().settings().game_mode.control != control;
        if persist {
            document
                .edit(View::Shell, |config| {
                    config.update_settings(|settings| settings.game_mode.control = control)
                })
                .map_err(|error| Msg::from(&error))?;
        }
    }
    let status = gamemode::set_control(control);
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
    let source =
        |source: GameSource| availability_message(source.availability(&status.detectors)).to_ui();
    state.set_game_steam_availability(source(GameSource::Steam));
    state.set_game_lutris_availability(source(GameSource::Lutris));
    state.set_game_heroic_availability(source(GameSource::Heroic));
    state.set_game_umu_availability(source(GameSource::Umu));
    state.set_game_gamescope_availability(source(GameSource::Gamescope));
    state.set_game_category_availability(source(GameSource::AppCategory));
    state.set_game_bar_availability(source(GameSource::GameBar));
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
        Some(GameModeMethod::KnownGame(name)) => Msg::GameModeKnownGameActive(name.clone()),
        Some(GameModeMethod::Fullscreen) => Msg::GameModeFullscreenActive,
        Some(GameModeMethod::Process(name)) => Msg::GameModeProcessActive(name.clone()),
        Some(GameModeMethod::Manual) | None if !status.automatic_available => {
            Msg::GameModeAutoUnavailable
        }
        Some(GameModeMethod::Manual) | None => match &status.skipped_window {
            Some(name) => Msg::GameModeWindowSkipped(name.clone()),
            None => Msg::GameModeAutoInactive,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_control_is_atomic() {
        // ConfigDocument/game mode are process globals; isolate the regression
        // from other document tests rather than depending on test ordering.
        const CHILD: &str = "LHC_GAME_MODE_ATOMIC_TEST";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "game_mode::tests::persisted_control_is_atomic",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let paths =
            lhc_core::StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
        let document = Document::load(paths.clone()).unwrap();
        apply(GameMode::On, Some(&document)).unwrap();
        assert_eq!(gamemode::status().control, gamemode::GameModeControl::On);
        let mut raw: serde_json::Value =
            serde_json::from_str(&paths.load_config().unwrap()).unwrap();
        assert_eq!(raw["settings"]["gameMode"]["control"], "on");
        raw["settings"]["gameMode"]["control"] = "off".into();
        paths.save_config(&raw.to_string()).unwrap();
        // A conflicting save reloads the external choice, never applies Auto.
        assert!(apply(GameMode::Auto, Some(&document)).is_err());
        assert_eq!(gamemode::status().control, gamemode::GameModeControl::Off);
        assert_eq!(
            document.read().settings().game_mode.control,
            gamemode::GameModeControl::Off
        );
        // Also cover an I/O failure, independent of conflict handling.
        std::fs::remove_file(paths.config_path()).unwrap();
        std::fs::create_dir(paths.config_path()).unwrap();
        assert!(apply(GameMode::On, Some(&document)).is_err());
        assert_eq!(gamemode::status().control, gamemode::GameModeControl::Off);
    }
}
