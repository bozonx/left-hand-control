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
    slint_shell::bind_game_mode(&ui);
    let game_mode_config = paths.load_config()?;
    let game_state = ui.global::<AppState>();
    for (control, active) in [(GameModeControl::On, true), (GameModeControl::Off, false)] {
        game_state.invoke_set_game_control(control);
        assert_eq!(game_state.get_game_control(), control);
        assert_eq!(game_state.get_game_active(), active);
        assert!(game_state.get_game_state_available());
        assert_eq!(lhc_core::runtime_state::game_mode(), (active, true));
    }
    game_state.invoke_set_game_control(GameModeControl::Auto);
    assert_eq!(game_state.get_game_control(), GameModeControl::Auto);
    assert_eq!(paths.load_config()?, game_mode_config);
    let drag = ui.global::<DragDrop>();
    for (index, y, height) in [(0, 0.0, 80.0), (1, 90.0, 240.0), (2, 340.0, 80.0)] {
        drag.invoke_row(99, index, y, height);
    }
    assert_eq!(drag.invoke_locate(99, 0, 340.0, 3), 2);
    assert_eq!(drag.invoke_locate(99, 2, -340.0, 3), 0);
    assert_eq!(drag.invoke_locate(99, 0, 340.0, 2), 1);
    assert_eq!(drag.invoke_locate(99, 0, 6.0, 3), 0);
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
    assert_eq!(settings.get_message().id, "");
    let loaded = ConfigDocument::load(paths.clone())?;
    assert_eq!(loaded.settings(), document.read().settings());
    assert_eq!(loaded.settings().default_hold_timeout_ms, 310);
    assert_eq!(loaded.settings().game_mode.process_matchers.len(), 1);
    assert_eq!(
        loaded.settings().linux_wayland_text_mode.as_deref(),
        Some("ydotool")
    );
    assert!(settings.get_keyboard_manual());
    assert!(settings.get_mouse_manual());
    settings.invoke_select_device(0);
    settings.invoke_select_mouse(0);
    assert!(!settings.get_keyboard_manual());
    assert!(!settings.get_mouse_manual());
    assert!(document.read().settings().input_device_path.is_none());
    assert!(document.read().settings().input_mouse_device_path.is_none());
    settings.invoke_select_device(settings.get_input_devices().row_count() as i32 - 1);
    settings.invoke_select_mouse(settings.get_mouse_devices().row_count() as i32 - 1);
    assert!(settings.get_keyboard_manual());
    assert!(settings.get_mouse_manual());
    settings.set_keyboard_device_path("/dev/input/test-keyboard".into());
    settings.set_mouse_device_path("/dev/input/test-mouse".into());
    settings.invoke_save();
    settings.invoke_refresh_devices();
    assert!(settings.get_keyboard_manual());
    assert!(settings.get_mouse_manual());
    assert_eq!(settings.get_keyboard_device_path(), "/dev/input/test-keyboard");
    assert_eq!(settings.get_mouse_device_path(), "/dev/input/test-mouse");

    // Another process changes a setting while the form has an unsaved edit:
    // saving keeps the other change.
    let mut other = ConfigDocument::load(paths.clone())?;
    other.update_settings(|s| s.default_double_tap_timeout_ms = 999)?;
    document.reload()?;
    settings.set_macro_pause("40".into());
    settings.invoke_save();
    assert_eq!(settings.get_message().id, "");
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
    ui.global::<LayersEditor>().invoke_reorder(0, 1);
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
    assert!(library.get_rules_dirty());
    assert!(library.get_layers_dirty());
    assert!(!library.get_macros_dirty());
    library.invoke_save_current();
    assert!(!library.get_dirty());
    assert!(!library.get_rules_dirty());
    assert_eq!(
        lhc_core::profile::layout_file::serialize(
            &document.read().load_layout("user:Test layout")?
        ),
        lhc_core::profile::layout_file::serialize(document.read().layout())
    );
    ui.global::<KeyEditor>().invoke_edit(33);
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some("text:Changed")
    );
    ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
    assert_eq!(ui.global::<MenuEditor>().get_cells().row_count(), 15);
    for kind in [MenuKind::Quick, MenuKind::Commands, MenuKind::Emoji] {
        ui.invoke_navigate(Page::Menus, kind);
        assert_eq!(ui.global::<MenuEditor>().get_kind(), kind);
    }
    document.edit(slint_shell::document::View::Library, |config| {
        config.update_layout(|layout| {
            use lhc_core::profile::model::{Command, LayerRule, Macro, MacroStep};
            layout.rules = ["F13", "F14", "F15"]
                .into_iter()
                .enumerate()
                .map(|(index, key)| {
                    let mut rule = LayerRule::new(format!("drag-rule-{index}"), key);
                    rule.tap_action = Some("Escape".into());
                    rule
                })
                .collect();
            layout.macros = (0..3)
                .map(|index| Macro {
                    id: format!("dragMacro{index}"),
                    name: format!("Macro {index}"),
                    steps: ["KeyA", "KeyB", "KeyC"]
                        .into_iter()
                        .enumerate()
                        .map(|(step, action)| MacroStep {
                            id: format!("drag-step-{index}-{step}"),
                            action: action.into(),
                        })
                        .collect(),
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                })
                .collect();
            layout.commands = (0..3)
                .map(|index| Command {
                    id: format!("dragCommand{index}"),
                    name: format!("Command {index}"),
                    linux: "printf test".into(),
                })
                .collect();
        })
    })?;
    ui.global::<RulesEditor>().invoke_move(0, 2);
    assert_eq!(
        document
            .read()
            .layout()
            .rules
            .iter()
            .map(|row| row.key.as_str())
            .collect::<Vec<_>>(),
        ["F14", "F15", "F13"]
    );
    assert_eq!(ui.global::<RulesEditor>().get_selected(), 2);
    ui.global::<MacroEditor>().invoke_move(0, 2);
    assert_eq!(
        document
            .read()
            .layout()
            .macros
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["dragMacro1", "dragMacro2", "dragMacro0"]
    );
    ui.global::<MacroEditor>().invoke_move_step(0, 0, 2);
    assert_eq!(
        document.read().layout().macros[0]
            .steps
            .iter()
            .map(|row| row.action.as_str())
            .collect::<Vec<_>>(),
        ["KeyB", "KeyC", "KeyA"]
    );
    ui.global::<MenuEditor>().invoke_open(MenuKind::Commands);
    ui.global::<MenuEditor>().invoke_move_command(0, 2);
    assert_eq!(
        document
            .read()
            .layout()
            .commands
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        ["dragCommand1", "dragCommand2", "dragCommand0"]
    );
    for key in ["F13", "F14", "F15"] {
        document.edit(slint_shell::document::View::Library, |config| {
            config.set_layer_extra(&second, None, key, Some("Escape".into()))
        })?;
    }
    ui.global::<LayersEditor>().invoke_move_extra(0, 2);
    assert_eq!(
        document.read().layout().layer_keymaps[&second]
            .extras
            .iter()
            .map(|row| row.key.as_str())
            .collect::<Vec<_>>(),
        ["F14", "F15", "F13"]
    );
    let rules = ui.global::<RulesEditor>();
    let count = document.read().layout().rules.len();
    rules.invoke_add();
    assert_eq!(document.read().layout().rules.len(), count);
    ui.global::<ActionPicker>().invoke_dismiss_picker();
    assert_eq!(document.read().layout().rules.len(), count);
    rules.invoke_add();
    let picker = ui.global::<ActionPicker>();
    picker.set_value("CapsLock".into());
    picker.invoke_apply();
    assert_eq!(rules.get_selected(), 0);
    assert_eq!(document.read().layout().rules[0].key, "CapsLock");
    assert_eq!(document.read().layout().rules[1].key, "F14");
    rules.invoke_open_dialog(0, RuleDialog::Tap);
    picker.set_ignore_key(true);
    picker.set_value("".into());
    picker.invoke_apply();
    assert_eq!(document.read().layout().rules[0].tap_action, None);
    rules.invoke_open_dialog(0, RuleDialog::Tap);
    picker.set_ignore_key(false);
    picker.set_value("".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[0].tap_action.as_deref(),
        Some("")
    );
    rules.invoke_open_dialog(0, RuleDialog::Advanced);
    rules.set_hold_timeout("invalid".into());
    rules.set_double_timeout("300".into());
    rules.invoke_apply_advanced();
    assert_eq!(
        document.read().layout().rules[0].double_tap_timeout_ms,
        None
    );
    rules.set_hold_timeout("250".into());
    rules.invoke_apply_advanced();
    assert_eq!(document.read().layout().rules[0].hold_timeout_ms, Some(250));
    assert_eq!(
        document.read().layout().rules[0].double_tap_timeout_ms,
        Some(300)
    );
    rules.invoke_remove();
    let layers = ui.global::<LayersEditor>();
    layers.invoke_add_extra();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras.len(),
        3
    );
    let picker = ui.global::<ActionPicker>();
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras.len(),
        3
    );
    layers.invoke_add_extra();
    picker.set_value("F16".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras.len(),
        4
    );
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras[3].key,
        "F16"
    );
    layers.invoke_pick_extra(3, false);
    assert!(picker.get_layer_action());
    picker.set_ignore_key(true);
    picker.set_value("".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras[3].action,
        None
    );
    layers.invoke_pick_extra(3, true);
    picker.set_value("F17".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras[3].key,
        "F17"
    );
    assert_eq!(
        document.read().layout().layer_keymaps[&second].extras[3].action,
        None
    );
    layers.invoke_remove_extra(3);
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    let picker = ui.global::<ActionPicker>();
    assert!(picker.get_opened());
    assert!(picker.get_layer_action());
    assert_eq!(layers.get_dialog(), LayerDialog::None);
    picker.set_ignore_key(true);
    picker.set_value("".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().layer_key(&second, "Escape"),
        lhc_core::config_document::KeyAssignment::Swallow
    );
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    assert!(picker.get_ignore_key());
    picker.set_ignore_key(false);
    picker.invoke_apply();
    assert_eq!(
        document.read().layer_key(&second, "Escape"),
        lhc_core::config_document::KeyAssignment::Transparent
    );
    assert!(library.get_rules_dirty());
    assert!(library.get_layers_dirty());
    assert!(library.get_macros_dirty());
    assert!(library.get_commands_dirty());
    library.invoke_save_current();
    assert!(
        !library.get_dirty(),
        "status {}; saved:\n{}current:\n{}",
        library.get_status().id,
        lhc_core::profile::layout_file::serialize(
            &document.read().load_layout("user:Test layout")?
        ),
        lhc_core::profile::layout_file::serialize(document.read().layout())
    );
    assert!(!library.get_rules_dirty());
    assert!(!library.get_layers_dirty());
    assert!(!library.get_macros_dirty());
    assert!(!library.get_commands_dirty());
    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
    settings.set_appearance_index(2);
    settings.set_locale_index(2);
    settings.invoke_save();
    assert_eq!(
        document.read().settings().appearance,
        lhc_core::profile::model::Appearance::Dark
    );
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
            9 => { snapshot(&ui, "commands-ru"); ui.invoke_navigate(Page::Settings, MenuKind::Emoji); ui.global::<SettingsEditor>().set_appearance_index(1); ui.global::<SettingsEditor>().set_locale_index(1); ui.global::<SettingsEditor>().invoke_save(); ui.global::<Theme>().set_dark(false); ui.global::<Theme>().invoke_apply(); ui.window().set_size(slint::LogicalSize::new(940.0, 700.0)); }
            10 => { snapshot(&ui, "settings-light-en-small");
                for (x, expected) in [(60.0, Page::Layouts), (170.0, Page::Settings)] {
                    let position = slint::LogicalPosition::new(x, 27.0);
                    for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                    assert_eq!(ui.get_page(), expected);
                }
                ui.global::<Theme>().set_dark(false); ui.global::<Theme>().invoke_apply();
                ui.global::<AppState>().invoke_set_game_control(GameModeControl::On); }
            11 => { snapshot(&ui, "game-mode-on-light"); ui.global::<Theme>().set_dark(true); ui.global::<Theme>().invoke_apply(); }
            12 => { snapshot(&ui, "game-mode-on-dark"); ui.global::<AppState>().invoke_set_game_control(GameModeControl::Off); }
            13 => { snapshot(&ui, "game-mode-off-dark"); ui.global::<AppState>().invoke_set_game_control(GameModeControl::Auto); }
            14 => { snapshot(&ui, "game-mode-auto-dark");
                let position = slint::LogicalPosition::new(600.0, 27.0);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
            }
            15 => { snapshot(&ui, "game-mode-menu-dark");
                for text in [slint::platform::Key::DownArrow.into(), "\n".into()] {
                    ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text });
                }
                assert_eq!(ui.global::<AppState>().get_game_control(), GameModeControl::On);
            }
            16 => { snapshot(&ui, "game-mode-menu-selected"); ui.invoke_navigate(Page::Settings, MenuKind::Emoji); ui.global::<SettingsEditor>().set_hold_timeout("321".into()); ui.global::<SettingsEditor>().invoke_schedule_save(); }
            18 => { assert_eq!(document.read().settings().default_hold_timeout_ms, 321); ui.global::<SettingsEditor>().set_appearance_index(3); ui.global::<SettingsEditor>().invoke_save(); assert_eq!(ConfigDocument::load(paths.clone()).unwrap().settings().appearance, lhc_core::profile::model::Appearance::EInk); ui.global::<Theme>().set_eink(true); ui.global::<Theme>().set_dark(false); slint_shell::ui::apply_theme(&ui.global::<Theme>()); }
            19 => { snapshot(&ui, "settings-eink"); ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(500.0, 450.0), delta_x: 0.0, delta_y: -450.0 }); }
            20 => { snapshot(&ui, "settings-behavior-eink"); ui.global::<SettingsEditor>().set_message(Message { id: "timeout-invalid".into(), arg: "".into(), count: 0 }); }
            21 => { snapshot(&ui, "settings-error-toast"); println!("Interactions passed: settings persistence and merge, validation, navigation, library save, key editing, shared UI state, popup search and game mode overrides and dropdown keyboard selection"); slint::quit_event_loop().unwrap(); }
            _ => {}
        }
        step.set(n + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
