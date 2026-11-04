mod support;
use lhc_core::{
    config_document::ConfigDocument,
    profile::{bundled::BUNDLED, layout_file, model::LayoutMode},
    storage::StoragePaths,
};
use slint::{ComponentHandle, Model};
use slint_shell::{
    Document, bind_document,
    command::{Command, Popup},
    popup_model,
    ui::*,
};

/// Grip point and row center in window coordinates.
fn reorder_handle(ui: &SettingsWindow, group: i32, index: i32) -> (slint::LogicalPosition, f32) {
    use i_slint_core::{
        accessibility::AccessibleStringProperty, item_tree::ItemRc, window::WindowInner,
    };
    let window = WindowInner::from_pub(ui.window());
    window.ensure_tree_instantiated();
    let id = format!("lhc-reorder-{group}-{index}");
    let mut pending = vec![(ItemRc::new_root(window.component()), None)];
    while let Some((item, mut center)) = pending.pop() {
        let origin = item.map_to_window(item.geometry().origin);
        if (0..item.element_count().unwrap_or(0)).any(|index| {
            item.element_type_names_and_ids(index)
                .is_some_and(|entries| entries.iter().any(|(kind, _)| kind == "AutoRuleRow"))
        }) {
            center = Some(origin.y + item.geometry().height() / 2.);
        }
        if item
            .accessible_string_property(AccessibleStringProperty::Id)
            .is_some_and(|value| value.as_str() == id)
        {
            return (
                slint::LogicalPosition::new(
                    origin.x + item.geometry().width() / 2.,
                    origin.y + item.geometry().height() / 2.,
                ),
                center.expect("automatic-rule handle must belong to a row"),
            );
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push((next, center));
        }
    }
    panic!("reorder handle {id} not found");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Document::load(paths.clone())?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    let development = ui.global::<slint_shell::ui::SettingsEditor>();
    development.set_in_development(true);
    development.invoke_save();
    ui.global::<slint_shell::ui::AppState>()
        .set_in_development(true);
    let library = ui.global::<LayoutLibrary>();
    library.invoke_open_create(LayoutSource::Empty, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::Create);
    assert!(!library.get_create_name().is_empty());
    library.invoke_create("A".into(), "".into(), LayoutSource::Empty, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    let summary = library.get_rows().row_data(0).unwrap().summary;
    assert_eq!(summary.emoji_pages, 0);
    assert_eq!(summary.action_pages, 0);
    assert!(
        document
            .read()
            .load_layout("user:A")
            .unwrap()
            .emoji_pages
            .iter()
            .all(|page| page.cells.is_empty())
    );
    let error = |name: &str, except: &str| library.invoke_name_error(name.into(), except.into()).id;
    assert_eq!(error("a", ""), "layout-name-taken");
    assert_eq!(error("a", "A"), "");
    assert_eq!(error("a/b", ""), "name-invalid");
    assert_eq!(error(" ", ""), "layout-name-required");
    library.invoke_create("B".into(), "".into(), LayoutSource::Empty, 0);
    assert_eq!(library.get_names().row_count(), 2);
    // A stale selection must not redirect an action on another card.
    library.invoke_select(1);
    library.invoke_action(0, LibraryAction::Activate);
    library.invoke_load(1);
    assert_eq!(
        document.read().settings().editing_layout_id.as_deref(),
        Some("user:B")
    );
    assert_eq!(
        document.read().settings().manual_layout_id.as_deref(),
        Some("user:A")
    );
    assert!(library.invoke_rename(1, "Профиль".into()));
    assert!(library.invoke_set_description(1, "Описание профиля".into()));
    assert_eq!(library.get_status().id, "");
    assert!(paths.load_user_layout("B").is_err());
    assert!(paths.load_user_layout("Профиль")?.contains("Описание"));
    assert_eq!(library.get_current_label(), "Профиль");
    // Entering Auto preserves an explicit passthrough default.
    library.invoke_set_automatic(true);
    assert_eq!(
        document.read().settings().auto_default_layout_id.as_deref(),
        None
    );
    assert_eq!(library.get_auto_default(), -1);
    assert_eq!(library.invoke_new_rule_target(), 0);
    library.invoke_set_automatic(false);
    // "Профиль" is the second row: a rule for it, then an "off" rule above it.
    library.invoke_open_rule(-1, 1);
    assert_eq!(library.get_dialog(), LibraryDialog::Rule);
    library.set_rule_layouts("us, ru".into());
    library.set_rule_apps("kate, terminal".into());
    library.invoke_save_rule();
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    library.invoke_open_rule(-1, -1);
    library.set_rule_game(GameCondition::On);
    library.invoke_save_rule();
    assert_eq!(
        library.get_auto_rules().row_data(0).unwrap().game,
        GameCondition::On
    );
    library.invoke_move_rule(0, 1);
    assert_eq!(
        library.get_auto_rules().row_data(1).unwrap().game,
        GameCondition::On
    );
    library.invoke_move_rule(1, 0);
    library.invoke_set_auto_default(0);
    assert_eq!(
        ConfigDocument::load(paths.clone())?.settings().auto_rules,
        document.read().settings().auto_rules
    );
    {
        let config = document.read();
        let settings = config.settings();
        assert_eq!(settings.auto_rules.len(), 2);
        assert_eq!(settings.auto_rules[0].layout_id, None);
        assert_eq!(
            settings.auto_rules[0].conditions.game_mode.as_deref(),
            Some("on")
        );
        assert_eq!(
            settings.auto_rules[1].layout_id.as_deref(),
            Some("user:Профиль")
        );
        assert_eq!(settings.auto_default_layout_id.as_deref(), Some("user:A"));
    }
    assert_eq!(library.get_auto_rules().row_data(1).unwrap().target, 1);
    assert_eq!(library.get_rows().row_data(1).unwrap().rule_count, 1);
    library.invoke_open_rule(1, 1);
    assert_eq!(library.get_rule_apps(), "kate, terminal");
    library.invoke_delete_rule(0);
    assert_eq!(library.get_auto_rules().row_count(), 1);
    assert_eq!(
        ConfigDocument::load(paths.clone())?.settings().auto_rules,
        document.read().settings().auto_rules
    );
    // A game-mode rule, then a narrower one it always shadows.
    library.invoke_add_game_rule();
    library.invoke_open_rule(-1, -1);
    library.set_rule_game(GameCondition::On);
    library.set_rule_apps("kate".into());
    library.invoke_save_rule();
    library.invoke_move_rule(0, 2);
    library.invoke_move_rule(0, 1);
    let rules = library.get_auto_rules();
    assert_eq!(rules.row_count(), 3);
    assert_eq!(rules.row_data(1).unwrap().shadowed_by, -1);
    assert_eq!(rules.row_data(2).unwrap().shadowed_by, 1);
    library.invoke_set_rule_enabled(1, false);
    assert!(!document.read().settings().auto_rules[1].enabled);
    assert!(!ConfigDocument::load(paths.clone())?.settings().auto_rules[1].enabled);
    let rules = library.get_auto_rules();
    assert!(!rules.row_data(1).unwrap().enabled);
    assert_eq!(rules.row_data(2).unwrap().shadowed_by, -1);
    library.invoke_delete_rule(2);
    library.invoke_delete_rule(1);
    assert_eq!(library.get_auto_rules().row_count(), 1);
    library.invoke_action(1, LibraryAction::MoveUp);
    assert_eq!(library.get_names().row_data(0).unwrap(), "Профиль");
    library.invoke_set_automatic(true);
    assert_eq!(document.read().settings().layout_mode, LayoutMode::Auto);
    assert!(library.get_automatic());
    library.invoke_delete(0);
    assert!(document.read().settings().auto_rules.is_empty());
    assert_eq!(library.get_names().row_count(), 1);
    // Creating opens the new layout without asking again: unsaved changes
    // are confirmed before the create dialog opens.
    library.invoke_create("Copy".into(), "".into(), LayoutSource::Copy, 0);
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    assert_eq!(library.get_current_label(), "Copy");
    assert_eq!(
        paths.load_user_layout("A")?,
        paths.load_user_layout("Copy")?
    );
    library.invoke_set_description(0, "Inline".into());
    assert!(paths.load_user_layout("A")?.contains("Inline"));
    assert_eq!(library.get_names().row_count(), 2);
    library.invoke_rename(0, "Renamed".into());
    assert!(paths.load_user_layout("A").is_err());
    assert!(paths.load_user_layout("Renamed")?.contains("Inline"));
    library.invoke_reorder(0, 1);
    assert_eq!(library.get_names().row_data(1).unwrap(), "Renamed");
    library.invoke_set_automatic(true);
    assert_eq!(library.get_names().row_data(1).unwrap(), "Renamed");
    library.invoke_set_automatic(false);
    assert_eq!(library.get_names().row_data(1).unwrap(), "Renamed");
    let saved = document.read().layout().clone();
    document.edit(slint_shell::document::View::Rules, |config| {
        config.update_layout(|layout| layout.description = Some("Discard me".into()))
    })?;
    assert!(library.get_dirty());
    library.invoke_discard_current();
    assert!(!library.get_dirty());
    assert_eq!(document.read().layout(), &saved);
    // Saving over a file changed elsewhere asks first; formatting alone
    // does not count as a change.
    let describe = |description: &str| {
        document.edit(slint_shell::document::View::Rules, |config| {
            config.update_layout(|layout| layout.description = Some(description.into()))
        })
    };
    let mut theirs = saved.clone();
    theirs.description = Some("Theirs".into());
    let theirs = layout_file::serialize(&theirs);
    let disk = paths.load_user_layout("Copy")?;
    paths.save_user_layout("Copy", &format!("\n{disk}\n"), true)?;
    describe("Formatting")?;
    library.invoke_save_current();
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    assert!(paths.load_user_layout("Copy")?.contains("Formatting"));
    describe("Mine")?;
    paths.save_user_layout("Copy", &theirs, true)?;
    library.invoke_save_current();
    assert_eq!(library.get_dialog(), LibraryDialog::Conflict);
    assert_eq!(paths.load_user_layout("Copy")?, theirs);
    library.invoke_cancel_dialog();
    assert!(library.get_dirty());
    library.invoke_save_current();
    library.invoke_overwrite_current();
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    assert!(paths.load_user_layout("Copy")?.contains("Mine"));
    assert!(!library.get_dirty());
    describe("Lost")?;
    paths.save_user_layout("Copy", &theirs, true)?;
    library.invoke_save_current();
    assert_eq!(library.get_dialog(), LibraryDialog::Conflict);
    library.invoke_cancel_dialog();
    library.invoke_discard_current();
    assert_eq!(
        document.read().layout().description.as_deref(),
        Some("Theirs")
    );
    assert!(!library.get_dirty());
    let choices = ui.global::<ConditionChoices>();
    let app = choices.invoke_add("kate".into(), "A title, with comma".into());
    assert_eq!(choices.invoke_items(app.clone()).row_count(), 2);
    assert!(choices.invoke_contains(app, "A title, with comma".into()));
    assert_eq!(choices.invoke_add("kate".into(), "kate".into()), "kate");
    assert_eq!(
        choices.invoke_toggle("us, ru".into(), "ru".into(), false),
        "us"
    );
    assert_eq!(
        choices.invoke_remove("kate, terminal".into(), 0),
        "terminal"
    );
    let reloaded = ConfigDocument::load(paths.clone())?;
    assert!(reloaded.settings().auto_rules.is_empty());
    assert_eq!(
        reloaded.settings().editing_layout_id.as_deref(),
        Some("user:Copy")
    );
    document.edit(slint_shell::document::View::Shell, |config| {
        config.update_settings(|settings| {
            settings.layout_mode = LayoutMode::Manual;
            settings.manual_layout_id = Some("user:missing".into());
        })
    })?;
    let bundled_name = BUNDLED[0].display_name();
    assert_eq!(
        library.get_bundled_names().row_data(0).unwrap().as_str(),
        bundled_name.as_str()
    );
    library.invoke_open_create(LayoutSource::Bundled, 0);
    assert_eq!(library.get_create_name().as_str(), bundled_name.as_str());
    library.invoke_create("Ivan K".into(), "".into(), LayoutSource::Bundled, 0);
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
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    assert!(paths.load_user_layout("Empty").is_ok());
    assert_eq!(
        document.read().settings().editing_layout_id.as_deref(),
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
    let position = |name: &str| {
        library
            .get_names()
            .iter()
            .position(|item| item == name)
            .unwrap() as i32
    };
    // A conflict during "save and continue" still finishes the switch.
    document.edit(slint_shell::document::View::Rules, |config| {
        config.update_layout(|layout| layout.description = Some("Pending".into()))
    })?;
    let mut theirs = document.read().load_layout("user:Ivan K")?;
    theirs.description = Some("Changed elsewhere".into());
    paths.save_user_layout("Ivan K", &layout_file::serialize(&theirs), true)?;
    library.invoke_edit(position("Empty"));
    assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
    library.invoke_save_and_continue();
    assert_eq!(library.get_dialog(), LibraryDialog::Conflict);
    library.invoke_overwrite_current();
    library.invoke_finish_save();
    assert_eq!(library.get_current_label(), "Empty");
    assert!(paths.load_user_layout("Ivan K")?.contains("Pending"));
    // A layout file that does not parse can still be renamed and deleted,
    // and a dialog never shows the error of an earlier action.
    std::fs::write(paths.layout_path("Broken")?, "not json")?;
    library.invoke_refresh();
    assert!(!library.invoke_rename(position("Broken"), "a/b".into()));
    assert_ne!(library.get_status().id, "");
    assert!(library.invoke_rename(position("Broken"), "Still broken".into()));
    assert_eq!(paths.load_user_layout("Still broken")?, "not json");
    assert!(!library.invoke_rename(position("Still broken"), "a/b".into()));
    library.invoke_open_delete(position("Still broken"));
    assert_eq!(library.get_dialog(), LibraryDialog::Delete);
    assert_eq!(library.get_status().id, "");
    library.invoke_delete(library.get_selected());
    assert!(paths.load_user_layout("Still broken").is_err());
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    // A working copy without a library file can be saved from the
    // unsaved-changes question: it asks for a name, then switches.
    library.invoke_open_delete(position("Empty"));
    library.invoke_delete(library.get_selected());
    assert_eq!(library.get_current_label(), "");
    document.edit(slint_shell::document::View::Rules, |config| {
        config.update_layout(|layout| layout.description = Some("Orphan".into()))
    })?;
    library.invoke_edit(position("Ivan K"));
    assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
    library.invoke_save_and_continue();
    assert!(library.get_save_as_open());
    library.invoke_cancel_save_as();
    assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
    library.invoke_save_and_continue();
    library.set_save_as_name("Orphan".into());
    library.invoke_save_as("Orphan".into());
    library.invoke_finish_save();
    assert!(paths.load_user_layout("Orphan")?.contains("Orphan"));
    assert_eq!(library.get_current_label(), "Ivan K");
    assert_eq!(library.get_dialog(), LibraryDialog::None);
    let emoji = EmojiPopup::new()?;
    let quick = QuickPopup::new()?;
    let mut menus = popup_model::ConfiguredMenus::default();
    menus.layout.emoji_pages = (0..5)
        .map(|_| lhc_core::profile::model::EmojiPage::default_page())
        .collect();
    menus.layout.quick_action_pages = (0..5)
        .map(|i| lhc_core::profile::model::QuickActionPage::new(i.to_string(), format!("Page {i}")))
        .collect();
    menus.apply_emoji(&emoji);
    menus.apply_quick(&quick);
    popup_model::select_page(Popup::Emoji, 4, &emoji, &quick);
    assert_eq!(emoji.get_page(), 3);
    popup_model::select_page(Popup::Quick, 3, &emoji, &quick);
    assert_eq!(quick.get_page(), 2);
    assert_eq!(
        Command::parse("show emoji 4")?,
        Command::ShowPage(Popup::Emoji, 4)
    );
    assert!(Command::parse("show emoji 0").is_err());
    assert_eq!(
        Command::parse("show quick 6")?,
        Command::ShowPage(Popup::Quick, 6)
    );
    ui.invoke_navigate(Page::Layouts, MenuKind::Emoji);
    if std::env::var_os("LHC_LIBRARY_AUTO").is_some() {
        library.invoke_set_automatic(true);
    }
    if std::env::var_os("LHC_LIBRARY_RULE_DRAG").is_some() {
        library.invoke_set_automatic(true);
        for target in 0..3 {
            library.invoke_open_rule(-1, target);
            library.invoke_save_rule();
        }
    }
    if let Ok(dialog) = std::env::var("LHC_LIBRARY_DIALOG") {
        ui.global::<ConditionChoices>().invoke_refresh();
        library.invoke_select(0);
        library.set_dialog(match dialog.as_str() {
            "create" => LibraryDialog::Create,
            "rule" => LibraryDialog::Rule,
            "delete" => LibraryDialog::Delete,
            "unsaved" => LibraryDialog::Unsaved,
            _ => LibraryDialog::None,
        });
    }
    if let Ok(locale) = std::env::var("LHC_LIBRARY_LOCALE") {
        slint::select_bundled_translation(&locale)?;
    }
    ui.show()?;
    slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
        let library = ui.global::<LayoutLibrary>();
        let row_height = if library.get_automatic() { 118.0 } else { 96.0 };
        assert_eq!(
            ui.global::<DragDrop>().invoke_locate(
                ui.global::<DragGroups>().get_layouts(),
                0,
                10_000.0,
                library.get_names().row_count() as i32,
            ),
            library.get_names().row_count() as i32 - 1
        );
        if std::env::var_os("LHC_LIBRARY_DRAG").is_some() {
            let first = library.get_names().row_data(0).unwrap();
            let start = slint::LogicalPosition::new(50.0, 218.0 + row_height / 2.0);
            let end = slint::LogicalPosition::new(50.0, start.y + row_height * 2.0);
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                    position: start,
                    button: slint::platform::PointerEventButton::Left,
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerMoved { position: end });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                    position: end,
                    button: slint::platform::PointerEventButton::Left,
                });
            assert_eq!(library.get_names().row_data(2).unwrap(), first);
            println!("Library drag-and-drop passed");
        }
        if std::env::var_os("LHC_LIBRARY_RULE_DRAG").is_some() {
            let first = document.read().settings().auto_rules[0].id.clone();
            let group = ui.global::<DragGroups>().get_library_rules();
            let (start, source_center) = reorder_handle(&ui, group, 0);
            let (target, target_center) = reorder_handle(&ui, group, 2);
            // Rows with warnings have different heights: align row centers, not grips.
            let end =
                slint::LogicalPosition::new(target.x, start.y + target_center - source_center + 1.);
            let window = ui.window();
            window.dispatch_event(slint::platform::WindowEvent::PointerMoved { position: start });
            window.dispatch_event(slint::platform::WindowEvent::PointerPressed {
                position: start,
                button: slint::platform::PointerEventButton::Left,
            });
            window.dispatch_event(slint::platform::WindowEvent::PointerMoved { position: end });
            assert!(ui.global::<DragDrop>().get_dragging());
            assert_eq!(ui.global::<DragDrop>().get_source(), 0);
            assert_eq!(ui.global::<DragDrop>().get_target(), 2);
            window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
                position: end,
                button: slint::platform::PointerEventButton::Left,
            });
            assert_eq!(document.read().settings().auto_rules[2].id, first);
            assert_eq!(
                ConfigDocument::load(document.read().paths().clone())
                    .unwrap()
                    .settings()
                    .auto_rules,
                document.read().settings().auto_rules
            );
            println!("Automatic rule drag-and-drop passed");
        }
        if let Some(path) = std::env::var_os("LHC_LIBRARY_SNAPSHOT") {
            support::write_snapshot(&ui, std::path::Path::new(&path));
        }
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
