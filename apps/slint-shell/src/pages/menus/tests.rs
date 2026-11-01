//! Draft persistence regressions without a Slint window.

use super::*;

fn state(document: &Document) -> State {
    let baseline = document.read().layout().clone();
    State {
        layout: draft(&baseline),
        baseline,
        autosave: Autosave::default(),
        commands: Vec::new(),
    }
}

#[test]
fn menu_draft_saves_without_a_window_and_repeated_save_is_a_noop() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.layout.emoji_pages[0].name = "Saved without waiting".into();
    assert!(persist(&document, &mut state).unwrap().is_some());
    assert!(!menu_changed(&state));
    assert_eq!(
        document.read().layout().emoji_pages[0].name,
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
    state.layout.emoji_pages[0].name = "My draft".into();
    document
        .edit(View::Menus, |config| {
            config.update_layout(|layout| {
                layout.emoji_pages[0].name = "External change".into();
            })
        })
        .unwrap();
    let baseline = state.baseline.clone();
    assert!(matches!(
        persist(&document, &mut state),
        Err(lhc_core::config_document::ConfigError::EditConflict)
    ));
    assert_eq!(state.baseline, baseline);
    assert_eq!(state.layout.emoji_pages[0].name, "My draft");
    assert_eq!(
        document.read().layout().emoji_pages[0].name,
        "External change"
    );
    assert!(menu_changed(&state));
}

#[test]
fn invalid_menu_does_not_overwrite_saved_pages_and_can_be_corrected() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.layout.emoji_pages[0]
        .cells
        .insert("KeyQ".into(), "x".repeat(101));
    let saved = document.read().layout().emoji_pages.clone();
    assert!(persist(&document, &mut state).is_err());
    assert_eq!(document.read().layout().emoji_pages, saved);
    assert!(menu_changed(&state));
    state.layout.emoji_pages[0]
        .cells
        .insert("KeyQ".into(), "🙂".into());
    assert!(persist(&document, &mut state).unwrap().is_some());
    assert!(!menu_changed(&state));
}

#[test]
fn draft_has_pages_and_quick_pages_start_without_a_selection() {
    let layout = draft(&LayoutPreset::default());
    assert_eq!(layout.emoji_pages.len(), 1);
    assert_eq!(layout.quick_action_pages.len(), 1);
    assert_eq!(initial_cell(MenuKind::Emoji, &layout, 0), 0);
    assert_eq!(initial_cell(MenuKind::Quick, &layout, 0), -1);
    assert_eq!(
        initial_cell(MenuKind::Quick, &layout, 5),
        -1,
        "missing pages are empty"
    );
    assert_eq!(initial_cell(MenuKind::Quick, &layout, -1), -1);
}

#[test]
fn first_filled_skips_blank_actions() {
    let mut layout = draft(&LayoutPreset::default());
    let cells = &mut layout.quick_action_pages[0].cells;
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
    assert_eq!(first_filled(&layout, 0), 2);
}

#[test]
fn no_break_space_survives_saving() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    menus::set_emoji(&mut state.layout, 0, 0, "\u{a0}");
    menus::set_emoji(&mut state.layout, 0, 1, "TODO: ");
    assert!(persist(&document, &mut state).unwrap().is_some());
    let cells = document.read().layout().emoji_pages[0].cells.clone();
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
