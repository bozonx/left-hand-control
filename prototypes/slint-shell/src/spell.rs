use crate::{Command, Dispatch, EmojiPopup, QuickPopup, ipc, metrics};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use spell_framework::{
    SpellAssociatedNew,
    layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf},
    wayland_adapter::SpellWin,
};
use std::{
    cell::Cell,
    sync::{Arc, mpsc},
    time::Instant,
};
use tracing_subscriber::prelude::*;

thread_local! { static CURRENT: Cell<&'static str> = const { Cell::new("emoji") }; }

enum Event {
    Command(Command, &'static str, Instant),
    Key(&'static str, String),
    Choose(&'static str, i32),
    Hide(&'static str),
    Filter(String),
    Focus(&'static str, bool),
    Frame(&'static str),
}

struct FocusEvents(mpsc::Sender<Event>);
impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for FocusEvents {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        struct Message(String);
        impl tracing::field::Visit for Message {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        if !event
            .metadata()
            .target()
            .starts_with("spell_framework::wayland_adapter::window::input")
        {
            return;
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        let focused = match message.0.as_str() {
            "Keyboard focus entered" => true,
            "Keyboard focus left" => false,
            _ => return,
        };
        let _ = self.0.send(Event::Focus(CURRENT.with(Cell::get), focused));
    }
}

fn configuration(height: u32) -> Result<WindowConf, Box<dyn std::error::Error>> {
    let mut builder = WindowConf::builder();
    builder
        .width(520_u32)
        .height(height)
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

pub fn run(start: Instant) -> Result<(), Box<dyn std::error::Error>> {
    if !crate::backend::layer_shell_available()? {
        return Err("layer-shell unavailable".into());
    }
    let unmap = match std::env::var("SLINT_SHELL_SPELL_LIFECYCLE").as_deref() {
        Ok("unmap") => true,
        Ok("transparent") | Err(_) => false,
        _ => return Err("SLINT_SHELL_SPELL_LIFECYCLE must be transparent or unmap".into()),
    };
    let initial = match std::env::var("SLINT_SHELL_SPELL_INITIAL").as_deref() {
        Ok("emoji") => Some("emoji"),
        Ok("quick") => Some("quick"),
        Err(_) => None,
        _ => return Err("SLINT_SHELL_SPELL_INITIAL must be emoji or quick".into()),
    };
    let (tx, rx) = mpsc::channel();
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(FocusEvents(tx.clone()))
            .with(
                tracing_subscriber::fmt::layer()
                    .with_filter(tracing_subscriber::filter::LevelFilter::WARN),
            ),
    )?;
    let mut metrics = metrics::Metrics::new(start)?;
    CURRENT.with(|c| c.set("emoji"));
    let mut emoji_way = SpellWin::invoke_spell("lhc-slint-emoji", configuration(460)?);
    let emoji = EmojiPopup::new()?;
    emoji.set_presented(initial == Some("emoji"));
    if initial != Some("emoji") {
        emoji_way.subtract_input_region(0, 0, 520, 460);
        if unmap {
            emoji_way.hide();
        }
    }
    emoji.set_emojis(ModelRc::new(VecModel::from(
        (0x1f600..=0x1f64f)
            .chain(0x1f300..=0x1f5ff)
            .filter_map(char::from_u32)
            .take(240)
            .map(|c| c.to_string().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    metrics.ready("emoji");
    CURRENT.with(|c| c.set("quick"));
    let mut quick_way = SpellWin::invoke_spell("lhc-slint-quick", configuration(500)?);
    let quick = QuickPopup::new()?;
    quick.set_presented(initial == Some("quick"));
    if initial != Some("quick") {
        quick_way.subtract_input_region(0, 0, 520, 500);
        if unmap {
            quick_way.hide();
        }
    }
    let actions: Vec<slint::SharedString> = (1..=30)
        .map(|i| format!("Действие {i:02} / Action {i:02}").into())
        .collect();
    quick.set_items(ModelRc::new(VecModel::from(actions.clone())));
    metrics.ready("quick");
    for (name, window) in [("emoji", emoji.window()), ("quick", quick.window())] {
        let sender = tx.clone();
        if let Err(error) = window.set_rendering_notifier(move |state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                let _ = sender.send(Event::Frame(name));
            }
        }) {
            log::warn!("Spell {name}: t3 unavailable: {error:?}");
        }
    }
    let sender = tx.clone();
    emoji.on_key(move |key| {
        let _ = sender.send(Event::Key("emoji", key.into()));
    });
    let sender = tx.clone();
    emoji.on_choose(move |index| {
        let _ = sender.send(Event::Choose("emoji", index));
    });
    let sender = tx.clone();
    emoji.on_dismiss(move || {
        let _ = sender.send(Event::Hide("emoji"));
    });
    let sender = tx.clone();
    quick.on_key(move |key| {
        let _ = sender.send(Event::Key("quick", key.into()));
    });
    let sender = tx.clone();
    quick.on_choose(move |index| {
        let _ = sender.send(Event::Choose("quick", index));
    });
    let sender = tx.clone();
    quick.on_dismiss(move || {
        let _ = sender.send(Event::Hide("quick"));
    });
    let sender = tx.clone();
    quick.on_filter(move |query| {
        let _ = sender.send(Event::Filter(query.into()));
    });
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
        let _ = parent_sender.send(Event::Command(Command::Quit, "ipc", Instant::now()));
    });
    let server = ipc::Server::bind()?;
    server.start(dispatch)?;
    let mut visible = initial;
    let mut focused = None;
    if let Some(name) = initial {
        metrics.begin(name, "diagnostic", start);
        if name == "emoji" {
            emoji.invoke_prepare();
            emoji_way.grab_focus();
        } else {
            quick.invoke_prepare();
            quick_way.grab_focus();
        }
        metrics.mark(name, "t2_shown");
    }
    log::info!(
        "Spell ready: overlay, bottom center, margin=24, exclusive_zone=0, keyboard=exclusive, output={:?}",
        std::env::var("SLINT_SHELL_OUTPUT")
    );
    loop {
        for (name, way) in [("emoji", &mut emoji_way), ("quick", &mut quick_way)] {
            CURRENT.with(|c| c.set(name));
            way.on_call()?;
        }
        while let Ok(event) = rx.try_recv() {
            let mut dismiss = None;
            match event {
                Event::Command(Command::Quit, _, _) => {
                    emoji_way.hide();
                    quick_way.hide();
                    return Ok(());
                }
                Event::Command(Command::Show(name @ ("emoji" | "quick")), source, start) => {
                    if let Some(previous) = visible.take() {
                        if previous == "emoji" {
                            emoji_way.remove_focus();
                            emoji.set_presented(false);
                            emoji_way.subtract_input_region(0, 0, 520, 460);
                            if unmap {
                                emoji_way.hide();
                            }
                        } else {
                            quick_way.remove_focus();
                            quick.set_presented(false);
                            quick_way.subtract_input_region(0, 0, 520, 500);
                            if unmap {
                                quick_way.hide();
                            }
                        }
                        metrics.end(previous);
                        if previous == name {
                            continue;
                        }
                    }
                    metrics.begin(name, source, start);
                    focused = None;
                    visible = Some(name);
                    if name == "emoji" {
                        emoji.set_presented(true);
                        emoji.invoke_prepare();
                        emoji_way.add_input_region(0, 0, 520, 460);
                        if unmap {
                            emoji_way.show_again();
                        }
                        emoji_way.grab_focus();
                    } else {
                        quick.set_presented(true);
                        quick.invoke_prepare();
                        quick_way.add_input_region(0, 0, 520, 500);
                        if unmap {
                            quick_way.show_again();
                        }
                        quick_way.grab_focus();
                    }
                    metrics.mark(name, "t2_shown");
                }
                Event::Command(Command::Hide, _, _) => dismiss = visible,
                Event::Command(_, _, _) => {}
                Event::Focus(name, active) => {
                    let window = if name == "emoji" {
                        emoji.window()
                    } else {
                        quick.window()
                    };
                    window
                        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(active));
                    if visible == Some(name) {
                        if active {
                            focused = Some(name);
                            metrics.mark(name, "t4_focused");
                        } else if focused == Some(name) {
                            focused = None;
                            dismiss = Some(name);
                        } else {
                            metrics.mark(name, "focus_left_before_enter");
                        }
                    }
                }
                Event::Frame(name) => metrics.mark(name, "t3_first_frame"),
                Event::Hide(name) => dismiss = Some(name),
                Event::Choose(name, index) if visible == Some(name) => {
                    if name == "quick" && quick.get_items().row_data(index as usize).is_none() {
                        continue;
                    }
                    log::info!("Spell selected {name} index={index} (stub)");
                    metrics.mark(name, "selected");
                    dismiss = Some(name);
                }
                Event::Choose(_, _) => {}
                Event::Filter(query) => {
                    quick.set_items(ModelRc::new(VecModel::from(
                        actions
                            .iter()
                            .filter(|s| s.to_lowercase().contains(&query.to_lowercase()))
                            .cloned()
                            .collect::<Vec<_>>(),
                    )));
                    quick.set_selected(0);
                    metrics.mark("quick", "t5_first_key");
                }
                Event::Key(name, key) if visible == Some(name) => {
                    metrics.mark(name, "t5_first_key");
                    let is = |k: slint::platform::Key| key == slint::SharedString::from(k).as_str();
                    if is(slint::platform::Key::Escape) {
                        dismiss = Some(name);
                    } else if is(slint::platform::Key::Return) || key == "\n" {
                        let index = if name == "emoji" {
                            emoji.get_selected()
                        } else {
                            quick.get_selected()
                        };
                        let _ = tx.send(Event::Choose(name, index));
                    } else if name == "emoji"
                        && let Ok(page @ 1..=5) = key.parse::<i32>()
                    {
                        emoji.set_page(page - 1);
                        emoji.set_selected(0);
                    } else {
                        let delta = if is(slint::platform::Key::DownArrow) {
                            if name == "emoji" { 8 } else { 1 }
                        } else if is(slint::platform::Key::UpArrow) {
                            if name == "emoji" { -8 } else { -1 }
                        } else if name == "emoji" && is(slint::platform::Key::LeftArrow) {
                            -1
                        } else if name == "emoji" && is(slint::platform::Key::RightArrow) {
                            1
                        } else {
                            0
                        };
                        if delta != 0 {
                            if name == "emoji" {
                                emoji.set_selected(
                                    (emoji.get_selected() + delta)
                                        .rem_euclid(if emoji.get_page() == 5 { 1500 } else { 48 }),
                                );
                            } else {
                                let count = quick.get_items().row_count() as i32;
                                if count > 0 {
                                    quick.set_selected(
                                        (quick.get_selected() + delta).rem_euclid(count),
                                    );
                                }
                            }
                            metrics.mark(name, "navigation_handled");
                        }
                    }
                }
                Event::Key(_, _) => {}
            }
            if let Some(name) = dismiss
                && visible == Some(name)
            {
                if name == "emoji" {
                    emoji_way.remove_focus();
                    emoji.set_presented(false);
                    emoji_way.subtract_input_region(0, 0, 520, 460);
                    if unmap {
                        emoji_way.hide();
                    }
                } else {
                    quick_way.remove_focus();
                    quick.set_presented(false);
                    quick_way.subtract_input_region(0, 0, 520, 500);
                    if unmap {
                        quick_way.hide();
                    }
                }
                visible = None;
                focused = None;
                metrics.end(name);
            }
        }
    }
}
