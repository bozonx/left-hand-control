use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model};
use slint_shell::{Document, bind_document, ui::*};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    check_atomic_commands()?;
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    slint::select_bundled_translation("ru")?;
    bind_document(&ui, &document);
    // Hidden pages and action categories stay unavailable outside development mode.
    ui.invoke_navigate(Page::Macros, MenuKind::Emoji);
    assert_eq!(ui.get_page(), Page::Layouts);
    ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
    assert_eq!(ui.get_page(), Page::Layouts);
    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
    ui.global::<RulesEditor>().invoke_add();
    let restricted = ui.global::<ActionPicker>();
    restricted.set_query("Mouse".into());
    restricted.invoke_refresh();
    assert_eq!(restricted.get_items().row_count(), 0);
    assert!(
        restricted
            .get_items()
            .iter()
            .all(|item| item.category != PickerCategory::Mouse)
    );
    restricted.invoke_dismiss_picker();
    let settings = ui.global::<SettingsEditor>();
    settings.set_in_development(true);
    settings.invoke_save();
    ui.global::<AppState>().set_in_development(true);

    let rules = ui.global::<RulesEditor>();
    let count = document.read().layout().rules.len();
    rules.invoke_add();
    let picker = ui.global::<ActionPicker>();
    assert!(picker.get_opened());
    assert!(picker.get_key_only());
    assert_eq!(
        rules.get_dialog(),
        RuleDialog::None,
        "the picker replaces the dialog"
    );
    assert_eq!(
        picker
            .get_categories()
            .iter()
            .map(|entry| entry.count)
            .collect::<Vec<_>>(),
        [48, 9, 35, 29, 36, 44, 5, 69, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    picker.set_category(PickerCategory::Modifiers);
    picker.invoke_refresh();
    assert_eq!(picker.get_items().row_count(), 9);
    assert!(
        picker
            .get_items()
            .iter()
            .any(|item| item.value == "ControlLeft")
    );
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Tab".into());
    picker.invoke_dismiss_picker();
    assert_eq!(document.read().layout().rules.len(), count);
    rules.invoke_add();
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert!(picker.get_opened(), "a trigger must be a key");
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Tab".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    let index = rules.get_selected();
    assert!(index >= 0);
    assert_eq!(document.read().layout().rules[index as usize].key, "Tab");
    rules.invoke_open_dialog(index, RuleDialog::Tap);
    picker.set_category(PickerCategory::Text);
    picker.set_query("ck".into());
    picker.invoke_refresh();
    // Backspace, MediaTrackNext and MouseBack.
    for category in [
        PickerCategory::ControlKeys,
        PickerCategory::Media,
        PickerCategory::Mouse,
    ] {
        assert!(
            picker
                .get_items()
                .iter()
                .any(|item| item.category == category)
        );
    }
    assert_eq!(picker.get_category(), PickerCategory::Text);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("KeyA".into());
    picker.invoke_refresh();
    assert_eq!(picker.get_value_category(), PickerCategory::Letters);
    picker.set_query("рабочий стол".into());
    picker.invoke_refresh();
    assert!(picker.get_items().row_count() >= 2);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[index as usize]
            .tap_action
            .as_deref(),
        Some("builtin:copyLineText")
    );
    rules.invoke_open_dialog(index, RuleDialog::Tap);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("KeyB".into());
    picker.invoke_select_behavior(PickerBehavior::Swallow);
    assert_eq!(picker.get_value(), "KeyB");
    picker.invoke_select_behavior(PickerBehavior::Action);
    assert_eq!(picker.get_value(), "KeyB");
    picker.set_value("".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert!(!picker.get_valid());
    picker.invoke_dismiss_picker();
    assert_eq!(
        document.read().layout().rules[index as usize]
            .tap_action
            .as_deref(),
        Some("builtin:copyLineText")
    );
    rules.invoke_open_dialog(index, RuleDialog::Hold);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    rules.invoke_open_dialog(index, RuleDialog::Key);
    picker.set_value("MouseLeft".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    let macros = ui.global::<MacroEditor>();
    macros.invoke_add("Self".into());
    macros.invoke_set_field(0, MacroField::Id, "self".into());
    macros.invoke_add_step(0, "KeyA".into());
    macros.set_picker_macro(0);
    picker.invoke_open(PickerTarget::MacroStep, 0, "KeyA".into(), false);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("macro:self".into());
    picker.invoke_apply();
    assert!(picker.get_opened(), "a macro cannot call itself");
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_category(PickerCategory::Pause);
    picker.invoke_refresh();
    assert_eq!(picker.get_value(), "pause:20");
    macros.invoke_set_field(0, MacroField::StepPause, "75".into());
    picker.set_category(PickerCategory::Modifiers);
    picker.set_value("KeyA".into());
    picker.invoke_refresh();
    picker.set_category(PickerCategory::Pause);
    picker.invoke_refresh();
    assert_eq!(picker.get_value(), "pause:75");
    picker.set_value("pause:250".into());
    picker.invoke_refresh();
    assert_eq!(
        picker.get_value(),
        "pause:250",
        "refresh preserves explicit pauses"
    );
    picker.set_value("pause:".into());
    picker.invoke_refresh();
    assert!(!picker.get_valid(), "a pause requires an explicit duration");
    picker.set_value("pause:250".into());
    picker.invoke_apply();
    assert_eq!(
        macros
            .get_macros()
            .row_data(0)
            .unwrap()
            .steps
            .row_data(0)
            .unwrap()
            .action,
        "pause:250"
    );
    assert_eq!(
        document.read().layout().macros[0].steps[0].action,
        "pause:250"
    );
    ui.invoke_navigate(Page::Menus, MenuKind::Quick);
    picker.invoke_open(PickerTarget::QuickAction, 1, "".into(), false);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:Привет\nмир".into());
    picker.invoke_apply();
    let menus = ui.global::<MenuEditor>();
    assert_eq!(menus.get_value(), "text:Привет\nмир");
    assert_eq!(menus.get_selected_cell(), 1);
    assert_eq!(
        quick_action(document.read().layout(), 0, "KeyW"),
        "text:Привет\nмир"
    );
    let layers = ui.global::<LayersEditor>();
    layers.set_dialog(LayerDialog::EditKey);
    picker.invoke_open(PickerTarget::LayerAction, 0, "".into(), false);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Ctrl+KeyC".into());
    picker.invoke_apply();
    assert_eq!(layers.get_dialog_action(), "Ctrl+KeyC");
    layers.set_dialog(LayerDialog::None);
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:example".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some("text:example")
    );
    rules.invoke_open_dialog(index, RuleDialog::Key);
    if let Ok(category) = std::env::var("LHC_PICKER_CATEGORY") {
        rules.invoke_open_dialog(index, RuleDialog::Tap);
        picker.set_category(parse_category(&category)?);
        picker.invoke_refresh();
    }
    if let Ok(query) = std::env::var("LHC_PICKER_QUERY") {
        picker.set_query(query.into());
        picker.invoke_refresh();
    }
    if let Ok(value) = std::env::var("LHC_PICKER_VALUE") {
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_value(value.into());
        picker.invoke_refresh();
    }
    if std::env::var_os("LHC_RULE_ADVANCED_PREVIEW").is_some() {
        ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
        picker.invoke_dismiss_picker();
        rules.invoke_open_dialog(index, RuleDialog::Advanced);
    }
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(954.0, 700.0));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(600), move || {
        let ui = weak.unwrap();
        if let Some(path) = std::env::var_os("LHC_PICKER_SNAPSHOT") {
            let pixels = ui.window().take_snapshot().unwrap();
            let mut bytes = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
            for pixel in pixels.as_slice() {
                bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
            }
            std::fs::write(path, bytes).unwrap();
        }
        println!("Picker passed: catalog, trigger validation, cancel, rule actions, macro steps and quick actions");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}

/// `LHC_PICKER_CATEGORY` value: the kebab-case name of a `PickerCategory`.
fn parse_category(name: &str) -> Result<PickerCategory, String> {
    use PickerCategory as C;
    Ok(match name {
        "modifiers" => C::Modifiers,
        "letters" => C::Letters,
        "control-keys" => C::ControlKeys,
        "navigation" => C::Navigation,
        "media" => C::Media,
        "system-keys" => C::SystemKeys,
        "mouse" => C::Mouse,
        "other-keys" => C::OtherKeys,
        "macros" => C::Macros,
        "builtin-macros" => C::BuiltinMacros,
        "commands" => C::Commands,
        "app" => C::App,
        "system-actions" => C::SystemActions,
        "desktop-shortcuts" => C::DesktopShortcuts,
        "text" => C::Text,
        "pause" => C::Pause,
        other => return Err(format!("unknown picker category: {other}")),
    })
}

/// Action of quick action cell `key` on `page`, empty when the cell is empty.
fn quick_action(layout: &lhc_core::profile::model::LayoutPreset, page: usize, key: &str) -> String {
    layout.quick_action_pages[page]
        .cells
        .get(key)
        .map(|item| item.action.clone())
        .unwrap_or_default()
}

/// Command edits stay with their target, including invalid page drafts.
fn check_atomic_commands() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    let settings = ui.global::<SettingsEditor>();
    settings.set_commands_enabled(true);
    settings.set_in_development(true);
    settings.invoke_save();
    ui.global::<AppState>().set_in_development(true);
    let picker = ui.global::<ActionPicker>();
    let edit_command = |script: &str| {
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_command_script(script.into());
        picker.invoke_edit_command();
    };
    let before = document.read().layout().clone();
    picker.invoke_open(PickerTarget::BaseKey, i32::MAX, "".into(), false);
    edit_command("echo invalid-target");
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_eq!(picker.get_error().id, "config-edit-conflict");
    assert_eq!(document.read().layout(), &before);
    picker.invoke_dismiss_picker();
    ui.global::<KeyEditor>().invoke_edit(33);
    edit_command("echo assigned");
    let action = picker.get_value().to_string();
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some(action.as_str())
    );
    assert_eq!(document.read().layout().commands.len(), 1);
    let before = document.read().layout().clone();
    picker.invoke_open(PickerTarget::BaseKey, i32::MAX, action.into(), false);
    edit_command("echo must-not-overwrite");
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_eq!(document.read().layout(), &before);
    picker.invoke_dismiss_picker();

    let macros = ui.global::<MacroEditor>();
    macros.invoke_add("Draft".into());
    macros.invoke_set_field(0, MacroField::ModifierDelay, "invalid".into());
    macros.invoke_add_step(0, "".into());
    edit_command("echo macro-draft");
    let action = picker.get_value();
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(document.read().layout().commands.len(), 1);
    assert!(document.read().layout().macros[0].steps.is_empty());
    assert!(
        !ui.invoke_flush_drafts(),
        "quit must keep an invalid macro draft"
    );
    assert_eq!(ui.get_page(), Page::Macros);
    // Reopening a pending command restores its fields without committing it.
    macros.set_picker_macro(0);
    picker.invoke_open(PickerTarget::MacroStep, 0, action, false);
    assert_eq!(picker.get_command_script(), "echo macro-draft");
    picker.invoke_dismiss_picker();
    macros.invoke_set_field(0, MacroField::ModifierDelay, "10".into());
    assert!(ui.invoke_flush_drafts());
    assert_eq!(document.read().layout().commands.len(), 2);
    assert_eq!(document.read().layout().macros[0].steps.len(), 1);

    // A quick-menu conflict keeps both the pending command and the local menu.
    let menus = ui.global::<MenuEditor>();
    menus.invoke_open(MenuKind::Quick);
    menus.set_page_name("Local".into());
    menus.invoke_rename_page();
    document.edit(slint_shell::document::View::Shell, |config| {
        config.update_layout(|layout| {
            layout.quick_action_pages[0].name = "Concurrent".into();
        })
    })?;
    picker.invoke_open(PickerTarget::QuickAction, 0, "".into(), false);
    edit_command("echo menu-draft");
    let action = picker.get_value();
    picker.invoke_apply();
    assert_eq!(document.read().layout().commands.len(), 2);
    assert!(
        !ui.invoke_flush_drafts(),
        "quit must keep a conflicting menu draft"
    );
    assert_eq!(ui.get_page(), Page::Menus);
    assert_eq!(menus.get_value(), action);
    picker.invoke_open(PickerTarget::QuickAction, 0, action, false);
    assert_eq!(picker.get_command_script(), "echo menu-draft");
    picker.invoke_dismiss_picker();
    menus.invoke_refresh();
    assert!(ui.invoke_flush_drafts());
    assert_eq!(
        document.read().layout().commands.len(),
        2,
        "discarding the menu discards its command"
    );

    settings.set_hold_timeout("invalid".into());
    assert!(!ui.invoke_flush_drafts(), "quit must keep invalid settings");
    assert_eq!(ui.get_page(), Page::Settings);
    settings.set_hold_timeout(settings.get_default_hold_timeout());
    assert!(ui.invoke_flush_drafts());

    // The layer-key dialog keeps its command until its own Apply button.
    document.edit(slint_shell::document::View::Shell, |config| {
        config.create_layer("Layer", "")
    })?;
    let layers = ui.global::<LayersEditor>();
    layers.invoke_choose(0);
    layers.set_dialog_index(0);
    layers.set_assignment(Assignment::Action);
    layers.set_dialog(LayerDialog::EditKey);
    picker.invoke_open(PickerTarget::LayerAction, 0, "".into(), false);
    edit_command("echo layer-draft");
    let action = picker.get_value();
    picker.invoke_apply();
    assert_eq!(document.read().layout().commands.len(), 2);
    assert_eq!(layers.get_dialog_action(), action);
    picker.invoke_open(PickerTarget::LayerAction, 0, action.clone(), false);
    assert_eq!(picker.get_command_script(), "echo layer-draft");
    picker.invoke_dismiss_picker();
    layers.invoke_apply_dialog();
    assert_eq!(layers.get_dialog(), LayerDialog::None);
    assert_eq!(document.read().layout().commands.len(), 3);
    assert_eq!(
        document.read().layer_key(
            &layers.get_selected_id(),
            slint_shell::keyboard::layer_key(0).unwrap()
        ),
        lhc_core::config_document::KeyAssignment::Action(action.to_string())
    );
    Ok(())
}
