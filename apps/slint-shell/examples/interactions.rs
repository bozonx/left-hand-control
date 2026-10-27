use lhc_core::{
    config_document::{ConfigDocument, KeyAssignment},
    storage::StoragePaths,
};
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

// Resolve the actual field bounds: font metrics and window decorations differ
// across desktops, so a fixed click position can miss the input entirely.
fn accessible_position(
    ui: &SettingsWindow,
    role: i_slint_core::items::AccessibleRole,
    label: &str,
    prefix: bool,
) -> slint::LogicalPosition {
    use i_slint_core::{
        accessibility::AccessibleStringProperty, item_tree::ItemRc, window::WindowInner,
    };
    // Resolve pending layout before locating controls during page transitions.
    let _ = ui.window().take_snapshot().unwrap();
    let mut pending = vec![ItemRc::new_root(
        WindowInner::from_pub(ui.window()).component(),
    )];
    let mut matches = Vec::new();
    while let Some(item) = pending.pop() {
        if item.is_visible()
            && item.accessible_role() == role
            && item
                .accessible_string_property(AccessibleStringProperty::Label)
                .as_deref()
                .is_some_and(|value| {
                    if prefix {
                        value.starts_with(label)
                    } else {
                        value == label
                    }
                })
        {
            let center = item.map_to_window(item.geometry().center());
            matches.push(slint::LogicalPosition::new(center.x, center.y));
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
    }
    assert_eq!(matches.len(), 1, "expected one visible {role:?}: {label}");
    matches[0]
}

fn numeric_field_position(ui: &SettingsWindow, label: &str) -> slint::LogicalPosition {
    accessible_position(
        ui,
        i_slint_core::items::AccessibleRole::Spinbox,
        label,
        false,
    )
}

fn check_first_layout_activation() -> Result<(), Box<dyn std::error::Error>> {
    for automatic in [false, true] {
        let dir = tempfile::tempdir()?;
        let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
        let document = Document::load(paths)?;
        let ui = SettingsWindow::new()?;
        bind_document(&ui, &document);
        let library = ui.global::<LayoutLibrary>();
        library.invoke_set_automatic(automatic);
        library.invoke_create("First".into(), "".into(), LayoutSource::Empty, 0);
        assert_eq!(
            document.read().settings().manual_layout_id.as_deref(),
            if automatic { None } else { Some("user:First") }
        );
        library.invoke_create("Second".into(), "".into(), LayoutSource::Empty, 0);
        assert_eq!(
            document.read().settings().manual_layout_id.as_deref(),
            if automatic { None } else { Some("user:First") }
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    check_first_layout_activation()?;
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Document::load(paths.clone())?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    slint_shell::bind_game_mode(&ui);
    let game_mode_config = paths.load_config()?;
    let game_state = ui.global::<AppState>();
    // Game mode is an in-development feature: without the setting On does nothing.
    game_state.invoke_set_game_control(GameModeControl::On);
    assert!(!game_state.get_game_active());
    assert!(!lhc_core::runtime_state::game_mode_active());
    lhc_core::gamemode::set_settings(&document.read().settings().game_mode, true);
    for (control, active) in [(GameModeControl::On, true), (GameModeControl::Off, false)] {
        game_state.invoke_set_game_control(control);
        assert_eq!(game_state.get_game_control(), control);
        assert_eq!(game_state.get_game_active(), active);
        assert!(game_state.get_game_state_available());
        assert_eq!(lhc_core::runtime_state::game_mode_active(), active);
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
    assert_eq!(drag.invoke_locate(99, 0, 6.0, 3), 0);
    // Scrolling the list under a stationary pointer moves the drop target too.
    drag.set_group(99);
    drag.set_source(0);
    drag.set_target(0);
    drag.set_count(3);
    drag.set_dragging(true);
    drag.invoke_scrolled(340.0);
    assert_eq!(drag.get_target(), 2);
    drag.invoke_scrolled(-340.0);
    assert_eq!(drag.get_target(), 0);
    drag.invoke_reset();
    // A shorter list ignores (and forgets) rows past its end.
    assert_eq!(drag.invoke_locate(99, 0, 340.0, 2), 1);
    let settings = ui.global::<SettingsEditor>();
    assert!(!settings.get_in_development());
    settings.set_in_development(true);
    settings.invoke_save();
    assert!(
        ConfigDocument::load(paths.clone())?
            .settings()
            .in_development
    );
    settings.set_in_development(false);
    settings.invoke_save();
    assert!(
        !ConfigDocument::load(paths.clone())?
            .settings()
            .in_development
    );
    settings.set_in_development(true);
    settings.invoke_save();
    ui.global::<AppState>().set_in_development(true);
    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
    assert!(!ui.global::<LayoutLibrary>().get_editor_open());
    settings.set_hold_timeout("not a number".into());
    settings.invoke_save();
    assert_eq!(settings.get_message().id, "settings-timing-invalid");
    settings.set_hold_timeout("310".into());
    settings.set_double_tap_timeout("270".into());
    settings.set_trigger_chord_timeout("80".into());
    settings.set_macro_pause("35".into());
    settings.set_modifier_delay("12".into());
    settings.set_appearance(AppearanceChoice::Light);
    settings.set_locale_index(1);
    settings.set_keyboard_device_path("/dev/input/test-keyboard".into());
    settings.set_mouse_device_path("/dev/input/test-mouse".into());
    settings.set_use_fullscreen(true);
    settings.set_use_gamemoded(false);
    settings.set_text_mode_index(4);
    settings.set_ydotool_path("/usr/bin/ydotool".into());
    settings.set_paste_shortcut_index(2);
    settings.set_terminal_paste_shortcut_index(0);
    settings.invoke_save_process(-1, "test-game".into(), true, false);
    settings.invoke_save();
    assert_eq!(settings.get_message().id, "");
    let loaded = ConfigDocument::load(paths.clone())?;
    assert_eq!(loaded.settings(), document.read().settings());
    assert_eq!(loaded.settings().default_hold_timeout_ms, 310);
    assert_eq!(loaded.settings().trigger_chord_timeout_ms, 80);
    assert_eq!(loaded.settings().game_mode.process_matchers.len(), 1);
    assert!(loaded.settings().game_mode.process_matchers[0].only_active_window);
    assert_eq!(settings.get_game_processes().row_count(), 1);
    settings.invoke_save_process(0, "test-game".into(), false, true);
    assert_eq!(settings.get_game_processes().row_count(), 0);
    assert_eq!(settings.get_excluded_processes().row_count(), 1);
    let loaded = ConfigDocument::load(paths.clone())?;
    let matcher = &loaded.settings().game_mode.process_matchers[0];
    assert!(!matcher.only_active_window && matcher.is_blacklist);
    settings.invoke_delete_process(0);
    assert!(
        ConfigDocument::load(paths.clone())?
            .settings()
            .game_mode
            .process_matchers
            .is_empty()
    );
    assert_eq!(
        loaded.settings().linux_wayland_text_mode,
        lhc_core::profile::model::TextMode::Ydotool
    );
    assert_eq!(
        loaded.settings().linux_paste_shortcut,
        lhc_core::profile::model::PasteShortcut::CtrlShiftV
    );
    assert_eq!(loaded.settings().linux_terminal_paste_shortcut, None);
    // Windows has no device paths: the mapper takes the whole session.
    if cfg!(target_os = "windows") {
        settings.invoke_select_device("windows:keyboard".into());
        settings.invoke_select_mouse("windows:mouse".into());
        assert_eq!(settings.get_keyboard_device_path(), "windows:keyboard");
        assert_eq!(settings.get_mouse_device_path(), "windows:mouse");
    } else {
        // Saved devices that are not connected stay listed as their own rows.
        for (devices, path) in [
            (settings.get_input_devices(), "/dev/input/test-keyboard"),
            (settings.get_mouse_devices(), "/dev/input/test-mouse"),
        ] {
            let row = devices
                .iter()
                .find(|row| row.path == path)
                .expect("saved device row");
            assert_eq!(row.group, DeviceGroup::Missing);
        }
        settings.invoke_select_device("".into());
        settings.invoke_select_mouse("".into());
        assert!(document.read().settings().input_device_path.is_none());
        assert!(document.read().settings().input_mouse_device_path.is_none());
        // A path typed by hand is kept even when no such device exists.
        settings.invoke_select_device(" /dev/input/test-keyboard ".into());
        settings.invoke_select_mouse("/dev/input/test-mouse".into());
        settings.invoke_refresh_devices();
        assert_eq!(
            document.read().settings().input_device_path.as_deref(),
            Some("/dev/input/test-keyboard")
        );
        assert_eq!(settings.get_mouse_device_path(), "/dev/input/test-mouse");
    }

    // Changes through the document refresh the settings form.
    document.edit(slint_shell::document::View::Shell, |config| {
        config.update_settings(|s| s.default_double_tap_timeout_ms = 999)
    })?;
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
    ui.global::<LayersEditor>().set_label_mode(LabelMode::Code);
    let reloaded = Document::load(paths.clone())?;
    let second_ui = SettingsWindow::new()?;
    bind_document(&second_ui, &reloaded);
    assert!(reloaded.read().layout().layers.is_empty());
    assert_eq!(second_ui.global::<LayersEditor>().get_selected(), -1);
    assert_eq!(
        second_ui.global::<LayersEditor>().get_label_mode(),
        LabelMode::Label
    );
    ui.global::<LayersEditor>().invoke_reorder(0, 1);
    assert_eq!(ui.global::<LayersEditor>().get_selected(), 0);
    assert_eq!(document.read().layout().layers[0].id, second);
    document.edit(slint_shell::document::View::Library, |config| {
        config.delete_layer(&first)
    })?;

    let quick_popup = QuickPopup::new()?;
    quick_popup.invoke_prepare();

    let picker = ui.global::<ActionPicker>();
    let library = ui.global::<LayoutLibrary>();
    ui.invoke_navigate(Page::Keyboard, MenuKind::Emoji);
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:Привет 👋".into());
    picker.invoke_apply();
    library.invoke_save_as("Test layout".into());
    assert!(!library.get_save_as_open());
    assert!(!library.get_dirty());
    assert_eq!(library.get_current_label(), "Test layout");
    assert!(library.get_editor_open());
    ui.invoke_close_layout();
    assert!(!library.get_editor_open());
    assert_eq!(ui.get_page(), Page::Layouts);
    assert_eq!(library.get_current_label(), "Test layout");
    ui.invoke_navigate(Page::Keyboard, MenuKind::Emoji);
    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
    ui.invoke_close_layout();
    assert!(!library.get_editor_open());
    assert_eq!(ui.get_page(), Page::Settings);
    ui.invoke_navigate(Page::Keyboard, MenuKind::Emoji);
    assert!(library.get_editor_open());
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:Changed".into());
    picker.invoke_apply();
    assert!(library.get_dirty());
    assert!(library.get_rules_dirty());
    assert!(library.get_layers_dirty());
    assert!(!library.get_macros_dirty());
    library.invoke_save_current();
    assert!(
        ui.global::<NotificationCenter>()
            .get_items()
            .iter()
            .any(|item| item.message.id == "layout-saved" && item.message.arg == "Test layout")
    );
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
    for kind in [MenuKind::Quick, MenuKind::Emoji] {
        ui.invoke_navigate(Page::Menus, kind);
        assert_eq!(ui.global::<MenuEditor>().get_kind(), kind);
    }
    document.edit(slint_shell::document::View::Library, |config| {
        config.update_layout(|layout| {
            use lhc_core::profile::model::{Command, KeyRule, Macro, MacroStep};
            layout.rules = ["F13", "F14", "F15"]
                .into_iter()
                .enumerate()
                .map(|(index, key)| {
                    let mut rule = KeyRule::new(format!("drag-rule-{index}"), key);
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
                    posix_script: "printf test".into(),
                    powershell_script: "Write-Output test".into(),
                    working_directory: None,
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
    for key in ["F13", "F14", "F15"] {
        document.edit(slint_shell::document::View::Library, |config| {
            config.set_layer_key(&second, key, KeyAssignment::Action("Escape".into()))
        })?;
    }
    let rules = ui.global::<RulesEditor>();
    let count = document.read().layout().rules.len();
    rules.invoke_add();
    assert_eq!(document.read().layout().rules.len(), count);
    ui.global::<ActionPicker>().invoke_dismiss_picker();
    assert_eq!(document.read().layout().rules.len(), count);
    rules.invoke_add();
    let picker = ui.global::<ActionPicker>();
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("CapsLock".into());
    picker.invoke_apply();
    assert_eq!(rules.get_selected(), 0);
    assert_eq!(document.read().layout().rules[0].key, "CapsLock");
    assert_eq!(document.read().layout().rules[1].key, "F14");
    rules.invoke_open_dialog(0, RuleDialog::Tap);
    picker.invoke_select_behavior(PickerBehavior::Swallow);
    picker.set_value("".into());
    picker.invoke_apply();
    assert_eq!(document.read().layout().rules[0].tap_action, None);
    rules.invoke_open_dialog(0, RuleDialog::Tap);
    picker.invoke_select_behavior(PickerBehavior::Native);
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
    for (selection, expected) in [
        (
            HoldDetection::Release,
            Some(lhc_core::profile::model::TapDecision::PermissiveHold),
        ),
        (
            HoldDetection::Press,
            Some(lhc_core::profile::model::TapDecision::HoldOnOtherKeyPress),
        ),
        (HoldDetection::Inherit, None),
    ] {
        rules.invoke_open_dialog(0, RuleDialog::Advanced);
        rules.set_hold_detection(selection);
        assert!(rules.get_advanced_dirty());
        rules.invoke_apply_advanced();
        assert_eq!(document.read().layout().rules[0].tap_decision, expected);
        rules.invoke_open_dialog(0, RuleDialog::Advanced);
        assert_eq!(rules.get_hold_detection(), selection);
        assert!(!rules.get_advanced_dirty());
        rules.set_dialog(RuleDialog::None);
    }
    rules.invoke_open_dialog(0, RuleDialog::Key);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("ShiftLeft+ControlRight".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[0].key,
        "ShiftLeft+ControlRight"
    );
    rules.invoke_set_hold_mode(HoldBehavior::Action);
    assert_eq!(rules.get_hold_mode(), HoldBehavior::Action);
    rules.invoke_open_dialog(0, RuleDialog::LongHold);
    assert!(!picker.get_hold_action());
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[0].long_hold_action,
        "builtin:copyLineText"
    );
    assert!(document.read().layout().rules[0].layer_id.is_empty());
    rules.invoke_open_dialog(0, RuleDialog::Advanced);
    rules.set_long_hold_timeout("0".into());
    rules.invoke_apply_advanced();
    assert_eq!(rules.get_status().id, "long-hold-invalid");
    rules.set_long_hold_timeout("1500".into());
    rules.invoke_apply_advanced();
    assert_eq!(
        document.read().layout().rules[0].long_hold_timeout_ms,
        Some(1500)
    );
    settings.set_long_hold_timeout("2500".into());
    settings.invoke_save();
    assert_eq!(
        document.read().settings().default_long_hold_timeout_ms,
        2500
    );
    assert_eq!(
        document.read().layout().rules[0].long_hold_action,
        "builtin:copyLineText"
    );
    rules.invoke_set_hold_mode(HoldBehavior::Layer);
    assert!(
        document.read().layout().rules[0]
            .long_hold_action
            .is_empty()
    );
    rules.invoke_open_dialog(0, RuleDialog::Layer);
    rules.invoke_choose(second.clone().into());
    rules.invoke_open_dialog(0, RuleDialog::Advanced);
    rules.set_double_timeout("350".into());
    rules.invoke_apply_advanced();
    rules.invoke_open_dialog(0, RuleDialog::Hold);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("AltLeft".into());
    picker.invoke_apply();
    assert_eq!(rules.get_dialog(), RuleDialog::None);
    assert_eq!(rules.get_double_timeout(), "350");
    assert_eq!(
        document.read().layout().rules[0].hold_action.as_deref(),
        Some("AltLeft")
    );
    assert_eq!(
        document.read().layout().rules[0].double_tap_timeout_ms,
        Some(350)
    );
    rules.invoke_remove();
    let layers = ui.global::<LayersEditor>();
    layers.invoke_add_extra();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys.len(),
        3
    );
    let picker = ui.global::<ActionPicker>();
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys.len(),
        3
    );
    // Choosing the key only moves on to its action; cancelling there adds nothing.
    layers.invoke_add_extra();
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("F16".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert!(!picker.get_key_only());
    assert!(!picker.get_allow_default());
    assert_eq!(picker.get_subject(), "F16");
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys.len(),
        3
    );
    picker.invoke_dismiss_picker();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys.len(),
        3
    );
    layers.invoke_add_extra();
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("F16".into());
    picker.invoke_apply();
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("F1".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys.len(),
        4
    );
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys["F16"].as_deref(),
        Some("F1")
    );
    layers.invoke_reorder_extra(3, 0);
    assert_eq!(layers.get_extras().row_data(0).unwrap().key, "F16");
    layers.invoke_pick_extra(0, false);
    assert!(picker.get_layer_action());
    picker.invoke_select_behavior(PickerBehavior::Swallow);
    picker.set_value("".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys["F16"],
        None
    );
    layers.invoke_pick_extra(0, true);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("F17".into());
    picker.invoke_apply();
    assert!(
        document.read().layout().layer_keymaps[&second]
            .keys
            .contains_key("F17")
    );
    assert_eq!(
        document.read().layout().layer_keymaps[&second].keys["F17"],
        None
    );
    assert_eq!(layers.get_extras().row_data(0).unwrap().key, "F17");
    layers.invoke_remove_extra(0);
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    let picker = ui.global::<ActionPicker>();
    assert!(picker.get_opened());
    assert!(picker.get_layer_action());
    assert_eq!(layers.get_dialog(), LayerDialog::None);
    picker.invoke_select_behavior(PickerBehavior::Swallow);
    picker.set_value("".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().layer_key(&second, "Backquote"),
        lhc_core::config_document::KeyAssignment::Swallow
    );
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    assert!(picker.get_ignore_key());
    picker.invoke_select_behavior(PickerBehavior::Native);
    picker.invoke_apply();
    assert_eq!(
        document.read().layer_key(&second, "Backquote"),
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
    settings.set_appearance(AppearanceChoice::Dark);
    settings.set_locale_index(2);
    settings.invoke_save();
    assert_eq!(
        document.read().settings().appearance,
        lhc_core::profile::model::Appearance::Dark
    );
    library.set_active_label("Test layout".into());
    ui.global::<AppState>().set_keyboard_language("EN".into());
    slint_shell::select_ui_language("ru")?;
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let step = Rc::new(std::cell::Cell::new(-5));
    let command_waits = std::cell::Cell::new(0);
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(400), move || {
        let ui = weak.unwrap();
        let n = step.get();
        match n {
            -5 => {
                let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::Tab, "Маппер", false);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                assert_eq!(ui.global::<SettingsEditor>().get_tab(), SettingsTab::Mapper);
                ui.window().set_size(slint::LogicalSize::new(940.0, 700.0));
            }
            -4 => {
                snapshot(&ui, "settings-keyboard-narrow");
                if !cfg!(target_os = "windows") {
                    // The picker is disabled where the mapper is unsupported.
                    ui.global::<AppState>().set_mapper_supported(true);
                    let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::Combobox, "Устройство клавиатуры", false);
                    for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                    assert!(ui.global::<DeviceDialog>().get_open());
                }
            }
            -3 => {
                if !cfg!(target_os = "windows") {
                    snapshot(&ui, "device-dialog-suggested");
                    let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::RadioButton, "Все устройства", false);
                    for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                }
            }
            -2 => {
                if !cfg!(target_os = "windows") {
                    snapshot(&ui, "device-dialog-all");
                    let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::RadioButton, "Указать путь", false);
                    for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                    snapshot(&ui, "device-dialog-manual");
                    ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Escape.into() });
                    ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Escape.into() });
                    assert!(!ui.global::<DeviceDialog>().get_open());
                    ui.global::<AppState>().set_mapper_supported(false);
                }
                ui.window().set_size(slint::LogicalSize::new(1400.0, 760.0));
            }
            -1 => {
                // Wayland may keep a tiled window at its compositor-controlled size.
                assert!(ui.window().size().width >= (940.0 * ui.window().scale_factor()) as u32);
                snapshot(&ui, "settings-keyboard-wide");
                let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::Tab, "Основное", false);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
            }
            0 => { snapshot(&ui, "settings-dark-ru"); ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(500.0, 450.0), delta_x: 0.0, delta_y: -1200.0 }); }
            1 => { snapshot(&ui, "settings-bottom-ru"); ui.invoke_navigate(Page::Keyboard, MenuKind::Emoji); }
            2 => { snapshot(&ui, "keyboard-ru"); ui.invoke_navigate(Page::Layouts, MenuKind::Emoji); }
            3 => { snapshot(&ui, "layouts-ru"); ui.invoke_navigate(Page::Rules, MenuKind::Emoji); }
            4 => { snapshot(&ui, "rules-ru"); ui.invoke_navigate(Page::Layers, MenuKind::Emoji); }
            5 => {
                ui.global::<MacroEditor>().invoke_add("Shortcut save".into());
                assert!(ui.global::<LayoutLibrary>().get_dirty());
                for active in [false, true] {
                    ui.global::<InlineEditors>().set_window_active(active);
                    for event in [
                        slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Control.into() },
                        slint::platform::WindowEvent::KeyPressed { text: "s".into() },
                        slint::platform::WindowEvent::KeyReleased { text: "s".into() },
                        slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Control.into() },
                    ] { ui.window().dispatch_event(event); }
                    assert_eq!(ui.global::<LayoutLibrary>().get_dirty(), !active,
                        "Ctrl+S saves only when the settings window is active: picker={}, rules={:?}, layers={:?}, library={:?}, save-as={}, status={}",
                        ui.global::<ActionPicker>().get_opened(), ui.global::<RulesEditor>().get_dialog(),
                        ui.global::<LayersEditor>().get_dialog(), ui.global::<LayoutLibrary>().get_dialog(),
                        ui.global::<LayoutLibrary>().get_save_as_open(), ui.global::<LayoutLibrary>().get_status().id);

                }
                ui.global::<LayoutLibrary>().set_save_as_open(true);
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Escape.into() });
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Escape.into() });
                assert!(!ui.global::<LayoutLibrary>().get_save_as_open());
                ui.global::<LayersEditor>().invoke_open_dialog(slint_shell::ui::LayerDialog::Create, -1);
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Escape.into() });
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Escape.into() });
                assert_eq!(ui.global::<LayersEditor>().get_dialog(), slint_shell::ui::LayerDialog::None);
                snapshot(&ui, "layers-ru"); ui.invoke_navigate(Page::Macros, MenuKind::Emoji); }
            6 => { snapshot(&ui, "macros-ru"); ui.invoke_navigate(Page::Menus, MenuKind::Emoji); }
            7 => { snapshot(&ui, "emoji-ru"); ui.invoke_navigate(Page::Menus, MenuKind::Quick); }
            8 => { snapshot(&ui, "quick-ru"); ui.invoke_navigate(Page::Settings, MenuKind::Emoji); }
            9 => { snapshot(&ui, "settings-commands-ru"); ui.invoke_navigate(Page::Settings, MenuKind::Emoji); ui.global::<SettingsEditor>().set_appearance(AppearanceChoice::Light); ui.global::<SettingsEditor>().set_locale_index(1); ui.global::<SettingsEditor>().invoke_save(); slint_shell::select_ui_language("en").unwrap(); ui.global::<Theme>().set_dark(false); ui.global::<Theme>().invoke_apply(); ui.window().set_size(slint::LogicalSize::new(940.0, 700.0)); }
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
                let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::Button, "Game mode:", true);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
            }
            15 => { snapshot(&ui, "game-mode-menu-dark");
                for text in [slint::platform::Key::DownArrow.into(), "\n".into()] {
                    ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text });
                }
                assert_eq!(ui.global::<AppState>().get_game_control(), GameModeControl::On);
            }
            16 => { snapshot(&ui, "game-mode-menu-selected"); ui.invoke_navigate(Page::Settings, MenuKind::Emoji); ui.global::<SettingsEditor>().set_hold_timeout("321".into()); ui.global::<SettingsEditor>().invoke_schedule_save(); }
            17 => ui.global::<SettingsEditor>().set_tab(SettingsTab::GameMode),
            18 => {
                snapshot(&ui, "settings-game-mode");
                let position = slint::LogicalPosition::new(500.0, 450.0);
                ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: -900.0 });
                snapshot(&ui, "settings-game-mode-bottom");
                ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: 900.0 });
                ui.global::<SettingsEditor>().set_tab(SettingsTab::General); assert_eq!(document.read().settings().default_hold_timeout_ms, 321); ui.global::<SettingsEditor>().set_appearance(AppearanceChoice::EInk); ui.global::<SettingsEditor>().invoke_save(); assert_eq!(ConfigDocument::load(paths.clone()).unwrap().settings().appearance, lhc_core::profile::model::Appearance::EInk); ui.global::<Theme>().set_eink(true); ui.global::<Theme>().set_dark(false); slint_shell::ui::apply_theme(&ui.global::<Theme>()); }
            19 => { snapshot(&ui, "settings-eink"); ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(500.0, 450.0), delta_x: 0.0, delta_y: -450.0 }); }
            20 => { snapshot(&ui, "settings-behavior-eink"); ui.global::<SettingsEditor>().set_message(Message { id: "settings-timing-invalid".into(), arg: "".into(), count: 0 }); }
            21 => {
                snapshot(&ui, "settings-validation");
                ui.global::<SettingsEditor>().set_message(Message::default());
                ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
                ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
                let rules = ui.global::<RulesEditor>();
                let picker = ui.global::<ActionPicker>();
                rules.invoke_add();
                picker.invoke_select_behavior(PickerBehavior::Action);
                picker.set_value("ShiftLeft+ControlRight".into());
                picker.invoke_apply();
                rules.invoke_set_hold_mode(HoldBehavior::Action);
                rules.invoke_open_dialog(0, RuleDialog::LongHold);
                picker.invoke_select_behavior(PickerBehavior::Action);
                picker.set_value("text:hello".into());
                picker.invoke_apply();
            }
            22 => { snapshot(&ui, "rules-long-hold"); ui.global::<RulesEditor>().invoke_open_dialog(0, RuleDialog::Advanced); }
            23 => {
                snapshot(&ui, "rules-long-hold-advanced");
                let rules = ui.global::<RulesEditor>();
                rules.set_dialog(RuleDialog::None);
                rules.invoke_set_hold_mode(HoldBehavior::Layer);
                rules.invoke_open_dialog(0, RuleDialog::Layer);
                let id = document.read().layout().layers[0].id.clone();
                rules.invoke_choose(id.into());
            }
            24 => { snapshot(&ui, "rules-layer"); ui.global::<RulesEditor>().invoke_open_dialog(1, RuleDialog::Advanced); ui.global::<RulesEditor>().set_hold_detection(HoldDetection::Release); }
            25 => { snapshot(&ui, "rules-hold-detection"); ui.global::<RulesEditor>().set_dialog(RuleDialog::None); ui.global::<RulesEditor>().invoke_open_dialog(0, RuleDialog::Conditions); }
            26 => {
                snapshot(&ui, "rules-conditions");
                ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(700.0, 450.0), delta_x: 0.0, delta_y: -700.0 });
            }
            27 => {
                snapshot(&ui, "rules-conditions-exclusions");
                ui.global::<RulesEditor>().set_dialog(RuleDialog::None);
                ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
                ui.global::<SettingsEditor>().set_tab(SettingsTab::KeyBehavior);
                ui.window().set_size(slint::LogicalSize::new(940.0, 700.0));
            }
            28 => {
                snapshot(&ui, "settings-behavior");
                let position = numeric_field_position(&ui, "Long hold");
                let wheel = |delta_y| ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y });
                let duration = || ui.global::<SettingsEditor>().get_long_hold_timeout().parse::<u64>().unwrap();
                assert_eq!(duration(), 2500);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                wheel(60.0);
                assert_eq!(duration(), 2600, "duration after wheel");
                wheel(-60.0);
                assert_eq!(duration(), 2500);
                ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
                ui.global::<RulesEditor>().invoke_open_dialog(1, RuleDialog::Advanced);
                ui.global::<RulesEditor>().invoke_open_dialog(1, RuleDialog::Hold);
                ui.global::<ActionPicker>().invoke_select_behavior(PickerBehavior::Action);
                ui.global::<ActionPicker>().set_value("Ctrl".into());
            }
            29 => {
                let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::TextInput, "Action:", false);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                for text in [slint::platform::Key::End.into(), slint::platform::Key::Shift.into(), "+".into()] {
                    ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text });
                }
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Shift.into() });
                assert_eq!(ui.global::<ActionPicker>().get_value(), "Ctrl+", "typing + preserves an incomplete combination");
                for text in ["K", "e", "y", "C"] {
                    ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.into() });
                }
                assert_eq!(ui.global::<ActionPicker>().get_value(), "Ctrl+KeyC");
                assert!(ui.global::<ActionPicker>().get_valid());
                snapshot(&ui, "rules-held-combination");
                ui.global::<ActionPicker>().invoke_apply();
                assert_eq!(ui.global::<RulesEditor>().get_hold(), "Ctrl+KeyC");
                ui.global::<RulesEditor>().set_hold_timeout("333".into());
                ui.global::<RulesEditor>().set_double_timeout("444".into());
            }
            30 => {
                let label = format!("Use the default from Settings: {} ms", ui.global::<RulesEditor>().get_default_hold_timeout());
                let position = accessible_position(&ui, i_slint_core::items::AccessibleRole::Button, &label, false);
                for event in [slint::platform::WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left }, slint::platform::WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left }] { ui.window().dispatch_event(event); }
                let rules = ui.global::<RulesEditor>();
                assert_eq!(rules.get_hold_timeout(), "");
                assert_eq!(rules.get_double_timeout(), "444");
                assert_eq!(rules.get_hold(), "Ctrl+KeyC");
                rules.invoke_apply_advanced();
                assert_eq!(document.read().layout().rules[1].hold_timeout_ms, None);
                assert_eq!(document.read().layout().rules[1].double_tap_timeout_ms, Some(444));
                rules.invoke_open_dialog(1, RuleDialog::Advanced);
                slint_shell::select_ui_language("ru").unwrap();
                ui.global::<Theme>().set_eink(false);
                ui.global::<Theme>().set_dark(true);
                slint_shell::ui::apply_theme(&ui.global::<Theme>());
            }
            31 => {
                snapshot(&ui, "rules-advanced-dark-ru");
                paths.save_user_layout("Broken overview", "invalid json", true).unwrap();
                document.edit(slint_shell::document::View::Settings, |config| config.update_settings(|s| s.reduce_motion = !s.reduce_motion)).unwrap();
                assert!(!ui.global::<SettingsEditor>().get_commands_loading(), "a hidden command panel must not read the library");
                ui.global::<RulesEditor>().set_dialog(RuleDialog::None);
                ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
                ui.global::<SettingsEditor>().set_tab(SettingsTab::Commands);
                ui.global::<SettingsEditor>().invoke_refresh_commands();
            }
            32 | 33 | 35 => {
                let settings = ui.global::<SettingsEditor>();
                if settings.get_commands_loading() {
                    command_waits.set(command_waits.get() + 1);
                    assert!(command_waits.get() < 50, "command overview load timed out");
                    return;
                }
                if n == 32 || n == 35 {
                    assert_eq!(settings.get_commands_error().id, "config-parse");
                    assert!(!settings.get_commands_loaded());
                    paths.delete_user_layout("Broken overview").unwrap();
                    settings.invoke_refresh_commands();
                } else {
                    assert!(settings.get_commands_loaded());
                    assert_eq!(settings.get_commands_error().id, "");
                    paths.save_user_layout("Broken overview", "invalid json", true).unwrap();
                    document.edit(slint_shell::document::View::Menus, |config| config.update_layout(|layout| layout.emoji_pages[0].name = "Cached overview".into())).unwrap();
                    settings.invoke_refresh_commands();
                    assert!(!settings.get_commands_loading(), "emoji-only changes must reuse the overview");
                }
            }
            34 => {
                assert_eq!(ui.global::<SettingsEditor>().get_commands_error().id, "");
                ui.global::<LayoutLibrary>().invoke_refresh();
                ui.global::<SettingsEditor>().invoke_refresh_commands();
            }
            36 => {
                let settings = ui.global::<SettingsEditor>();
                if settings.get_commands_loading() { return; }
                assert!(settings.get_commands_loaded());
                assert_eq!(settings.get_commands_error().id, "");
                println!("Interactions passed: settings, rules, persistence, navigation, background command overview, caching and error recovery");
                slint::quit_event_loop().unwrap();
            }

            _ => {}
        }
        step.set(n + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
