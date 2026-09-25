#[path = "../src/app_storage.rs"]
#[allow(dead_code)]
mod app_storage;
#[path = "../src/config_state.rs"]
#[allow(dead_code)]
mod config_state;
#[path = "../src/editor.rs"]
mod editor;
use slint::{ComponentHandle, Model};
slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = SettingsWindow::new()?;
    editor::bind_with_config(&ui, None);
    if std::env::args().any(|arg| arg == "--smoke") {
        assert_eq!(ui.get_keys().row_count(), 80);
        assert_eq!(ui.get_actions().row_count(), 500);
        ui.invoke_edit_key(33);
        assert!(ui.get_editing());
        assert!(!ui.get_validation().is_empty());
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
        assert!(!ui.get_validation().is_empty());
        ui.set_value("250".into());
        ui.invoke_save();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "250");
        ui.invoke_change_kind(2);
        ui.invoke_save();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "Копировать");
        ui.invoke_change_kind(3);
        ui.set_capturing(true);
        ui.invoke_capture("k".into(), true, false, true, false);
        assert_eq!(ui.get_value(), "Ctrl+Shift+K");
        assert!(!ui.get_capturing());
        ui.invoke_save();
        ui.invoke_edit_key(33);
        assert_eq!(ui.get_value(), "Ctrl+Shift+K");
        ui.set_query("ПРИВЕТ".into());
        ui.invoke_filter();
        assert_eq!(ui.get_actions().row_count(), 125);
        ui.set_query("".into());
        ui.invoke_filter();
        ui.invoke_pick_action(321);
        ui.invoke_update_action();
        assert_eq!(ui.get_selected_action(), 321);
        assert!(
            ui.get_actions()
                .row_data(321)
                .unwrap()
                .label
                .contains("обновлено 1")
        );
        ui.invoke_cancel();
        ui.show()?;
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
            let ui = weak.unwrap();
            snapshot(&ui, "keyboard");
            let position = slint::LogicalPosition::new(120.0, 340.0);
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                });
            assert!(ui.get_editing());
            assert_eq!(ui.get_selected_key(), 33);
            ui.set_catalog_scroll_y(-10800.0);
            ui.invoke_pick_action(301);
            let weak = ui.as_weak();
            slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
                let ui = weak.unwrap();
                let position = ui.get_catalog_scroll_y();
                ui.invoke_update_action();
                assert_eq!(ui.get_selected_action(), 301);
                assert_eq!(ui.get_catalog_scroll_y(), position);
                assert!(position < -1000.0);
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
                assert_eq!(ui.get_value(), "Esc");
                assert!(ui.get_editing());
                println!(
                    "Editor smoke: passed (80 keys, 500 rows, draft, validation, four types, row update)"
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
