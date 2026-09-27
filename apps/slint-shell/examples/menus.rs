use lhc_core::{config_document::ConfigDocument, storage::StoragePaths};
use slint::{ComponentHandle, Model};
use slint_shell::{
    menu_editor,
    ui::{MenuEditor, SettingsWindow, Theme},
};
use std::{cell::RefCell, rc::Rc, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
    let document = Rc::new(RefCell::new(ConfigDocument::load(paths.clone())?));
    let ui = SettingsWindow::new()?;
    menu_editor::bind(&ui, Some(document.clone()));
    ui.set_page(6);
    ui.global::<Theme>().invoke_apply();
    let e = ui.global::<MenuEditor>();
    let loaded = || ConfigDocument::load(paths.clone()).unwrap();

    e.invoke_open(0);
    e.set_value("Привет 👋".into());
    e.invoke_set_cell();
    e.set_page_name("Мои эмоджи".into());
    e.invoke_rename_page();
    assert_eq!(loaded().layout().emoji_pages[0].cells["KeyQ"], "Привет 👋");
    assert_eq!(loaded().layout().emoji_pages[0].name, "Мои эмоджи");
    e.invoke_add_page("Страница 2".into());
    assert_eq!(e.get_selected_page(), 1);
    e.set_value("✨".into());
    e.invoke_set_cell();
    e.invoke_move_cell(0, 2);
    assert_eq!(loaded().layout().emoji_pages[1].cells["KeyE"], "✨");
    e.invoke_move_page(-1);
    assert_eq!(loaded().layout().emoji_pages[0].cells["KeyE"], "✨");
    e.invoke_select_cell(2);
    e.invoke_clear_cell();
    assert!(!loaded().layout().emoji_pages[0].cells.contains_key("KeyE"));
    e.invoke_remove_page();
    assert_eq!(loaded().layout().emoji_pages.len(), 1);

    e.invoke_open(2);
    e.invoke_add_command("Привет".into());
    assert!(e.get_has_errors());
    assert_eq!(e.get_commands().row_data(0).unwrap().error.id, "menu-empty-command");
    assert!(loaded().layout().commands.is_empty());
    e.invoke_set_command(0, 0, "hello".into());
    e.invoke_set_command(0, 2, "printf hello".into());
    assert!(!e.get_has_errors());
    assert_eq!(document.borrow().layout().commands.len(), 1);
    e.invoke_trust(true);
    assert!(document.borrow().commands_trusted());
    e.invoke_set_command(0, 2, "printf changed".into());
    assert!(!document.borrow().commands_trusted());
    assert!(!e.get_trusted());
    e.invoke_add_command("Второй".into());
    e.invoke_set_command(0, 0, "hello".into());
    assert_eq!(e.get_commands().row_data(1).unwrap().error.id, "menu-duplicate-command");
    e.invoke_trust(true);
    assert_eq!(e.get_status().id.as_str(), "menu-save-first");
    e.invoke_remove_command(0);
    assert!(!e.get_has_errors());

    e.invoke_open(1);
    assert_eq!(e.get_selected_cell(), -1);
    e.invoke_set_action(0, "cmd:hello".into(), "Привет".into());
    assert_eq!(e.get_name().as_str(), "Привет");
    e.set_name("Запуск".into());
    e.invoke_set_name();
    e.invoke_set_action(1, "text:Здравствуйте".into(), "".into());
    e.invoke_move_cell(1, 2);
    let layout = loaded().layout().clone();
    assert_eq!(layout.quick_actions[2].action, "text:Здравствуйте");
    assert_eq!(layout.quick_actions[2].name, "text:Здравствуйте");
    let menus = slint_shell::popup_model::ConfiguredMenus { layout };
    assert_eq!(menus.quick("ЗАПУСК")[0].1, "cmd:hello");
    e.invoke_set_action(0, "macro:missing".into(), "".into());
    assert_ne!(e.get_status().id.as_str(), "");
    assert_eq!(loaded().layout().quick_actions[0].action, "cmd:hello");
    e.invoke_open(1);
    assert_eq!(e.get_cells().row_count(), 15);
    assert_eq!(e.get_value().as_str(), "cmd:hello");
    paths.save_current_layout("rules: []\n")?;
    e.set_name("conflict".into());
    e.invoke_set_name();
    assert_ne!(e.get_status().id.as_str(), "");
    *document.borrow_mut() = ConfigDocument::load(paths.clone())?;
    e.invoke_open(2);
    e.invoke_add_command("Плеер".into());
    e.invoke_set_command(0, 0, "hello".into());
    e.invoke_set_command(0, 2, "playerctl play-pause".into());
    e.invoke_open(1);
    e.invoke_set_action(0, "cmd:hello".into(), "Плеер".into());
    e.invoke_set_action(6, "Ctrl+KeyC".into(), "Ctrl+KeyC".into());
    assert_eq!(document.borrow().layout().quick_actions[6].action, "Ctrl+KeyC");
    e.invoke_open(
        std::env::var("LHC_MENUS_PAGE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    );
    slint::select_bundled_translation("ru")?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    ui.show()?;
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(slint::TimerMode::SingleShot,Duration::from_millis(700),move || {
        let ui = weak.upgrade().unwrap();
        if std::env::var_os("LHC_MENUS_NAV").is_some() {
            for expected in [1, 2, 0] {
                ui.invoke_navigate(6, expected);
                assert_eq!(ui.global::<MenuEditor>().get_kind(), expected);
                assert_eq!(ui.get_page(), 6);
            }
        }
        if std::env::var_os("LHC_MENUS_SCROLL").is_some() {
            ui.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                position: slint::LogicalPosition::new(30.0, 400.0), delta_x: 0.0, delta_y: -600.0,
            });
        }
        if let Ok(path) = std::env::var("LHC_MENUS_SNAPSHOT") {
            let pixels = ui.window().take_snapshot().unwrap();
            let mut data = format!("P6\n{} {}\n255\n",pixels.width(),pixels.height()).into_bytes();
            for p in pixels.as_slice() { data.extend([p.r,p.g,p.b]); }
            std::fs::write(path,data).unwrap();
        }
        println!("Menus smoke passed: pages, cells, drag moves, commands, trust, auto-save, validation and conflicts");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
