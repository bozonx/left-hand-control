//! Draft persistence regressions without a Slint window.

use super::*;

fn state(document: &Document) -> State {
    State {
        draft: load(document),
        baseline: load(document),
        ..State::default()
    }
}

fn entry() -> Entry {
    Entry::new(Macro {
        id: "testMacro".into(),
        name: "Draft".into(),
        steps: vec![step("text:hello")],
        step_pause_ms: None,
        modifier_delay_ms: None,
    })
}

#[test]
fn valid_draft_saves_immediately_and_repeated_save_is_a_noop() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.draft.insert(0, entry());
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
    assert!(state.draft == state.baseline);
    assert_eq!(state.draft[0].saved_id.as_deref(), Some("testMacro"));
    let revision = document.revision();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Unchanged)
    ));
    assert_eq!(document.revision(), revision);
    state.draft[0].item.name = "Saved without a timer".into();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
    assert_eq!(
        document.read().layout().macros[0].name,
        "Saved without a timer"
    );
}

#[test]
fn invalid_draft_is_kept_until_corrected_without_overwriting_saved_macros() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.draft.insert(0, entry());
    persist(&document, &mut state).unwrap();
    let saved = document.read().layout().macros.clone();
    for bad_delay in ["abc", "2001"] {
        state.draft[0].modifier_delay = bad_delay.into();
        assert!(matches!(
            persist(&document, &mut state),
            Ok(SaveResult::Invalid)
        ));
        assert_eq!(document.read().layout().macros, saved);
        assert!(state.draft != state.baseline);
    }
    state.draft[0].modifier_delay.clear();
    state.draft[0].item.id = "invalid id".into();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Invalid)
    ));
    state.draft[0].item.id = "renamed".into();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
    assert!(state.draft == state.baseline);
}

#[test]
fn failed_commit_keeps_the_baseline_and_draft_for_retry() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    let mut item = entry();
    // Step IDs are checked by the document when the draft is committed.
    item.item.steps.push(item.item.steps[0].clone());
    state.draft.insert(0, item);
    let saved = document.read().layout().macros.clone();
    assert!(persist(&document, &mut state).is_err());
    assert!(state.draft != state.baseline);
    assert_eq!(state.draft[0].saved_id, None);
    assert_eq!(document.read().layout().macros, saved);
    state.draft[0].item.steps.pop();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
    assert!(state.draft == state.baseline);
}

#[test]
fn picker_rejects_stale_indices_and_keeps_invalid_steps_in_the_draft() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.draft = vec![entry()];
    for (at, step_index) in [(-1, 0), (1, 0), (0, -2), (0, 1)] {
        assert!(choose_step(&mut state, at, step_index, "KeyA", None).is_err());
        assert_eq!(state.draft[0].item.steps.len(), 1);
    }
    choose_step(&mut state, 0, -1, "macro:testMacro", None).unwrap();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Invalid)
    ));
    state.draft[0].item.steps.pop();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
}

#[test]
fn delays_parse_within_range() {
    assert_eq!(parse_delay(""), Ok(None));
    assert_eq!(parse_delay(" 25 "), Ok(Some(25)));
    assert!(parse_delay("2001").is_err());
    assert!(parse_delay("x").is_err());
    let entry = Entry {
        step_pause: "abc".into(),
        ..Entry::new(Macro {
            id: "m".into(),
            name: String::new(),
            steps: vec![],
            step_pause_ms: None,
            modifier_delay_ms: None,
        })
    };
    assert!(entry.saved().is_none());
}

#[test]
fn ids_allow_builtin_macro_names() {
    let builtin = BUILTIN_MACROS[0].id;
    assert_eq!(unique_id(&[], builtin), builtin);
}
