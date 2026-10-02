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
    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
    let rules = ui.global::<RulesEditor>();
    rules.invoke_add();
    let index = rules.get_selected();
    assert!(index >= 0);
    let picker = ui.global::<ActionPicker>();
    rules.invoke_open_dialog(index, RuleDialog::Key);
    assert!(picker.get_opened());
    assert!(picker.get_key_only());
    assert_eq!(rules.get_dialog(), RuleDialog::None, "the picker replaces the dialog");
    assert_eq!(
        picker.get_counts().iter().collect::<Vec<_>>(),
        [40, 37, 27, 15, 8, 43, 0, 0, 0, 0, 0]
    );
    picker.set_value("Tab".into());
    picker.invoke_close();
    assert_eq!(document.read().layout().rules[index as usize].key, "");
    rules.invoke_open_dialog(index, RuleDialog::Key);
    picker.set_value("macro:copyLine".into());
    picker.invoke_apply();
    assert!(picker.get_opened(), "a trigger must be a key");
    picker.set_value("Tab".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(document.read().layout().rules[index as usize].key, "Tab");
    rules.invoke_open_dialog(index, RuleDialog::Tap);
    picker.set_query("громкость".into());
    picker.invoke_refresh();
    assert!(picker.get_items().row_count() >= 2);
    picker.set_value("macro:copyLine".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[index as usize].tap_action.as_deref(),
        Some("macro:copyLine")
    );
    let macros = ui.global::<MacroEditor>();
    macros.invoke_add("Self".into());
    macros.invoke_set_field(0, MacroField::Id, "self".into());
    macros.invoke_add_step(0, "KeyA".into());
    macros.set_picker_macro(0);
    picker.invoke_open(PickerTarget::MacroStep, 0, "KeyA".into(), false);
    picker.set_value("macro:self".into());
    picker.invoke_apply();
    assert!(picker.get_opened(), "a macro cannot call itself");
    picker.set_value("pause:250".into());
    picker.invoke_apply();
    assert_eq!(
        macros.get_macros().row_data(0).unwrap().steps.row_data(0).unwrap().action,
        "pause:250"
    );
    assert_eq!(document.read().layout().macros[0].steps[0].action, "pause:250");
    ui.invoke_navigate(Page::Menus, MenuKind::Quick);
    picker.invoke_open(PickerTarget::QuickAction, 1, "".into(), false);
    picker.set_value("text:Привет\nмир".into());
    picker.invoke_apply();
    let menus = ui.global::<MenuEditor>();
    assert_eq!(menus.get_value(), "text:Привет\nмир");
    assert_eq!(menus.get_selected_cell(), 1);
    assert_eq!(document.read().layout().quick_actions[1].action, "text:Привет\nмир");
    let layers = ui.global::<LayersEditor>();
    layers.set_dialog_key("F13".into());
    picker.invoke_open(PickerTarget::LayerKey, 0, "F13".into(), true);
    picker.set_value("NumpadEnter".into());
    picker.invoke_apply();
    assert_eq!(layers.get_dialog_key(), "NumpadEnter");
    picker.invoke_open(PickerTarget::LayerAction, 0, "".into(), false);
    picker.set_value("Ctrl+KeyC".into());
    picker.invoke_apply();
    assert_eq!(layers.get_dialog_action(), "Ctrl+KeyC");
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.set_value("text:example".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(document.read().base_tap_action("KeyQ"), Some("text:example"));
    rules.invoke_open_dialog(index, RuleDialog::Key);
    if let Ok(category) = std::env::var("LHC_PICKER_CATEGORY") {
        rules.invoke_open_dialog(index, RuleDialog::Tap);
        picker.set_category(category.parse()?);
        picker.invoke_refresh();
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
