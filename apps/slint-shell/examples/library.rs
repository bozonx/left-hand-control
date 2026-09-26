use lhc_core::{
    config_document::ConfigDocument, profile::model::LayoutMode, storage::StoragePaths,
};
use slint::{ComponentHandle, Model};
use slint_shell::{
    bind_document,
    command::{Command, Popup},
    popup_model,
    ui::*,
};
use std::{cell::RefCell, rc::Rc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Rc::new(RefCell::new(ConfigDocument::load(paths.clone())?));
    let ui = SettingsWindow::new()?;
    let _editor = bind_document(&ui, Some(document.clone()));
    ui.invoke_create_layout_preset("A".into(), false);
    assert_eq!(ui.get_library_dialog(), 0);
    ui.invoke_create_layout_preset("B".into(), false);
    assert_eq!(ui.get_layout_names().row_count(), 2);
    ui.invoke_select_layout(0);
    ui.invoke_library_action(0);
    ui.invoke_select_layout(1);
    ui.invoke_load_layout();
    assert_eq!(
        document.borrow().settings().current_layout_id.as_deref(),
        Some("user:B")
    );
    assert_eq!(
        document
            .borrow()
            .settings()
            .manual_active_layout_id
            .as_deref(),
        Some("user:A")
    );
    ui.set_layout_name("Профиль".into());
    ui.set_layout_description("Описание профиля".into());
    ui.invoke_library_action(4);
    assert_eq!(ui.get_layout_status().id, "library-saved");
    assert!(paths.load_user_layout("B").is_err());
    assert!(paths.load_user_layout("Профиль")?.contains("Описание"));
    ui.set_layout_auto_enabled(true);
    ui.set_layout_white_layouts("us, ru".into());
    ui.set_layout_white_apps("kate, terminal".into());
    ui.set_layout_black_game(1);
    ui.invoke_library_action(3);
    ui.invoke_library_action(1);
    assert_eq!(ui.get_layout_names().row_data(0).unwrap(), "Профиль");
    ui.invoke_set_layout_mode(1);
    assert_eq!(document.borrow().settings().layout_mode, LayoutMode::Auto);
    ui.invoke_select_layout(0);
    assert!(ui.get_layout_auto_enabled());
    assert_eq!(ui.get_layout_black_game(), 1);
    assert_eq!(ui.get_layout_white_apps(), "kate, terminal");
    ui.invoke_delete_layout();
    assert_eq!(ui.get_layout_names().row_count(), 1);
    ui.invoke_select_layout(0);
    ui.set_layout_name("Copy".into());
    ui.set_library_dialog(6);
    ui.invoke_library_action(6);
    assert_eq!(ui.get_library_dialog(), 0);
    assert_eq!(
        paths.load_user_layout("A")?,
        paths.load_user_layout("Copy")?
    );
    assert_eq!(ui.get_layout_names().row_count(), 2);
    let reloaded = ConfigDocument::load(paths)?;
    assert!(reloaded.settings().layout_conditions.is_empty());
    assert!(reloaded.settings().current_layout_id.is_none());
    document.borrow_mut().update_settings(|settings| {
        settings.layout_mode = LayoutMode::Manual;
        settings.manual_active_layout_id = Some("user:missing".into());
    })?;
    ui.invoke_create_layout_preset("Ivan K".into(), true);
    assert_eq!(ui.get_layout_status().id, "library-saved");
    assert_eq!(ui.get_page(), 3);
    assert_eq!(
        ui.get_rule_rows().row_count(),
        document.borrow().layout().rules.len()
    );
    assert!(ui.get_rule_rows().row_count() > 0);
    ui.invoke_navigate(4, 0);
    assert!(ui.get_layer_names().row_count() > 0);
    ui.invoke_navigate(6, 0);
    assert!(ui.global::<MenuEditor>().get_pages().row_count() > 0);
    ui.invoke_create_layout_preset("Ivan K".into(), true);
    assert_eq!(
        document.borrow().settings().current_layout_id.as_deref(),
        Some("user:Ivan K (2)")
    );
    ui.invoke_create_layout_preset("Empty".into(), false);
    assert!(document.borrow().layout().rules.is_empty());
    assert_eq!(ui.get_rule_rows().row_count(), 0);
    let ivan = ui
        .get_layout_names()
        .iter()
        .position(|name| name == "Ivan K")
        .unwrap();
    ui.invoke_select_layout(ivan as i32);
    ui.invoke_load_layout();
    let emoji = EmojiPopup::new()?;
    let quick = QuickPopup::new()?;
    let mut menus = popup_model::ConfiguredMenus::default();
    menus.layout.emoji_pages = (0..5)
        .map(|_| lhc_core::profile::model::EmojiPage::default_page())
        .collect();
    menus.layout.quick_action_pages = (0..5)
        .map(|i| lhc_core::profile::model::QuickActionPage {
            id: i.to_string(),
            name: format!("Page {i}"),
        })
        .collect();
    menus.apply_emoji(&emoji);
    menus.apply_quick(&quick);
    quick.set_query("previous search".into());
    popup_model::select_page(Popup::Emoji, 4, &emoji, &quick);
    assert_eq!(emoji.get_page(), 3);
    popup_model::select_page(Popup::Quick, 3, &emoji, &quick);
    assert_eq!(quick.get_page(), 2);
    assert_eq!(quick.get_query(), "");
    assert_eq!(
        Command::parse("show emoji 4")?,
        Command::ShowPage(Popup::Emoji, 4)
    );
    assert!(Command::parse("show emoji 0").is_err());
    assert!(Command::parse("show quick 6").is_err());
    ui.invoke_navigate(1, 0);
    if let Ok(dialog) = std::env::var("LHC_LIBRARY_DIALOG") {
        ui.invoke_select_layout(0);
        ui.set_library_dialog(dialog.parse()?);
    }
    if let Ok(page) = std::env::var("LHC_LIBRARY_PAGE") {
        ui.invoke_navigate(page.parse()?, 0);
    }
    ui.show()?;
    slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
        if let Some(path) = std::env::var_os("LHC_LIBRARY_SNAPSHOT") {
            let pixels = ui.window().take_snapshot().unwrap();
            let mut bytes =
                format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
            for pixel in pixels.as_slice() {
                bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
            }
            std::fs::write(path, bytes).unwrap();
        }
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
