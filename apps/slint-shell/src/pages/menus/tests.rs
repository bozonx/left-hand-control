//! Draft persistence regressions without a Slint window.

use super::*;

fn state(document: &Document) -> State {
    let baseline = document.read().layout().clone();
    let mut layout = baseline.clone();
    normalize(&mut layout);
    State {
        baseline,
        layout,
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
fn normalize_fills_pages() {
    let mut layout = LayoutPreset::default();
    normalize(&mut layout);
    assert_eq!(layout.emoji_pages.len(), 1);
    assert_eq!(layout.quick_action_pages.len(), 1);
    assert_eq!(first_filled(&layout, 0), -1);
    assert_eq!(first_filled(&layout, 5), -1, "missing pages are empty");
}

#[test]
fn move_cell_swaps_without_shifting() {
    let mut cells: BTreeMap<String, &str> = [("KeyQ", "q"), ("KeyW", "w"), ("KeyE", "e")]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
    move_cell(&mut cells, 0, 2);
    assert_eq!(cells["KeyQ"], "e");
    assert_eq!(cells["KeyW"], "w", "cells between stay in place");
    assert_eq!(cells["KeyE"], "q");
    move_cell(&mut cells, 1, 4);
    assert!(!cells.contains_key("KeyW"), "an empty target moves back");
    assert_eq!(cells["KeyT"], "w");
}
