use slint::{ComponentHandle, Model};
use slint_shell::{editor, ui::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = SettingsWindow::new()?;
    let _editor = editor::bind_with_config(&ui, None);
    ui.set_page(0);
    if std::env::args().any(|arg| arg == "--smoke") {
        assert_eq!(ui.get_keys().row_count(), 80);
        let loaded = |count| Message {
            id: "config-loaded".into(),
            arg: "".into(),
            count,
        };
        let locale = ui.global::<Locale>();
        assert_eq!(
            locale.invoke_text(loaded(5)),
            "Конфигурация загружена: 5 правил"
        );
        assert_eq!(
            locale.invoke_text(loaded(2)),
            "Конфигурация загружена: 2 правила"
        );
        slint::select_bundled_translation("en")?;
        assert_eq!(
            locale.invoke_text(loaded(1)),
            "Configuration loaded: 1 rule"
        );
        slint::select_bundled_translation("ru")?;
        assert_eq!(ui.get_actions().row_count(), 83);
        ui.invoke_edit_key(33);
        assert!(ui.get_editing());
        assert!(!ui.get_validation().id.is_empty());
        ui.set_value("Привет 👋".into());
        ui.invoke_validate();
        ui.invoke_save();
        assert!(!ui.get_editing());
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "Привет 👋");
        ui.set_value("Отменить".into());
        ui.invoke_cancel();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "Привет 👋");
        ui.invoke_change_kind(1);
        ui.set_value("10001".into());
        ui.invoke_save();
        assert!(ui.get_editing());
        assert!(!ui.get_validation().id.is_empty());
        ui.set_value("250".into());
        ui.invoke_save();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "250");
        ui.invoke_change_kind(2);
        ui.invoke_save();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "Ctrl+KeyC");
        ui.invoke_change_kind(3);
        ui.set_capturing(true);
        ui.invoke_capture("k".into(), true, false, true, false);
        assert_eq!(ui.get_value(), "Ctrl+Shift+KeyK");
        assert!(!ui.get_capturing());
        ui.invoke_save();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "Ctrl+Shift+KeyK");
        ui.set_query("копировать".into());
        ui.invoke_filter();
        assert_eq!(ui.get_actions().row_count(), 1);
        ui.set_query("".into());
        ui.invoke_filter();
        ui.invoke_pick_action(0);
        assert_eq!(ui.get_value(), "Ctrl+KeyC");
        ui.invoke_cancel();
        ui.show()?;
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
            let ui = weak.unwrap();
            snapshot(&ui, "keyboard");
            ui.invoke_edit_key(33);
            assert!(ui.get_editing());
            ui.invoke_pick_action(0);
            let weak = ui.as_weak();
            slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
                let ui = weak.unwrap();
                assert_eq!(ui.get_selected_action(), 0);
                snapshot(&ui, "editor");
                ui.invoke_change_kind(3);
                ui.invoke_begin_capture();
                for text in [
                    slint::platform::Key::Control.into(),
                    slint::platform::Key::Return.into(),
                ] {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text });
                }
                assert_eq!(ui.get_value(), "Ctrl+Enter");
                assert!(ui.get_editing());
                for text in [
                    slint::platform::Key::Return.into(),
                    slint::platform::Key::Control.into(),
                ] {
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
                }
                ui.invoke_begin_capture();
                ui.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                        text: slint::platform::Key::Escape.into(),
                    });
                assert_eq!(ui.get_value(), "Escape");
                assert!(ui.get_editing());
                println!(
                    "Editor smoke: passed (80 keys, real actions, draft, validation, shortcut capture)"
                );
                slint::quit_event_loop().unwrap();
            });
        });
    }
    ui.run()?;
    Ok(())
}

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
