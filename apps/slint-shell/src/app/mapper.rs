//! Mapper lifecycle, core events and keeping the loaded configuration in
//! sync with changes other processes make.

use super::{App, post};
use crate::{command::Source, document::View, i18n::Msg, pages, ui::AppState};
use lhc_core::{
    CoreEvent,
    profile::{
        auto_switch::{self, AutoSwitchContext},
        model::LayoutMode,
    },
};
use slint::ComponentHandle;
use std::time::{Duration, Instant};

/// Attempts after the first one to start the mapper on launch. At login the
/// keyboard or `/dev/input` permissions may not be ready yet, and a quickly
/// restarted app may wait for the old process to release the devices.
const LAUNCH_RETRIES: u32 = 15;
const LAUNCH_RETRY_DELAY: Duration = Duration::from_secs(2);
/// How often the shown uptime is updated.
const UPTIME_TICK: Duration = Duration::from_secs(20);

/// Forward core events to the UI thread. Subscribed once at startup.
pub(super) fn forward_core_events() {
    lhc_core::events::bus().subscribe(|event| match event {
        CoreEvent::MapperStopped(error) => {
            let error = error.clone();
            post(move |app| {
                app.set_error(Msg::MapperStopped(Some(error)));
                app.refresh_mapper_status();
            });
        }
        CoreEvent::CommandFinished { script, result } => {
            let label = script
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect::<String>();
            let result = result.clone();
            post(move |app| match result {
                Ok(()) => {
                    app.clear_action_error();
                    crate::notifications::send(&app.settings, Msg::CommandStarted(label));
                }
                Err(error) => app.set_error(Msg::ActionFailed(error)),
            });
        }
        CoreEvent::AppAction(name) => {
            if let Some(slot) = name
                .strip_prefix("activate_layout_")
                .and_then(|slot| slot.parse::<u32>().ok())
            {
                post(move |app| app.activate_layout(slot));
                return;
            }
            let Some(command) = crate::command::Command::from_app_action(name) else {
                return;
            };
            post(move |app| app.command(command, Source::Mapper, Instant::now(), None));
        }
        CoreEvent::ActiveLayerChanged(_) | CoreEvent::LayerKeyPressed => {
            post(|app| app.sync_held_layer());
        }
        CoreEvent::GameModeChanged(status) => {
            let status = status.clone();
            post(move |app| app.game_mode_changed(&status));
        }
        CoreEvent::TextInputFailed(failure) => {
            let failure = failure.clone();
            post(move |app| app.set_error(Msg::TextInputFailed(failure)));
        }
        CoreEvent::MapperActionFailed(error) => {
            let error = error.clone();
            post(move |app| app.set_error(Msg::ActionFailed(error)));
        }
        CoreEvent::ActionUnavailable(action) => {
            log::info!("[mapper] skipped unavailable action {action:?}");
            post(|app| crate::notifications::send(&app.settings, Msg::ActionUnavailable));
        }
        CoreEvent::LayoutChanged(_) | CoreEvent::ActiveWindowChanged(_) => {
            post(|app| app.context_changed())
        }
    });
}

impl App {
    /// Show the game-mode status; the active layout only depends on
    /// whether game mode is on, not on which detector reported it.
    pub(super) fn game_mode_changed(&self, status: &lhc_core::gamemode::GameModeStatus) {
        // Events from different watcher threads can reach the UI out of order.
        let current = lhc_core::gamemode::status();
        let status = if status.revision < current.revision {
            &current
        } else {
            status
        };
        crate::game_mode::refresh(&self.settings, status);
        crate::platform::hotkey::set_game_active(status.active);
        self.supervisor
            .borrow_mut()
            .cancel_blocked_show(status.active);
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_game(status);
        }
        if self.game_active.replace(Some(status.active)) != Some(status.active) {
            self.context_changed();
        }
    }

    /// `activateLayoutN`: make layout `slot` (1-based, library order) the
    /// single active one, leaving automatic mode.
    pub(super) fn activate_layout(&self, slot: u32) {
        let Some(document) = self.document() else {
            self.set_error(Msg::LoadConfigFirst);
            return;
        };
        let id = document.read().ordered_layout_ids().map(|ids| {
            usize::try_from(slot)
                .ok()
                .and_then(|slot| ids.get(slot.checked_sub(1)?).cloned())
        });
        let id = match id {
            Ok(Some(id)) => id,
            Ok(None) => {
                crate::notifications::send(&self.settings, Msg::LayoutSlotEmpty(slot));
                return;
            }
            Err(error) => {
                self.set_error(Msg::from(&error));
                return;
            }
        };
        let result = document.edit(View::Shell, |config| {
            config.update_settings(|settings| {
                auto_switch::set_mode(settings, LayoutMode::Manual);
                settings.manual_active_layout_id = Some(id);
            })
        });
        match result {
            Ok(saved) => {
                if let Err(error) = saved.runtime {
                    self.set_error(Msg::SavedMapperNotUpdated(error));
                }
            }
            Err(error) => self.set_error(Msg::from(&error)),
        }
        self.document_changed();
    }

    /// Show the layout OSD when the active layout differs from the one
    /// last seen. The first observation and a stopped mapper stay silent.
    pub(super) fn announce_layout(&self) {
        let Some(document) = self.document() else {
            return;
        };
        let (id, enabled) = {
            let config = document.read();
            let Ok(id) = config.active_layout_id(&AutoSwitchContext::current()) else {
                return;
            };
            (id, config.settings().layout_osd)
        };
        let previous = self.announced_layout.replace(Some(id.clone()));
        if !enabled
            || previous.is_none_or(|previous| previous == id)
            || !lhc_core::mapper::runtime::status().running
        {
            return;
        }
        let name = id
            .as_deref()
            .and_then(lhc_core::profile::model::user_layout_name);
        let text = crate::i18n::layout_osd(self.preferences.get().language, name);
        std::thread::spawn(move || {
            if let Err(error) = lhc_core::platform::osd::show(&text) {
                log::warn!("[osd] {error}");
            }
        });
    }

    /// The system context (layout, game mode, window) changed: the active
    /// layout may differ now.
    pub(super) fn context_changed(&self) {
        pages::conditions::show_active_window(
            &self.settings,
            lhc_core::active_window::cached_active_window().as_ref(),
        );
        let state = self.settings.global::<AppState>();
        crate::game_mode::refresh(&self.settings, &lhc_core::gamemode::status());
        state.set_keyboard_language(
            lhc_core::runtime_state::layout()
                .map(|layout| {
                    let code = layout.short.to_uppercase();
                    if code.chars().count() > 8 {
                        let mut label: String = code.chars().take(7).collect();
                        label.push('…');
                        label
                    } else {
                        code
                    }
                })
                .unwrap_or_default()
                .into(),
        );
        let Some(document) = self.document() else {
            return;
        };
        match document.sync_runtime(false) {
            Ok(()) => self.clear_runtime_error(),
            Err(error) => self.set_error(Msg::SavedMapperNotUpdated(error)),
        }
        self.refresh_mapper_status();
        pages::refresh_active(&self.settings, &document);
        self.invalidate_menus();
        self.announce_layout();
    }

    fn clear_runtime_error(&self) {
        if self.settings.global::<AppState>().get_backend_error().id == "saved-mapper-not-updated" {
            self.set_error(Msg::None);
        }
    }

    pub(super) fn refresh_mapper_status(&self) {
        let status = lhc_core::mapper::runtime::status();
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_enabled(status.running);
        }
        self.settings
            .global::<AppState>()
            .set_mapper_running(status.running);
        self.track_uptime(status.running, status.last_error.is_some());
        let current = (status.running, status.last_error.clone());
        if self.last_mapper_status.borrow().as_ref() == Some(&current) {
            return;
        }
        *self.last_mapper_status.borrow_mut() = Some(current);
        if !status.running
            && let Some(error) = &status.last_error
        {
            self.set_mapper_error(error.clone());
        }
        let message = if status.running {
            Msg::MapperRunning(status.last_error)
        } else {
            Msg::MapperStopped(status.last_error)
        };
        self.settings
            .global::<AppState>()
            .set_status(crate::notifications::report(&self.settings, &message));
    }

    /// Remember when the mapper started and, when it stopped by itself, how
    /// long it ran, so the settings can show it.
    fn track_uptime(&self, running: bool, failed: bool) {
        let now = Instant::now();
        if running {
            if self.mapper_since.get().is_none() {
                self.mapper_since.set(Some(now));
                self.mapper_crash.set(None);
            }
        } else if let Some(since) = self.mapper_since.take() {
            self.mapper_crash.set(failed.then(|| (now - since, now)));
        }
        let shown = self.mapper_since.get().is_some() || self.mapper_crash.get().is_some();
        if !shown {
            self.uptime_clock.stop();
        } else if !self.uptime_clock.running() {
            self.uptime_clock
                .start(slint::TimerMode::Repeated, UPTIME_TICK, || {
                    super::with_app(|app| app.show_uptime())
                });
        }
        self.show_uptime();
    }

    fn show_uptime(&self) {
        let now = Instant::now();
        let minutes = |time: Duration| i32::try_from(time.as_secs() / 60).unwrap_or(i32::MAX);
        let (ran, ago) = self
            .mapper_crash
            .get()
            .map_or((-1, -1), |(ran, at)| (minutes(ran), minutes(now - at)));
        let state = self.settings.global::<AppState>();
        state.set_mapper_uptime(
            self.mapper_since
                .get()
                .map_or(-1, |since| minutes(now - since)),
        );
        state.set_mapper_ran(ran);
        state.set_mapper_stopped_ago(ago);
    }

    fn set_mapper_busy(&self, busy: bool) {
        self.settings.global::<AppState>().set_mapper_busy(busy);
    }

    pub(super) fn toggle_mapper(&self) {
        if self.settings.global::<AppState>().get_mapper_busy() {
            return;
        }
        if lhc_core::mapper::runtime::status().running {
            self.stop_mapper(false);
        } else {
            self.start_mapper(0);
        }
    }

    /// Start the mapper as the settings ask on launch, retrying for a while.
    pub(super) fn start_mapper_on_launch(&self) {
        self.start_mapper(LAUNCH_RETRIES);
    }

    /// Restart a running mapper so it picks up the saved devices.
    pub(super) fn restart_mapper(&self) {
        if !self.settings.global::<AppState>().get_mapper_busy()
            && lhc_core::mapper::runtime::status().running
        {
            self.stop_mapper(true);
        }
    }

    fn stop_mapper(&self, restart: bool) {
        self.set_mapper_busy(true);
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::stop();
            post(move |app| {
                app.set_mapper_busy(false);
                match result {
                    Ok(()) => {
                        app.set_error(Msg::None);
                        app.set_mapper_error(String::new());
                        if restart {
                            app.start_mapper(0);
                            return;
                        }
                        crate::notifications::send(&app.settings, Msg::MapperStopped(None));
                    }
                    Err(error) => app.set_error(Msg::Error(error)),
                }
                app.refresh_mapper_status();
            });
        });
    }

    fn set_mapper_error(&self, error: String) {
        self.settings.global::<AppState>().set_mapper_error(
            if error.is_empty() {
                Msg::None
            } else {
                Msg::MapperFailed(error)
            }
            .to_ui(),
        );
    }

    /// Start the mapper; a failed start is tried again `retries` times.
    fn start_mapper(&self, retries: u32) {
        if !lhc_core::mapper::runtime::SUPPORTED {
            self.set_error(Msg::MapperUnsupported);
            return;
        }
        let state = self.settings.global::<AppState>();
        let Some(document) = self.document() else {
            self.set_error(Msg::LoadConfigFirst);
            return;
        };
        let (device, mouse) = {
            let config = document.read();
            (
                config.input_device().map(str::to_owned),
                config.mouse_device().map(str::to_owned),
            )
        };
        let Some(device) = device else {
            self.set_error(Msg::SelectInputDevice);
            return;
        };
        let runtime = match document.runtime_config() {
            Ok(runtime) => runtime,
            Err(error) => {
                self.set_error(Msg::from(&error));
                return;
            }
        };
        lhc_core::active_window::set_titles_needed(runtime.uses_titles);
        self.mapper_crash.set(None);
        self.set_mapper_busy(true);
        state.set_status(Msg::MapperStarting.to_ui());
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::start(&device, mouse.as_deref(), &runtime.json);
            post(move |app| {
                app.set_mapper_busy(false);
                match result {
                    Ok(()) => {
                        app.set_error(Msg::None);
                        app.set_mapper_error(String::new());
                        crate::notifications::send(&app.settings, Msg::MapperRunning(None));
                        if let Some(document) = app.document() {
                            document.mapper_started(runtime.layout_id);
                            // The context may have changed while starting.
                            if let Err(error) = document.sync_runtime(false) {
                                app.set_error(Msg::SavedMapperNotUpdated(error));
                            }
                        }
                    }
                    Err(error) if retries > 0 => {
                        log::info!("mapper start failed, retrying: {error}");
                        // Stay busy until the next attempt.
                        app.set_mapper_busy(true);
                        slint::Timer::single_shot(LAUNCH_RETRY_DELAY, move || {
                            super::with_app(|app| {
                                app.set_mapper_busy(false);
                                app.start_mapper(retries - 1);
                            })
                        });
                        return;
                    }
                    Err(error) => {
                        app.set_mapper_error(error.clone());
                        app.set_error(Msg::Error(error));
                        pages::check_input_access(&app.settings);
                    }
                }
                app.refresh_mapper_status();
            });
        });
    }
}
