//! Mapper lifecycle, core events and keeping the loaded configuration in
//! sync with changes other processes make.

use super::{App, post};
use crate::{command::Source, i18n::Msg, pages, ui::AppState};
use lhc_core::CoreEvent;
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
                    crate::notifications::send(&app.settings, Msg::CommandCompleted(label));
                }
                Err(error) => app.set_error(Msg::ActionFailed(error)),
            });
        }
        CoreEvent::AppAction(name) => {
            let Some(command) = crate::command::Command::from_app_action(name) else {
                return;
            };
            post(move |app| app.command(command, Source::Mapper, Instant::now(), None));
        }
        CoreEvent::LayoutChanged(_)
        | CoreEvent::GameModeChanged(_)
        | CoreEvent::ActiveWindowChanged(_) => post(|app| app.context_changed()),
    });
}

impl App {
    /// The system context (layout, game mode, window) changed: the active
    /// layout may differ now.
    pub(super) fn context_changed(&self) {
        let state = self.settings.global::<AppState>();
        crate::game_mode::refresh(&self.settings, &lhc_core::gamemode::status());
        state.set_keyboard_language(
            lhc_core::runtime_state::layout()
                .map(|layout| {
                    let variant = if layout.variant.is_empty() {
                        &layout.short
                    } else {
                        &layout.variant
                    };
                    format!("{}-{variant}", layout.short.to_uppercase())
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

    /// Pick up edits made by the Tauri shell or by hand. A file that cannot
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
