use lhc_core::{
    config_document::ConfigDocument, profile::model::LayoutMode, storage::StoragePaths,
};
use slint::{ComponentHandle, Model};
use slint_shell::{
    Document, bind_document,
    command::{Command, Popup},
    popup_model,
    ui::*,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Document::load(paths.clone())?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    let library = ui.global::<LayoutLibrary>();
    library.invoke_open_create(LayoutSource::Empty, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::Create);
    assert!(!library.get_create_name().is_empty());
    library.invoke_create("A".into(), "".into(), LayoutSource::Empty, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    assert_eq!(library.invoke_name_issue("A".into()), NameIssue::Taken);
    assert_eq!(library.invoke_name_issue("a/b".into()), NameIssue::Invalid);
    assert_eq!(library.invoke_name_issue(" ".into()), NameIssue::Empty);
    library.invoke_create("B".into(), "".into(), LayoutSource::Empty, 0);
    assert_eq!(library.get_names().row_count(), 2);
    library.invoke_select(0);
    library.invoke_action(LibraryAction::Activate);
    library.invoke_select(1);
    library.invoke_load();
    assert_eq!(document.read().settings().current_layout_id.as_deref(), Some("user:B"));
    assert_eq!(
        document.read().settings().manual_active_layout_id.as_deref(),
        Some("user:A")
    );
    library.set_name("Профиль".into());
    library.set_description("Описание профиля".into());
    library.invoke_action(LibraryAction::SaveDetails);
    assert_eq!(library.get_status().id, "library-saved");
    assert!(paths.load_user_layout("B").is_err());
    assert!(paths.load_user_layout("Профиль")?.contains("Описание"));
    assert_eq!(library.get_current_label(), "Профиль");
    library.set_auto_enabled(true);
    library.set_white_layouts("us, ru".into());
    library.set_white_apps("kate, terminal".into());
    library.set_black_game(GameCondition::On);
    library.invoke_action(LibraryAction::SaveConditions);
    library.invoke_action(LibraryAction::MoveUp);
    assert_eq!(library.get_names().row_data(0).unwrap(), "Профиль");
    library.invoke_set_automatic(true);
    assert_eq!(document.read().settings().layout_mode, LayoutMode::Auto);
    assert!(library.get_automatic());
    library.invoke_select(0);
    assert!(library.get_auto_enabled());
    assert_eq!(library.get_black_game(), GameCondition::On);
    assert_eq!(library.get_white_apps(), "kate, terminal");
    library.invoke_delete();
    assert_eq!(library.get_names().row_count(), 1);
    library.invoke_create("Copy".into(), "".into(), LayoutSource::Copy, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    assert_eq!(paths.load_user_layout("A")?, paths.load_user_layout("Copy")?);
    library.invoke_set_description(0, "Inline".into());
    assert!(paths.load_user_layout("A")?.contains("Inline"));
    assert_eq!(library.get_names().row_count(), 2);
    let reloaded = ConfigDocument::load(paths.clone())?;
    assert!(reloaded.settings().layout_conditions.is_empty());
    assert_eq!(reloaded.settings().current_layout_id.as_deref(), Some("user:Copy"));
    document.edit(slint_shell::document::View::Shell, |config| {
        config.update_settings(|settings| {
            settings.layout_mode = LayoutMode::Manual;
            settings.manual_active_layout_id = Some("user:missing".into());
        })
    })?;
    library.invoke_create("Ivan K".into(), "".into(), LayoutSource::Author, 0);
    assert_eq!(ui.get_page(), Page::Rules);
    assert_eq!(
        ui.global::<RulesEditor>().get_rows().row_count(),
        document.read().layout().rules.len()
    );
    assert!(ui.global::<RulesEditor>().get_rows().row_count() > 0);
    ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
    assert!(ui.global::<LayersEditor>().get_names().row_count() > 0);
    ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
    assert!(ui.global::<MenuEditor>().get_pages().row_count() > 0);
    assert_eq!(library.invoke_suggest_name("Ivan K".into()), "Ivan K (2)");
    // Unsaved edits must survive until the user decides what to do with them.
    document.edit(slint_shell::document::View::Rules, |config| {
        config.update_layout(|layout| layout.rules.clear())
    })?;
    assert!(library.get_dirty());
    library.invoke_open_create(LayoutSource::Empty, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
    library.invoke_save_and_continue();
    assert!(!library.get_dirty());
    assert!(document.read().load_layout("user:Ivan K")?.rules.is_empty());
    assert_eq!(library.get_dialog(), LibraryDialog::Create);
    library.set_create_name("Empty".into());
    library.invoke_submit_create();
    assert!(paths.load_user_layout("Empty").is_ok());
    assert_eq!(
        document.read().settings().current_layout_id.as_deref(),
        Some("user:Empty")
    );
    assert!(document.read().layout().rules.is_empty());
    assert_eq!(ui.global::<RulesEditor>().get_rows().row_count(), 0);
    let ivan = library
        .get_names()
        .iter()
        .position(|name| name == "Ivan K")
        .unwrap();
    library.invoke_edit(ivan as i32);
    assert_eq!(library.get_current_label(), "Ivan K");
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
    ui.invoke_navigate(Page::Layouts, MenuKind::Emoji);
    if std::env::var_os("LHC_LIBRARY_AUTO").is_some() {
        library.invoke_set_automatic(true);
    }
    if let Ok(dialog) = std::env::var("LHC_LIBRARY_DIALOG") {
        library.invoke_select(0);
        library.set_dialog(match dialog.as_str() {
            "create" => LibraryDialog::Create,
            "details" => LibraryDialog::Details,
            "conditions" => LibraryDialog::Conditions,
            "delete" => LibraryDialog::Delete,
            "unsaved" => LibraryDialog::Unsaved,
            _ => LibraryDialog::None,
        });
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
