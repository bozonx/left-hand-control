//! Mapper lifecycle, input-device selection, core events and keeping the
//! loaded config in sync with changes made by other processes.

use super::{App, post};
use crate::{
    command::{Source, Window},
    i18n::Msg,
    ui::SettingsWindow,
};
use lhc_core::{
    CoreEvent, config_document::ConfigDocument, profile::auto_switch::AutoSwitchContext,
};
use slint::{ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc, time::Instant};

/// Forward core events to the UI thread. Subscribed once at startup.
pub(super) fn forward_core_events() {
    lhc_core::events::bus().subscribe(|event| match event {
        CoreEvent::MapperStopped(error) => {
            let error = error.clone();
            post(move |app| {
                app.set_error(Msg::Error(error));
                app.refresh_mapper_status();
            });
        }
        CoreEvent::AppAction(name) => {
            let window = if name.starts_with("show_quick_menu_") {
                Window::QUICK
            } else if name.starts_with("show_emoji_menu_") {
                Window::EMOJI
            } else {
                return;
            };
            post(move |app| app.show(window, Source::Mapper, Instant::now(), None));
        }
        CoreEvent::LayoutChanged(_)
        | CoreEvent::GameModeChanged(_)
        | CoreEvent::ActiveWindowChanged(_) => {}
    });
}

/// Fill the device picker; returns device paths in picker order.
pub(super) fn bind_devices(
    settings: &SettingsWindow,
    config: Option<&Rc<RefCell<ConfigDocument>>>,
) -> Vec<String> {
    let mut devices = lhc_core::mapper::runtime::list_keyboards().unwrap_or_else(|error| {
        log::warn!("keyboard discovery: {error}");
        Vec::new()
    });
    let saved = config.and_then(|config| config.borrow().input_device().map(str::to_owned));
    // Keep a saved device selectable even when it is not currently readable.
    if let Some(path) = &saved
        && !devices.iter().any(|device| &device.path == path)
    {
        devices.insert(
            0,
            lhc_core::mapper_types::KeyboardDevice {
                path: path.clone(),
                name: String::new(),
            },
        );
    }
    let selected = saved
        .and_then(|path| devices.iter().position(|device| device.path == path))
        .map_or(-1, |index| index as i32);
    let labels: Vec<SharedString> = devices
        .iter()
        .map(|device| {
            if device.name.is_empty() {
                device.path.as_str().into()
            } else {
                format!("{} · {}", device.name, device.path).into()
            }
        })
        .collect();
    settings.set_input_devices(ModelRc::new(VecModel::from(labels)));
    settings.set_selected_device(selected);
    devices.into_iter().map(|device| device.path).collect()
}

impl App {
    pub(super) fn refresh_mapper_status(&self) {
        let status = lhc_core::mapper::runtime::status();
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_enabled(status.running);
        }
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
        self.settings.set_status(message.to_ui());
    }

    pub(super) fn toggle_mapper(&self) {
        if lhc_core::mapper::runtime::status().running {
            std::thread::spawn(|| {
                let result = lhc_core::mapper::runtime::stop();
                post(move |app| {
                    if let Err(error) = result {
                        app.set_error(Msg::Error(error));
                    }
                    app.refresh_mapper_status();
                });
            });
            return;
        }
        let Some(config) = &self.config else {
            self.set_error(Msg::LoadConfigFirst);
            return;
        };
        let config = config.borrow();
        let Some(device) = config.input_device().map(str::to_owned) else {
            self.set_error(Msg::SelectInputDevice);
            return;
        };
        let mouse = config.mouse_device().map(str::to_owned);
        let raw = match config.runtime_config(&AutoSwitchContext::current()) {
            Ok(runtime) => runtime.json,
            Err(error) => {
                self.set_error(Msg::from(&error));
                return;
            }
        };
        self.settings.set_status(Msg::MapperStarting.to_ui());
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::start(&device, mouse.as_deref(), &raw);
            post(move |app| {
                app.set_error(result.err().map_or(Msg::None, Msg::Error));
                app.refresh_mapper_status();
            });
        });
    }

    pub(super) fn select_device(&self, index: i32) {
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|index| self.devices.borrow().get(index).cloned())
        else {
            return;
        };
        let Some(config) = &self.config else {
            return;
        };
        match config.borrow_mut().set_input_device(&path) {
            Ok(()) => self
                .settings
                .set_config_status(Msg::DeviceSaved(path.clone()).to_ui()),
            Err(error) => self
                .settings
                .set_backend_error(Msg::from(&error).to_ui()),
        }
    }

    /// Pick up edits made by the Tauri shell or by hand while running.
    pub(super) fn reload_config_if_changed(&self) {
        let Some(config) = &self.config else {
            return;
        };
        let mut document = config.borrow_mut();
        match document.reload_if_changed() {
            Ok(false) => {}
            Ok(true) => {
                self.editor.reload(&document);
                self.settings
                    .set_config_status(Msg::ConfigReloaded(document.layout().rules.len()).to_ui());
                match document.runtime_config(&AutoSwitchContext::current()) {
                    Ok(runtime) => {
                        if let Err(error) =
                            lhc_core::mapper::runtime::update_config_if_running(&runtime.json)
                        {
                            self.set_error(Msg::Error(error));
                        }
                    }
                    Err(error) => self.set_error(Msg::from(&error)),
                }
            }
            Err(error) => {
                log::debug!("reload config: {error}");
                self.settings
                    .set_config_status(Msg::ConfigUnavailable(error.to_string()).to_ui());
            }
        }
    }
}
