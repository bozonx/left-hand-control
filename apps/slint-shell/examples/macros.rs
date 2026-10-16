use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model};
use slint_shell::{
    Document, bind_document,
    ui::{Locale, MacroEditor, MacroField, Page, SettingsWindow, Theme},
};
use std::{rc::Rc, time::Duration};

/// Let the delayed save of typed fields run.
fn settle() {
    std::thread::sleep(Duration::from_millis(450));
    slint::platform::update_timers_and_animations();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
    let document = Document::load(paths.clone())?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    ui.set_page(Page::Macros);
    ui.global::<Theme>().invoke_apply();
    slint::select_bundled_translation("ru")?;
    ui.global::<Locale>().set_english(false);
    let editor = ui.global::<MacroEditor>();
    editor.invoke_clone_builtin(0, "(копия)".into());
    let first = |document: &Rc<Document>| document.read().layout().macros[0].clone();
    assert_eq!(first(&document).steps.len(), 3);
    assert_eq!(first(&document).id, "moveLineDownCopy");
    editor.invoke_set_field(0, MacroField::Id, "testMacro".into());
    editor.invoke_set_field(0, MacroField::Name, "Тестовый макрос".into());
    editor.invoke_add_step(0, "pause:100".into());
    editor.invoke_add_step(0, "text: hello ".into());
    editor.invoke_move_step(0, 4, 3);
    editor.invoke_set_field(0, MacroField::StepPause, "0".into());
    settle();
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
    editor.invoke_remove_step(0, 5);
    assert!(!editor.get_has_errors());
    editor.invoke_set_field(0, MacroField::ModifierDelay, "abc".into());
    settle();
    assert!(editor.get_has_errors());
    editor.invoke_set_field(0, MacroField::ModifierDelay, "".into());
    settle();
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
    for _ in 0..12 {
        editor.invoke_add_step(0, "KeyB".into());
    }
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let doc2 = document.clone();
    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(700), move || {
        let ui = weak.upgrade().unwrap();
        let manual = std::env::var_os("PROBE_MANUAL").is_some();
        if !manual {
            use slint::platform::{PointerEventButton, WindowEvent};
            let position = slint::LogicalPosition::new(645.0, 351.0);
            let button = PointerEventButton::Left;
            ui.window().dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed { position, button });
            ui.window().dispatch_event(WindowEvent::PointerReleased { position, button });
            let step = |ui: &SettingsWindow| editor_steps(ui);
            eprintln!("PROBE before {:?}", step(&ui));
            for phase in [i_slint_core::input::TouchPhase::Started, i_slint_core::input::TouchPhase::Moved] {
                i_slint_core::window::WindowInner::from_pub(ui.window()).process_mouse_input(i_slint_core::input::MouseEvent::Wheel {
                    position: i_slint_core::lengths::LogicalPoint::new(645.0, 351.0), delta_x: 0.0, delta_y: 60.0, phase });
                eprintln!("PROBE after {:?} doc-draft-step {:?}", phase, ui.global::<MacroEditor>().get_macros().row_data(0).map(|r| r.error.id));
            }
        }
        if std::env::var_os("SLINT_SHELL_MACROS_SCROLL").is_some() {
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(500.0, 400.0), delta_x: 0.0, delta_y: -600.0,
            });
        }
        if let Ok(path) = std::env::var("SLINT_SHELL_MACROS_SNAPSHOT") {
            let pixels = ui.window().take_snapshot().unwrap();
            let mut data = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
            for p in pixels.as_slice() { data.extend([p.r, p.g, p.b]); }
            std::fs::write(path, data).unwrap();
        }
        let doc3 = doc2.clone();
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(if manual { 20000 } else { 600 }), move || {
            let ui = weak.upgrade().unwrap();
            eprintln!("PROBE doc saved {:?}", doc3.read().layout().macros[0].steps[4].action);
            let before = ui.global::<MacroEditor>().get_macros();
            eprintln!("PROBE reload {:?}", doc3.reload().map(|r| r.is_some()));
            doc3.refresh_all();
            eprintln!("PROBE same model after refresh_all: {}", before == ui.global::<MacroEditor>().get_macros());
            let position = slint::LogicalPosition::new(645.0, 351.0);
            if !manual { ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: 60.0 }); }
            let weak = ui.as_weak();
            slint::Timer::single_shot(Duration::from_millis(600), move || {
                let ui = weak.upgrade().unwrap();
                eprintln!("PROBE doc after wheel post-save {:?}", doc3.read().layout().macros[0].steps[4].action);
                if let Ok(path) = std::env::var("SLINT_SHELL_MACROS_SNAPSHOT") {
                    let pixels = ui.window().take_snapshot().unwrap();
                    let mut data = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
                    for p in pixels.as_slice() { data.extend([p.r, p.g, p.b]); }
                    std::fs::write(path, data).unwrap();
                }
                println!("Macros smoke: passed (built-in copy, steps, pauses, order, cycle validation, delayed save and reload)");
                slint::quit_event_loop().unwrap();
            });
        });
    });
    slint::run_event_loop()?;
    Ok(())
}

fn editor_steps(ui: &SettingsWindow) -> Vec<String> {
    let row = ui.global::<MacroEditor>().get_macros().row_data(0).unwrap();
    row.steps.iter().map(|s| s.action.to_string()).collect()
}
