mod backend;
mod editor;
mod focus;
mod hotkey;
mod ipc;
mod metrics;
#[cfg(feature = "spell")]
mod return_input;
#[cfg(feature = "spell")]
mod spell;
#[cfg(feature = "spell")]
mod test_keyboard;
mod tray;

use slint::winit_030::{WinitWindowAccessor, winit};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Instant};
use winit::{event::WindowEvent, platform::startup_notify::WindowExtStartupNotify};

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

struct App {
    settings: SettingsWindow,
    emoji: EmojiPopup,
    quick: QuickPopup,
    metrics: RefCell<metrics::Metrics>,
    focus: RefCell<focus::Activation>,
    tray: RefCell<Option<ksni::blocking::Handle<tray::Tray>>>,
    pending: RefCell<Option<(&'static str, Instant, winit::event_loop::AsyncRequestSerial)>>,
    actions: Vec<String>,
    worker: Option<backend::Worker>,
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
    fn report_worker_error(&self, error: &dyn std::fmt::Display) {
        let message = format!("Spell popup process unavailable: {error}");
        log::error!("{message}");
        self.settings.set_backend_error(message.into());
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
        if name != "settings"
            && let Some(worker) = &self.worker
        {
            if let Err(error) = worker.send(format!("show {name}"), source, start, token) {
                self.report_worker_error(error.as_ref());
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
                if let Some(worker) = &self.worker
                    && let Err(error) = worker.send("hide".into(), source, start, token)
                {
                    self.report_worker_error(error.as_ref());
                }
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
                if let Some(tray) = self.tray.borrow().as_ref() {
                    tray.update(|tray| tray.enabled = !tray.enabled);
                }
            }
            Command::Preferences(dark, english) => {
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
                if let Some(worker) = &self.worker
                    && let Err(error) = worker.send(
                        format!(
                            "preferences {} {}",
                            if dark { "dark" } else { "light" },
                            if english { "en" } else { "ru" }
                        ),
                        source,
                        start,
                        None,
                    )
                {
                    self.report_worker_error(error.as_ref());
                }
            }
            Command::Ping => {}
            Command::Quit => {
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
            let delta = if key
                == slint::SharedString::from(slint::platform::Key::LeftArrow).as_str()
            {
                -1
            } else if key == slint::SharedString::from(slint::platform::Key::RightArrow).as_str() {
                1
            } else if key == slint::SharedString::from(slint::platform::Key::UpArrow).as_str() {
                -8
            } else if key == slint::SharedString::from(slint::platform::Key::DownArrow).as_str() {
                8
            } else {
                0
            };
            if delta != 0 {
                self.emoji.set_selected((index + delta).rem_euclid(max));
                self.metrics
                    .borrow_mut()
                    .mark("emoji", "navigation_handled");
            }
        }
    }

    fn choose_emoji(&self, index: i32) {
        let offset = if self.emoji.get_page() == 5 {
            0
        } else {
            self.emoji.get_page() * 48
        };
        if let Some(value) = self
            .emoji
            .get_emojis()
            .row_data(((offset + index) % 240) as usize)
        {
            log::info!("selected emoji: {value}");
        }
        self.defer_hide("emoji");
    }

    fn filter(&self, query: &str) {
        let query = query.to_lowercase();
        let values: Vec<slint::SharedString> = self
            .actions
            .iter()
            .filter(|s| s.to_lowercase().contains(&query))
            .map(|s| s.into())
            .collect();
        self.quick.set_items(ModelRc::new(VecModel::from(values)));
        self.quick.set_selected(0);
        self.metrics.borrow_mut().mark("quick", "t5_first_key");
    }

    fn choose_quick(&self, index: i32) {
        if let Some(value) = self.quick.get_items().row_data(index as usize) {
            log::info!("selected action (stub): {value}");
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
    let args: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(feature = "spell")]
    if args == ["--spell-worker"] {
        return spell::run(start);
    }
    if !args.is_empty() {
        return ipc::client(args.join(" "));
    }
    let server = ipc::Server::bind()?;
    let worker = backend::start()?;
    let popup_attributes = Rc::new(std::cell::Cell::new(false));
    let hook_popup = popup_attributes.clone();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attrs| {
            use winit::platform::x11::{WindowAttributesExtX11, WindowType};
            if hook_popup.get() {
                attrs.with_x11_window_type(vec![WindowType::Utility])
            } else {
                attrs
            }
        })
        .select()?;
    let mut metrics = metrics::Metrics::new(start)?;
    let settings = SettingsWindow::new()?;
    editor::bind(&settings);
    metrics.ready("settings");
    popup_attributes.set(true);
    let emoji = EmojiPopup::new()?;
    let emojis: Vec<slint::SharedString> = (0x1f600..=0x1f64f)
        .chain(0x1f300..=0x1f5ff)
        .filter_map(char::from_u32)
        .take(240)
        .map(|c| c.to_string().into())
        .collect();
    emoji.set_emojis(ModelRc::new(VecModel::from(emojis)));
    metrics.ready("emoji");
    let quick = QuickPopup::new()?;
    metrics.ready("quick");
    let actions = (1..=30)
        .map(|i| format!("Действие {i:02} / Action {i:02}"))
        .collect();
    let app = Rc::new(App {
        settings,
        emoji,
        quick,
        metrics: RefCell::new(metrics),
        focus: RefCell::new(focus::Activation::default()),
        tray: RefCell::new(None),
        pending: RefCell::new(None),
        actions,
        worker,
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
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    for name in ["settings", "emoji", "quick"] {
        observe(&app, name);
    }
    app.settings.on_show_popup(|name| {
        with_app(|app| {
            let target = if name == "emoji" { "emoji" } else { "quick" };
            if app.worker.is_some() {
                app.show(target, "button", Instant::now(), None);
                return;
            }
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
        })
    });
    app.settings.on_toggle_mapper(|| {
        with_app(|app| app.command(Command::ToggleMapper, "button", Instant::now(), None))
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
                let count = app.quick.get_items().row_count() as i32;
                let delta = if is(slint::platform::Key::UpArrow) {
                    -1
                } else {
                    1
                };
                if count > 0 {
                    app.quick
                        .set_selected((app.quick.get_selected() + delta).rem_euclid(count));
                }
                app.metrics.borrow_mut().mark("quick", "navigation_handled");
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
    match tray::start(dispatch.clone()) {
        Ok(tray) => {
            *app.tray.borrow_mut() = Some(tray);
            app.metrics.borrow_mut().ready("tray");
        }
        Err(error) => log::error!("tray unavailable: {error}"),
    }
    server.start(dispatch.clone())?;
    hotkey::start(dispatch);
    log::info!("ready; backend={:?}", std::env::var("SLINT_BACKEND"));
    slint::run_event_loop_until_quit()?;
    APP.with(|slot| slot.borrow_mut().take());
    Ok(())
}
