#[path = "../src/editor.rs"]
mod editor;
use slint::{ComponentHandle, Model};
use std::{cell::Cell, rc::Rc, time::Duration};
slint::include_modules!();

fn resize(ui: &SettingsWindow) {
    let scale = ui.window().scale_factor();
    ui.window().set_size(slint::PhysicalSize::new(
        (940.0 * scale).round() as u32,
        (700.0 * scale).round() as u32,
    ));
}

fn show_popup(ui: &impl ComponentHandle, width: u32, height: u32) {
    ui.show().unwrap();
    let scale = ui.window().scale_factor();
    ui.window().set_size(slint::PhysicalSize::new(
        (width as f32 * scale).round() as u32,
        (height as f32 * scale).round() as u32,
    ));
}

fn key(ui: &SettingsWindow, value: impl Into<slint::SharedString>) {
    let text = value.into();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
}

fn chord(ui: &SettingsWindow, modifier: slint::platform::Key, text: &str) {
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: modifier.into(),
        });
    key(ui, text);
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased {
            text: modifier.into(),
        });
}

fn pointer(ui: &SettingsWindow, x: f32, y: f32, down: Option<bool>) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let position = slint::LogicalPosition::new(x, y);
    if down == Some(true) {
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
    }
    ui.window().dispatch_event(match down {
        Some(true) => WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        },
        Some(false) => WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        },
        None => WindowEvent::PointerMoved { position },
    });
}

fn snapshot(ui: &impl ComponentHandle, name: &str) {
    if let Some(dir) = std::env::var_os("LHC_EDITOR_SNAPSHOTS") {
        let pixels = ui.window().take_snapshot().unwrap();
        let mut bytes = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
        for pixel in pixels.as_slice() {
            bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
        }
        std::fs::write(
            std::path::PathBuf::from(dir).join(format!("{name}.ppm")),
            bytes,
        )
        .unwrap();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = SettingsWindow::new()?;
    editor::bind(&ui);
    let emoji = EmojiPopup::new()?;
    emoji.set_emojis(slint::ModelRc::new(slint::VecModel::from(
        vec![slint::SharedString::from("😀"); 240],
    )));
    let quick = QuickPopup::new()?;
    quick.set_items(slint::ModelRc::new(slint::VecModel::from(
        vec![slint::SharedString::from("Привет, мир! · A long quick action label"); 30],
    )));
    resize(&ui);
    ui.set_grid_open(true);
    ui.show()?;
    resize(&ui);
    let weak = ui.as_weak();
    let step = Rc::new(Cell::new(-1));
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(400),
        move || {
            let ui = weak.unwrap();
            let n = step.get();
            step.set(n + 1);
            match n {
                -1 => { resize(&ui); }
                0 => {
                    pointer(&ui, 42.0, 240.0, Some(true));
                    pointer(&ui, 42.0, 240.0, Some(false));
                    snapshot(&ui, "grid-dark-ru");
                    ui.invoke_preferences(false, true);
                }
                1 => {
                    snapshot(&ui, "grid-light-en");
                    ui.invoke_edit_key(33);
                }
                2 => {
                    snapshot(&ui, "form-light-en");
                    pointer(&ui, 160.0, 185.0, Some(true));
                    pointer(&ui, 160.0, 185.0, Some(false));
                    key(&ui, "Привет, мир!");
                    assert_eq!(ui.get_value(), "Привет, мир!", "Cyrillic input");
                    chord(&ui, slint::platform::Key::Control, "a");
                    chord(&ui, slint::platform::Key::Control, "c");
                    key(&ui, slint::platform::Key::Backspace);
                    assert_eq!(ui.get_value(), "", "selection deletion");
                    chord(&ui, slint::platform::Key::Control, "v");
                    assert_eq!(ui.get_value(), "Привет, мир!", "clipboard round trip");
                    key(&ui, slint::platform::Key::Tab);
                    chord(
                        &ui,
                        slint::platform::Key::Shift,
                        &slint::SharedString::from(slint::platform::Key::Tab),
                    );
                    let before = ui.get_value();
                    key(&ui, "!");
                    assert_ne!(ui.get_value(), before, "Shift+Tab returns to field");
                    let saved = ui.get_value();
                    key(&ui, slint::platform::Key::Return);
                    assert!(!ui.get_editing(), "Enter in a valid field saves");
                    key(&ui, slint::platform::Key::Return);
                    assert_eq!(ui.get_value(), saved, "saved text reopens");
                    key(&ui, slint::platform::Key::Escape);
                    assert!(!ui.get_editing(), "Escape closes modal");
                    key(&ui, slint::platform::Key::Return);
                    assert!(ui.get_editing(), "focus returns to keyboard");
                    pointer(&ui, 160.0, 360.0, Some(true));
                    pointer(&ui, 160.0, 360.0, Some(false));
                    let selected = ui.get_selected_action();
                    key(&ui, slint::platform::Key::DownArrow);
                    assert_eq!(ui.get_selected_action(), selected + 1, "catalog keyboard navigation");
                    ui.invoke_change_kind(3);
                    ui.invoke_begin_capture();
                    key(&ui, slint::platform::Key::Return);
                    assert!(ui.get_editing());
                    assert_eq!(ui.get_value(), "Enter");
                    ui.invoke_begin_capture();
                    key(&ui, slint::platform::Key::Escape);
                    assert!(ui.get_editing());
                    assert_eq!(ui.get_value(), "Esc");
                    ui.invoke_cancel();
                    ui.invoke_rename_quick(0, "Длинная подпись — Привет, мир! 👋".into());
                    ui.invoke_move_quick(0, 60);
                    assert!(
                        ui.get_quick_items()
                            .row_data(59)
                            .unwrap()
                            .contains("Привет")
                    );
                    ui.set_grid_open(false);
                    ui.set_grid_open(true);
                    assert!(
                        ui.get_quick_items()
                            .row_data(59)
                            .unwrap()
                            .contains("Привет")
                    );
                }
                3 => {
                    snapshot(&ui, "grid-reordered");

                    pointer(&ui, 42.0, 240.0, Some(true));
                    pointer(&ui, 390.0, 305.0, None);
                    snapshot(&ui, "drag-marker");
                    pointer(&ui, 390.0, 305.0, Some(false));
                    assert!(
                        ui.get_quick_items().row_data(3).unwrap().starts_with("02"),
                        "pointer reorders to insertion boundary"
                    );
                    pointer(&ui, 42.0, 240.0, Some(true));
                    pointer(&ui, 390.0, 305.0, None);
                    key(&ui, slint::platform::Key::Escape);
                    pointer(&ui, 390.0, 305.0, Some(false));
                    assert!(
                        ui.get_quick_items().row_data(0).unwrap().starts_with("03"),
                        "Escape cancels drag"
                    );
                    pointer(&ui, 42.0, 240.0, Some(true));
                    pointer(&ui, 42.0, 490.0, None);
                }
                4 => {}
                5 => {
                    snapshot(&ui, "drag-autoscroll");
                    pointer(&ui, 42.0, 490.0, Some(false));
                    assert!(
                        ui.get_quick_items().iter().position(|item| item.starts_with("03")).unwrap() > 11,
                        "edge scroll drop"
                    );
                    snapshot(&ui, "drag-finished");
                    pointer(&ui, 750.0, 652.0, Some(true));
                    pointer(&ui, 750.0, 652.0, Some(false));
                }
                6 => {
                    snapshot(&ui, "dropdown-edge");
                    key(&ui, slint::platform::Key::UpArrow);
                    key(&ui, slint::platform::Key::Return);
                    assert!(!ui.global::<Locale>().get_english(), "edge dropdown selection");
                    ui.invoke_preferences(false, true);
                    emoji.global::<Theme>().set_dark(false);
                    emoji.global::<Theme>().invoke_apply();
                    quick.global::<Theme>().set_dark(false);
                    quick.global::<Theme>().invoke_apply();
                    show_popup(&emoji, 520, 460);
                }
                7 => { snapshot(&emoji, "emoji-light-en"); emoji.hide().unwrap(); show_popup(&quick, 520, 500); }
                8 => {
                    snapshot(&quick, "quick-light-en"); quick.hide().unwrap();
                    ui.invoke_preferences(true, false);
                    emoji.global::<Theme>().set_dark(true); emoji.global::<Theme>().invoke_apply();
                    quick.global::<Theme>().set_dark(true); quick.global::<Theme>().invoke_apply();
                    show_popup(&emoji, 520, 460);
                }
                9 => { snapshot(&emoji, "emoji-dark-ru"); emoji.hide().unwrap(); show_popup(&quick, 520, 500); }
                10 => {
                    snapshot(&quick, "quick-dark-ru"); quick.hide().unwrap();
                    println!("3B interactions: passed (clipboard, keyboard, modal, DnD, cancellation, edge scrolling, theme, locale, hidden popups)");
                    slint::quit_event_loop().unwrap();
                }
                _ => unreachable!(),
            }
        },
    );
    slint::run_event_loop()?;
    Ok(())
}
