use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model};
use slint_shell::{Document, bind_document, ui::*};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    picker.set_value("builtin:copyLine".into());
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
    picker.set_value("builtin:copyLine".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[index as usize]
            .tap_action
            .as_deref(),
        Some("builtin:copyLine")
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
        Some("builtin:copyLine")
    );
    rules.invoke_open_dialog(index, RuleDialog::Hold);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLine".into());
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
