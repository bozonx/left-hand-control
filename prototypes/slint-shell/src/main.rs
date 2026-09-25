mod app_storage;
#[cfg(target_os = "linux")]
mod backend;
#[cfg(not(target_os = "linux"))]
#[path = "platform/backend.rs"]
mod backend;
mod config_state;
mod editor;
#[cfg(target_os = "linux")]
mod focus;
#[cfg(not(target_os = "linux"))]
#[path = "platform/focus.rs"]
mod focus;
#[cfg(target_os = "linux")]
mod hotkey;
#[cfg(not(target_os = "linux"))]
#[path = "platform/hotkey.rs"]
mod hotkey;
mod ipc;
mod metrics;
mod popup_model;
#[cfg(not(target_os = "linux"))]
#[path = "platform/return_input.rs"]
mod return_input;
#[cfg(all(feature = "spell", target_os = "linux"))]
mod return_input;
#[cfg(all(feature = "spell", target_os = "linux"))]
mod spell;
#[cfg(all(feature = "spell", target_os = "linux"))]
mod test_keyboard;
#[cfg(target_os = "linux")]
mod tray;
#[cfg(not(target_os = "linux"))]
#[path = "platform/tray.rs"]
mod tray;

use slint::winit_030::{WinitWindowAccessor, winit};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::event::WindowEvent;
#[cfg(target_os = "linux")]
use winit::platform::startup_notify::WindowExtStartupNotify;

slint::include_modules!();

#[derive(Clone)]
enum Command {
    Show(&'static str),
    Hide,
    ToggleSettings,
    ToggleMapper,
    Quit,
    Ping,
    Preferences(bool, bool),
}

impl Command {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "show emoji" => Ok(Self::Show("emoji")),
            "show quick" => Ok(Self::Show("quick")),
            "show settings" => Ok(Self::Show("settings")),
            "hide" => Ok(Self::Hide),
            "toggle-mapper" => Ok(Self::ToggleMapper),
            "quit" => Ok(Self::Quit),
            "preferences dark ru" => Ok(Self::Preferences(true, false)),
            "preferences dark en" => Ok(Self::Preferences(true, true)),
            "preferences light ru" => Ok(Self::Preferences(false, false)),
            "preferences light en" => Ok(Self::Preferences(false, true)),
            "ping" => Ok(Self::Ping),
            _ => Err(
                "usage: slint-shell [show emoji|quick|settings | hide | toggle-mapper | ping | quit]"
                    .into(),
            ),
        }
    }
}

type Dispatch = Arc<dyn Fn(Command, &'static str, Instant, Option<String>) + Send + Sync>;

struct SlintMapperHost;

impl lhc_core::mapper::MapperHost for SlintMapperHost {
    fn mapper_stopped(&self, error: &str) {
        let error = error.to_owned();
        let _ = slint::invoke_from_event_loop(move || {
            with_app(|app| {
                app.settings.set_backend_error(error.into());
                app.refresh_mapper_status();
            })
        });
    }

    fn app_event(&self, name: &str) {
        let target = if name.starts_with("show_quick_menu_") {
            Some("quick")
        } else if name.starts_with("show_emoji_menu_") {
            Some("emoji")
        } else {
            None
        };
        if let Some(target) = target {
            let _ = slint::invoke_from_event_loop(move || {
                with_app(|app| {
                    app.show(target, "mapper", Instant::now(), None);
                })
            });
        }
    }

    fn refresh_layout(&self) {}
}

struct App {
    settings: SettingsWindow,
    emoji: EmojiPopup,
    quick: QuickPopup,
    metrics: RefCell<metrics::Metrics>,
    focus: RefCell<focus::Activation>,
    tray: RefCell<Option<tray::Handle>>,
    #[cfg(target_os = "linux")]
    pending: RefCell<Option<(&'static str, Instant, winit::event_loop::AsyncRequestSerial)>>,
    actions: Vec<String>,
    worker: RefCell<Option<backend::Worker>>,
    use_spell: bool,
    restart_pending: Cell<bool>,
    restart_history: RefCell<Vec<Instant>>,
    preferences: Cell<(bool, bool)>,
    worker_watch: slint::Timer,
    status_watch: slint::Timer,
    last_mapper_status: RefCell<Option<(bool, Option<String>)>>,
    config: Option<Rc<RefCell<config_state::ConfigState>>>,
    devices: Vec<String>,
    #[cfg(not(target_os = "linux"))]
    return_input: RefCell<return_input::ReturnInput>,
}

thread_local! { static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) }; }

fn with_app(f: impl FnOnce(&Rc<App>)) {
    APP.with(|app| {
        if let Some(app) = app.borrow().as_ref() {
            f(app);
        }
    });
}

impl App {
    fn send_worker(
        &self,
        command: String,
        source: &'static str,
        start: Instant,
        token: Option<String>,
    ) -> bool {
        let result = self.worker.borrow().as_ref().map(|worker| {
            worker
                .send(command, source, start, token)
                .map_err(|error| error.to_string())
        });
        if let Some(Err(error)) = result.as_ref() {
            self.report_worker_error(error);
        }
        result.is_some()
    }

    fn refresh_mapper_status(&self) {
        let status = lhc_core::mapper::runtime::status();
        if let Some(tray) = self.tray.borrow().as_ref() {
            tray.set_enabled(status.running);
        }
        let current = (status.running, status.last_error.clone());
        if self.last_mapper_status.borrow().as_ref() == Some(&current) {
            return;
        }
        *self.last_mapper_status.borrow_mut() = Some(current);
        let label = if status.running {
            "Mapper работает"
        } else {
            "Mapper остановлен"
        };
        self.settings.set_status(
            status
                .last_error
                .map_or(label.to_owned(), |error| format!("{label}: {error}"))
                .into(),
        );
    }

    fn toggle_mapper(&self) {
        if lhc_core::mapper::runtime::status().running {
            std::thread::spawn(|| {
                let result = lhc_core::mapper::runtime::stop();
                let _ = slint::invoke_from_event_loop(move || {
                    with_app(|app| {
                        if let Err(error) = result {
                            app.settings.set_backend_error(error.into());
                        }
                        app.refresh_mapper_status();
                    })
                });
            });
            return;
        }
        let Some(config) = &self.config else {
            self.settings.set_backend_error(
                "Загрузите корректную конфигурацию перед запуском mapper".into(),
            );
            return;
        };
        let config = config.borrow();
        let Some(device) = config.input_device().map(str::to_owned) else {
            self.settings
                .set_backend_error("Выберите inputDevicePath в настройках".into());
            return;
        };
        let mouse = config.mouse_device().map(str::to_owned);
        let Ok(raw) = config.raw() else {
            return;
        };
        self.settings.set_status("Mapper запускается…".into());
        std::thread::spawn(move || {
            let result = lhc_core::mapper::runtime::start(&device, mouse.as_deref(), &raw);
            let _ = slint::invoke_from_event_loop(move || {
                with_app(|app| {
                    if let Err(error) = result {
                        app.settings.set_backend_error(error.into());
                    } else {
                        app.settings.set_backend_error("".into());
                    }
                    app.refresh_mapper_status();
                })
            });
        });
    }
    fn report_worker_error(&self, error: &dyn std::fmt::Display) {
        let message = format!("Spell popup process unavailable: {error}");
        log::error!("{message}");
        self.settings.set_backend_error(message.into());
        self.restart_worker();
    }

    fn restart_worker(&self) {
        if self.restart_pending.replace(true) {
            return;
        }
        let now = Instant::now();
        let mut history = self.restart_history.borrow_mut();
        history.retain(|attempt| now.duration_since(*attempt) < Duration::from_secs(60));
        if history.len() >= 3 {
            self.settings.set_backend_error(
                "Spell worker: превышен лимит перезапусков (3 за минуту)".into(),
            );
            self.restart_pending.set(false);
            return;
        }
        history.push(now);
        drop(history);
        self.worker.borrow_mut().take();
        std::thread::spawn(|| {
            let result = backend::start().map_err(|error| error.to_string());
            let _ = slint::invoke_from_event_loop(move || {
                with_app(|app| {
                    app.restart_pending.set(false);
                    match result {
                        Ok(Some(worker)) => {
                            let (dark, english) = app.preferences.get();
                            let preference = format!(
                                "preferences {} {}",
                                if dark { "dark" } else { "light" },
                                if english { "en" } else { "ru" }
                            );
                            if let Err(error) = worker.send(preference, "ipc", Instant::now(), None)
                            {
                                app.settings
                                    .set_backend_error(format!("Spell worker: {error}").into());
                            } else {
                                app.settings.set_backend_error("".into());
                            }
                            *app.worker.borrow_mut() = Some(worker);
                        }
                        Ok(None) => app
                            .settings
                            .set_backend_error("Spell worker не запущен".into()),
                        Err(error) => app
                            .settings
                            .set_backend_error(format!("Spell worker: {error}").into()),
                    }
                })
            });
        });
    }

    fn window(&self, name: &str) -> &slint::Window {
        match name {
            "emoji" => self.emoji.window(),
            "quick" => self.quick.window(),
            _ => self.settings.window(),
        }
    }

    fn hide(&self, name: &'static str) {
        if let Err(error) = self.window(name).hide() {
            log::error!("hide {name}: {error}");
        }
        self.metrics.borrow_mut().end(name);
    }

    fn defer_hide(&self, name: &'static str) {
        let trial = self.metrics.borrow().trial(name);
        let _ = slint::invoke_from_event_loop(move || {
            with_app(|app| {
                if app.metrics.borrow().trial(name) == trial {
                    app.hide(name);
                }
            })
        });
    }

    fn show(
        &self,
        name: &'static str,
        source: &'static str,
        start: Instant,
        token: Option<String>,
    ) {
        #[cfg(not(target_os = "linux"))]
        if name != "settings" {
            if source == "tray" {
                self.return_input.borrow_mut().discard();
            } else {
                self.return_input.borrow_mut().capture();
            }
        }
        if name != "settings" && self.use_spell {
            if !self.send_worker(format!("show {name}"), source, start, token.clone()) {
                self.settings
                    .set_backend_error("Spell worker перезапускается".into());
            }
            return;
        }
        if name != "settings" {
            self.hide(if name == "emoji" { "quick" } else { "emoji" });
        }
        if self.window(name).is_visible() {
            self.hide(name);
        }
        if source != "button" {
            self.metrics.borrow_mut().begin(name, source, start);
        }
        let trial = self.metrics.borrow().trial(name);
        log::info!(
            "show {name}, source={source}, activation_token={}",
            token.is_some()
        );
        let result = self.window(name).show();
        if let Err(error) = result {
            log::error!("show {name}: {error}");
            self.metrics.borrow_mut().mark(name, "show_error");
            return;
        }
        self.metrics.borrow_mut().mark(name, "t2_shown");
        match name {
            "emoji" => self.emoji.invoke_prepare(),
            "quick" => self.quick.invoke_prepare(),
            _ => {}
        }
        if let Err(error) = slint::spawn_local(async move {
            let app = APP.with(|slot| slot.borrow().as_ref().cloned());
            if let Some(app) = app {
                match app.window(name).winit_window().await {
                    Ok(window) => {
                        if !app.window(name).is_visible()
                            || app.metrics.borrow().trial(name) != trial
                        {
                            return;
                        }
                        if let Err(error) =
                            app.focus.borrow_mut().activate(&window, token.as_deref())
                        {
                            log::warn!("activation: {error}");
                        }
                    }
                    Err(error) => log::warn!("native window: {error}"),
                }
            }
        }) {
            log::error!("activation dispatch: {error}");
        }
    }

    fn command(
        &self,
        command: Command,
        source: &'static str,
        start: Instant,
        token: Option<String>,
    ) {
        match command {
            Command::Show(name) => self.show(name, source, start, token),
            Command::Hide => {
                self.send_worker("hide".into(), source, start, token);
                self.hide("emoji");
                self.hide("quick");
            }
            Command::ToggleSettings => {
                if self.settings.window().is_visible() {
                    self.hide("settings");
                } else {
                    self.show("settings", source, start, token);
                }
            }
            Command::ToggleMapper => {
                self.toggle_mapper();
            }
            Command::Preferences(dark, english) => {
                self.preferences.set((dark, english));
                slint::select_bundled_translation(if english { "en" } else { "ru" }).unwrap();
                self.settings.global::<Theme>().set_dark(dark);
                self.settings.global::<Theme>().invoke_apply();
                self.settings.global::<Locale>().set_english(english);
                self.emoji.global::<Theme>().set_dark(dark);
                self.emoji.global::<Theme>().invoke_apply();
                self.emoji.global::<Locale>().set_english(english);
                self.quick.global::<Theme>().set_dark(dark);
                self.quick.global::<Theme>().invoke_apply();
                self.quick.global::<Locale>().set_english(english);
                self.send_worker(
                    format!(
                        "preferences {} {}",
                        if dark { "dark" } else { "light" },
                        if english { "en" } else { "ru" }
                    ),
                    source,
                    start,
                    None,
                );
            }
            Command::Ping => {}
            Command::Quit => {
                let _ = lhc_core::mapper::runtime::stop();
                let _ = slint::quit_event_loop();
            }
        }
    }

    fn emoji_key(&self, key: &str) {
        self.metrics.borrow_mut().mark("emoji", "t5_first_key");
        let max = if self.emoji.get_page() == 5 { 1500 } else { 48 };
        let index = self.emoji.get_selected();
        if let Ok(page @ 1..=6) = key.parse::<i32>() {
            self.emoji.set_page(page - 1);
            self.emoji.set_selected(0);
        } else if key == slint::SharedString::from(slint::platform::Key::Escape).as_str() {
            self.defer_hide("emoji");
        } else if key == slint::SharedString::from(slint::platform::Key::Return).as_str()
            || key == "\n"
        {
            self.choose_emoji(index);
        } else {
            if let Some(delta) = popup_model::key_delta("emoji", key) {
                self.emoji
                    .set_selected(popup_model::advance(index, delta, max as usize));
                self.metrics
                    .borrow_mut()
                    .mark("emoji", "navigation_handled");
            }
        }
    }

    fn choose_emoji(&self, index: i32) {
        if let Some(value) = popup_model::emoji_index(self.emoji.get_page(), index)
            .and_then(|index| self.emoji.get_emojis().row_data(index % 240))
        {
            log::info!("selected emoji: {value}");
            #[cfg(not(target_os = "linux"))]
            self.return_input.borrow_mut().selected(value.to_string());
        }
        self.defer_hide("emoji");
    }

    fn filter(&self, query: &str) {
        let values = popup_model::filter(&self.actions, query);
        self.quick.set_items(ModelRc::new(VecModel::from(values)));
        self.quick.set_selected(0);
        self.metrics.borrow_mut().mark("quick", "t5_first_key");
    }

    fn choose_quick(&self, index: i32) {
        if let Some(value) = self.quick.get_items().row_data(index as usize) {
            log::info!("selected action (stub): {value}");
            #[cfg(not(target_os = "linux"))]
            self.return_input.borrow_mut().selected(value.to_string());
            self.defer_hide("quick");
        }
    }
}

fn observe(app: &Rc<App>, name: &'static str) {
    let weak = Rc::downgrade(app);
    app.window(name).on_close_requested(move || {
        if let Some(app) = weak.upgrade() {
            app.defer_hide(name);
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let weak = Rc::downgrade(app);
    if let Err(error) = app.window(name).set_rendering_notifier(move |state, _| {
        if matches!(state, slint::RenderingState::AfterRendering)
            && let Some(app) = weak.upgrade()
        {
            app.metrics.borrow_mut().mark(name, "t3_first_frame");
        }
    }) {
        log::warn!("{name} rendering notifier unavailable: {error:?}");
    }
    let weak = Rc::downgrade(app);
    app.window(name).on_winit_window_event(move |_, event| {
        let Some(app) = weak.upgrade() else {
            return slint::winit_030::EventResult::Propagate;
        };
        match event {
            WindowEvent::Focused(true) => app.metrics.borrow_mut().mark(name, "t4_focused"),
            WindowEvent::Focused(false) if name != "settings" => app.defer_hide(name),
            #[cfg(target_os = "linux")]
            WindowEvent::ActivationTokenDone { token, serial } if name == "settings" => {
                let pending = if app
                    .pending
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| p.2 == *serial)
                {
                    app.pending.borrow_mut().take()
                } else {
                    None
                };
                if let Some((target, start, _)) = pending {
                    app.show(target, "button", start, Some(token.clone().into_raw()));
                }
            }
            _ => {}
        }
        slint::winit_030::EventResult::Propagate
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    lhc_core::mapper::set_host(Arc::new(SlintMapperHost));
    #[cfg(target_os = "linux")]
    if let Ok(paths) = app_storage::paths() {
        let _ = paths.ensure();
        lhc_core::mapper::runtime::set_portal_token_dir(paths.data_dir().clone());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(all(feature = "spell", target_os = "linux"))]
    if args == ["--spell-worker"] {
        return spell::run(start);
    }
    if !args.is_empty() {
        return ipc::client(args.join(" "));
    }
    let server = ipc::Server::bind()?;
    let worker = backend::start()?;
    let popup_attributes = Rc::new(std::cell::Cell::new(false));
    select_backend(popup_attributes.clone())?;
    let mut metrics = metrics::Metrics::new(start)?;
    let settings = SettingsWindow::new()?;
    let config = match config_state::ConfigState::load() {
        Ok(config) => {
            settings.set_config_status(
                format!("Конфигурация загружена: {} правил", config.rule_count()).into(),
            );
            Some(Rc::new(RefCell::new(config)))
        }
        Err(error) => {
            log::warn!("configuration unavailable: {error}");
            settings.set_config_status(format!("Конфигурация: {error}").into());
            None
        }
    };
    editor::bind_with_config(&settings, config.clone());
    let mut devices = match lhc_core::mapper::runtime::list_keyboards() {
        Ok(devices) => devices,
        Err(error) => {
            log::warn!("keyboard discovery: {error}");
            Vec::new()
        }
    };
    if let Some(path) = config
        .as_ref()
        .and_then(|config| config.borrow().input_device().map(str::to_owned))
        && !devices.iter().any(|device| device.path == path)
    {
        devices.insert(
            0,
            lhc_core::mapper_types::KeyboardDevice {
                path,
                name: "Сохранённое устройство".into(),
            },
        );
    }
    let selected_device = config
        .as_ref()
        .and_then(|config| config.borrow().input_device().map(str::to_owned))
        .and_then(|path| devices.iter().position(|device| device.path == path))
        .map_or(-1, |index| index as i32);
    settings.set_input_devices(ModelRc::new(VecModel::from(
        devices
            .iter()
            .map(|device| format!("{} · {}", device.name, device.path).into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    settings.set_selected_device(selected_device);
    let device_paths: Vec<String> = devices.into_iter().map(|device| device.path).collect();
    metrics.ready("settings");
    popup_attributes.set(true);
    let emoji = EmojiPopup::new()?;
    let emojis = popup_model::emoji_items();
    emoji.set_emojis(ModelRc::new(VecModel::from(emojis)));
    metrics.ready("emoji");
    let quick = QuickPopup::new()?;
    metrics.ready("quick");
    let actions = popup_model::quick_items();
    let app = Rc::new(App {
        settings,
        emoji,
        quick,
        metrics: RefCell::new(metrics),
        focus: RefCell::new(focus::Activation::default()),
        tray: RefCell::new(None),
        #[cfg(target_os = "linux")]
        pending: RefCell::new(None),
        actions,
        use_spell: worker.is_some(),
        worker: RefCell::new(worker),
        restart_pending: Cell::new(false),
        restart_history: RefCell::new(Vec::new()),
        preferences: Cell::new((true, false)),
        worker_watch: slint::Timer::default(),
        status_watch: slint::Timer::default(),
        last_mapper_status: RefCell::new(None),
        config,
        devices: device_paths,
        #[cfg(not(target_os = "linux"))]
        return_input: RefCell::new(return_input::ReturnInput::default()),
    });
    app.settings.on_preferences(|dark, english| {
        with_app(|app| {
            app.command(
                Command::Preferences(dark, english),
                "button",
                Instant::now(),
                None,
            )
        })
    });
    app.command(
        Command::Preferences(true, false),
        "button",
        Instant::now(),
        None,
    );
    app.filter("");
    app.refresh_mapper_status();
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    app.status_watch
        .start(slint::TimerMode::Repeated, Duration::from_secs(1), || {
            with_app(|app| app.refresh_mapper_status());
        });
    if app.use_spell {
        app.worker_watch
            .start(slint::TimerMode::Repeated, Duration::from_secs(1), || {
                with_app(|app| {
                    let dead = app
                        .worker
                        .borrow_mut()
                        .as_mut()
                        .is_some_and(|worker| !worker.is_alive());
                    if dead {
                        app.report_worker_error(&"worker exited");
                    } else if app.worker.borrow().is_none() && !app.restart_pending.get() {
                        app.restart_worker();
                    }
                });
            });
    }
    for name in ["settings", "emoji", "quick"] {
        observe(&app, name);
    }
    app.settings.on_show_popup(|name| {
        with_app(|app| {
            let target = if name == "emoji" { "emoji" } else { "quick" };
            if app.use_spell {
                app.show(target, "button", Instant::now(), None);
                return;
            }
            #[cfg(not(target_os = "linux"))]
            {
                app.show(target, "button", Instant::now(), None);
                return;
            }
            #[cfg(target_os = "linux")]
            {
                if app.pending.borrow().is_some() {
                    return;
                }
                let start = Instant::now();
                app.hide(target);
                app.metrics.borrow_mut().begin(target, "button", start);
                let requested = app
                    .settings
                    .window()
                    .with_winit_window(|window| window.request_activation_token().ok())
                    .flatten();
                if let Some(serial) = requested {
                    *app.pending.borrow_mut() = Some((target, start, serial));
                    slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
                        with_app(|app| {
                            let pending =
                                if app.pending.borrow().as_ref().is_some_and(|p| p.2 == serial) {
                                    app.pending.borrow_mut().take()
                                } else {
                                    None
                                };
                            if let Some((target, start, _)) = pending {
                                log::warn!("activation token timeout");
                                app.show(target, "button", start, None);
                            }
                        })
                    });
                } else {
                    app.show(target, "button", start, None);
                }
            }
        })
    });
    app.settings.on_toggle_mapper(|| {
        with_app(|app| app.command(Command::ToggleMapper, "button", Instant::now(), None))
    });
    app.settings.on_select_device(|index| {
        with_app(|app| {
            let Some(path) = app.devices.get(index as usize) else {
                return;
            };
            let Some(config) = &app.config else {
                return;
            };
            match config.borrow_mut().save_device(path) {
                Ok(()) => app
                    .settings
                    .set_config_status(format!("Устройство сохранено: {path}").into()),
                Err(error) => app.settings.set_backend_error(error.into()),
            }
        })
    });
    app.emoji.on_key(|key| with_app(|app| app.emoji_key(&key)));
    app.emoji
        .on_choose(|index| with_app(|app| app.choose_emoji(index)));
    app.emoji
        .on_dismiss(|| with_app(|app| app.defer_hide("emoji")));
    app.quick.on_key(|key| {
        with_app(|app| {
            app.metrics.borrow_mut().mark("quick", "t5_first_key");
            let is = |k: slint::platform::Key| key == slint::SharedString::from(k);
            if is(slint::platform::Key::Escape) {
                app.defer_hide("quick");
            } else if is(slint::platform::Key::Return) {
                app.choose_quick(app.quick.get_selected());
            } else {
                if let Some(delta) = popup_model::key_delta("quick", &key) {
                    app.quick.set_selected(popup_model::advance(
                        app.quick.get_selected(),
                        delta,
                        app.quick.get_items().row_count(),
                    ));
                    app.metrics.borrow_mut().mark("quick", "navigation_handled");
                }
            }
        })
    });
    app.quick
        .on_filter(|query| with_app(|app| app.filter(&query)));
    app.quick
        .on_choose(|index| with_app(|app| app.choose_quick(index)));
    app.quick
        .on_dismiss(|| with_app(|app| app.defer_hide("quick")));
    let dispatch: Dispatch = Arc::new(|command, source, start, token| {
        if let Err(error) = slint::invoke_from_event_loop(move || {
            with_app(|app| app.command(command, source, start, token))
        }) {
            log::error!("UI dispatch: {error}");
        }
    });
    server.start(dispatch.clone())?;
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        with_app(|app| match tray::start(dispatch.clone()) {
            Ok(tray) => {
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
    let _ = lhc_core::mapper::runtime::stop();
    APP.with(|slot| slot.borrow_mut().take());
    Ok(())
}

#[cfg(target_os = "linux")]
fn select_backend(popup_attributes: Rc<std::cell::Cell<bool>>) -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attrs| {
            use winit::platform::x11::{WindowAttributesExtX11, WindowType};
            if popup_attributes.get() {
                attrs.with_x11_window_type(vec![WindowType::Utility])
            } else {
                attrs
            }
        })
        .select()
}

#[cfg(not(target_os = "linux"))]
fn select_backend(_: Rc<std::cell::Cell<bool>>) -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()
}
