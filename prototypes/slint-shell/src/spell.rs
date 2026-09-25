use crate::{Command, Dispatch, EmojiPopup, QuickPopup, ipc, metrics, popup_model};
use slint::ComponentHandle;
use slint::{Model, ModelRc, VecModel};
use spell_framework::{
    SpellAssociatedNew,
    layer_properties::{BoardType, LayerAnchor, LayerType, WindowConf},
    wayland_adapter::SpellWin,
};
use std::{
    sync::{Arc, mpsc},
    time::Instant,
};

enum Event {
    Command(Command, &'static str, Instant),
    Key(&'static str, String),
    Choose(&'static str, i32),
    Hide(&'static str),
    Filter(String),
    Focus(&'static str, bool),
    Frame(&'static str),
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
        Ok("unmap") | Err(_) => true,
        Ok("transparent") => false,
        _ => return Err("SLINT_SHELL_SPELL_LIFECYCLE must be transparent or unmap".into()),
    };
    let initial = match std::env::var("SLINT_SHELL_SPELL_INITIAL").as_deref() {
        Ok("emoji") => Some("emoji"),
        Ok("quick") => Some("quick"),
        Err(_) => None,
        _ => return Err("SLINT_SHELL_SPELL_INITIAL must be emoji or quick".into()),
    };
    let (tx, rx) = mpsc::channel();
    let mut metrics = metrics::Metrics::new(start)?;
    let mut emoji_way = SpellWin::invoke_spell("lhc-slint-emoji", configuration(460)?);
    let emoji = EmojiPopup::new()?;
    emoji.set_presented(initial == Some("emoji"));
    if initial != Some("emoji") {
        emoji_way.subtract_input_region(0, 0, 520, 460);
        if unmap {
            emoji_way.hide();
        }
    }
    emoji.set_emojis(ModelRc::new(VecModel::from(popup_model::emoji_items())));
    metrics.ready("emoji");
    let mut quick_way = SpellWin::invoke_spell("lhc-slint-quick", configuration(500)?);
    let quick = QuickPopup::new()?;
    quick.set_presented(initial == Some("quick"));
    if initial != Some("quick") {
        quick_way.subtract_input_region(0, 0, 520, 500);
        if unmap {
            quick_way.hide();
        }
    }
    let actions = popup_model::quick_items();
    quick.set_items(ModelRc::new(VecModel::from(popup_model::filter(
        &actions, "",
    ))));
    metrics.ready("quick");
    for (name, way) in [("emoji", &mut emoji_way), ("quick", &mut quick_way)] {
        let sender = tx.clone();
        way.set_event_handler(move |event| {
            use spell_framework::wayland_adapter::WindowEvent;
            let event = match event {
                WindowEvent::Focus(active) => Event::Focus(name, active),
                WindowEvent::Frame => Event::Frame(name),
                WindowEvent::Closed => Event::Hide(name),
            };
            let _ = sender.send(event);
        });
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
    let mut return_input = crate::return_input::ReturnInput::new()?;
    let mut pending_return = None;
    let mut keyboard_owner = None;
    let mut visible = initial;
    let mut focused = None;
    let mut navigation_count = 0;
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
        for way in [&mut emoji_way, &mut quick_way] {
            way.on_call()?;
        }
        while let Ok(event) = rx.try_recv() {
            let mut dismiss = None;
            let mut selected = None;
            match event {
                Event::Command(Command::Preferences(dark, english), _, _) => {
                    slint::select_bundled_translation(if english { "en" } else { "ru" }).unwrap();
                    emoji.global::<crate::Theme>().set_dark(dark);
                    emoji.global::<crate::Theme>().invoke_apply();
                    emoji.global::<crate::Locale>().set_english(english);
                    quick.global::<crate::Theme>().set_dark(dark);
                    quick.global::<crate::Theme>().invoke_apply();
                    quick.global::<crate::Locale>().set_english(english);
                    log::info!(
                        "preferences applied: dark={dark}, english={english}, visible={visible:?}"
                    );
                }
                Event::Command(Command::Quit, _, _) => {
                    emoji_way.hide();
                    quick_way.hide();
                    return Ok(());
                }
                Event::Command(Command::Show(name @ ("emoji" | "quick")), source, start) => {
                    if let Some(input) = &mut return_input {
                        if visible.is_none() {
                            input.capture();
                        } else {
                            input.cancel();
                        }
                    }
                    if let Some(previous) = pending_return.take() {
                        metrics.end(previous);
                    }
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
                    navigation_count = 0;
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
                    if active {
                        keyboard_owner = Some(name);
                    } else if keyboard_owner == Some(name) {
                        keyboard_owner = None;
                    }
                    if !active && pending_return == Some(name) {
                        metrics.mark(name, "keyboard_released");
                    }
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
                    let value = if name == "emoji" {
                        popup_model::emoji_index(emoji.get_page(), index)
                            .and_then(|index| emoji.get_emojis().row_data(index % 240))
                    } else {
                        quick.get_items().row_data(index as usize)
                    };
                    let Some(value) = value else { continue };
                    log::info!("Spell selected {name} index={index}: {value}");
                    metrics.mark(name, "selected");
                    selected = Some(value.to_string());
                    dismiss = Some(name);
                }
                Event::Choose(_, _) => {}
                Event::Filter(query) => {
                    if query
                        .chars()
                        .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c))
                    {
                        metrics.mark("quick", "filter_cyrillic");
                    }
                    quick.set_items(ModelRc::new(VecModel::from(popup_model::filter(
                        &actions, &query,
                    ))));
                    quick.set_selected(0);
                    metrics.mark("quick", "t5_first_key");
                    metrics.mark("quick", "filter_changed");
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
                        && let Ok(page @ 1..=6) = key.parse::<i32>()
                    {
                        metrics.mark(name, "page_changed");
                        if page == 6 {
                            metrics.mark(name, "stress_page");
                        }
                        emoji.set_page(page - 1);
                        emoji.set_selected(0);
                    } else {
                        if let Some(delta) = popup_model::key_delta(name, &key) {
                            if name == "emoji" {
                                emoji.set_selected(popup_model::advance(
                                    emoji.get_selected(),
                                    delta,
                                    if emoji.get_page() == 5 { 1500 } else { 48 },
                                ));
                            } else {
                                quick.set_selected(popup_model::advance(
                                    quick.get_selected(),
                                    delta,
                                    quick.get_items().row_count(),
                                ));
                            }
                            metrics.mark(name, "navigation_handled");
                            navigation_count += 1;
                            if navigation_count > 1 {
                                metrics.mark(name, "navigation_repeated");
                            }
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
                if let (Some(value), Some(input)) = (selected, &mut return_input) {
                    metrics.mark(name, "hidden");
                    pending_return = Some(name);
                    if !input.selected(value) {
                        metrics.mark(name, "return_target_missing");
                        metrics.end(name);
                        pending_return = None;
                    }
                } else {
                    if let Some(input) = &mut return_input {
                        input.cancel();
                    }
                    metrics.end(name);
                }
            }
        }
        if let (Some(name), Some(input)) = (pending_return, &mut return_input)
            && let Some(event) = input.poll(keyboard_owner.is_none())
        {
            metrics.mark(name, event);
            metrics.end(name);
            pending_return = None;
        }
    }
}
