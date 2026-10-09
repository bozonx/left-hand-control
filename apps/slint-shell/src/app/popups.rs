//! Showing, hiding and driving the popups in the settings process.
//!
//! With a Spell worker every popup command is forwarded to it; otherwise the
//! popups are ordinary winit windows owned here.

use super::{APP, App, with_app};
use crate::{
    command::{Command, Popup, Source, Window},
    popup_model::{self, ConfiguredMenus, KeyOutcome},
};
use slint::ComponentHandle;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use std::{rc::Rc, time::Instant};

/// Run a chosen action at the latest this long after its popup hid, even
/// if the window system did not report the focus leaving the popup.
const FOCUS_RETURN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(300);

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

    /// Menus of the active layout, loaded once per change.
    fn menus(&self) -> Rc<ConfiguredMenus> {
        if let Some(menus) = self.menus.borrow().as_ref() {
            return menus.clone();
        }
        let layout = self
            .document()
            .as_ref()
            .and_then(|document| {
                document
                    .read()
                    .active_layout(&lhc_core::profile::auto_switch::AutoSwitchContext::current())
                    .map_err(|error| log::warn!("popup menus: {error}"))
                    .ok()
            })
            .unwrap_or_default();
        let menus = Rc::new(ConfiguredMenus { layout });
        *self.menus.borrow_mut() = Some(menus.clone());
        menus
    }

    /// Forget cached menus after the configuration or context changed.
    pub(super) fn invalidate_menus(&self) {
        self.menus.borrow_mut().take();
        self.menu_generation.set(self.menu_generation.get() + 1);
    }

    /// Tell the worker which layout's menus to show, when that changed.
    pub(super) fn sync_worker_menus(
        &self,
        source: Source,
        start: Instant,
    ) -> Result<(), crate::i18n::Msg> {
        let id = self.document().as_ref().and_then(|document| {
            document
                .read()
                .active_layout_id(&lhc_core::profile::auto_switch::AutoSwitchContext::current())
                .ok()
                .flatten()
        });
        let current = (id.clone(), self.menu_generation.get());
        if self.menus_sent.borrow().as_ref() == Some(&current) {
            return Ok(());
        }
        let contents = lhc_core::profile::layout_file::serialize(&self.menus().layout);
        self.send_worker(&Command::PopupContents(contents), source, start, None)?;
        *self.menus_sent.borrow_mut() = Some(current);
        Ok(())
    }

    pub(super) fn show(
        &self,
        window: Window,
        source: Source,
        start: Instant,
        token: Option<String>,
        page: Option<u8>,
    ) {
        if let Some(popup) = window.popup() {
            if self.supervisor.borrow().enabled {
                let command =
                    page.map_or(Command::Show(window), |page| Command::ShowPage(popup, page));
                self.supervisor.borrow_mut().pending_show = Some(super::worker::PendingShow {
                    command,
                    source,
                    start,
                    token,
                });
                self.check_worker();
                self.start_worker(true);
                self.flush_worker_show();
                return;
            }
            #[cfg(not(target_os = "linux"))]
            {
                // The tray opens interactive UI but never returns input.
                if source == Source::Tray {
                    self.return_input.borrow_mut().discard();
                } else {
                    self.return_input.borrow_mut().capture();
                }
            }
            self.refresh_popup_data();
            if let Some(page) = page {
                popup_model::select_page(popup, page, &self.emoji, &self.quick);
                self.filter_quick();
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

    fn popup_key(&self, popup: Popup, key: &str) {
        self.metrics.borrow_mut().mark(popup.name(), "t5_first_key");
        let outcome = match popup {
            Popup::Emoji => popup_model::emoji_key(&self.emoji, key),
            Popup::Quick => popup_model::quick_key(&self.quick, key),
        };
        self.popup_outcome(popup, outcome);
    }

    fn popup_outcome(&self, popup: Popup, outcome: KeyOutcome) {
        match outcome {
            KeyOutcome::Dismiss => self.defer_hide(Window::Popup(popup)),
            KeyOutcome::ChooseCell(index) => self.choose(popup, index),
            KeyOutcome::PageChanged => {
                self.filter_quick();
                self.metrics
                    .borrow_mut()
                    .mark(popup.name(), "navigation_handled");
            }
            KeyOutcome::Ignored => {}
        }
    }

    fn choose(&self, popup: Popup, index: i32) {
        let action = match popup {
            Popup::Emoji => self
                .menus()
                .emoji(&self.emoji, index)
                .map(|emoji| format!("text:{emoji}")),
            Popup::Quick => self.menus().quick_cell(self.quick.get_page(), index),
        };
        let Some(action) = action else { return };
        self.choose_action(popup, action);
    }

    fn choose_action(&self, popup: Popup, action: String) {
        self.defer_hide(Window::Popup(popup));
        #[cfg(not(target_os = "linux"))]
        if let Some(text) = action.strip_prefix("text:") {
            self.return_input.borrow_mut().selected(text.to_owned());
            return;
        }
        // Run once the popup no longer has the keyboard focus.
        *self.pending_action.borrow_mut() = Some((popup, action));
        slint::Timer::single_shot(FOCUS_RETURN_TIMEOUT, move || {
            with_app(|app| app.run_pending_action(popup));
        });
    }

    fn run_pending_action(&self, popup: Popup) {
        let action = self
            .pending_action
            .borrow_mut()
            .take_if(|(pending, _)| *pending == popup)
            .map(|(_, action)| action);
        if let Some(action) = action {
            self.command(
                Command::Execute(action),
                Source::Button,
                Instant::now(),
                None,
            );
        }
    }

    fn refresh_popup_data(&self) {
        let menus = self.menus();
        menus.apply_emoji(&self.emoji);
        menus.apply_quick(&self.quick);
        self.filter_quick();
    }

    pub(super) fn filter_quick(&self) {
        self.menus().fill_quick(&self.quick);
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
    let modifiers =
        std::cell::Cell::new(slint::winit_030::winit::keyboard::ModifiersState::empty());
    app.window(window)
        .on_winit_window_event(move |native, event| {
            // winit keeps delivering events to a hidden Wayland window until its
            // surface is recreated; show and hide it once to release it.
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
            // Inline editors commit on focus loss, but not when the whole window is deactivated.
            if window == Window::Settings
                && let WindowEvent::Focused(active) = event
            {
                app.settings
                    .global::<crate::ui::InlineEditors>()
                    .set_window_active(*active);
            }
            if window == Window::Settings && crate::pages::capture(&app.settings, event) {
                return EventResult::PreventDefault;
            }
            if let WindowEvent::ModifiersChanged(next) = event {
                modifiers.set(next.state());
            }
            if let Some(popup) = window.popup()
                && app.window(window).is_visible()
                && let WindowEvent::KeyboardInput { event, .. } = event
                && let slint::winit_030::winit::keyboard::PhysicalKey::Code(code) =
                    event.physical_key
                && let Some(shortcut) =
                    popup_model::shortcut(&format!("{code:?}"), modifiers.get().shift_key())
            {
                if event.state == slint::winit_030::winit::event::ElementState::Pressed
                    && !event.repeat
                {
                    app.metrics.borrow_mut().mark(popup.name(), "t5_first_key");
                    let outcome =
                        popup_model::apply_shortcut(popup, shortcut, &app.emoji, &app.quick);
                    app.popup_outcome(popup, outcome);
                }
                return EventResult::PreventDefault;
            }
            if crate::text_editing::dispatch_shortcut(native, event, modifiers.get()) {
                return EventResult::PreventDefault;
            }
            match event {
                WindowEvent::Focused(true) => {
                    app.metrics.borrow_mut().mark(window.name(), "t4_focused")
                }
                WindowEvent::Focused(false) => {
                    if let Some(popup) = window.popup() {
                        app.defer_hide(window);
                        app.run_pending_action(popup);
                    }
                }
                WindowEvent::ThemeChanged(_) if window == Window::Settings => {
                    app.apply_preferences()
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
    app.settings
        .global::<crate::ui::MenuEditor>()
        .on_show_page(|| {
            with_app(|app| {
                let editor = app.settings.global::<crate::ui::MenuEditor>();
                let popup = match editor.get_kind() {
                    crate::ui::MenuKind::Emoji => Popup::Emoji,
                    crate::ui::MenuKind::Quick => Popup::Quick,
                };
                if let Ok(page) = u8::try_from(editor.get_selected_page() + 1) {
                    app.command(
                        Command::ShowPage(popup, page),
                        Source::Button,
                        Instant::now(),
                        None,
                    );
                }
            });
        });
    app.emoji
        .on_key(|key| with_app(|app| app.popup_key(Popup::Emoji, &key)));
    app.emoji
        .on_choose(|index| with_app(|app| app.choose(Popup::Emoji, index)));
    app.emoji
        .on_dismiss(|| with_app(|app| app.defer_hide(Window::EMOJI)));
    app.quick
        .on_key(|key| with_app(|app| app.popup_key(Popup::Quick, &key)));
    app.quick
        .on_choose(|index| with_app(|app| app.choose(Popup::Quick, index)));
    app.quick
        .on_dismiss(|| with_app(|app| app.defer_hide(Window::QUICK)));
}
