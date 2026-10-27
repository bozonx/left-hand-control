//! Mapper lifecycle, core events and keeping the loaded configuration in
//! sync with changes other processes make.

use super::{App, post};
use crate::{
    command::Source,
    document::View,
    i18n::Msg,
    pages,
    ui::{AppState, MapperPhase, MapperState},
};
use lhc_core::{
    CoreEvent,
    profile::{
        auto_switch::{self, AutoSwitchContext},
        model::LayoutMode,
    },
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

/// Attempts after the first one to start the mapper on launch. At login the
/// keyboard or `/dev/input` permissions may not be ready yet, and a quickly
/// restarted app may wait for the old process to release the devices.
const LAUNCH_RETRIES: u32 = 15;
const LAUNCH_RETRY_DELAY: Duration = Duration::from_secs(2);
/// How often the shown uptime is updated.
const UPTIME_TICK: Duration = Duration::from_secs(20);
/// Longest command label shown in the "command started" notification.
const COMMAND_LABEL_CHARS: usize = 80;

/// Forward core events to the UI thread. Subscribed once at startup.
pub(super) fn forward_core_events() -> Result<lhc_core::events::Subscription, String> {
    lhc_core::events::bus().subscribe(|event| match event {
        CoreEvent::MapperDeviceChanged => post(|app| app.refresh_mapper_status()),
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
                .take(COMMAND_LABEL_CHARS)
                .collect::<String>();
            let result = result.clone();
            post(move |app| match result {
                Ok(()) => crate::notifications::send(&app.settings, Msg::CommandStarted(label)),
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
    })
}

/// Operations owned by the shell before/after a core worker exists.
#[derive(Clone, Copy, Default)]
enum MapperOperation {
    #[default]
    Idle,
    Starting,
    Retrying,
    Stopping,
}

fn mapper_phase(
    operation: MapperOperation,
    phase: lhc_core::mapper::runtime::MapperPhase,
) -> MapperPhase {
    use lhc_core::mapper::runtime::MapperPhase as Core;
    match operation {
        MapperOperation::Starting => MapperPhase::Starting,
        MapperOperation::Retrying => MapperPhase::Retrying,
        MapperOperation::Stopping => MapperPhase::Stopping,
        MapperOperation::Idle => match phase {
            Core::Stopped => MapperPhase::Stopped,
            Core::Starting => MapperPhase::Starting,
            Core::Running => MapperPhase::Running,
            Core::Stopping => MapperPhase::Stopping,
        },
    }
}

/// What the settings show about the mapper's state and uptime.
#[derive(Default)]
pub(super) struct MapperController {
    operation: Cell<MapperOperation>,
    game_active: Cell<Option<bool>>,
    announced_layout: RefCell<Option<Option<String>>>,
    /// Lifecycle, device wait and error last shown.
    last: RefCell<Option<(MapperPhase, bool, Option<String>)>>,
    /// When the running mapper started.
    since: Cell<Option<Instant>>,
    /// How long the mapper ran before it stopped unexpectedly, and when.
    crash: Cell<Option<(Duration, Instant)>>,
    /// Keeps the shown uptime current.
    uptime_clock: slint::Timer,
}

impl MapperController {
    pub(super) fn reset_status(&self) {
        self.last.borrow_mut().take();
    }

    fn game_changed(&self, active: bool) -> bool {
        self.game_active.replace(Some(active)) != Some(active)
    }

    fn layout_changed(&self, id: &Option<String>) -> bool {
        self.announced_layout
            .replace(Some(id.clone()))
            .is_some_and(|previous| previous != *id)
    }

    fn status_changed(&self, current: (MapperPhase, bool, Option<String>)) -> bool {
        if self.last.borrow().as_ref() == Some(&current) {
            return false;
        }
        *self.last.borrow_mut() = Some(current);
        true
    }

    /// Remember when the mapper started and, when it stopped by itself, how
    /// long it ran. Returns whether there is an uptime to show.
    fn track(&self, running: bool, failed: bool, now: Instant) -> bool {
        if running {
            if self.since.get().is_none() {
                self.since.set(Some(now));
                self.crash.set(None);
            }
        } else if let Some(since) = self.since.take() {
            self.crash.set(failed.then(|| (now - since, now)));
        }
        self.since.get().is_some() || self.crash.get().is_some()
    }

    /// Minutes the mapper runs, ran before a crash and since that crash;
    /// -1 when unknown.
    fn minutes(&self, now: Instant) -> (i32, i32, i32) {
        let minutes = |time: Duration| i32::try_from(time.as_secs() / 60).unwrap_or(i32::MAX);
        let (ran, ago) = self
            .crash
            .get()
            .map_or((-1, -1), |(ran, at)| (minutes(ran), minutes(now - at)));
        let uptime = self.since.get().map_or(-1, |since| minutes(now - since));
        (uptime, ran, ago)
    }
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
        if self.mapper.game_changed(status.active) {
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
                settings.manual_layout_id = Some(id);
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
        let changed = self.mapper.layout_changed(&id);
        if !enabled || !changed || !lhc_core::mapper::runtime::status().running {
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
            lhc_core::runtime_state::keyboard_layout()
                .map(|layout| {
                    let code = layout.code.to_uppercase();
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
        if let Err(error) = document.sync_runtime(false) {
            self.set_error(Msg::SavedMapperNotUpdated(error));
        }
        self.refresh_mapper_status();
        pages::refresh_active(&self.settings, &document);
        self.invalidate_menus();
        self.announce_layout();
    }

    pub(super) fn refresh_mapper_status(&self) {
        let status = lhc_core::mapper::runtime::status();
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_enabled(status.running);
        }
        let phase = mapper_phase(self.mapper.operation.get(), status.phase);
        self.settings
            .global::<AppState>()
            .set_mapper_state(MapperState {
                phase,
                error: status
                    .last_error
                    .as_ref()
                    .map_or(Msg::None, |error| Msg::MapperFailed(error.clone()))
                    .to_ui(),
            });
        self.track_uptime(status.running, status.last_error.is_some());
        let current = (phase, status.waiting_for_device, status.last_error.clone());
        if !self.mapper.status_changed(current) {
            return;
        }
        let message = if matches!(phase, MapperPhase::Starting | MapperPhase::Retrying) {
            Msg::MapperStarting
        } else if status.waiting_for_device {
            Msg::MapperWaiting
        } else if status.running {
            Msg::MapperRunning(status.last_error)
        } else {
            Msg::MapperStopped(status.last_error)
        };
        self.settings
            .global::<AppState>()
            .set_status(message.to_ui());
    }

    /// Track the mapper's uptime and keep it shown while there is one.
    fn track_uptime(&self, running: bool, failed: bool) {
        let status = &self.mapper;
        if !status.track(running, failed, Instant::now()) {
            status.uptime_clock.stop();
        } else if !status.uptime_clock.running() {
            status
                .uptime_clock
                .start(slint::TimerMode::Repeated, UPTIME_TICK, || {
                    super::with_app(|app| app.show_uptime())
                });
        }
        self.show_uptime();
    }

    fn show_uptime(&self) {
        let (uptime, ran, ago) = self.mapper.minutes(Instant::now());
        let state = self.settings.global::<AppState>();
        state.set_mapper_uptime(uptime);
        state.set_mapper_ran(ran);
        state.set_mapper_stopped_ago(ago);
    }

    fn set_mapper_operation(&self, operation: MapperOperation) {
        self.mapper.operation.set(operation);
        self.refresh_mapper_status();
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

    /// The saved devices changed: restart a running mapper so it picks them
    /// up, or forget a stopped mapper's failure, which was about the old ones.
    pub(super) fn devices_changed(&self) {
        if self.settings.global::<AppState>().get_mapper_busy() {
            return;
        }
        if lhc_core::mapper::runtime::status().running {
            self.stop_mapper(true);
        } else {
            lhc_core::mapper::runtime::clear_stopped_error();
            self.refresh_mapper_status();
        }
    }

    fn stop_mapper(&self, restart: bool) {
        self.set_mapper_operation(MapperOperation::Stopping);
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::stop();
            post(move |app| {
                app.set_mapper_operation(MapperOperation::Idle);
                match result {
                    Ok(()) => {
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

    /// Start the mapper; a failed start is tried again `retries` times.
    fn start_mapper(&self, retries: u32) {
        if !lhc_core::mapper::runtime::SUPPORTED {
            self.set_error(Msg::MapperUnsupported);
            return;
        }
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
        let revision = document.revision();
        let runtime = match document.runtime_config() {
            Ok(runtime) => runtime,
            Err(error) => {
                self.set_error(Msg::from(&error));
                return;
            }
        };
        lhc_core::active_window::set_titles_needed(runtime.uses_titles);
        self.mapper.crash.set(None);
        self.set_mapper_operation(MapperOperation::Starting);
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::start(&device, mouse.as_deref(), &runtime.json);
            post(move |app| {
                app.set_mapper_operation(MapperOperation::Idle);
                match result {
                    Ok(()) => {
                        crate::notifications::send(&app.settings, Msg::MapperRunning(None));
                        if let Some(document) = app.document() {
                            document.mapper_started(revision, runtime.layout_id);
                            // Both the document and context may have changed while starting.
                            if let Err(error) = document.sync_runtime(false) {
                                app.set_error(Msg::SavedMapperNotUpdated(error));
                            }
                        }
                    }
                    Err(error) if retries > 0 => {
                        log::info!("mapper start failed, retrying: {error}");
                        // Stay busy until the next attempt.
                        app.set_mapper_operation(MapperOperation::Retrying);
                        slint::Timer::single_shot(LAUNCH_RETRY_DELAY, move || {
                            super::with_app(|app| {
                                app.set_mapper_operation(MapperOperation::Idle);
                                app.start_mapper(retries - 1);
                            })
                        });
                        return;
                    }
                    Err(error) => {
                        // A known cause says more than the generic failure.
                        match pages::check_input_access(&app.settings).first() {
                            Some(issue) => app.set_error(Msg::InputAccess(*issue)),
                            None => app.set_error(Msg::MapperFailed(error)),
                        }
                    }
                }
                app.refresh_mapper_status();
            });
        });
    }
}
