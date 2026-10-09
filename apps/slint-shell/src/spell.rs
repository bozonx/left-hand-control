//! Spell worker process: both popups as wlr layer-shell surfaces.
//!
//! Runs as `slint-shell --spell-worker`, started and supervised by the
//! settings process. It owns its own IPC socket (`SLINT_SHELL_SOCKET`),
//! sends chosen actions back to the parent (`SLINT_SHELL_PARENT_SOCKET`)
//! and exits when the parent closes its stdin.

use crate::{
    command::{Command, Dispatch, Popup, Preferences, Source, ThemeMode, Window},
    i18n::Language,
    ipc, metrics,
    popup_model::{self, ConfiguredMenus, KeyOutcome},
    ui::{EmojiPopup, Locale, QuickPopup, Theme},
};
use slint::ComponentHandle;
use spell_framework::{
    SpellAssociatedNew,
    layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf},
    wayland_adapter::SpellWin,
};
use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

const WIDTH: u32 = 520;
/// Poll interval while a popup is visible or was just used: pending
/// Wayland requests are flushed by the next dispatch.
const ACTIVE_POLL: Duration = Duration::from_millis(4);
/// How long the loop stays responsive after the last activity.
const ACTIVE_GRACE: Duration = Duration::from_millis(250);
/// Idle wake-up while everything is hidden; IPC wakes the loop at once.
const IDLE_POLL: Duration = Duration::from_millis(500);
/// Run a chosen action at the latest this long after the popup hid, even
/// if the compositor did not report the keyboard focus leaving it.
const FOCUS_RETURN_TIMEOUT: Duration = Duration::from_millis(300);

enum Event {
    Command(Command, Source, Instant),
    Key(Popup, String),
    Shortcut(Popup, popup_model::Shortcut),
    Choose(Popup, i32),
    Action(Popup, i32, String),
    Hide(Popup),
    Filter,
    Focus(Popup, bool),
    Frame(Popup),
}

/// Sends events to the loop from any thread and wakes it.
#[derive(Clone)]
struct Waker {
    sender: mpsc::Sender<Event>,
    pipe: Arc<UnixStream>,
}

impl Waker {
    fn send(&self, event: Event) {
        if self.sender.send(event).is_ok() {
            let _ = (&*self.pipe).write(&[0]);
        }
    }
}

fn height(popup: Popup) -> u32 {
    match popup {
        Popup::Emoji => 460,
        Popup::Quick => 500,
    }
}

fn configuration(popup: Popup) -> Result<WindowConf, Box<dyn std::error::Error>> {
    let mut builder = WindowConf::builder();
    builder
        .width(WIDTH)
        .height(height(popup))
        .anchor_1(LayerAnchor::BOTTOM)
        .margins(0, 0, 24, 0)
        .layer_type(LayerType::Overlay)
        .exclusive_zone(0)
        .board_interactivity(BoardType::None);
    if let Ok(monitor) = std::env::var("SLINT_SHELL_OUTPUT") {
        builder.monitor(monitor);
    }
    builder.build()
}

/// The two layer surfaces, their Slint components and the menus they show.
struct Layers {
    emoji_way: SpellWin,
    quick_way: SpellWin,
    emoji: EmojiPopup,
    quick: QuickPopup,
    /// `unmap` lifecycle: hidden layers commit an empty buffer.
    unmap: bool,
    menus: ConfiguredMenus,
    /// Actions of the quick items in display order.
    quick_actions: Vec<String>,
}

impl Layers {
    fn way(&mut self, popup: Popup) -> &mut SpellWin {
        match popup {
            Popup::Emoji => &mut self.emoji_way,
            Popup::Quick => &mut self.quick_way,
        }
    }

    fn set_presented(&self, popup: Popup, presented: bool) {
        match popup {
            Popup::Emoji => self.emoji.set_presented(presented),
            Popup::Quick => self.quick.set_presented(presented),
        }
    }

    fn conceal(&mut self, popup: Popup) {
        let unmap = self.unmap;
        self.set_presented(popup, false);
        let way = self.way(popup);
        way.remove_focus();
        way.subtract_input_region(0, 0, WIDTH as i32, height(popup) as i32);
        if unmap {
            way.hide();
        }
    }

    fn present(&mut self, popup: Popup) {
        let unmap = self.unmap;
        self.set_presented(popup, true);
        match popup {
            Popup::Emoji => self.emoji.invoke_prepare(),
            Popup::Quick => self.quick.invoke_prepare(),
        }
        let way = self.way(popup);
        way.add_input_region(0, 0, WIDTH as i32, height(popup) as i32);
        if unmap {
            way.show_again();
        }
        way.grab_focus();
    }

    fn apply(&self, preferences: Preferences) {
        preferences.language.select_bundled();
        for (theme, locale) in [
            (self.emoji.global::<Theme>(), self.emoji.global::<Locale>()),
            (self.quick.global::<Theme>(), self.quick.global::<Locale>()),
        ] {
            theme.set_dark(preferences.theme == ThemeMode::Dark);
            theme.set_eink(preferences.theme == ThemeMode::EInk);
            crate::ui::apply_theme(&theme);
            locale.set_english(preferences.language == Language::English);
        }
    }

    fn set_menus(&mut self, menus: ConfiguredMenus) {
        menus.apply_emoji(&self.emoji);
        menus.apply_quick(&self.quick);
        self.menus = menus;
        self.filter();
    }

    fn filter(&mut self) {
        self.quick_actions = self.menus.filter_quick(&self.quick);
    }

    /// Action of item `index` of `popup`.
    fn action(&self, popup: Popup, index: i32) -> Option<String> {
        match popup {
            Popup::Emoji => self
                .menus
                .emoji(&self.emoji, index)
                .map(|emoji| format!("text:{emoji}")),
            Popup::Quick => usize::try_from(index)
                .ok()
                .and_then(|index| self.quick_actions.get(index).cloned()),
        }
    }
}

/// Initial state: hidden without removing focus that was never granted.
fn start_hidden(way: &mut SpellWin, popup: Popup, unmap: bool) {
    way.subtract_input_region(0, 0, WIDTH as i32, height(popup) as i32);
    if unmap {
        way.hide();
    }
}

fn env_choice(
    name: &str,
    default: &'static str,
    allowed: &[&'static str],
) -> Result<&'static str, String> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(value) => allowed
            .iter()
            .copied()
            .find(|allowed| *allowed == value)
            .ok_or_else(|| format!("{name} must be one of: {}", allowed.join(", "))),
    }
}

/// Run `action` in the settings process, which owns the mapper.
fn execute(action: String) {
    let parent =
        std::env::var("SLINT_SHELL_PARENT_SOCKET").unwrap_or_else(|_| ipc::DEFAULT_SOCKET.into());
    std::thread::spawn(move || {
        if let Err(error) = ipc::send(
            &parent,
            &Command::Execute(action),
            Source::Ipc,
            Instant::now(),
            None,
        ) {
            log::error!("execute popup action: {error}");
        }
    });
}

/// Block until a Wayland connection or the waker has input, or `timeout`.
fn wait(fds: &[RawFd], timeout: Duration) {
    let mut polls: Vec<libc::pollfd> = fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    let timeout = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: `polls` is a valid, exclusively borrowed array of pollfd.
    let result = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, timeout) };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            log::warn!("poll: {error}");
        }
    }
}

/// A chosen action waiting for the keyboard focus to return.
struct PendingAction {
    popup: Popup,
    action: String,
    hidden: Instant,
}

pub fn run(start: Instant) -> Result<(), Box<dyn std::error::Error>> {
    if !crate::platform::backend::layer_shell_available()? {
        return Err("layer-shell unavailable".into());
    }
    let unmap = env_choice(
        "SLINT_SHELL_SPELL_LIFECYCLE",
        "unmap",
        &["unmap", "transparent"],
    )? == "unmap";
    let initial = match env_choice("SLINT_SHELL_SPELL_INITIAL", "", &["emoji", "quick"])? {
        "emoji" => Some(Popup::Emoji),
        "quick" => Some(Popup::Quick),
        _ => None,
    };
    let (sender, rx) = mpsc::channel();
    let (wake_read, wake_write) = UnixStream::pair()?;
    wake_read.set_nonblocking(true)?;
    wake_write.set_nonblocking(true)?;
    let waker = Waker {
        sender,
        pipe: Arc::new(wake_write),
    };
    let mut metrics = metrics::Metrics::from_env(start)?;
    let mut emoji_way = SpellWin::invoke_spell("lhc-slint-emoji", configuration(Popup::Emoji)?);
    let emoji = EmojiPopup::new()?;
    emoji.set_presented(initial == Some(Popup::Emoji));
    if initial != Some(Popup::Emoji) {
        start_hidden(&mut emoji_way, Popup::Emoji, unmap);
    }
    metrics.ready("emoji");
    let mut quick_way = SpellWin::invoke_spell("lhc-slint-quick", configuration(Popup::Quick)?);
    let quick = QuickPopup::new()?;
    quick.set_presented(initial == Some(Popup::Quick));
    if initial != Some(Popup::Quick) {
        start_hidden(&mut quick_way, Popup::Quick, unmap);
    }
    let wayland = [emoji_way.get_fd_owned(), quick_way.get_fd_owned()];
    let fds = [
        wayland[0].as_raw_fd(),
        wayland[1].as_raw_fd(),
        wake_read.as_raw_fd(),
    ];
    let mut layers = Layers {
        emoji_way,
        quick_way,
        emoji,
        quick,
        unmap,
        menus: ConfiguredMenus::default(),
        quick_actions: Vec::new(),
    };
    layers.filter();
    metrics.ready("quick");
    for popup in Popup::ALL {
        let keys = waker.clone();
        let quick = layers.quick.as_weak();
        layers
            .way(popup)
            .set_key_handler(move |code, shift, control| {
                let searching = quick.upgrade().is_some_and(|quick| quick.get_searching());
                if let Some(shortcut) =
                    popup_model::evdev_shortcut(popup, code, shift, control, searching)
                {
                    keys.send(Event::Shortcut(popup, shortcut));
                    true
                } else {
                    false
                }
            });
        let waker = waker.clone();
        layers.way(popup).set_event_handler(move |event| {
            use spell_framework::wayland_adapter::WindowEvent;
            waker.send(match event {
                WindowEvent::Focus(active) => Event::Focus(popup, active),
                WindowEvent::Frame => Event::Frame(popup),
                WindowEvent::Closed => Event::Hide(popup),
            });
        });
    }
    bind_callbacks(&layers, &waker);
    let ipc_waker = waker.clone();
    let dispatch: Dispatch = Arc::new(move |command, source, start, _| {
        ipc_waker.send(Event::Command(command, source, start));
    });
    let stdin_waker = waker.clone();
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0u8]);
        stdin_waker.send(Event::Command(Command::Quit, Source::Ipc, Instant::now()));
    });
    let server = ipc::Server::bind()?;
    server.start(dispatch)?;
    #[cfg(feature = "probes")]
    let mut probe = crate::platform::return_input::ReturnInput::new()?;
    let mut pending: Option<PendingAction> = None;
    let mut visible = initial;
    let mut focused: Option<Popup> = None;
    let mut last_activity = Instant::now();
    // Navigation keys handled since the popup opened (metrics only).
    let mut navigation_count = 0;
    if let Some(popup) = initial {
        metrics.begin(popup.name(), Source::Diagnostic.as_str(), start);
        layers.present(popup);
        metrics.mark(popup.name(), "t2_shown");
    }
    log::info!(
        "Spell ready: overlay, bottom center, margin=24, exclusive_zone=0, keyboard=exclusive, output={:?}",
        std::env::var("SLINT_SHELL_OUTPUT")
    );
    loop {
        for popup in Popup::ALL {
            layers.way(popup).on_call()?;
        }
        let mut active = false;
        while let Ok(event) = rx.try_recv() {
            active = true;
            let mut dismiss = None;
            match event {
                Event::Command(Command::Preferences(preferences), _, _) => {
                    layers.apply(preferences);
                    // Format parsed by scripts/check-preferences.py.
                    log::info!(
                        "preferences applied: dark={}, english={}, visible={:?}",
                        preferences.theme == ThemeMode::Dark,
                        preferences.language == Language::English,
                        visible.map(Popup::name)
                    );
                }
                Event::Command(Command::Quit, _, _) => {
                    for popup in Popup::ALL {
                        layers.way(popup).hide();
                    }
                    return Ok(());
                }
                Event::Command(Command::PopupContents(contents), _, _) => {
                    match lhc_core::profile::layout_file::parse(&contents) {
                        Ok(layout) => layers.set_menus(ConfiguredMenus {
                            layout: layout.unwrap_or_default(),
                        }),
                        Err(error) => log::error!("popup configuration: {error}"),
                    }
                }
                Event::Command(Command::PopupLayout(id), _, _) => {
                    let menus = ConfiguredMenus::load_for(id.as_deref()).unwrap_or_else(|error| {
                        log::error!("load popup configuration: {error}");
                        ConfiguredMenus::default()
                    });
                    layers.set_menus(menus);
                }
                Event::Command(
                    command @ (Command::Show(Window::Popup(_)) | Command::ShowPage(_, _)),
                    source,
                    start,
                ) => {
                    let (popup, page) = match command {
                        Command::ShowPage(popup, page) => (popup, Some(page)),
                        Command::Show(window) => (window.popup().unwrap_or(Popup::Emoji), None),
                        _ => continue,
                    };
                    #[cfg(feature = "probes")]
                    if let Some(probe) = &mut probe
                        && visible.is_none()
                    {
                        probe.capture();
                    }
                    navigation_count = 0;
                    show(
                        &mut layers,
                        &mut visible,
                        &mut focused,
                        &mut metrics,
                        popup,
                        page,
                        source,
                        start,
                    );
                }
                Event::Command(Command::Hide, _, _) => dismiss = visible,
                Event::Command(_, _, _) => {}
                Event::Focus(popup, active) => {
                    if !active && pending.as_ref().is_some_and(|p| p.popup == popup) {
                        metrics.mark(popup.name(), "keyboard_released");
                        if let Some(action) = pending.take() {
                            execute(action.action);
                        }
                    }
                    if visible == Some(popup) {
                        if active {
                            focused = Some(popup);
                            metrics.mark(popup.name(), "t4_focused");
                        } else if focused == Some(popup) {
                            dismiss = Some(popup);
                        } else {
                            metrics.mark(popup.name(), "focus_left_before_enter");
                        }
                    }
                }
                Event::Frame(popup) => metrics.mark(popup.name(), "t3_first_frame"),
                Event::Hide(popup) => dismiss = Some(popup),
                Event::Choose(popup, index) if visible == Some(popup) => {
                    if let Some(action) = layers.action(popup, index) {
                        waker.send(Event::Action(popup, index, action));
                    }
                }
                Event::Choose(_, _) => {}
                Event::Action(popup, index, action) if visible == Some(popup) => {
                    log::info!("Spell selected {} index={index}: {action}", popup.name());
                    metrics.mark(popup.name(), "selected");
                    // Diagnostic stand: the probe returns focus and pastes itself.
                    #[cfg(feature = "probes")]
                    let probed = probe.as_mut().is_some_and(|probe| {
                        probe.selected(action.strip_prefix("text:").unwrap_or(&action).to_owned())
                    });
                    #[cfg(not(feature = "probes"))]
                    let probed = false;
                    if !probed {
                        pending = Some(PendingAction {
                            popup,
                            action,
                            hidden: Instant::now(),
                        });
                    }
                    dismiss = Some(popup);
                }
                Event::Action(_, _, _) => {}
                Event::Filter => {
                    if layers
                        .quick
                        .get_query()
                        .chars()
                        .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c))
                    {
                        metrics.mark("quick", "filter_cyrillic");
                    }
                    layers.filter();
                    metrics.mark("quick", "filter_changed");
                }
                Event::Shortcut(popup, shortcut) if visible == Some(popup) => {
                    metrics.mark(popup.name(), "t5_first_key");
                    match popup_model::apply_shortcut(popup, shortcut, &layers.emoji, &layers.quick)
                    {
                        KeyOutcome::ChooseCell(index) => {
                            let action = if popup == Popup::Emoji {
                                layers.action(popup, index)
                            } else {
                                layers.menus.quick_cell(layers.quick.get_page(), index)
                            };
                            if let Some(action) = action {
                                waker.send(Event::Action(popup, index, action));
                            }
                        }
                        KeyOutcome::PageChanged => {
                            layers.filter();
                            metrics.mark(popup.name(), "navigation_handled");
                            navigation_count += 1;
                            if navigation_count > 1 {
                                metrics.mark(popup.name(), "navigation_repeated");
                            }
                        }
                        _ => {}
                    }
                }
                Event::Shortcut(_, _) => {}
                Event::Key(popup, key) if visible == Some(popup) => {
                    metrics.mark(popup.name(), "t5_first_key");
                    let outcome = match popup {
                        Popup::Emoji => popup_model::emoji_key(&layers.emoji, &key),
                        Popup::Quick => popup_model::quick_key(&layers.quick, &key),
                    };
                    match outcome {
                        KeyOutcome::Dismiss => dismiss = Some(popup),
                        KeyOutcome::Choose(index) => waker.send(Event::Choose(popup, index)),
                        KeyOutcome::Moved => {
                            metrics.mark(popup.name(), "navigation_handled");
                            navigation_count += 1;
                            if navigation_count > 1 {
                                metrics.mark(popup.name(), "navigation_repeated");
                            }
                        }
                        KeyOutcome::Ignored
                        | KeyOutcome::ChooseCell(_)
                        | KeyOutcome::PageChanged => {}
                    }
                }
                Event::Key(_, _) => {}
            }
            if let Some(popup) = dismiss
                && visible == Some(popup)
            {
                layers.conceal(popup);
                visible = None;
                focused = None;
                metrics.mark(popup.name(), "hidden");
                metrics.end(popup.name());
            }
        }
        if let Some(action) = pending.take_if(|p| p.hidden.elapsed() >= FOCUS_RETURN_TIMEOUT) {
            metrics.mark(action.popup.name(), "focus_return_timeout");
            execute(action.action);
        }
        #[cfg(feature = "probes")]
        if let Some(probe) = &mut probe
            && let Some(event) = probe.poll(focused.is_none())
        {
            log::info!("probe: {event}");
            for popup in Popup::ALL {
                metrics.mark(popup.name(), event);
            }
        }
        if active {
            last_activity = Instant::now();
            continue;
        }
        let busy = visible.is_some() || pending.is_some() || last_activity.elapsed() < ACTIVE_GRACE;
        let timeout = slint::platform::duration_until_next_timer_update()
            .unwrap_or(Duration::MAX)
            .min(if busy { ACTIVE_POLL } else { IDLE_POLL });
        wait(&fds, timeout);
        while (&wake_read).read(&mut [0u8; 64]).is_ok_and(|n| n > 0) {}
    }
}

#[allow(clippy::too_many_arguments)]
fn show(
    layers: &mut Layers,
    visible: &mut Option<Popup>,
    focused: &mut Option<Popup>,
    metrics: &mut metrics::Metrics,
    popup: Popup,
    page: Option<u8>,
    source: Source,
    start: Instant,
) {
    layers.quick.set_query("".into());
    layers.quick.set_searching(false);
    layers.filter();
    if let Some(page) = page {
        popup_model::select_page(popup, page, &layers.emoji, &layers.quick);
        layers.filter();
    }
    if let Some(previous) = visible.take() {
        layers.conceal(previous);
        metrics.end(previous.name());
        if previous == popup && page.is_none() {
            return;
        }
    }
    metrics.begin(popup.name(), source.as_str(), start);
    *focused = None;
    *visible = Some(popup);
    layers.present(popup);
    metrics.mark(popup.name(), "t2_shown");
}

fn bind_callbacks(layers: &Layers, waker: &Waker) {
    let w = waker.clone();
    layers
        .emoji
        .on_key(move |key| w.send(Event::Key(Popup::Emoji, key.into())));
    let w = waker.clone();
    layers
        .emoji
        .on_choose(move |index| w.send(Event::Choose(Popup::Emoji, index)));
    let w = waker.clone();
    layers
        .emoji
        .on_dismiss(move || w.send(Event::Hide(Popup::Emoji)));
    let w = waker.clone();
    layers
        .quick
        .on_key(move |key| w.send(Event::Key(Popup::Quick, key.into())));
    let w = waker.clone();
    layers
        .quick
        .on_choose(move |index| w.send(Event::Choose(Popup::Quick, index)));
    let w = waker.clone();
    layers
        .quick
        .on_dismiss(move || w.send(Event::Hide(Popup::Quick)));
    let w = waker.clone();
    layers.quick.on_change_page(move |_| w.send(Event::Filter));
    let w = waker.clone();
    layers.quick.on_filter(move |_| w.send(Event::Filter));
}
