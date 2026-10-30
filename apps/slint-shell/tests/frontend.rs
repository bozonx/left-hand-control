//! Callback and debounce regressions run in CI without a desktop or real config.

use lhc_core::{profile::timing, storage::StoragePaths};
use slint::{
    ComponentHandle, Model,
    platform::{EventLoopProxy, Platform, WindowAdapter, software_renderer::MinimalSoftwareWindow},
};
use slint_shell::{Document, bind_document, ui::*};
use std::{
    cell::Cell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

type Tasks = Arc<Mutex<VecDeque<Box<dyn FnOnce() + Send>>>>;
struct TestPlatform(Rc<Cell<Duration>>, Tasks);
struct Proxy(Tasks);
impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }
    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        self.0.lock().unwrap().push_back(event);
        Ok(())
    }
}
fn drain(tasks: &Tasks) {
    loop {
        let task = tasks.lock().unwrap().pop_front();
        let Some(task) = task else { break };
        task();
    }
}
fn wait(tasks: &Tasks, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        drain(tasks);
        slint::platform::update_timers_and_animations();
        if ready() {
            return;
        }
        assert!(Instant::now() < deadline, "background request timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(MinimalSoftwareWindow::new(Default::default()))
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.1.clone())))
    }
    fn duration_since_start(&self) -> Duration {
        self.0.get()
    }
}

#[test]
fn editor_callbacks_save_flush_and_debounce_without_a_desktop() {
    let clock = Rc::new(Cell::new(Duration::ZERO));
    let tasks = Tasks::default();
    slint::platform::set_platform(Box::new(TestPlatform(clock.clone(), tasks.clone()))).unwrap();
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
    clock.set(Duration::from_millis(798));
    slint::platform::update_timers_and_animations();
    assert_eq!(document.read().layout().macros[0].name, "Draft");
    clock.set(Duration::from_millis(799));
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
    assert!(library.invoke_flush_drafts());
    assert_eq!(
        document.revision(),
        revision,
        "flushing an idle draft is a no-op"
    );
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
    // The activation callback must release its read guard before editing.
    library.invoke_activate_current();
    assert_eq!(
        document.read().settings().manual_layout_id.as_deref(),
        Some("user:Draft regression")
    );

    // A corrupt file is an error row, and an older refresh cannot overwrite
    // the metadata from a newer request.
    let library_path = document.read().paths().layouts_dir();
    let path = library_path.join("Draft regression.json");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, "not json").unwrap();
    library.invoke_refresh();
    wait(&tasks, || {
        library
            .get_rows()
            .iter()
            .any(|row| row.loaded && !row.error.id.is_empty())
    });
    std::fs::write(&path, &original).unwrap();
    library.invoke_refresh();
    library.invoke_refresh();
    wait(&tasks, || {
        library
            .get_rows()
            .iter()
            .all(|row| row.loaded && row.error.id.is_empty())
    });
    assert!(
        !library
            .get_rows()
            .row_data(0)
            .unwrap()
            .summary
            .macros
            .eq(&0)
    );

    // Disk failure is acknowledged by the inline callback without losing
    // the last successfully persisted library contents.
    std::fs::remove_file(&path).unwrap();
    assert!(!library.invoke_rename(0, "Cannot rename".into()));
    std::fs::write(&path, original).unwrap();
    assert!(library.invoke_rename(0, "Renamed".into()));
}
