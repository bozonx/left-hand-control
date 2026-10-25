//! Delayed saving of a page draft while the user types, shared by the
//! macros, menus and settings pages.

use crate::{
    document::{Document, View},
    ui::SettingsWindow,
};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

/// How long typing must pause before a draft is saved.
pub(super) const TYPING_DELAY: Duration = Duration::from_millis(400);

/// Pending save of a page's draft.
#[derive(Default)]
pub(super) struct Autosave(slint::Timer);

impl Autosave {
    /// Whether typed changes wait to be saved.
    pub fn pending(&self) -> bool {
        self.0.running()
    }

    pub fn cancel(&self) {
        self.0.stop();
    }
}

/// A page state with a delayed save.
pub(super) trait Draft: 'static {
    fn autosave(&self) -> &Autosave;
    /// Save the draft now.
    fn save(&mut self, ui: &SettingsWindow, document: &Document);
}

/// Save `state` once typing pauses; a later call restarts the wait.
pub(super) fn schedule<S: Draft>(
    ui: &SettingsWindow,
    document: &Rc<Document>,
    state: &Rc<RefCell<S>>,
) {
    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), Rc::downgrade(state));
    state
        .borrow()
        .autosave()
        .0
        .start(slint::TimerMode::SingleShot, TYPING_DELAY, move || {
            if let (Some(ui), Some(state)) = (weak.upgrade(), shared.upgrade()) {
                state.borrow_mut().save(&ui, &doc);
            }
        });
}

/// Save a pending draft right away.
pub(super) fn flush<S: Draft>(ui: &SettingsWindow, document: &Document, state: &mut S) {
    if state.autosave().pending() {
        state.autosave().cancel();
        state.save(ui, document);
    }
}

/// Call `changed` when another view changed the document. It is skipped
/// while this page's own save holds the state: that save reloads by itself.
pub(super) fn subscribe<S: 'static>(
    ui: &SettingsWindow,
    document: &Document,
    view: View,
    state: &Rc<RefCell<S>>,
    changed: impl Fn(&SettingsWindow, &Document, &mut S) + 'static,
) {
    let weak = ui.as_weak();
    let shared = state.clone();
    document.subscribe(view, move |document| {
        if let (Some(ui), Ok(mut state)) = (weak.upgrade(), shared.try_borrow_mut()) {
            changed(&ui, document, &mut state);
        }
    });
}
