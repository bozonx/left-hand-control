//! Shared popup grid: page offsets, focus, keyboard routing and cell clicks.

mod support;
use slint::{
    ComponentHandle, ModelRc, VecModel,
    platform::{PointerEventButton, WindowEvent},
};
use slint_shell::ui::{EmojiPopup, QuickPopup};
use std::{cell::RefCell, rc::Rc, time::Duration};

fn click(ui: &impl ComponentHandle, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    for event in [
        WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        },
        WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        },
    ] {
        ui.window().dispatch_event(event);
    }
}

fn check_modifiers(ui: &impl ComponentHandle, count: impl Fn() -> usize) {
    use slint::platform::Key;
    let press =
        |text: slint::SharedString| ui.window().dispatch_event(WindowEvent::KeyPressed { text });
    let release = |text: slint::SharedString| {
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text })
    };
    let before = count();
    for modifier in [Key::Control, Key::Alt, Key::Meta] {
        press(modifier.into());
        for text in [
            slint::SharedString::from("q"),
            "1".into(),
            Key::Tab.into(),
            Key::Escape.into(),
        ] {
            press(text.clone());
            release(text);
        }
        release(modifier.into());
    }
    assert_eq!(count(), before);
    for text in [
        slint::SharedString::from("q"),
        Key::Tab.into(),
        Key::Escape.into(),
    ] {
        press(text.clone());
        release(text);
    }
    assert_eq!(count(), before + 3);
    press(Key::Shift.into());
    press("Q".into());
    release("Q".into());
    release(Key::Shift.into());
    assert_eq!(count(), before + 4);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let emoji = EmojiPopup::new()?;
    emoji.set_emojis(ModelRc::new(VecModel::from(
        (0..30)
            .map(|index| {
                if index < 15 {
                    "X".into()
                } else {
                    "🙂".into()
                }
            })
            .collect::<Vec<slint::SharedString>>(),
    )));
    emoji.set_page_names(ModelRc::new(VecModel::from(vec![
        "First".into(),
        "Second".into(),
    ])));
    emoji.set_page(1);
    let quick = QuickPopup::new()?;
    quick.set_items(ModelRc::new(VecModel::from(
        (0..15)
            .map(|index| format!("Action {}", index + 1).into())
            .collect::<Vec<slint::SharedString>>(),
    )));
    quick.set_page_names(ModelRc::new(VecModel::from(vec!["Current page".into()])));
    let keys = Rc::new(RefCell::new(Vec::new()));
    let cells = Rc::new(RefCell::new(Vec::new()));
    let observed = keys.clone();
    emoji.on_key(move |key| {
        if key == slint::SharedString::from(slint::platform::Key::Shift) {
            return false;
        }
        observed.borrow_mut().push(("emoji", key.to_string()));
        true
    });
    let observed = keys.clone();
    quick.on_key(move |key| {
        if key == slint::SharedString::from(slint::platform::Key::Shift) {
            return false;
        }
        observed.borrow_mut().push(("quick", key.to_string()));
        true
    });
    let observed = cells.clone();
    emoji.on_choose(move |index| observed.borrow_mut().push(("emoji", index)));
    let observed = cells.clone();
    quick.on_choose(move |index| observed.borrow_mut().push(("quick", index)));
    emoji.show()?;
    quick.show()?;
    slint::Timer::single_shot(Duration::from_millis(150), move || {
        // Resolve layout before injecting pointer events.
        let _ = emoji.window().take_snapshot().unwrap();
        let _ = quick.window().take_snapshot().unwrap();
        emoji.invoke_prepare();
        emoji
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: "q".into() });
        quick.invoke_prepare();
        quick
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: "w".into() });
        assert_eq!(
            *keys.borrow(),
            [("emoji", String::from("q")), ("quick", String::from("w"))]
        );
        check_modifiers(&emoji, || keys.borrow().len());
        check_modifiers(&quick, || keys.borrow().len());
        click(&emoji, 65.0, 110.0);
        click(&quick, 100.0, 110.0);
        assert_eq!(*cells.borrow(), [("emoji", 0), ("quick", 0)]);
        support::snapshot(&emoji, "popup-emoji");
        support::snapshot(&quick, "popup-quick");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
