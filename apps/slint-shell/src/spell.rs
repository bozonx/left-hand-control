//! Spell worker process: both popups as wlr layer-shell surfaces.
//!
//! Runs as `slint-shell --spell-worker`, started and supervised by the
//! settings process. It owns its own IPC socket (`SLINT_SHELL_SOCKET`) and
//! exits when the parent closes its stdin.

use crate::{
    command::{Command, Dispatch, Popup, Preferences, Source},
    ipc, metrics, popup_model,
    ui::{EmojiPopup, Locale, QuickPopup, Theme},
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use spell_framework::{
    SpellAssociatedNew,
    layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf},
    wayland_adapter::SpellWin,
};
use std::{
    sync::{Arc, mpsc},
    time::Instant,
};

const WIDTH: u32 = 520;

enum Event {
    Command(Command, Source, Instant),
    Key(Popup, String),
    Choose(Popup, i32),
    Hide(Popup),
    Filter(String),
    Focus(Popup, bool),
    Frame(Popup),
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

/// The two layer surfaces and their Slint components.
struct Layers {
    emoji_way: SpellWin,
    quick_way: SpellWin,
    emoji: EmojiPopup,
    quick: QuickPopup,
    /// `unmap` lifecycle: hidden layers commit an empty buffer.
    unmap: bool,
    actions: Vec<String>,
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

    fn prepare(&self, popup: Popup) {
        match popup {
            Popup::Emoji => self.emoji.invoke_prepare(),
            Popup::Quick => self.quick.invoke_prepare(),
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
        self.prepare(popup);
        let way = self.way(popup);
        way.add_input_region(0, 0, WIDTH as i32, height(popup) as i32);
        if unmap {
            way.show_again();
        }
        way.grab_focus();
    }

    fn apply(&self, preferences: Preferences) {
        if let Err(error) = slint::select_bundled_translation(preferences.language()) {
            log::error!("select translation: {error}");
        }
        for (theme, locale) in [
            (self.emoji.global::<Theme>(), self.emoji.global::<Locale>()),
            (self.quick.global::<Theme>(), self.quick.global::<Locale>()),
        ] {
            theme.set_dark(preferences.dark);
            theme.invoke_apply();
            locale.set_english(preferences.english);
        }
    }

    fn selected_index(&self, popup: Popup) -> i32 {
        match popup {
            Popup::Emoji => self.emoji.get_selected(),
            Popup::Quick => self.quick.get_selected(),
        }
    }

    fn value(&self, popup: Popup, index: i32) -> Option<String> {
        let value = match popup {
            Popup::Emoji => popup_model::emoji_index(self.emoji.get_page(), index)
                .and_then(|index| self.emoji.get_emojis().row_data(index)),
            Popup::Quick => self
                .quick
                .get_items()
                .row_data(usize::try_from(index).ok()?),
        };
        value.map(|value| value.to_string())
    }

    /// Move the selection; returns `true` when the key was an arrow.
    fn navigate(&self, popup: Popup, key: &str) -> bool {
        let Some(delta) = popup_model::key_delta(popup, key) else {
            return false;
        };
        match popup {
            Popup::Emoji => self.emoji.set_selected(popup_model::advance(
                self.emoji.get_selected(),
                delta,
                popup_model::emoji_cells(self.emoji.get_page()),
            )),
            Popup::Quick => self.quick.set_selected(popup_model::advance(
                self.quick.get_selected(),
                delta,
                self.quick.get_items().row_count(),
            )),
        }
        true
    }

    fn filter(&self, query: &str) {
        self.quick
            .set_items(ModelRc::new(VecModel::from(popup_model::filter(
                &self.actions,
                query,
            ))));
        self.quick.set_selected(0);
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
    let (tx, rx) = mpsc::channel();
    let mut metrics = metrics::Metrics::from_env(start)?;
    let mut emoji_way = SpellWin::invoke_spell("lhc-slint-emoji", configuration(Popup::Emoji)?);
    let emoji = EmojiPopup::new()?;
    emoji.set_presented(initial == Some(Popup::Emoji));
    if initial != Some(Popup::Emoji) {
        start_hidden(&mut emoji_way, Popup::Emoji, unmap);
    }
    emoji.set_emojis(ModelRc::new(VecModel::from(popup_model::emoji_items())));
    metrics.ready("emoji");
    let mut quick_way = SpellWin::invoke_spell("lhc-slint-quick", configuration(Popup::Quick)?);
    let quick = QuickPopup::new()?;
    quick.set_presented(initial == Some(Popup::Quick));
    if initial != Some(Popup::Quick) {
        start_hidden(&mut quick_way, Popup::Quick, unmap);
    }
    let mut layers = Layers {
        emoji_way,
        quick_way,
        emoji,
        quick,
        unmap,
        actions: popup_model::quick_items(),
    };
    layers.filter("");
    metrics.ready("quick");
    for popup in Popup::ALL {
        let sender = tx.clone();
        layers.way(popup).set_event_handler(move |event| {
            use spell_framework::wayland_adapter::WindowEvent;
            let event = match event {
                WindowEvent::Focus(active) => Event::Focus(popup, active),
                WindowEvent::Frame => Event::Frame(popup),
                WindowEvent::Closed => Event::Hide(popup),
            };
            let _ = sender.send(event);
        });
    }
    bind_callbacks(&layers, &tx);
    let sender = tx.clone();
    let dispatch: Dispatch = Arc::new(move |command, source, start, _| {
        let sender = sender.clone();
        if let Err(error) = slint::invoke_from_event_loop(move || {
            let _ = sender.send(Event::Command(command, source, start));
        }) {
            log::error!("Spell dispatch: {error}");
        }
    });
    let parent_sender = tx.clone();
    std::thread::spawn(move || {
        use std::io::Read;
        let _ = std::io::stdin().read(&mut [0u8]);
        let _ = parent_sender.send(Event::Command(Command::Quit, Source::Ipc, Instant::now()));
    });
    let server = ipc::Server::bind()?;
    server.start(dispatch)?;
    let mut return_input = crate::platform::return_input::ReturnInput::new()?;
    let mut pending_return: Option<Popup> = None;
    let mut keyboard_owner: Option<Popup> = None;
    let mut visible = initial;
    let mut focused: Option<Popup> = None;
    let mut navigation_count = 0;
    if let Some(popup) = initial {
        metrics.begin(popup.name(), Source::Diagnostic.as_str(), start);
        layers.prepare(popup);
        layers.way(popup).grab_focus();
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
        while let Ok(event) = rx.try_recv() {
            let mut dismiss = None;
            let mut selected = None;
            match event {
                Event::Command(Command::Preferences(preferences), _, _) => {
                    layers.apply(preferences);
                    // Format parsed by scripts/check-preferences.py.
                    log::info!(
                        "preferences applied: dark={}, english={}, visible={:?}",
                        preferences.dark,
                        preferences.english,
                        visible.map(Popup::name)
                    );
                }
                Event::Command(Command::Quit, _, _) => {
                    for popup in Popup::ALL {
                        layers.way(popup).hide();
                    }
                    return Ok(());
                }
                Event::Command(Command::Show(window), source, start) => {
                    let Some(popup) = window.popup() else {
                        continue;
                    };
                    if let Some(input) = &mut return_input {
                        if visible.is_none() {
                            input.capture();
                        } else {
                            input.cancel();
                        }
                    }
                    if let Some(previous) = pending_return.take() {
                        metrics.end(previous.name());
                    }
                    if let Some(previous) = visible.take() {
                        layers.conceal(previous);
                        metrics.end(previous.name());
                        if previous == popup {
                            continue;
                        }
                    }
                    metrics.begin(popup.name(), source.as_str(), start);
                    navigation_count = 0;
                    focused = None;
                    visible = Some(popup);
                    layers.present(popup);
                    metrics.mark(popup.name(), "t2_shown");
                }
                Event::Command(Command::Hide, _, _) => dismiss = visible,
                Event::Command(_, _, _) => {}
                Event::Focus(popup, active) => {
                    if active {
                        keyboard_owner = Some(popup);
                    } else if keyboard_owner == Some(popup) {
                        keyboard_owner = None;
                    }
                    if !active && pending_return == Some(popup) {
                        metrics.mark(popup.name(), "keyboard_released");
                    }
                    if visible == Some(popup) {
                        if active {
                            focused = Some(popup);
                            metrics.mark(popup.name(), "t4_focused");
                        } else if focused == Some(popup) {
                            focused = None;
                            dismiss = Some(popup);
                        } else {
                            metrics.mark(popup.name(), "focus_left_before_enter");
                        }
                    }
                }
                Event::Frame(popup) => metrics.mark(popup.name(), "t3_first_frame"),
                Event::Hide(popup) => dismiss = Some(popup),
                Event::Choose(popup, index) if visible == Some(popup) => {
                    let Some(value) = layers.value(popup, index) else {
                        continue;
                    };
                    log::info!("Spell selected {} index={index}: {value}", popup.name());
                    metrics.mark(popup.name(), "selected");
                    selected = Some(value);
                    dismiss = Some(popup);
                }
                Event::Choose(_, _) => {}
                Event::Filter(query) => {
                    if query
                        .chars()
                        .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c))
                    {
                        metrics.mark("quick", "filter_cyrillic");
                    }
                    layers.filter(&query);
                    metrics.mark("quick", "t5_first_key");
                    metrics.mark("quick", "filter_changed");
                }
                Event::Key(popup, key) if visible == Some(popup) => {
                    metrics.mark(popup.name(), "t5_first_key");
                    if popup_model::is_key(&key, slint::platform::Key::Escape) {
                        dismiss = Some(popup);
                    } else if popup_model::is_enter(&key) {
                        let _ = tx.send(Event::Choose(popup, layers.selected_index(popup)));
                    } else if popup == Popup::Emoji
                        && let Ok(page @ 1..=6) = key.parse::<i32>()
                    {
                        metrics.mark(popup.name(), "page_changed");
                        if page - 1 == popup_model::STRESS_PAGE {
                            metrics.mark(popup.name(), "stress_page");
                        }
                        layers.emoji.set_page(page - 1);
                        layers.emoji.set_selected(0);
                    } else if layers.navigate(popup, &key) {
                        metrics.mark(popup.name(), "navigation_handled");
                        navigation_count += 1;
                        if navigation_count > 1 {
                            metrics.mark(popup.name(), "navigation_repeated");
                        }
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
                if let (Some(value), Some(input)) = (selected, &mut return_input) {
                    metrics.mark(popup.name(), "hidden");
                    pending_return = Some(popup);
                    if !input.selected(value) {
                        metrics.mark(popup.name(), "return_target_missing");
                        metrics.end(popup.name());
                        pending_return = None;
                    }
                } else {
                    if let Some(input) = &mut return_input {
                        input.cancel();
                    }
                    metrics.end(popup.name());
                }
            }
        }
        if let (Some(popup), Some(input)) = (pending_return, &mut return_input)
            && let Some(event) = input.poll(keyboard_owner.is_none())
        {
            metrics.mark(popup.name(), event);
            metrics.end(popup.name());
            pending_return = None;
        }
    }
}

fn bind_callbacks(layers: &Layers, tx: &mpsc::Sender<Event>) {
    let sender = tx.clone();
    layers.emoji.on_key(move |key| {
        let _ = sender.send(Event::Key(Popup::Emoji, key.into()));
    });
    let sender = tx.clone();
    layers.emoji.on_choose(move |index| {
        let _ = sender.send(Event::Choose(Popup::Emoji, index));
    });
    let sender = tx.clone();
    layers.emoji.on_dismiss(move || {
        let _ = sender.send(Event::Hide(Popup::Emoji));
    });
    let sender = tx.clone();
    layers.quick.on_key(move |key| {
        let _ = sender.send(Event::Key(Popup::Quick, key.into()));
    });
    let sender = tx.clone();
    layers.quick.on_choose(move |index| {
        let _ = sender.send(Event::Choose(Popup::Quick, index));
    });
    let sender = tx.clone();
    layers.quick.on_dismiss(move || {
        let _ = sender.send(Event::Hide(Popup::Quick));
    });
    let sender = tx.clone();
    layers.quick.on_filter(move |query| {
        let _ = sender.send(Event::Filter(query.into()));
    });
}
