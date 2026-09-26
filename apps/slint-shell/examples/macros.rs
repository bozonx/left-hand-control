use lhc_core::{config_document::ConfigDocument, storage::StoragePaths};
use slint::{ComponentHandle, Model};
use slint_shell::{
    macro_editor,
    ui::{Locale, MacroEditor, SettingsWindow, Theme},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
    let document = Rc::new(RefCell::new(ConfigDocument::load(paths.clone())?));
    let ui = SettingsWindow::new()?;
    macro_editor::bind(&ui, Some(document.clone()));
    ui.set_page(5);
    ui.global::<Theme>().invoke_apply();
    slint::select_bundled_translation("ru")?;
    ui.global::<Locale>().set_english(false);
    let editor = ui.global::<MacroEditor>();
    editor.invoke_clone_system(0);
    assert!(editor.get_editing());
    assert_eq!(editor.get_steps().row_count(), 3);
    editor.set_macro_id("testMacro".into());
    editor.set_name("Тестовый макрос".into());
    editor.invoke_add_step("pause:100".into());
    editor.invoke_add_step("text: hello ".into());
    editor.invoke_move_step(4, -1);
    editor.set_step_pause("0".into());
    editor.invoke_save();
    assert!(!editor.get_editing());
    assert_eq!(
        document.borrow().layout().macros[0].steps[3].action,
        "text: hello "
    );
    editor.invoke_edit(0);
    editor.invoke_add_step("macro:testMacro".into());
    editor.invoke_save();
    assert!(editor.get_editing());
    assert_eq!(document.borrow().layout().macros[0].steps.len(), 5);
    editor.invoke_remove_step(5);
    editor.invoke_save();
    assert!(!editor.get_editing());
    assert_eq!(ConfigDocument::load(paths)?.layout().macros.len(), 1);
    editor.invoke_edit(0);
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(940.0, 700.0));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(700), move || {
        let ui = weak.upgrade().unwrap();
        if let Ok(path) = std::env::var("SLINT_SHELL_MACROS_SNAPSHOT") {
            let pixels = ui.window().take_snapshot().unwrap();
            let mut data = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
            for p in pixels.as_slice() { data.extend([p.r, p.g, p.b]); }
            std::fs::write(path, data).unwrap();
        }
        println!("Macros smoke: passed (system copy, steps, pauses, order, cycle validation, save and reload)");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
