//! Draft persistence regressions without a Slint window.

use super::*;

fn state(document: &Document) -> State {
    let baseline = document.read().keymap().clone();
    State {
        keymap: draft(&baseline),
        baseline,
        autosave: Autosave::default(),
        commands: Vec::new(),
    }
}

#[test]
fn menu_draft_saves_without_a_window_and_repeated_save_is_a_noop() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.keymap.emoji_pages[0].name = "Saved without waiting".into();
    assert!(persist(&document, &mut state).unwrap().is_some());
    assert!(!menu_changed(&state));
    assert_eq!(
        document.read().keymap().emoji_pages[0].name,
        "Saved without waiting"
    );
    let revision = document.revision();
    assert!(persist(&document, &mut state).unwrap().is_none());
    assert_eq!(document.revision(), revision);
}

#[test]
fn conflicting_save_preserves_draft_and_baseline() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.keymap.emoji_pages[0].name = "My draft".into();
    document
        .edit(View::Menus, |config| {
            config.update_keymap(|keymap| {
                keymap.emoji_pages[0].name = "External change".into();
            })
        })
        .unwrap();
    let baseline = state.baseline.clone();
    assert!(matches!(
        persist(&document, &mut state),
        Err(tecloid_core::config_document::ConfigError::EditConflict)
    ));
    assert_eq!(state.baseline, baseline);
    assert_eq!(state.keymap.emoji_pages[0].name, "My draft");
    assert_eq!(
        document.read().keymap().emoji_pages[0].name,
        "External change"
    );
    assert!(menu_changed(&state));
}

#[test]
fn invalid_menu_does_not_overwrite_saved_pages_and_can_be_corrected() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.keymap.emoji_pages[0]
        .cells
        .insert("KeyQ".into(), "x".repeat(101));
    let saved = document.read().keymap().emoji_pages.clone();
    assert!(persist(&document, &mut state).is_err());
    assert_eq!(document.read().keymap().emoji_pages, saved);
    assert!(menu_changed(&state));
    state.keymap.emoji_pages[0]
        .cells
        .insert("KeyQ".into(), "🙂".into());
    assert!(persist(&document, &mut state).unwrap().is_some());
    assert!(!menu_changed(&state));
}

#[test]
fn draft_has_pages_and_quick_pages_start_without_a_selection() {
    let keymap = draft(&KeymapPreset::default());
    assert_eq!(keymap.emoji_pages.len(), 1);
    assert_eq!(keymap.quick_action_pages.len(), 1);
    assert_eq!(initial_cell(MenuKind::Emoji, &keymap, 0), 0);
    assert_eq!(initial_cell(MenuKind::Quick, &keymap, 0), -1);
    assert_eq!(
        initial_cell(MenuKind::Quick, &keymap, 5),
        -1,
        "missing pages are empty"
    );
    assert_eq!(initial_cell(MenuKind::Quick, &keymap, -1), -1);
}

#[test]
fn first_filled_skips_blank_actions() {
    let mut keymap = draft(&KeymapPreset::default());
    let cells = &mut keymap.quick_action_pages[0].cells;
    cells.insert(
        "KeyQ".into(),
        QuickAction {
            action: " ".into(),
            ..Default::default()
        },
    );
    cells.insert(
        "KeyE".into(),
        QuickAction {
            action: "KeyA".into(),
            ..Default::default()
        },
    );
    assert_eq!(first_filled(&keymap, 0), 2);
}

#[test]
fn no_break_space_survives_saving() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    menus::set_emoji(&mut state.keymap, 0, 0, "\u{a0}");
    menus::set_emoji(&mut state.keymap, 0, 1, "TODO: ");
    assert!(persist(&document, &mut state).unwrap().is_some());
    let cells = document.read().keymap().emoji_pages[0].cells.clone();
    assert_eq!(cells["KeyQ"], "\u{a0}");
    assert_eq!(cells["KeyW"], "TODO: ");
}

#[test]
fn bundled_catalog_parses_and_every_category_has_a_title() {
    let categories = catalog().unwrap();
    assert!(!categories.is_empty());
    for category in &categories {
        assert!(
            CATEGORY_IDS.contains(&category.id.as_str()),
            "{}",
            category.id
        );
        assert!(!category.items.is_empty(), "{}", category.id);
    }
}

fn headless_window() -> SettingsWindow {
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
    struct Platform;
    impl slint::platform::Platform for Platform {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
        }
    }
    slint::platform::set_platform(Box::new(Platform)).unwrap();
    SettingsWindow::new().unwrap()
}

#[test]
fn cut_is_cancelled_after_reordering_removing_or_reloading_pages() {
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    let _state = bind(&ui, &document);
    let e = ui.global::<MenuEditor>();
    for kind in [MenuKind::Emoji, MenuKind::Quick] {
        e.invoke_open(kind);
        e.invoke_add_page("Source".into());
        if kind == MenuKind::Emoji {
            e.set_value("🙂".into());
            e.invoke_set_cell();
            assert!(e.invoke_flush_pending());
        } else {
            e.invoke_set_action(0, "text:source".into(), "Source".into());
        }
        e.invoke_mark_cell(0, false);
        e.invoke_reorder_page(1, 0);
        assert_eq!(e.get_keyboard_source_cell(), -1);
        let saved = document.read().keymap().clone();
        e.invoke_select_cell(1);
        e.invoke_drop_cell(1);
        assert_eq!(document.read().keymap(), &saved);

        e.invoke_mark_cell(0, false);
        e.invoke_remove_page();
        assert_eq!(e.get_keyboard_source_cell(), -1);
        e.invoke_mark_cell(0, false);
        e.invoke_refresh();
        assert_eq!(e.get_keyboard_source_cell(), -1);
        e.invoke_mark_cell(0, false);
        document
            .edit(View::Library, |config| {
                config.update_keymap(|keymap| {
                    menus::rename_page(keymap, core_kind(kind), 0, "Reloaded elsewhere");
                })
            })
            .unwrap();
        assert_eq!(e.get_keyboard_source_cell(), -1);
    }
}

#[test]
fn preview_saves_pending_edits_and_rejects_invalid_or_conflicting_drafts() {
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    document
        .edit_library(|config| config.save_current_keymap_as("Active"))
        .unwrap();
    document
        .edit_library(|config| config.save_current_keymap_as("Edited"))
        .unwrap();
    document
        .edit(View::Settings, |config| {
            config.update_settings(|settings| {
                settings.manual_keymap_id = Some("user:Active".into());
            })
        })
        .unwrap();
    let _state = bind(&ui, &document);
    let e = ui.global::<MenuEditor>();
    e.invoke_open(MenuKind::Emoji);
    e.set_value("Just typed 🙂".into());
    e.invoke_set_cell();
    assert!(e.get_has_unsaved_changes());
    assert!(e.invoke_show_page());
    assert!(!e.get_has_unsaved_changes());
    assert_eq!(
        document.read().keymap().emoji_pages[0].cells["KeyQ"],
        "Just typed 🙂"
    );
    assert_eq!(e.get_cells().row_data(0).unwrap().value, "Just typed 🙂");

    let (active_id, active) = document.popup_keymap(false).unwrap();
    assert_eq!(active_id.as_deref(), Some("user:Active"));
    assert_ne!(
        active.emoji_pages[0].cells.get("KeyQ").map(String::as_str),
        Some("Just typed 🙂")
    );

    e.set_value("🙂".repeat(101).into());
    e.invoke_set_cell();
    assert!(!e.invoke_show_page());
    assert_eq!(e.get_status().id, "menu-emoji-cell");
    e.set_value("Corrected".into());
    e.invoke_set_cell();
    assert!(e.invoke_show_page());

    e.set_page_name("My draft".into());
    e.invoke_rename_page();
    document
        .edit(View::Shell, |config| {
            config.update_keymap(|keymap| {
                keymap.emoji_pages[0].name = "Other writer".into();
            })
        })
        .unwrap();
    assert!(!e.invoke_show_page());
    assert_eq!(e.get_status().id, "config-edit-conflict");
    e.invoke_refresh();

    e.set_value("Corrected after conflict".into());
    e.invoke_set_cell();
    assert!(e.invoke_show_page());
    assert!(!e.get_has_unsaved_changes());
}

#[test]
fn picker_stays_open_on_menu_save_failure_and_can_retry() {
    use crate::ui::{ActionPicker, PickerTarget};
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    let menus = bind(&ui, &document);
    let macros = super::super::macros::bind(&ui, &document);
    super::super::picker::bind(
        &ui,
        &document,
        &Rc::default(),
        super::super::picker::Drafts { macros, menus },
    );
    let e = ui.global::<MenuEditor>();
    e.invoke_open(MenuKind::Quick);
    e.invoke_select_cell(0);
    e.set_page_name("Pending page name".into());
    e.invoke_rename_page();
    let picker = ui.global::<ActionPicker>();
    picker.invoke_open(PickerTarget::QuickAction, 0, "".into(), false);
    picker.set_value("text:chosen".into());
    picker.invoke_refresh();
    assert!(picker.get_valid());
    document
        .edit(View::Shell, |config| {
            config.update_keymap(|keymap| {
                keymap.quick_action_pages[0].name = "Other writer".into();
            })
        })
        .unwrap();
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_ne!(picker.get_error().id, "");
    assert!(e.get_has_unsaved_changes());
    assert!(
        !document.read().keymap().quick_action_pages[0]
            .cells
            .contains_key("KeyQ")
    );
    picker.invoke_dismiss_picker();
    e.invoke_refresh();
    picker.invoke_open(PickerTarget::QuickAction, 0, "".into(), false);
    picker.set_value("text:chosen".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert!(!e.get_has_unsaved_changes());
    assert_eq!(
        document.read().keymap().quick_action_pages[0].cells["KeyQ"].action,
        "text:chosen"
    );
}

#[test]
fn replacement_resolves_the_previous_catalog_label_in_the_editor() {
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    document
        .edit(View::Shell, |config| {
            config.update_settings(|settings| settings.in_development = true)?;
            config.update_keymap(|keymap| {
                for (id, name) in [("first", "First"), ("second", "Second")] {
                    keymap.macros.push(Macro {
                        id: id.into(),
                        name: name.into(),
                        steps: vec![],
                        step_pause_ms: None,
                        modifier_delay_ms: None,
                    });
                }
            })
        })
        .unwrap();
    let _state = bind(&ui, &document);
    let e = ui.global::<MenuEditor>();
    e.invoke_open(MenuKind::Quick);
    e.invoke_set_action(0, "macro:first".into(), "First".into());
    e.invoke_set_action(0, "macro:second".into(), "Second".into());
    assert_eq!(e.get_name(), "Second");
    e.set_name("My macro".into());
    e.invoke_set_name();
    e.invoke_set_action(0, "macro:first".into(), "First".into());
    assert_eq!(e.get_name(), "My macro");
    assert_eq!(
        document.read().keymap().quick_action_pages[0].cells["KeyQ"].name,
        "My macro"
    );
}
