use lhc_core::{config_document::ConfigDocument, storage::StoragePaths};
use slint::{ComponentHandle, Model};
use slint_shell::{bind_document, ui::*};
use std::{cell::RefCell, rc::Rc, time::Duration};

fn snapshot(ui: &SettingsWindow, name: &str) {
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
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Rc::new(RefCell::new(ConfigDocument::load(paths.clone())?));
    let ui = SettingsWindow::new()?;
    let _editor = bind_document(&ui, Some(document.clone()));
    ui.invoke_navigate(2, 0);
    ui.set_hold_timeout("not a number".into());
    ui.invoke_save_settings();
    assert_eq!(ui.get_settings_message().id, "timeout-invalid");
    ui.set_hold_timeout("310".into());
    ui.set_double_tap_timeout("270".into());
    ui.set_macro_pause("35".into());
    ui.set_modifier_delay("12".into());
    ui.set_appearance_index(1);
    ui.set_locale_index(1);
    ui.set_keyboard_device_path("/dev/input/test-keyboard".into());
    ui.set_mouse_device_path("/dev/input/test-mouse".into());
    ui.set_use_fullscreen(true);
    ui.set_use_gamemoded(false);
    ui.set_text_mode_index(4);
    ui.set_ydotool_path("/usr/bin/ydotool".into());
    ui.set_process_name("test-game".into());
    ui.set_process_only_active(true);
    ui.invoke_add_process();
    ui.invoke_save_settings();
    assert_eq!(ui.get_settings_message().id, "settings-saved");
    assert!(!ui.global::<Theme>().get_dark());
    assert!(ui.global::<Locale>().get_english());
    let loaded = ConfigDocument::load(paths.clone())?;
    assert_eq!(loaded.settings(), document.borrow().settings());
    assert_eq!(loaded.settings().default_hold_timeout_ms, 310);
    assert_eq!(loaded.settings().game_mode.process_matchers.len(), 1);
    ui.invoke_navigate(0, 0);
    ui.invoke_edit_key(33);
    ui.set_value("Привет 👋".into());
    ui.invoke_save();
    assert!(!ui.get_editing());
    ui.invoke_save_layout_as("Test layout".into());
    assert!(!ui.get_save_as_open());
    assert!(!ui.get_layout_dirty());
    assert_eq!(ui.get_current_layout_label(), "Test layout");
    ui.invoke_edit_key(33);
    ui.set_value("Changed".into());
    ui.invoke_save();
    ui.invoke_refresh_layout_context();
    assert!(ui.get_layout_dirty());
    ui.invoke_save_current_layout();
    assert!(!ui.get_layout_dirty());
    assert_eq!(
        lhc_core::profile::layout_file::serialize(
            &document.borrow().load_layout("user:Test layout")?
        ),
        lhc_core::profile::layout_file::serialize(document.borrow().layout())
    );
    ui.invoke_edit_key(33);
    ui.set_value("Cancel me".into());
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: slint::platform::Key::Escape.into(),
        });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased {
            text: slint::platform::Key::Escape.into(),
        });
    assert!(!ui.get_editing());
    ui.invoke_edit_key(33);
    assert_eq!(ui.get_value(), "Changed");
    ui.invoke_cancel();
    ui.invoke_navigate(6, 0);
    assert_eq!(ui.global::<MenuEditor>().get_cells().row_count(), 15);
    for kind in [1, 2, 0] {
        ui.invoke_navigate(6, kind);
        assert_eq!(ui.global::<MenuEditor>().get_kind(), kind);
    }
    ui.invoke_navigate(2, 0);
    ui.set_appearance_index(2);
    ui.set_locale_index(2);
    ui.invoke_save_settings();
    assert!(ui.global::<Theme>().get_dark());
    assert!(!ui.global::<Locale>().get_english());
    ui.set_active_layout_label("Test layout".into());
    ui.set_keyboard_language("EN".into());
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let step = Rc::new(std::cell::Cell::new(0));
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(400), move || {
        let ui = weak.unwrap();
        let n = step.get();
        match n {
            0 => { snapshot(&ui, "settings-dark-ru"); ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(500.0, 450.0), delta_x: 0.0, delta_y: -1200.0 }); }
            1 => { snapshot(&ui, "settings-bottom-ru"); ui.invoke_navigate(0, 0); }
            2 => { snapshot(&ui, "keyboard-ru"); ui.invoke_navigate(1, 0); }
            3 => { snapshot(&ui, "layouts-ru"); ui.invoke_navigate(3, 0); }
            4 => { snapshot(&ui, "rules-ru"); ui.invoke_navigate(4, 0); }
            5 => { snapshot(&ui, "layers-ru"); ui.invoke_navigate(5, 0); }
            6 => { snapshot(&ui, "macros-ru"); ui.invoke_navigate(6, 0); }
            7 => { snapshot(&ui, "emoji-ru"); ui.invoke_navigate(6, 1); }
            8 => { snapshot(&ui, "quick-ru"); ui.invoke_navigate(6, 2); }
            9 => { snapshot(&ui, "commands-ru"); ui.invoke_navigate(2, 0); ui.set_appearance_index(1); ui.set_locale_index(1); ui.invoke_save_settings(); ui.window().set_size(slint::LogicalSize::new(940.0, 700.0)); }
            10 => { snapshot(&ui, "settings-light-en-small");
                for (x, expected) in [(60.0, 1), (ui.window().size().width as f32 / ui.window().scale_factor() - 55.0, 2)] {
                    let position = slint::LogicalPosition::new(x, 27.0);
                    for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                    assert_eq!(ui.get_page(), expected);
                }
                println!("Interactions passed: settings persistence, validation, theme, locale, navigation, library save, key editing and Escape"); slint::quit_event_loop().unwrap(); }
            _ => {}
        }
        step.set(n + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
