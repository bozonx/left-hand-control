//! Callback and debounce regressions run in CI without a desktop or real config.

use lhc_core::{profile::timing, storage::StoragePaths};
use slint::{
    ComponentHandle, Model,
    platform::{Platform, WindowAdapter, software_renderer::MinimalSoftwareWindow},
};
use slint_shell::{Document, bind_document, ui::*};
use std::{cell::Cell, rc::Rc, time::Duration};

struct TestPlatform(Rc<Cell<Duration>>);

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(MinimalSoftwareWindow::new(Default::default()))
    }

    fn duration_since_start(&self) -> Duration {
        self.0.get()
    }
}

#[test]
fn editor_callbacks_save_flush_and_debounce_without_a_desktop() {
    let clock = Rc::new(Cell::new(Duration::ZERO));
    slint::platform::set_platform(Box::new(TestPlatform(clock.clone()))).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let paths = StoragePaths::new(dir.path().join("config"), dir.path().join("data"));
    let document = Document::load(paths).unwrap();
    let ui = SettingsWindow::new().unwrap();
    bind_document(&ui, &document);
    ui.global::<SettingsEditor>().set_in_development(true);
    ui.global::<SettingsEditor>().invoke_save();
    let editor = ui.global::<MacroEditor>();
    editor.invoke_add("Draft".into());
    let id = editor.get_macros().row_data(0).unwrap().id;
    editor.invoke_set_field(0, MacroField::Name, "Delayed".into());
    assert!(editor.get_has_unsaved_changes());
    clock.set(Duration::from_millis(399));
    slint::platform::update_timers_and_animations();
    assert_eq!(document.read().layout().macros[0].name, "Draft");
    // A new keystroke restarts the debounce deadline.
    editor.invoke_set_field(0, MacroField::Name, "Latest".into());
    clock.set(Duration::from_millis(401));
    slint::platform::update_timers_and_animations();
    assert_eq!(document.read().layout().macros[0].name, "Draft");
    clock.set(Duration::from_millis(800));
    slint::platform::update_timers_and_animations();
    assert_eq!(document.read().layout().macros[0].name, "Latest");
    assert!(!editor.get_has_unsaved_changes());

    editor.invoke_set_field(0, MacroField::Id, "invalid id".into());
    let library = ui.global::<LayoutLibrary>();
    assert!(!library.invoke_flush_drafts());
    assert_eq!(document.read().layout().macros[0].id, id.as_str());
    assert!(editor.get_has_errors());
    editor.invoke_set_field(0, MacroField::Id, id);
    editor.invoke_set_field(0, MacroField::Name, "Saved without waiting".into());
    assert!(library.invoke_flush_drafts());
    assert_eq!(
        document.read().layout().macros[0].name,
        "Saved without waiting"
    );
    let revision = document.revision();
    clock.set(Duration::from_millis(1300));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        document.revision(),
        revision,
        "flush cancels the pending save"
    );

    library.set_save_as_name("Draft regression".into());
    library.invoke_save_as("Draft regression".into());
    editor.invoke_set_field(0, MacroField::Name, "Saved to library immediately".into());
    library.invoke_save_current();
    assert_eq!(
        document
            .read()
            .load_layout("user:Draft regression")
            .unwrap()
            .macros[0]
            .name,
        "Saved to library immediately"
    );

    let menu = ui.global::<MenuEditor>();
    menu.invoke_open(MenuKind::Emoji);
    menu.set_page_name("Menu draft".into());
    menu.invoke_rename_page();
    assert!(menu.get_has_unsaved_changes());
    assert!(library.invoke_flush_drafts());
    assert_eq!(document.read().layout().emoji_pages[0].name, "Menu draft");
    assert!(!menu.get_has_unsaved_changes());

    let bounds = ui.global::<Timing>();
    for (actual, expected) in [
        (bounds.get_tap_hold(), timing::TAP_HOLD),
        (bounds.get_long_hold(), timing::LONG_HOLD),
        (bounds.get_trigger_chord(), timing::TRIGGER_CHORD),
        (bounds.get_macro_delay(), timing::MACRO_DELAY),
        (bounds.get_pause(), timing::PAUSE),
        (bounds.get_macro_timeout(), timing::MACRO_TIMEOUT),
    ] {
        assert_eq!(actual.minimum, expected.min as f32);
        assert_eq!(actual.maximum, expected.max as f32);
    }
}
