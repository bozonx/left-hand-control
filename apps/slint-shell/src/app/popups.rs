//! Showing, hiding and driving the popups in the settings process.
//!
//! With a Spell worker every popup command is forwarded to it; otherwise the
//! popups are ordinary winit windows owned here.

use super::{APP, App, with_app};
use crate::{
    command::{Command, Popup, Source, Window},
    i18n::Msg,
    popup_model,
};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{rc::Rc, time::Instant};

/// A button-triggered show waiting for the settings window's activation token.
#[cfg(target_os = "linux")]
pub(super) struct PendingActivation {
    popup: Popup,
    start: Instant,
    serial: slint::winit_030::winit::event_loop::AsyncRequestSerial,
}

impl App {
    pub(super) fn hide(&self, window: Window) {
        if let Err(error) = self.window(window).hide() {
            log::error!("hide {}: {error}", window.name());
        }
        self.metrics.borrow_mut().end(window.name());
    }

    /// Hide on the next event-loop turn unless the window was shown again.
    pub(super) fn defer_hide(&self, window: Window) {
        let generation = self.generation(window);
        super::post(move |app| {
            if app.generation(window) == generation {
                app.hide(window);
            }
        });
    }

    pub(super) fn show(
        &self,
        window: Window,
        source: Source,
        start: Instant,
        token: Option<String>,
    ) {
        self.show_page(window, source, start, token, None);
    }

    pub(super) fn show_page(
        &self,
        window: Window,
        source: Source,
        start: Instant,
        token: Option<String>,
        page: Option<u8>,
    ) {
        #[cfg(not(target_os = "linux"))]
        if window != Window::Settings {
            // The tray opens interactive UI but never returns input.
            if source == Source::Tray {
                self.return_input.borrow_mut().discard();
            } else {
                self.return_input.borrow_mut().capture();
            }
        }
        if window.popup().is_some() {
            self.refresh_popup_data();
        }
        if let Some(popup) = window.popup() {
            if let Some(page) = page {
                popup_model::select_page(popup, page, &self.emoji, &self.quick);
                self.filter_quick(&self.quick.get_query());
            }
            if self.use_spell {
                let id = self
                    .config
                    .as_ref()
                    .and_then(|config| {
                        config
                            .borrow()
                            .active_layout_id(
                                &lhc_core::profile::auto_switch::AutoSwitchContext::current(),
                            )
                            .ok()
                    })
                    .flatten();
                self.send_worker(&Command::PopupLayout(id), source, start, None);
                if !self.send_worker(
                    &page.map_or(Command::Show(window), |page| Command::ShowPage(popup, page)),
                    source,
                    start,
                    token,
                ) {
                    self.set_error(Msg::WorkerRestarting);
                }
                return;
            }
            self.hide(Window::Popup(popup.other()));
        }
        if self.window(window).is_visible() {
            self.hide(window);
        }
        if source != Source::Button {
            self.metrics
                .borrow_mut()
                .begin(window.name(), source.as_str(), start);
        }
        let generation = self.next_generation(window);
        log::info!(
            "show {}, source={}, activation_token={}",
            window.name(),
            source.as_str(),
            token.is_some()
        );
        if let Err(error) = self.window(window).show() {
            log::error!("show {}: {error}", window.name());
            self.metrics.borrow_mut().mark(window.name(), "show_error");
            return;
        }
        self.metrics.borrow_mut().mark(window.name(), "t2_shown");
        match window {
            Window::EMOJI => self.emoji.invoke_prepare(),
            Window::QUICK => self.quick.invoke_prepare(),
            Window::Settings => {}
        }
        if let Err(error) = slint::spawn_local(async move {
            let Some(app) = APP.with(|slot| slot.borrow().clone()) else {
                return;
            };
            match app.window(window).winit_window().await {
                Ok(native) => {
                    if !app.window(window).is_visible() || app.generation(window) != generation {
                        return;
                    }
                    if let Err(error) = app.focus.borrow_mut().activate(&native, token.as_deref()) {
                        log::warn!("activation: {error}");
                    }
                }
                Err(error) => log::warn!("native window: {error}"),
            }
        }) {
            log::error!("activation dispatch: {error}");
        }
    }

    /// Settings-window button: on Linux winit, request an activation token
    /// first so the compositor lets the popup take focus.
    fn show_from_button(&self, popup: Popup) {
        let window = Window::Popup(popup);
        #[cfg(target_os = "linux")]
        if !self.use_spell {
            use slint::winit_030::winit::platform::startup_notify::WindowExtStartupNotify;
            if self.pending_activation.borrow().is_some() {
                return;
            }
            let start = Instant::now();
            self.hide(window);
            self.metrics
                .borrow_mut()
                .begin(window.name(), Source::Button.as_str(), start);
            let requested = self
                .settings
                .window()
                .with_winit_window(|native| native.request_activation_token().ok())
                .flatten();
            let Some(serial) = requested else {
                self.show(window, Source::Button, start, None);
                return;
            };
            *self.pending_activation.borrow_mut() = Some(PendingActivation {
                popup,
                start,
                serial,
            });
            slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
                with_app(|app| {
                    if let Some(pending) = app.take_pending_activation(serial) {
                        log::warn!("activation token timeout");
                        app.show(
                            Window::Popup(pending.popup),
                            Source::Button,
                            pending.start,
                            None,
                        );
                    }
                })
            });
            return;
        }
        self.show(window, Source::Button, Instant::now(), None);
    }

    #[cfg(target_os = "linux")]
    fn take_pending_activation(
        &self,
        serial: slint::winit_030::winit::event_loop::AsyncRequestSerial,
    ) -> Option<PendingActivation> {
        let mut pending = self.pending_activation.borrow_mut();
        if pending.as_ref().is_some_and(|p| p.serial == serial) {
            pending.take()
        } else {
            None
        }
    }

    fn emoji_key(&self, key: &str) {
        self.metrics.borrow_mut().mark("emoji", "t5_first_key");
        let index = self.emoji.get_selected();
        if let Ok(page) = key.parse::<i32>()
            && page > 0
            && page as usize <= self.emoji.get_page_names().row_count()
        {
            self.emoji.set_page(page - 1);
            self.emoji.set_selected(0);
        } else if let Some(index) = "qwertasdfgzxcvb"
            .chars()
            .position(|c| key.eq_ignore_ascii_case(&c.to_string()))
        {
            self.choose_emoji(index as i32);
        } else if popup_model::is_key(key, slint::platform::Key::Escape) {
            self.defer_hide(Window::EMOJI);
        } else if popup_model::is_enter(key) {
            self.choose_emoji(index);
        } else if let Some(delta) = popup_model::key_delta(Popup::Emoji, key) {
            let cells = 15;
            let delta = if delta == 8 {
                5
            } else if delta == -8 {
                -5
            } else {
                delta
            };
            self.emoji
                .set_selected(popup_model::advance(index, delta, cells));
            self.metrics
                .borrow_mut()
                .mark("emoji", "navigation_handled");
        }
    }

    fn quick_key(&self, key: &str) {
        self.metrics.borrow_mut().mark("quick", "t5_first_key");
        if popup_model::is_key(key, slint::platform::Key::Escape) {
            self.defer_hide(Window::QUICK);
        } else if popup_model::is_key(key, slint::platform::Key::Return) {
            self.choose_quick(self.quick.get_selected());
        } else if let Some(delta) = popup_model::key_delta(Popup::Quick, key) {
            self.quick.set_selected(popup_model::advance(
                self.quick.get_selected(),
                delta,
                self.quick.get_items().row_count(),
            ));
            self.metrics
                .borrow_mut()
                .mark("quick", "navigation_handled");
        }
    }

    fn choose_emoji(&self, index: i32) {
        if let Some(value) = popup_model::configured_emoji_index(&self.emoji, index)
            .and_then(|index| self.emoji.get_emojis().row_data(index))
        {
            if value.is_empty() {
                return;
            }
            #[cfg(target_os = "linux")]
            self.execute_popup_action(format!("text:{value}"));
            #[cfg(not(target_os = "linux"))]
            self.return_input.borrow_mut().selected(value.to_string());
        }
        self.defer_hide(Window::EMOJI);
    }

    fn choose_quick(&self, index: i32) {
        if let Some((_, action)) = usize::try_from(index)
            .ok()
            .and_then(|i| self.actions.borrow().get(i).cloned())
        {
            self.defer_hide(Window::QUICK);
            self.execute_popup_action(action);
        }
    }

    fn execute_popup_action(&self, action: String) {
        slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
            with_app(|app| {
                app.command(
                    Command::Execute(action),
                    Source::Button,
                    Instant::now(),
                    None,
                )
            });
        });
    }

    fn refresh_popup_data(&self) {
        if let Some(config) = &self.config {
            let menus = popup_model::ConfiguredMenus {
                layout: config
                    .borrow()
                    .active_layout(&lhc_core::profile::auto_switch::AutoSwitchContext::current())
                    .unwrap_or_default(),
            };
            menus.apply_emoji(&self.emoji);
            menus.apply_quick(&self.quick);
            self.filter_quick(&self.quick.get_query());
        }
    }

    pub(super) fn filter_quick(&self, query: &str) {
        let values =
            self.config
                .as_ref()
                .map(|config| {
                    popup_model::ConfiguredMenus {
                        layout: config
                            .borrow()
                            .active_layout(
                                &lhc_core::profile::auto_switch::AutoSwitchContext::current(),
                            )
                            .unwrap_or_default(),
                    }
                    .quick_page(query, Some(self.quick.get_page() as usize))
                })
                .unwrap_or_default();
        self.quick.set_items(ModelRc::new(VecModel::from(
            values
                .iter()
                .map(|(name, _)| name.clone().into())
                .collect::<Vec<slint::SharedString>>(),
        )));
        *self.actions.borrow_mut() = values;
        self.quick.set_selected(0);
    }
}

fn observe(app: &Rc<App>, window: Window) {
    let weak = Rc::downgrade(app);
    app.window(window).on_close_requested(move || {
        if let Some(app) = weak.upgrade() {
            app.defer_hide(window);
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let weak = Rc::downgrade(app);
    if let Err(error) = app.window(window).set_rendering_notifier(move |state, _| {
        if matches!(state, slint::RenderingState::AfterRendering)
            && let Some(app) = weak.upgrade()
        {
            app.metrics
                .borrow_mut()
                .mark(window.name(), "t3_first_frame");
        }
    }) {
        log::warn!(
            "{} rendering notifier unavailable: {error:?}",
            window.name()
        );
    }
    let weak = Rc::downgrade(app);
    app.window(window)
        .on_winit_window_event(move |native, event| {
            #[cfg(target_os = "linux")]
            if !native.is_visible()
                && native.with_winit_window(|window| {
                    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    window
                        .window_handle()
                        .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)))
                }) == Some(true)
            {
                super::post(move |app| {
                    let native = app.window(window);
                    if !native.is_visible()
                        && native.has_winit_window()
                        && let Err(error) = native.show().and_then(|()| native.hide())
                    {
                        log::error!("dispose hidden {} window: {error}", window.name());
                    }
                });
                return EventResult::PreventDefault;
            }
            #[cfg(not(target_os = "linux"))]
            let _ = native;
            let Some(app) = weak.upgrade() else {
                return EventResult::Propagate;
            };
            match event {
                WindowEvent::Focused(true) => {
                    app.metrics.borrow_mut().mark(window.name(), "t4_focused")
                }
                WindowEvent::Focused(false) if window != Window::Settings => app.defer_hide(window),
                #[cfg(target_os = "linux")]
                WindowEvent::ActivationTokenDone { token, serial }
                    if window == Window::Settings =>
                {
                    if let Some(pending) = app.take_pending_activation(*serial) {
                        app.show(
                            Window::Popup(pending.popup),
                            Source::Button,
                            pending.start,
                            Some(token.clone().into_raw()),
                        );
                    }
                }
                _ => {}
            }
            EventResult::Propagate
        });
}

pub(super) fn bind(app: &Rc<App>) {
    for window in Window::ALL {
        observe(app, window);
    }
    app.settings.on_show_popup(|name| {
        with_app(|app| {
            let popup = if name == "emoji" {
                Popup::Emoji
            } else {
                Popup::Quick
            };
            app.show_from_button(popup);
        })
    });
    app.emoji.on_key(|key| with_app(|app| app.emoji_key(&key)));
    app.emoji
        .on_choose(|index| with_app(|app| app.choose_emoji(index)));
    app.emoji
        .on_dismiss(|| with_app(|app| app.defer_hide(Window::EMOJI)));
    app.quick
        .on_change_page(|_| with_app(|app| app.filter_quick(&app.quick.get_query())));
    app.quick.on_key(|key| with_app(|app| app.quick_key(&key)));
    app.quick
        .on_filter(|query| with_app(|app| app.filter_quick(&query)));
    app.quick
        .on_choose(|index| with_app(|app| app.choose_quick(index)));
    app.quick
        .on_dismiss(|| with_app(|app| app.defer_hide(Window::QUICK)));
}
