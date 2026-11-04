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
    let baseline = state.baseline.clone();
    let draft = state.draft.clone();
    let revision = document.revision();
    assert!(matches!(
        persist(&document, &mut state),
        Err(ConfigError::Invalid(_))
    ));
    assert!(state.baseline == baseline);
    assert!(state.draft == draft);
    assert_eq!(document.revision(), revision);
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

#[test]
fn rename_commits_nested_references_and_keeps_the_draft_in_sync() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    let inner = entry();
    let mut outer = entry();
    outer.item.id = "outer".into();
    outer.item.steps = vec![step("macro:testMacro")];
    state.draft = vec![inner, outer];
    persist(&document, &mut state).unwrap();
    state.draft[0].item.id = "renamed".into();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
    assert_eq!(
        document.read().layout().macros[1].steps[0].action,
        "macro:renamed"
    );
    assert_eq!(state.draft[1].item.steps[0].action, "macro:renamed");
    assert!(state.draft == state.baseline);
    state.draft[1].item.name = "Other edit".into();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Saved(_))
    ));
    assert_eq!(
        document.read().layout().macros[1].steps[0].action,
        "macro:renamed"
    );
}

#[test]
fn expansion_limit_is_an_inline_error_and_does_not_commit() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    state.draft = vec![entry()];
    state.draft[0].item.steps = (0..1001).map(|_| step("KeyA")).collect();
    assert_eq!(
        analysis(&document, &state.draft).errors[0],
        Some(ConfigError::Macro(macros::MacroIssue::ExpansionLimit))
    );
    let before = document.read().layout().macros.clone();
    assert!(matches!(
        persist(&document, &mut state),
        Ok(SaveResult::Invalid)
    ));
    assert_eq!(document.read().layout().macros, before);
}

#[test]
fn analysis_identifies_each_invalid_step_and_normalized_usage() {
    let (_dir, document) = super::super::test_document();
    let mut state = state(&document);
    let mut bad = entry();
    bad.item.steps = vec![step("pause:0"), step("KeyA"), step("macro:missing")];
    state.draft = vec![bad];
    let result = analysis(&document, &state.draft);
    assert!(result.step_errors[0][0].is_some());
    assert!(result.step_errors[0][1].is_none());
    assert!(result.step_errors[0][2].is_some());
}

fn headless_window() -> SettingsWindow {
    struct Headless;
    impl slint::platform::Platform for Headless {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::platform::PlatformError>
        {
            Ok(
                slint::platform::software_renderer::MinimalSoftwareWindow::new(
                    slint::platform::software_renderer::RepaintBufferType::ReusedBuffer,
                ),
            )
        }
    }
    slint::platform::set_platform(Box::new(Headless)).unwrap();
    SettingsWindow::new().unwrap()
}

#[test]
fn renamed_invalid_draft_keeps_incoming_references_and_cannot_be_deleted() {
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    let mut initial = state(&document);
    initial.draft = vec![entry()];
    persist(&document, &mut initial).unwrap();
    document
        .edit(View::Rules, |config| {
            config.update_layout(|layout| {
                let mut rule = lhc_core::profile::model::KeyRule::new("macroUsage".into(), "KeyA");
                rule.tap_action = Some("macro:testMacro".into());
                layout.rules.push(rule);
            })
        })
        .unwrap();
    let _state = bind(&ui, &document);
    let editor = ui.global::<MacroEditor>();
    editor.invoke_add_step(0, "pause:0".into());
    editor.invoke_set_field(0, MacroField::Id, "renamed".into());
    assert!(!editor.invoke_flush_pending());
    let row = editor.get_macros().row_data(0).unwrap();
    assert_eq!(row.id, "renamed");
    assert!(row.usage.iter().any(|place| place == "KeyA"));
    editor.invoke_remove(0);
    assert_eq!(editor.get_macros().row_count(), 1);
    assert_eq!(document.read().layout().macros[0].id, "testMacro");
    assert_eq!(editor.get_status().id, "macro-in-use");
}

#[test]
fn settings_changes_refresh_both_macro_default_delays_without_discarding_draft() {
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    let _state = bind(&ui, &document);
    let editor = ui.global::<MacroEditor>();
    editor.invoke_add("draft".into());
    editor.invoke_add_step(0, "pause:0".into());
    document
        .edit(View::Settings, |config| {
            config.update_settings(|settings| {
                settings.default_macro_step_pause_ms = 123;
                settings.default_macro_modifier_delay_ms = 45;
            })
        })
        .unwrap();
    assert_eq!(editor.get_default_step_pause(), "123");
    assert_eq!(editor.get_default_modifier_delay(), "45");
    assert!(editor.get_has_unsaved_changes());
    assert!(editor.get_has_errors());
    assert_eq!(
        editor.get_macros().row_data(0).unwrap().steps.row_count(),
        1
    );
}

#[test]
fn picker_uses_new_and_renamed_macro_ids_from_the_invalid_draft() {
    use crate::ui::{ActionPicker, PickerCategory};
    let ui = headless_window();
    let (_dir, document) = super::super::test_document();
    document
        .edit(View::Settings, |config| {
            config.update_settings(|settings| {
                settings.in_development = true;
            })
        })
        .unwrap();
    let macros = bind(&ui, &document);
    let menus = super::super::menus::bind(&ui, &document);
    super::super::picker::bind(
        &ui,
        &document,
        &Rc::default(),
        super::super::picker::Drafts { macros, menus },
    );
    let editor = ui.global::<MacroEditor>();
    editor.invoke_add("first".into());
    editor.invoke_add_step(0, "pause:0".into());
    editor.invoke_set_field(0, MacroField::Id, "renamed".into());
    editor.invoke_add("second".into());
    let second = editor.get_macros().row_data(0).unwrap().id;
    editor.invoke_add("caller".into());
    editor.invoke_add_step(0, "".into());
    let picker = ui.global::<ActionPicker>();
    picker.set_category(PickerCategory::Macros);
    for id in ["renamed", second.as_str()] {
        picker.set_value(format!("macro:{id}").into());
        picker.invoke_refresh();
        assert!(picker.get_valid(), "draft macro {id} was rejected");
        assert!(
            picker
                .get_items()
                .iter()
                .any(|item| item.value == format!("macro:{id}"))
        );
    }
    picker.set_value("macro:renamed".into());
    picker.invoke_apply();
    assert_eq!(
        editor
            .get_macros()
            .row_data(0)
            .unwrap()
            .steps
            .row_data(0)
            .unwrap()
            .action,
        "macro:renamed"
    );
    assert!(editor.get_has_errors());
    assert!(editor.get_has_unsaved_changes());
    // The persisted id is not a target in the draft anymore.
    editor.invoke_add_step(0, "".into());
    picker.set_value("macro:m1".into());
    picker.invoke_refresh();
    assert!(!picker.get_valid());
    assert!(
        document
            .read()
            .layout()
            .macros
            .iter()
            .all(|item| item.id != "renamed")
    );
}
