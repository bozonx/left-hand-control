use lhc_core::{config_document::ConfigDocument, storage::StoragePaths};
use slint::{ComponentHandle, Model};
use slint_shell::{Document, bind_document, ui::*};
use std::{rc::Rc, time::Duration};

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
    let document = Document::load(paths.clone())?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    let settings = ui.global::<SettingsEditor>();
    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
    settings.set_hold_timeout("not a number".into());
    settings.invoke_save();
    assert_eq!(settings.get_message().id, "timeout-invalid");
    settings.set_hold_timeout("310".into());
    settings.set_double_tap_timeout("270".into());
    settings.set_macro_pause("35".into());
    settings.set_modifier_delay("12".into());
    settings.set_appearance_index(1);
    settings.set_locale_index(1);
    settings.set_keyboard_device_path("/dev/input/test-keyboard".into());
    settings.set_mouse_device_path("/dev/input/test-mouse".into());
    settings.set_use_fullscreen(true);
    settings.set_use_gamemoded(false);
    settings.set_text_mode_index(4);
    settings.set_ydotool_path("/usr/bin/ydotool".into());
    settings.set_process_name("test-game".into());
    settings.set_process_only_active(true);
    settings.invoke_add_process();
    settings.invoke_save();
    assert_eq!(settings.get_message().id, "settings-saved");
    let loaded = ConfigDocument::load(paths.clone())?;
    assert_eq!(loaded.settings(), document.read().settings());
    assert_eq!(loaded.settings().default_hold_timeout_ms, 310);
    assert_eq!(loaded.settings().game_mode.process_matchers.len(), 1);
    assert_eq!(loaded.settings().linux_wayland_text_mode.as_deref(), Some("ydotool"));

    // Another process changes a setting while the form has an unsaved edit:
    // saving keeps the other change.
    let mut other = ConfigDocument::load(paths.clone())?;
    other.update_settings(|s| s.default_double_tap_timeout_ms = 999)?;
    document.reload()?;
    settings.set_macro_pause("40".into());
    settings.invoke_save();
    assert_eq!(settings.get_message().id, "settings-saved");
    let loaded = ConfigDocument::load(paths.clone())?;
    assert_eq!(loaded.settings().default_double_tap_timeout_ms, 999);
    assert_eq!(loaded.settings().default_macro_step_pause_ms, 40);

    let first = document
        .edit(slint_shell::document::View::Library, |config| {
            config.create_layer("First", "")
        })?
        .value;
    let second = document
        .edit(slint_shell::document::View::Library, |config| {
            config.create_layer("Second", "")
        })?
        .value;
    ui.global::<LayersEditor>().invoke_choose(1);
    ui.global::<LayersEditor>().invoke_set_label_mode(2);
    let reloaded = Document::load(paths.clone())?;
    let second_ui = SettingsWindow::new()?;
    bind_document(&second_ui, &reloaded);
    assert_eq!(second_ui.global::<LayersEditor>().get_selected(), 1);
    assert_eq!(second_ui.global::<LayersEditor>().get_label_mode(), 2);
    document.edit(slint_shell::document::View::Library, |config| {
        config.update_layout(|layout| layout.layers.reverse())
    })?;
    assert_eq!(ui.global::<LayersEditor>().get_selected(), 0);
    assert_eq!(document.read().layout().layers[0].id, second);
    document.edit(slint_shell::document::View::Library, |config| {
        config.delete_layer(&first)
    })?;

    let quick_popup = QuickPopup::new()?;
    quick_popup.set_query("old query".into());
    quick_popup.invoke_begin_search();
    assert!(quick_popup.get_searching());
    quick_popup.invoke_prepare();
    assert!(!quick_popup.get_searching());
    assert!(quick_popup.get_query().is_empty());

    let picker = ui.global::<ActionPicker>();
    let library = ui.global::<LayoutLibrary>();
    ui.invoke_navigate(Page::Keyboard, MenuKind::Emoji);
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.set_value("text:Привет 👋".into());
    picker.invoke_apply();
    library.invoke_save_as("Test layout".into());
    assert!(!library.get_save_as_open());
    assert!(!library.get_dirty());
    assert_eq!(library.get_current_label(), "Test layout");
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.set_value("text:Changed".into());
    picker.invoke_apply();
    assert!(library.get_dirty());
    library.invoke_save_current();
    assert!(!library.get_dirty());
    assert_eq!(
        lhc_core::profile::layout_file::serialize(&document.read().load_layout("user:Test layout")?),
        lhc_core::profile::layout_file::serialize(document.read().layout())
    );
    ui.global::<KeyEditor>().invoke_edit(33);
    assert!(picker.get_opened());
    picker.invoke_close();
    assert!(!picker.get_opened());
    assert_eq!(document.read().base_tap_action("KeyQ"), Some("text:Changed"));
    ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
    assert_eq!(ui.global::<MenuEditor>().get_cells().row_count(), 15);
    for kind in [MenuKind::Quick, MenuKind::Commands, MenuKind::Emoji] {
        ui.invoke_navigate(Page::Menus, kind);
        assert_eq!(ui.global::<MenuEditor>().get_kind(), kind);
    }
    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
    settings.set_appearance_index(2);
    settings.set_locale_index(2);
    settings.invoke_save();
    assert_eq!(document.read().settings().appearance, lhc_core::profile::model::Appearance::Dark);
    library.set_active_label("Test layout".into());
    ui.global::<AppState>().set_keyboard_language("EN".into());
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
            1 => { snapshot(&ui, "settings-bottom-ru"); ui.invoke_navigate(Page::Keyboard, MenuKind::Emoji); }
            2 => { snapshot(&ui, "keyboard-ru"); ui.invoke_navigate(Page::Layouts, MenuKind::Emoji); }
            3 => { snapshot(&ui, "layouts-ru"); ui.invoke_navigate(Page::Rules, MenuKind::Emoji); }
            4 => { snapshot(&ui, "rules-ru"); ui.invoke_navigate(Page::Layers, MenuKind::Emoji); }
            5 => { snapshot(&ui, "layers-ru"); ui.invoke_navigate(Page::Macros, MenuKind::Emoji); }
            6 => { snapshot(&ui, "macros-ru"); ui.invoke_navigate(Page::Menus, MenuKind::Emoji); }
            7 => { snapshot(&ui, "emoji-ru"); ui.invoke_navigate(Page::Menus, MenuKind::Quick); }
            8 => { snapshot(&ui, "quick-ru"); ui.invoke_navigate(Page::Menus, MenuKind::Commands); }
            9 => { snapshot(&ui, "commands-ru"); ui.invoke_navigate(Page::Settings, MenuKind::Emoji); ui.global::<SettingsEditor>().set_appearance_index(1); ui.global::<SettingsEditor>().set_locale_index(1); ui.global::<SettingsEditor>().invoke_save(); ui.window().set_size(slint::LogicalSize::new(940.0, 700.0)); }
            10 => { snapshot(&ui, "settings-light-en-small");
                for (x, expected) in [(60.0, Page::Layouts), (ui.window().size().width as f32 / ui.window().scale_factor() - 55.0, Page::Settings)] {
                    let position = slint::LogicalPosition::new(x, 27.0);
                    for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                    assert_eq!(ui.get_page(), expected);
                }
                println!("Interactions passed: settings persistence and merge, validation, navigation, library save, key editing, shared UI state and popup search"); slint::quit_event_loop().unwrap(); }
            _ => {}
        }
        step.set(n + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
