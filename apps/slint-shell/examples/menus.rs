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
    e.invoke_open(0);
    e.set_value("Привет 👋".into());
    e.set_page_name("Мои эмоджи".into());
    e.invoke_save();
    e.invoke_add();
    e.set_value("✨".into());
    e.invoke_save();
    e.invoke_move(-1);
    e.invoke_save();
    assert_eq!(
        ConfigDocument::load(paths.clone())?.layout().emoji_pages[0].cells["KeyQ"],
        "✨"
    );
    e.invoke_open(2);
    e.invoke_add();
    e.set_command_id("hello".into());
    e.set_page_name("Привет".into());
    e.set_value("printf hello".into());
    e.invoke_save();
    assert_eq!(document.borrow().layout().commands.len(), 1);
    e.invoke_trust(true);
    assert!(document.borrow().commands_trusted());
    e.set_value("printf changed".into());
    e.invoke_save();
    assert!(!document.borrow().commands_trusted());
    e.invoke_open(1);
    e.set_value("cmd:hello".into());
    e.set_name("Запуск".into());
    e.invoke_select_cell(1);
    e.set_value("text:Здравствуйте".into());
    e.invoke_save();
    e.invoke_move_cell(1);
    e.invoke_save();
    let loaded = ConfigDocument::load(paths.clone())?;
    assert_eq!(loaded.layout().quick_actions[2].action, "text:Здравствуйте");
    let menus = slint_shell::popup_model::ConfiguredMenus {
        layout: loaded.layout().clone(),
    };
    assert_eq!(menus.quick("ЗАПУСК")[0].1, "cmd:hello");
    e.set_value("macro:missing".into());
    e.invoke_save();
    assert_ne!(e.get_status().id.as_str(), "menu-saved");
    e.invoke_open(1);
    assert_eq!(e.get_cells().row_count(), 15);
    e.set_value("text:unsaved".into());
    e.invoke_open(1);
    assert_eq!(e.get_value().as_str(), "cmd:hello");
    paths.save_current_layout("rules: []\n")?;
    e.set_value("text:conflict".into());
    e.invoke_save();
    assert_ne!(e.get_status().id.as_str(), "menu-saved");
    e.invoke_open(
        std::env::var("LHC_MENUS_PAGE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    );
    slint::select_bundled_translation("ru")?;
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
        println!("Menus smoke passed: pages, cells, commands, trust, persistence, filtering, validation, cancel and conflicts");
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
