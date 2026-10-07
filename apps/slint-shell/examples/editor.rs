use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model};
use slint_shell::{Document, bind_document, ui::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    ui.set_page(Page::Keyboard);
    if std::env::args().any(|arg| arg == "--smoke") {
        let keys = ui.global::<KeyEditor>();
        assert_eq!(keys.get_keys().row_count(), 80);
        assert_eq!(keys.get_keys().row_data(33).unwrap().label, "Q");
        let loaded = |count| Message {
            id: "config-loaded".into(),
            arg: "".into(),
            count,
        };
        let locale = ui.global::<Locale>();
        slint_shell::select_ui_language("ru")?;
        assert_eq!(locale.invoke_text(loaded(1)), "Конфигурация загружена: 1 правило");
        assert_eq!(locale.invoke_text(loaded(5)), "Конфигурация загружена: 5 правил");
        assert_eq!(locale.invoke_text(loaded(2)), "Конфигурация загружена: 2 правила");
        slint_shell::select_ui_language("en")?;
        assert_eq!(locale.invoke_text(loaded(1)), "Configuration loaded: 1 rule");
        slint_shell::select_ui_language("ru")?;
        assert_eq!(locale.invoke_text(loaded(21)), "Конфигурация загружена: 21 правило");
        slint_shell::select_ui_language("en")?;

        // Editing a key opens the picker; applying saves the tap action.
        let picker = ui.global::<ActionPicker>();
        keys.invoke_edit(33);
        assert!(picker.get_opened());
        assert_eq!(keys.get_selected(), 33);
        picker.set_value("text:Привет 👋".into());
        picker.invoke_apply();
        assert!(!picker.get_opened());
        assert_eq!(document.read().base_tap_action("KeyQ"), Some("text:Привет 👋"));
        assert_eq!(keys.get_keys().row_data(33).unwrap().action, "text:Привет 👋");
        assert_eq!(ui.global::<AppState>().get_status().id, "action-saved");

        // Cancel keeps the saved value.
        keys.invoke_edit(33);
        assert_eq!(picker.get_value(), "text:Привет 👋");
        picker.set_value("Ctrl+KeyC".into());
        picker.invoke_dismiss_picker();
        assert_eq!(document.read().base_tap_action("KeyQ"), Some("text:Привет 👋"));

        // Pauses are only valid inside macros.
        keys.invoke_edit(33);
        picker.set_value("pause:250".into());
        picker.invoke_apply();
        assert!(picker.get_opened());
        assert!(!picker.get_valid());
        picker.set_value("Ctrl+Shift+KeyK".into());
        picker.invoke_apply();
        assert_eq!(document.read().base_tap_action("KeyQ"), Some("Ctrl+Shift+KeyK"));
        ui.show()?;
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
            let ui = weak.unwrap();
            snapshot(&ui, "keyboard");
            println!("Editor smoke: passed (80 keys, picker assignment, cancel, validation, translations)");
            slint::quit_event_loop().unwrap();
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
