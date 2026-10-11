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
use std::time::Instant;

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
        CoreEvent::GameModeChanged(status) => {
            let status = status.clone();
            post(move |app| app.game_mode_changed(&status));
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
        crate::game_mode::refresh(&self.settings, status);
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
                    if layout.variant.is_empty()
                        || layout.variant.eq_ignore_ascii_case(&layout.short)
                    {
                        code
                    } else {
                        format!("{code} ({})", layout.variant)
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
        let current = (status.running, status.last_error.clone());
        if self.last_mapper_status.borrow().as_ref() == Some(&current) {
            return;
        }
        *self.last_mapper_status.borrow_mut() = Some(current);
        let message = if status.running {
            Msg::MapperRunning(status.last_error)
        } else {
            Msg::MapperStopped(status.last_error)
        };
        self.settings
            .global::<AppState>()
            .set_status(crate::notifications::report(&self.settings, &message));
    }

    fn set_mapper_busy(&self, busy: bool) {
        self.settings.global::<AppState>().set_mapper_busy(busy);
    }

    pub(super) fn toggle_mapper(&self) {
        let state = self.settings.global::<AppState>();
        if state.get_mapper_busy() {
            return;
        }
        if lhc_core::mapper::runtime::status().running {
            self.set_mapper_busy(true);
            std::thread::spawn(|| {
                let result = lhc_core::mapper::runtime::stop();
                post(move |app| {
                    app.set_mapper_busy(false);
                    match result {
                        Ok(()) => {
                            app.set_error(Msg::None);
                            crate::notifications::send(&app.settings, Msg::MapperStopped(None));
                        }
                        Err(error) => app.set_error(Msg::Error(error)),
                    }
                    app.refresh_mapper_status();
                });
            });
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
        let runtime = match document.runtime_config() {
            Ok(runtime) => runtime,
            Err(error) => {
                self.set_error(Msg::from(&error));
                return;
            }
        };
        lhc_core::active_window::set_titles_needed(runtime.uses_titles);
        self.set_mapper_busy(true);
        state.set_status(Msg::MapperStarting.to_ui());
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::start(&device, mouse.as_deref(), &runtime.json);
            post(move |app| {
                app.set_mapper_busy(false);
                match result {
                    Ok(()) => {
                        app.set_error(Msg::None);
                        crate::notifications::send(&app.settings, Msg::MapperRunning(None));
                        if let Some(document) = app.document() {
                            document.mapper_started(runtime.layout_id);
                            // The context may have changed while starting.
                            if let Err(error) = document.sync_runtime(false) {
                                app.set_error(Msg::SavedMapperNotUpdated(error));
                            }
                        }
                    }
                    Err(error) => app.set_error(Msg::Error(error)),
                }
                app.refresh_mapper_status();
            });
        });
    }

    /// Pick up external configuration edits. A file that cannot
    /// be read right now (for example mid-write) leaves the mapper running.
    pub(super) fn reload_config(self: &std::rc::Rc<Self>) {
        let Some(document) = self.document() else {
            if let Some(document) = super::load_document(&self.settings) {
                self.install_document(document);
                log::info!("configuration recovered");
            }
            return;
        };
        let state = self.settings.global::<AppState>();
        match document.reload() {
            Ok(None) => {
                // Readable again after a failed read.
                if state.get_config_status().id == "config-unavailable" {
                    state.set_config_status(Msg::None.to_ui());
                }
            }
            Ok(Some(saved)) => {
                let rules = document.read().layout().rules.len();
                state.set_config_status(Msg::ConfigReloaded(rules).to_ui());
                match saved.runtime {
                    Ok(()) => self.clear_runtime_error(),
                    Err(error) => self.set_error(Msg::SavedMapperNotUpdated(error)),
                }
            }
            Err(error) => {
                log::warn!("reload config: {error}");
                state.set_config_status(crate::notifications::report_changed(
                    &self.settings,
                    &Msg::ConfigUnavailable(error.to_string()),
                    state.get_config_status(),
                ));
            }
        }
    }
}
