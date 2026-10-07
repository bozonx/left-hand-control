//! Settings process: owns the settings window, the winit popups (or the
//! Spell worker that replaces them), tray, hotkeys, IPC and the mapper.
//!
//! All state lives in one [`App`] on the Slint UI thread. Background
//! threads reach it only through `slint::invoke_from_event_loop` +
//! [`with_app`]; nothing here is shared across threads.

mod mapper;
mod popups;
mod worker;

use crate::{
    command::{Command, Dispatch, Popup, Preferences, Source, ThemeMode, Window},
    document::{Document, View},
    i18n::{Language, Msg},
    ipc, metrics, pages,
    platform::{backend, focus, hotkey, tray},
    popup_model::ConfiguredMenus,
    ui::{AppState, EmojiPopup, Locale, QuickPopup, SettingsWindow, Theme},
};
use lhc_core::{profile::model::Appearance, storage::StoragePaths};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

/// How often the configuration files are checked for changes made by
/// other processes. Unchanged files are not read (see `TrackedFile`).
const CONFIG_POLL: Duration = Duration::from_secs(1);
/// How often a running Spell worker is checked for crashes.
const WORKER_POLL: Duration = Duration::from_secs(1);

pub(crate) struct App {
    settings: SettingsWindow,
    emoji: EmojiPopup,
    quick: QuickPopup,
    document: RefCell<Option<Rc<Document>>>,
    metrics: RefCell<metrics::Metrics>,
    /// Bumped on every show; deferred work checks it to skip stale requests.
    generations: RefCell<HashMap<Window, u64>>,
    focus: RefCell<focus::Activation>,
    tray: RefCell<Option<tray::Handle>>,
    #[cfg(not(target_os = "linux"))]
    return_input: RefCell<crate::platform::return_input::ReturnInput>,
    /// Menus of the active layout for the winit popups.
    menus: RefCell<Option<Rc<ConfiguredMenus>>>,
    /// Bumped whenever the menus may have changed.
    menu_generation: Cell<u64>,
    /// Layout and generation last sent to the Spell worker.
    menus_sent: RefCell<Option<(Option<String>, u64)>>,
    /// Actions of the quick popup items in display order.
    quick_actions: RefCell<Vec<String>>,
    /// Action chosen in a popup, run once the popup lost the focus.
    pending_action: RefCell<Option<(Popup, String)>>,
    supervisor: RefCell<worker::Supervisor>,
    preferences: Cell<Preferences>,
    last_autostart: Cell<Option<bool>>,
    config_watch: slint::Timer,
    worker_watch: slint::Timer,
    last_mapper_status: RefCell<Option<(bool, Option<String>)>>,
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

impl App {
    fn document(&self) -> Option<Rc<Document>> {
        self.document.borrow().clone()
    }

    fn install_document(self: &Rc<Self>, document: Rc<Document>) {
        *self.document.borrow_mut() = Some(document.clone());
        pages::bind_document(&self.settings, &document);
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

    fn generation(&self, window: Window) -> u64 {
        self.generations
            .borrow()
            .get(&window)
            .copied()
            .unwrap_or_default()
    }

    fn next_generation(&self, window: Window) -> u64 {
        let mut generations = self.generations.borrow_mut();
        let generation = generations.entry(window).or_default();
        *generation += 1;
        *generation
    }

    fn set_error(&self, message: Msg) {
        self.settings
            .global::<AppState>()
            .set_backend_error(message.to_ui());
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
            Command::Preferences(preferences) => self.set_preferences(preferences),
            Command::Ping | Command::PopupLayout(_) => {}
            Command::Execute(action) => {
                if !lhc_core::mapper::runtime::status().running {
                    self.set_error(Msg::MapperRequired);
                } else if let Err(error) = lhc_core::mapper::runtime::execute_action(action) {
                    self.set_error(Msg::ActionFailed(error));
                }
            }
            Command::Quit => {
                let _ = lhc_core::mapper::runtime::stop();
                let _ = slint::quit_event_loop();
            }
        }
    }

    /// Theme and language resolved from the saved settings.
    fn configured_preferences(&self) -> Preferences {
        let Some(document) = self.document() else {
            return Preferences {
                theme: ThemeMode::Dark,
                language: Language::resolve(Default::default()),
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
        for (theme, locale) in [
            (
                self.settings.global::<Theme>(),
                self.settings.global::<Locale>(),
            ),
            (self.emoji.global::<Theme>(), self.emoji.global::<Locale>()),
            (self.quick.global::<Theme>(), self.quick.global::<Locale>()),
        ] {
            theme.set_dark(preferences.theme == ThemeMode::Dark);
            theme.set_eink(preferences.theme == ThemeMode::EInk);
            crate::ui::apply_theme(&theme);
            locale.set_english(preferences.language == Language::English);
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_english(preferences.language == Language::English);
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
        let launch_on_startup = document.read().settings().launch_on_startup;
        if cfg!(target_os = "linux") && self.last_autostart.get() != Some(launch_on_startup) {
            match lhc_core::autostart::set_enabled(document.read().paths(), launch_on_startup) {
                Ok(()) => self.last_autostart.set(Some(launch_on_startup)),
                Err(error) => self.set_error(Msg::Error(error)),
            }
        }
        let need_approval = {
            let config = document.read();
            !config.layout().commands.is_empty() && !config.commands_trusted()
        };
        self.settings
            .global::<AppState>()
            .set_commands_need_approval(need_approval);
        self.invalidate_menus();
        self.apply_preferences();
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
            state.set_config_status(Msg::ConfigUnavailable(error.to_string()).to_ui());
            None
        }
    }
}

fn start_core() {
    let paths = StoragePaths::resolve();
    if let Ok(paths) = &paths {
        if let Err(error) = paths.ensure() {
            log::warn!("storage: {error}");
        }
        lhc_core::mapper::runtime::set_portal_token_dir(paths.data_dir().clone());
    }
    lhc_core::layout::start_watcher();
    lhc_core::gamemode::start_watcher(paths.ok());
    lhc_core::active_window::start_watcher();
    mapper::forward_core_events();
}

fn stop_core() {
    let _ = lhc_core::mapper::runtime::stop();
    lhc_core::layout::stop_watcher();
    lhc_core::gamemode::stop_watcher();
    lhc_core::active_window::stop_watcher();
}

pub fn run(start: Instant) -> Result<(), Box<dyn std::error::Error>> {
    let server = ipc::Server::bind()?;
    let use_spell = backend::spell_requested()?;
    let popup_attributes = Rc::new(Cell::new(false));
    select_backend(popup_attributes.clone())?;
    // Core watchers post to the UI via `invoke_from_event_loop`, which fails
    // until a Slint platform exists.
    start_core();
    let mut metrics = metrics::Metrics::from_env(start)?;
    let settings = SettingsWindow::new()?;
    settings
        .global::<AppState>()
        .set_is_linux(cfg!(target_os = "linux"));
    let document = load_document(&settings);
    metrics.ready("settings");
    popup_attributes.set(true);
    let emoji = EmojiPopup::new()?;
    metrics.ready("emoji");
    let quick = QuickPopup::new()?;
    metrics.ready("quick");
    let app = Rc::new(App {
        settings,
        emoji,
        quick,
        document: RefCell::new(None),
        metrics: RefCell::new(metrics),
        generations: RefCell::default(),
        focus: RefCell::new(focus::Activation::default()),
        tray: RefCell::new(None),
        #[cfg(not(target_os = "linux"))]
        return_input: RefCell::default(),
        menus: RefCell::default(),
        menu_generation: Cell::new(0),
        menus_sent: RefCell::default(),
        quick_actions: RefCell::default(),
        pending_action: RefCell::default(),
        supervisor: RefCell::new(worker::Supervisor::new(use_spell)),
        preferences: Cell::new(Preferences::default()),
        last_autostart: Cell::new(None),
        config_watch: slint::Timer::default(),
        worker_watch: slint::Timer::default(),
        last_mapper_status: RefCell::new(None),
    });
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    app.set_preferences(app.configured_preferences());
    if let Some(document) = document {
        app.install_document(document);
    }
    app.refresh_mapper_status();
    crate::game_mode::bind(&app.settings);
    app.settings
        .global::<AppState>()
        .on_toggle_mapper(|| with_app(|app| app.toggle_mapper()));
    popups::bind(&app);
    app.config_watch
        .start(slint::TimerMode::Repeated, CONFIG_POLL, || {
            with_app(|app| app.reload_config())
        });
    if use_spell {
        app.start_worker(false);
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
        with_app(|app| match tray::start(dispatch.clone()) {
            Ok(tray) => {
                tray.set_english(app.preferences.get().language == Language::English);
                *app.tray.borrow_mut() = Some(tray);
                app.last_mapper_status.borrow_mut().take();
                app.refresh_mapper_status();
                app.metrics.borrow_mut().ready("tray");
            }
            Err(error) => log::error!("tray unavailable: {error}"),
        });
        hotkey::start(dispatch);
    });
    log::info!("ready; backend={:?}", std::env::var("SLINT_BACKEND"));
    slint::run_event_loop_until_quit()?;
    stop_core();
    APP.with(|slot| slot.borrow_mut().take());
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
        .select()
}

#[cfg(not(target_os = "linux"))]
fn select_backend(_: Rc<Cell<bool>>) -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()
}
