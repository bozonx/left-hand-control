//! Settings process: owns the settings window, the winit popups (or the
//! Spell worker that replaces them), tray, hotkeys, IPC and the mapper.
//!
//! All state lives in one [`App`] on the Slint UI thread. Background
//! threads reach it only through `slint::invoke_from_event_loop` +
//! [`with_app`]; nothing here is shared across threads.

mod layouts;
mod mapper;
mod popups;
mod settings_page;
mod worker;

use crate::{
    command::{Command, Dispatch, Preferences, Source, ThemeMode, Window},
    editor::{self, EditorHandle},
    i18n::{Language, Msg},
    ipc, metrics,
    platform::{backend, focus, hotkey, tray},
    popup_model,
    ui::{EmojiPopup, Locale, QuickPopup, SettingsWindow, Theme},
};
use lhc_core::{config_document::ConfigDocument, storage::StoragePaths};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) struct App {
    settings: SettingsWindow,
    emoji: EmojiPopup,
    quick: QuickPopup,
    metrics: RefCell<metrics::Metrics>,
    /// Bumped on every show; deferred work checks it to skip stale requests.
    generations: RefCell<HashMap<Window, u64>>,
    focus: RefCell<focus::Activation>,
    tray: RefCell<Option<tray::Handle>>,
    #[cfg(target_os = "linux")]
    pending_activation: RefCell<Option<popups::PendingActivation>>,
    #[cfg(not(target_os = "linux"))]
    return_input: RefCell<crate::platform::return_input::ReturnInput>,
    /// Quick-action fixture shown by the winit popup.
    actions: Vec<String>,
    worker: RefCell<Option<backend::Worker>>,
    use_spell: bool,
    restart_pending: Cell<bool>,
    restart_history: RefCell<Vec<Instant>>,
    preferences: Cell<Preferences>,
    worker_watch: slint::Timer,
    status_watch: slint::Timer,
    last_mapper_status: RefCell<Option<(bool, Option<String>)>>,
    config: Option<Rc<RefCell<ConfigDocument>>>,
    editor: EditorHandle,
    devices: RefCell<Vec<String>>,
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

impl App {
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
        self.settings.set_backend_error(message.to_ui());
    }

    fn command(&self, command: Command, source: Source, start: Instant, token: Option<String>) {
        match command {
            Command::Show(window) => self.show(window, source, start, token),
            Command::Hide => {
                self.send_worker(&Command::Hide, source, start, token);
                self.hide(Window::EMOJI);
                self.hide(Window::QUICK);
            }
            Command::ToggleSettings => {
                if self.settings.window().is_visible() {
                    self.hide(Window::Settings);
                } else {
                    self.show(Window::Settings, source, start, token);
                }
            }
            Command::ToggleMapper => self.toggle_mapper(),
            Command::Preferences(preferences) => {
                self.apply_preferences(preferences);
                self.send_worker(&command, source, start, None);
            }
            Command::Ping => {}
            Command::Execute(action) => {
                if let Err(error) = lhc_core::mapper::runtime::execute_action(action) {
                    self.set_error(Msg::ActionFailed(error));
                }
            }
            Command::Quit => {
                let _ = lhc_core::mapper::runtime::stop();
                let _ = slint::quit_event_loop();
            }
        }
    }

    fn apply_preferences(&self, preferences: Preferences) {
        self.preferences.set(preferences);
        if let Err(error) = slint::select_bundled_translation(preferences.language.code()) {
            log::error!("select translation: {error}");
        }
        for (theme, locale) in [
            (
                self.settings.global::<Theme>(),
                self.settings.global::<Locale>(),
            ),
            (self.emoji.global::<Theme>(), self.emoji.global::<Locale>()),
            (self.quick.global::<Theme>(), self.quick.global::<Locale>()),
        ] {
            theme.set_dark(preferences.theme == ThemeMode::Dark);
            theme.invoke_apply();
            locale.set_english(preferences.language == Language::English);
        }
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_english(preferences.language == Language::English);
        }
    }
}

fn load_config(settings: &SettingsWindow) -> Option<Rc<RefCell<ConfigDocument>>> {
    match StoragePaths::resolve()
        .map_err(lhc_core::config_document::ConfigError::Io)
        .and_then(ConfigDocument::load)
    {
        Ok(config) => {
            settings.set_config_status(Msg::ConfigLoaded(config.layout().rules.len()).to_ui());
            Some(Rc::new(RefCell::new(config)))
        }
        Err(error) => {
            log::warn!("configuration unavailable: {error}");
            settings.set_config_status(Msg::ConfigUnavailable(error.to_string()).to_ui());
            None
        }
    }
}

fn start_core() {
    let paths = StoragePaths::resolve();
    if let Ok(paths) = &paths {
        let _ = paths.ensure();
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
    start_core();
    let worker = backend::start()?;
    let popup_attributes = Rc::new(Cell::new(false));
    select_backend(popup_attributes.clone())?;
    let mut metrics = metrics::Metrics::from_env(start)?;
    let settings = SettingsWindow::new()?;
    let config = load_config(&settings);
    let editor = editor::bind_with_config(&settings, config.clone());
    layouts::bind(&settings, config.clone(), editor.clone());
    settings_page::bind(&settings, config.clone());
    let devices = mapper::bind_devices(&settings, config.as_ref());
    metrics.ready("settings");
    popup_attributes.set(true);
    let emoji = EmojiPopup::new()?;
    emoji.set_emojis(ModelRc::new(VecModel::from(popup_model::emoji_items())));
    metrics.ready("emoji");
    let quick = QuickPopup::new()?;
    metrics.ready("quick");
    let app = Rc::new(App {
        settings,
        emoji,
        quick,
        metrics: RefCell::new(metrics),
        generations: RefCell::default(),
        focus: RefCell::new(focus::Activation::default()),
        tray: RefCell::new(None),
        #[cfg(target_os = "linux")]
        pending_activation: RefCell::new(None),
        #[cfg(not(target_os = "linux"))]
        return_input: RefCell::default(),
        actions: popup_model::quick_items(),
        use_spell: worker.is_some(),
        worker: RefCell::new(worker),
        restart_pending: Cell::new(false),
        restart_history: RefCell::default(),
        preferences: Cell::new(Preferences::default()),
        worker_watch: slint::Timer::default(),
        status_watch: slint::Timer::default(),
        last_mapper_status: RefCell::new(None),
        config,
        editor,
        devices: RefCell::new(devices),
    });
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    let initial_preferences = app.config.as_ref().map_or(Preferences::default(), |config| {
        let config = config.borrow();
        Preferences {
            theme: if config.settings().appearance == lhc_core::profile::model::Appearance::Light { ThemeMode::Light } else { ThemeMode::Dark },
            language: Language::resolve(config.settings().locale),
        }
    });
    app.command(
        Command::Preferences(initial_preferences),
        Source::Button,
        Instant::now(),
        None,
    );
    app.filter_quick("");
    app.refresh_mapper_status();
    bind_settings(&app);
    popups::bind(&app);
    app.status_watch
        .start(slint::TimerMode::Repeated, Duration::from_secs(1), || {
            with_app(|app| {
                app.refresh_mapper_status();
                app.reload_config_if_changed();
            });
        });
    if app.use_spell {
        app.worker_watch
            .start(slint::TimerMode::Repeated, Duration::from_secs(1), || {
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

fn bind_settings(app: &App) {
    app.settings.on_preferences(|dark, english| {
        with_app(|app| {
            app.command(
                Command::Preferences(Preferences {
                    theme: if dark {
                        ThemeMode::Dark
                    } else {
                        ThemeMode::Light
                    },
                    language: if english {
                        Language::English
                    } else {
                        Language::Russian
                    },
                }),
                Source::Button,
                Instant::now(),
                None,
            )
        })
    });
    app.settings.on_settings_saved(|dark, english| {
        with_app(|app| {
            app.command(
                Command::Preferences(Preferences {
                    theme: if dark { ThemeMode::Dark } else { ThemeMode::Light },
                    language: if english { Language::English } else { Language::Russian },
                }),
                Source::Button,
                Instant::now(),
                None,
            );
        });
    });
    app.settings.on_refresh_devices(|| {
        with_app(|app| {
            *app.devices.borrow_mut() = mapper::bind_devices(&app.settings, app.config.as_ref());
            settings_page::refresh_mouse_devices(&app.settings);
        });
    });
    app.settings.on_toggle_mapper(|| {
        with_app(|app| app.command(Command::ToggleMapper, Source::Button, Instant::now(), None))
    });
    app.settings
        .on_select_device(|index| with_app(|app| app.select_device(index)));
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
