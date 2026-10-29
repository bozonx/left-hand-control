//! Settings process: owns the settings window, the winit popups (or the
//! Spell worker that replaces them), tray, hotkeys, IPC and the mapper.
//!
//! All state lives in one [`App`] on the Slint UI thread. Background
//! threads reach it only through `slint::invoke_from_event_loop` +
//! [`with_app`]; nothing here is shared across threads.

mod layer_hint;
mod mapper;
mod popups;
mod worker;

use crate::{
    command::{Command, Dispatch, Preferences, Source, ThemeMode, Window},
    document::{Document, View},
    i18n::{Language, Msg},
    ipc, metrics, pages,
    platform::{backend, focus, hotkey, tray},
    ui::{AppState, EmojiPopup, QuickPopup, SettingsWindow, Theme},
};
use lhc_core::{profile::model::Appearance, storage::StoragePaths};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

/// How often a running Spell worker is checked for crashes.
const WORKER_POLL: Duration = Duration::from_secs(1);

pub(crate) struct App {
    settings: SettingsWindow,
    emoji: EmojiPopup,
    quick: QuickPopup,
    document: RefCell<Option<Rc<Document>>>,
    /// Settings-window input state shared with the pages.
    input: Rc<pages::WindowInput>,
    metrics: RefCell<metrics::Metrics>,
    focus: RefCell<focus::Activation>,
    tray: RefCell<Option<tray::Handle>>,
    #[cfg(not(target_os = "linux"))]
    return_input: RefCell<crate::platform::return_input::ReturnInput>,
    popups: popups::PopupController,
    supervisor: RefCell<worker::Supervisor>,
    preferences: Cell<Preferences>,
    worker_watch: slint::Timer,
    mapper: mapper::MapperController,
    layer_hint: layer_hint::LayerHint,
    shutting_down: Cell<bool>,
}

thread_local! { static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) }; }

fn with_app(f: impl FnOnce(&Rc<App>)) {
    let app = APP.with(|slot| slot.borrow().clone());
    if let Some(app) = app {
        f(&app);
    }
}

/// Run `f` on the UI thread with the application, from any thread.
fn post(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    if let Err(error) = slint::invoke_from_event_loop(move || with_app(f)) {
        log::error!("UI dispatch: {error}");
    }
}

/// Whether the desktop prefers a dark theme; dark when unknown.
fn system_dark(settings: &SettingsWindow) -> bool {
    use slint::winit_030::WinitWindowAccessor;
    settings
        .window()
        .with_winit_window(|window| {
            window.theme() != Some(slint::winit_030::winit::window::Theme::Light)
        })
        .unwrap_or(true)
}

/// Whether `action` is a `sys:` action macOS cannot run.
fn unsupported_on_macos(action: &str) -> bool {
    use lhc_core::profile::actions::{self, Action};
    matches!(Action::parse(Some(action)), Action::System(id) if actions::unsupported_on_macos(&id))
}

impl App {
    fn document(&self) -> Option<Rc<Document>> {
        self.document.borrow().clone()
    }

    fn install_document(self: &Rc<Self>, document: Rc<Document>) {
        *self.document.borrow_mut() = Some(document.clone());
        if let Err(error) = document.edit(View::Shell, |config| config.sync_autostart()) {
            self.set_error(Msg::from(&error));
        }
        pages::bind_document_with_input(&self.settings, &document, &self.input);
        let weak = Rc::downgrade(self);
        document.subscribe(View::Shell, move |_| {
            if let Some(app) = weak.upgrade() {
                app.document_changed();
            }
        });
        self.document_changed();
        self.context_changed();
    }

    fn window(&self, window: Window) -> &slint::Window {
        match window {
            Window::Settings => self.settings.window(),
            Window::EMOJI => self.emoji.window(),
            Window::QUICK => self.quick.window(),
        }
    }

    /// Report a one-off failure as a toast.
    fn set_error(&self, message: Msg) {
        self.settings
            .global::<crate::ui::NotificationCenter>()
            .invoke_backend(message.to_ui());
    }

    fn command(&self, command: Command, source: Source, start: Instant, token: Option<String>) {
        match command {
            Command::Show(window) => self.show(window, source, start, token, None),
            Command::ShowPage(popup, page) => {
                self.show(Window::Popup(popup), source, start, token, Some(page))
            }
            Command::Hide => {
                self.supervisor.borrow_mut().pending_show.take();
                if self.supervisor.borrow().enabled {
                    let _ = self.send_worker(&Command::Hide, source, start, token);
                }
                self.hide(Window::EMOJI);
                self.hide(Window::QUICK);
            }
            Command::ToggleSettings => {
                if self.settings.window().is_visible() {
                    self.hide(Window::Settings);
                } else {
                    self.show(Window::Settings, source, start, token, None);
                }
            }
            Command::ToggleMapper => self.toggle_mapper(),
            Command::GameMode(mode) => {
                match crate::game_mode::apply(mode, self.document().as_deref()) {
                    Ok(status) => self.game_mode_changed(&status),
                    Err(error) => self.set_error(error),
                }
            }
            Command::Preferences(preferences) => self.set_preferences(preferences),
            Command::Ping
            | Command::PopupLayout(_)
            | Command::PopupContents(_)
            | Command::LayerHint(_) => {}
            Command::Execute(action) => {
                if cfg!(target_os = "macos") && unsupported_on_macos(&action) {
                    self.set_error(Msg::ActionUnsupportedMacos);
                } else if !lhc_core::mapper::runtime::status().running {
                    self.set_error(Msg::MapperRequired);
                } else {
                    if let Err(error) = lhc_core::mapper::runtime::execute_action(action) {
                        self.set_error(Msg::ActionFailed(error));
                    }
                }
            }
            Command::Quit => {
                if self.shutdown() {
                    let _ = slint::quit_event_loop();
                } else {
                    self.show(Window::Settings, Source::Button, start, None, None);
                }
            }
        }
    }

    /// Theme and language resolved from the saved settings.
    fn configured_preferences(&self) -> Preferences {
        let Some(document) = self.document() else {
            return Preferences {
                theme: ThemeMode::Dark,
                language: Language::resolve(Default::default()),
                ..Preferences::default()
            };
        };
        let config = document.read();
        let settings = config.settings();
        let dark = match settings.appearance {
            Appearance::Light | Appearance::EInk => false,
            Appearance::Dark => true,
            Appearance::System => system_dark(&self.settings),
        };
        Preferences {
            theme: if settings.appearance == Appearance::EInk {
                ThemeMode::EInk
            } else if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            language: Language::resolve(settings.locale),
            high_contrast: settings.high_contrast,
            reduce_motion: settings.reduce_motion,
        }
    }

    /// Apply the saved theme and language if they changed.
    fn apply_preferences(&self) {
        let preferences = self.configured_preferences();
        if preferences != self.preferences.get() {
            self.set_preferences(preferences);
        }
    }

    fn set_preferences(&self, preferences: Preferences) {
        let changed_language = preferences.language != self.preferences.get().language;
        self.preferences.set(preferences);
        preferences.language.select_bundled();
        for theme in [
            self.settings.global::<Theme>(),
            self.emoji.global::<Theme>(),
            self.quick.global::<Theme>(),
        ] {
            theme.set_dark(preferences.theme == ThemeMode::Dark);
            theme.set_eink(preferences.theme == ThemeMode::EInk);
            theme.set_high_contrast(preferences.high_contrast);
            theme.set_reduce_motion(preferences.reduce_motion);
            crate::ui::apply_theme(&theme);
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_language(preferences.language);
        }
        if self.supervisor.borrow().enabled {
            let _ = self.send_worker(
                &Command::Preferences(preferences),
                Source::Ipc,
                Instant::now(),
                None,
            );
        }
        // Action names on the pages are translated in Rust.
        if changed_language && let Some(document) = self.document() {
            document.refresh_all();
        }
    }

    /// Shell parts that depend on the document.
    fn document_changed(&self) {
        self.refresh_mapper_status();
        let Some(document) = self.document() else {
            return;
        };
        self.settings
            .global::<AppState>()
            .set_keyboard_selected(document.read().input_device().is_some());
        let in_development = document.read().settings().in_development;
        self.settings
            .global::<AppState>()
            .set_in_development(in_development);
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_in_development(in_development);
        }
        self.invalidate_menus();
        self.apply_preferences();
        self.announce_layout();
        self.sync_layer_hint_setting();
    }

    /// Start the mapper if configured to, and show the settings window
    /// while setup is unfinished or there is no tray to open it from.
    fn launched(&self, tray: bool) {
        let (start_mapper, setup_done) = self.document().map_or((false, false), |document| {
            let config = document.read();
            let settings = config.settings();
            let device = config.input_device().is_some();
            let layouts = config.ordered_layout_ids().is_ok_and(|ids| !ids.is_empty());
            (device && settings.start_mapper_on_launch, device && layouts)
        });
        if start_mapper {
            self.start_mapper_on_launch();
        }
        if !tray || !setup_done {
            self.show(Window::Settings, Source::Button, Instant::now(), None, None);
        }
    }

    fn shutdown(&self) -> bool {
        if self.shutting_down.get() {
            return true;
        }
        if self.document().is_some() && !self.settings.invoke_flush_drafts() {
            log::warn!("shutdown deferred: page drafts could not be saved");
            return false;
        }
        self.shutting_down.set(true);
        self.supervisor.borrow_mut().shutdown();
        true
    }
}

/// Callbacks of the settings window that drive the shell rather than a page.
fn bind_shell(app: &Rc<App>) {
    crate::game_mode::bind(&app.settings);
    app.settings
        .global::<AppState>()
        .on_set_game_control(|control| {
            let command = Command::GameMode(crate::game_mode::from_ui(control));
            with_app(|app| app.command(command, Source::Button, Instant::now(), None));
        });
    app.settings
        .global::<AppState>()
        .on_toggle_mapper(|| with_app(|app| app.toggle_mapper()));
    app.settings
        .global::<AppState>()
        .on_devices_changed(|| with_app(|app| app.devices_changed()));
    app.settings
        .global::<AppState>()
        .on_open_keyboard_settings(|| {
            with_app(|app| {
                app.settings
                    .global::<crate::ui::SettingsEditor>()
                    .set_tab(crate::ui::SettingsTab::Mapper);
                app.settings
                    .invoke_navigate(crate::ui::Page::Settings, crate::ui::MenuKind::Emoji);
            })
        });
}

impl App {
    /// Start the tray icon; `false` when the desktop has no tray.
    fn start_tray(&self, dispatch: Dispatch) -> bool {
        match tray::start(dispatch) {
            Ok(tray) => {
                tray.set_language(self.preferences.get().language);
                tray.set_game(&lhc_core::gamemode::status());
                tray.set_in_development(
                    self.document()
                        .is_some_and(|doc| doc.read().settings().in_development),
                );
                *self.tray.borrow_mut() = Some(tray);
                self.mapper.reset_status();
                self.refresh_mapper_status();
                self.metrics.borrow_mut().ready("tray");
                true
            }
            Err(error) => {
                log::error!("tray unavailable: {error}");
                false
            }
        }
    }
}

fn load_document(settings: &SettingsWindow) -> Option<Rc<Document>> {
    let state = settings.global::<AppState>();
    match StoragePaths::resolve()
        .map_err(lhc_core::config_document::ConfigError::Io)
        .and_then(Document::load)
    {
        Ok(document) => {
            let rules = document.read().layout().rules.len();
            state.set_config_status(Msg::ConfigLoaded(rules).to_ui());
            Some(document)
        }
        Err(error) => {
            log::warn!("configuration unavailable: {error}");
            state.set_config_status(crate::notifications::report_changed(
                settings,
                &Msg::ConfigUnavailable(error.to_string()),
                state.get_config_status(),
            ));
            None
        }
    }
}

fn start_core() -> Result<CoreGuard, String> {
    let paths = StoragePaths::resolve();
    if let Ok(paths) = &paths {
        if let Err(error) = paths.ensure() {
            log::warn!("storage: {error}");
        }
        lhc_core::mapper::runtime::set_portal_token_dir(paths.data_dir().clone());
    }
    let events = mapper::forward_core_events()?;
    let watchers = lhc_core::watchers::Watchers::start(paths.ok(), true);
    Ok(CoreGuard {
        watchers: Some(watchers),
        events: Some(events),
    })
}

fn stop_core() {
    let running = lhc_core::mapper::runtime::status().running;
    let result = lhc_core::mapper::runtime::stop();
    if running && let Err(error) = result {
        log::error!("stop mapper during shutdown: {error}");
    }
}

/// Stops the mapper first, then the watchers it reads from.
struct CoreGuard {
    watchers: Option<lhc_core::watchers::Watchers>,
    events: Option<lhc_core::events::Subscription>,
}

impl Drop for CoreGuard {
    fn drop(&mut self) {
        stop_core();
        self.watchers.take();
        self.events.take();
    }
}

pub fn run(start: Instant) -> Result<(), Box<dyn std::error::Error>> {
    let server = match ipc::Server::bind() {
        Ok(server) => server,
        // A second launch (e.g. from the application menu) brings the running
        // instance forward instead of failing.
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            log::info!("{error}; showing its settings window");
            return ipc::client(&Command::Show(Window::Settings));
        }
        Err(error) => return Err(error.into()),
    };
    let use_spell = backend::spell_requested()?;
    let layer_shell = use_spell || backend::layer_shell_supported();
    let popup_attributes = Rc::new(Cell::new(false));
    select_backend(popup_attributes.clone())?;
    // Core watchers post to the UI via `invoke_from_event_loop`, which fails
    // until a Slint platform exists.
    let _core_guard = start_core()?;
    let mut metrics = metrics::Metrics::from_env(start)?;
    let settings = SettingsWindow::new()?;
    crate::notifications::bind(&settings);
    settings
        .global::<AppState>()
        .set_is_linux(cfg!(target_os = "linux"));
    settings
        .global::<AppState>()
        .set_is_windows(cfg!(target_os = "windows"));
    settings
        .global::<AppState>()
        .set_mapper_supported(lhc_core::mapper::runtime::SUPPORTED);
    settings
        .global::<AppState>()
        .set_autostart_supported(lhc_core::autostart::SUPPORTED);
    let document = load_document(&settings);
    metrics.ready("settings");
    popup_attributes.set(true);
    let emoji = EmojiPopup::new()?;
    #[cfg(target_os = "linux")]
    i_slint_core::window::WindowInner::from_pub(emoji.window())
        .set_xdg_app_id(lhc_core::storage::POPUP_APP_ID.into());
    metrics.ready("emoji");
    let quick = QuickPopup::new()?;
    #[cfg(target_os = "linux")]
    i_slint_core::window::WindowInner::from_pub(quick.window())
        .set_xdg_app_id(lhc_core::storage::POPUP_APP_ID.into());
    metrics.ready("quick");
    let app = Rc::new(App {
        settings,
        emoji,
        quick,
        document: RefCell::new(None),
        input: Rc::default(),
        metrics: RefCell::new(metrics),
        focus: RefCell::default(),
        tray: RefCell::new(None),
        #[cfg(not(target_os = "linux"))]
        return_input: RefCell::default(),
        popups: Default::default(),
        supervisor: RefCell::new(worker::Supervisor::new(use_spell, layer_shell)),
        preferences: Cell::new(Preferences::default()),
        worker_watch: slint::Timer::default(),
        mapper: Default::default(),
        layer_hint: Default::default(),
        shutting_down: Cell::new(false),
    });
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    app.set_preferences(app.configured_preferences());
    if let Some(document) = document {
        app.install_document(document);
    }
    app.refresh_mapper_status();
    bind_shell(&app);
    popups::bind(&app);
    app.settings
        .global::<crate::ui::SettingsEditor>()
        .set_layer_hint_available(layer_shell);
    if use_spell {
        app.start_worker(false);
    }
    if layer_shell {
        app.worker_watch
            .start(slint::TimerMode::Repeated, WORKER_POLL, || {
                with_app(|app| app.check_worker())
            });
    }
    let dispatch: Dispatch = Arc::new(|command, source, start, token| {
        post(move |app| app.command(command, source, start, token));
    });
    server.start(dispatch.clone())?;
    slint::Timer::single_shot(Duration::ZERO, move || {
        with_app(|app| {
            let tray = app.start_tray(dispatch.clone());
            app.launched(tray);
        });
        hotkey::start(dispatch);
    });
    log::info!("ready; backend={:?}", std::env::var("SLINT_BACKEND"));
    let event_loop_result = slint::run_event_loop_until_quit();
    app.shutdown();
    server.stop();
    APP.with(|slot| slot.borrow_mut().take());
    event_loop_result?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn select_backend(popup_attributes: Rc<Cell<bool>>) -> Result<(), slint::PlatformError> {
    use slint::winit_030::winit::platform::x11::{WindowAttributesExtX11, WindowType};
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attrs| {
            if popup_attributes.get() {
                attrs.with_x11_window_type(vec![WindowType::Utility])
            } else {
                attrs
            }
        })
        .select()?;
    // Lets the compositor match windows to the installed desktop entry and icon.
    slint::set_xdg_app_id(lhc_core::storage::APP_ID)
}

#[cfg(not(target_os = "linux"))]
fn select_backend(_: Rc<Cell<bool>>) -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()
}
