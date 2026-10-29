mod support;
use slint::{ComponentHandle, Model};
use slint_shell::{
    Document,
    ui::{Locale, MacroEditor, MacroField, Page, Theme},
};
use std::{rc::Rc, time::Duration};
use support::{Fixture, wait_until};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Fixture {
        dir: _dir,
        paths: _,
        document,
        ui,
    } = Fixture::new()?;
    let development = ui.global::<slint_shell::ui::SettingsEditor>();
    development.set_in_development(true);
    development.invoke_save();
    ui.global::<slint_shell::ui::AppState>()
        .set_in_development(true);
    ui.set_page(Page::Macros);
    ui.global::<Theme>().invoke_apply();
    slint_shell::select_ui_language("ru")?;
    let editor = ui.global::<MacroEditor>();
    let name = ui
        .global::<Locale>()
        .invoke_text(editor.get_builtin_macros().row_data(0).unwrap().name);
    assert_eq!(name, "Пустая строка выше");
    slint_shell::select_ui_language("en")?;
    assert_eq!(
        ui.global::<Locale>()
            .invoke_text(editor.get_builtin_macros().row_data(0).unwrap().name),
        "Empty line above"
    );
    slint_shell::select_ui_language("ru")?;
    let copy_name = ui.global::<Locale>().invoke_text(slint_shell::ui::Message {
        id: "copy-name".into(),
        arg: name,
        count: 0,
    });
    editor.invoke_clone_builtin(0, copy_name);
    let first = |document: &Rc<Document>| document.read().layout().macros[0].clone();
    assert_eq!(first(&document).steps.len(), 3);
    assert_eq!(first(&document).name, "Пустая строка выше (копия)");
    assert_eq!(first(&document).id, "emptyLineAboveCopy");
    editor.invoke_set_field(0, MacroField::Id, "emptyLineAbove".into());
    wait_until("macro autosave", || !editor.get_has_unsaved_changes());
    assert!(!editor.get_has_errors());
    assert_eq!(first(&document).id, "emptyLineAbove");
    editor.invoke_set_field(0, MacroField::Id, "testMacro".into());
    editor.invoke_set_field(0, MacroField::Name, "Тестовый макрос".into());
    editor.invoke_add_step(0, "pause:100".into());
    editor.invoke_add_step(0, "text: hello ".into());
    editor.invoke_move_step(0, 4, 3);
    editor.invoke_set_field(0, MacroField::StepPause, "0".into());
    wait_until("macro autosave", || !editor.get_has_unsaved_changes());
    assert!(!editor.get_has_errors());
    assert_eq!(first(&document).id, "testMacro");
    assert_eq!(first(&document).steps[3].action, "text: hello ");
    assert_eq!(first(&document).step_pause_ms, Some(0));
    editor.invoke_set_step(0, 4, "pause:250".into());
    assert_eq!(first(&document).steps[4].action, "pause:250");
    editor.invoke_add_step(0, "macro:testMacro".into());
    assert!(editor.get_has_errors());
    assert_eq!(
        editor.get_macros().row_data(0).unwrap().error.id,
        "macro-cycle"
    );
    assert_eq!(first(&document).steps.len(), 5);
    document.edit(slint_shell::document::View::Settings, |config| {
        config.update_settings(|s| s.reduce_motion = !s.reduce_motion)
    })?;
    assert_eq!(
        editor.get_macros().row_data(0).unwrap().steps.row_count(),
        6
    );
    assert!(editor.get_has_errors());
    assert!(editor.get_has_unsaved_changes());
    assert!(
        !ui.global::<slint_shell::ui::LayoutLibrary>()
            .invoke_flush_drafts()
    );
    editor.invoke_remove_step(0, 5);
    assert!(!editor.get_has_errors());
    editor.invoke_set_field(0, MacroField::ModifierDelay, "abc".into());
    assert!(editor.get_has_errors());
    editor.invoke_set_field(0, MacroField::ModifierDelay, "".into());
    wait_until("macro autosave", || !editor.get_has_unsaved_changes());
    editor.invoke_add("Второй".into());
    editor.invoke_move(0, 1);
    assert_eq!(document.read().layout().macros[0].id, "testMacro");
    editor.invoke_remove(1);
    let loaded = document.read();
    assert_eq!(loaded.layout().macros.len(), 1);
    assert_eq!(loaded.layout().macros[0].name, "Тестовый макрос");
    drop(loaded);
    editor.invoke_add_step(0, "".into());
    let picker = ui.global::<slint_shell::ui::ActionPicker>();
    picker.set_value("KeyA".into());
    picker.invoke_apply();
    let library = ui.global::<slint_shell::ui::LayoutLibrary>();
    library.set_save_as_name("Draft regression".into());
    library.invoke_save_as("Draft regression".into());
    editor.invoke_set_field(0, MacroField::Name, "Saved without waiting".into());
    library.invoke_save_current();
    let saved = document.read().load_layout("user:Draft regression")?;
    assert_eq!(saved.macros[0].name, "Saved without waiting");
    editor.invoke_set_field(0, MacroField::Id, "invalid id".into());
    document.edit(slint_shell::document::View::Layers, |config| {
        config.create_layer("Draft test", "")
    })?;
    assert_eq!(editor.get_macros().row_data(0).unwrap().id, "invalid id");
    assert!(!library.invoke_flush_drafts());
    library.invoke_save_current();
    assert_eq!(
        document.read().load_layout("user:Draft regression")?.macros[0].id,
        saved.macros[0].id
    );
    editor.invoke_refresh();
    assert!(!editor.get_has_errors());
    assert!(!editor.get_has_unsaved_changes());
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(700), move || {
        let ui = weak.upgrade().unwrap();
        if std::env::var_os("LHC_MACROS_SCROLL").is_some() {
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(500.0, 400.0), delta_x: 0.0, delta_y: -600.0,
            });
        }
        if let Ok(path) = std::env::var("LHC_MACROS_SNAPSHOT") {
            support::write_snapshot(&ui, std::path::Path::new(&path));
        }
        println!("Macros smoke: passed (built-in copy, steps, pauses, order, cycle validation, delayed save and reload)");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
