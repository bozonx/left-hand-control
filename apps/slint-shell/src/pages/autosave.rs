//! Delayed saving of a page draft while the user types, shared by the
//! macros, menus and settings pages.

use crate::{
    document::{Document, View},
    ui::SettingsWindow,
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

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

/// Coalesces refreshes that cannot run until a page releases its state.
struct RefreshQueue {
    queued: Cell<bool>,
    /// Returns false while the page state is borrowed.
    apply: Box<dyn Fn() -> bool>,
}

impl RefreshQueue {
    fn request(self: &Rc<Self>) {
        if self.queued.get() || (self.apply)() {
            return;
        }
        self.queued.set(true);
        let weak = Rc::downgrade(self);
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(refresh) = weak.upgrade() {
                refresh.queued.set(false);
                refresh.request();
            }
        });
    }
}

/// Call `changed` when another view changed the document. A reentrant
/// notification is coalesced and retried after the current callback returns.
pub(super) fn subscribe<S: 'static>(
    ui: &SettingsWindow,
    document: &Rc<Document>,
    view: View,
    state: &Rc<RefCell<S>>,
    changed: impl Fn(&SettingsWindow, &Document, &mut S) + 'static,
) {
    let ui = ui.as_weak();
    let shared = Rc::downgrade(state);
    let document_weak = Rc::downgrade(document);
    let refresh = Rc::new(RefreshQueue {
        queued: Cell::new(false),
        apply: Box::new(move || {
            let (Some(ui), Some(document), Some(shared)) =
                (ui.upgrade(), document_weak.upgrade(), shared.upgrade())
            else {
                return true;
            };
            let Ok(mut state) = shared.try_borrow_mut() else {
                return false;
            };
            changed(&ui, &document, &mut state);
            true
        }),
    });
    document.subscribe(view, move |_| refresh.request());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reentrant_refreshes_coalesce_and_retry_until_state_is_available() {
        let state = Rc::new(RefCell::new(0));
        let observed = Rc::new(RefCell::new(Vec::new()));
        let (shared, values) = (state.clone(), observed.clone());
        let refresh = Rc::new(RefreshQueue {
            queued: Cell::new(false),
            apply: Box::new(move || {
                let Ok(state) = shared.try_borrow_mut() else {
                    return false;
                };
                values.borrow_mut().push(*state);
                true
            }),
        });
        let mut held = state.borrow_mut();
        *held = 1;
        refresh.request();
        refresh.request();
        assert!(observed.borrow().is_empty());
        slint::platform::update_timers_and_animations();
        assert!(observed.borrow().is_empty());
        *held = 2;
        drop(held);
        slint::platform::update_timers_and_animations();
        assert_eq!(*observed.borrow(), [2]);
        slint::platform::update_timers_and_animations();
        assert_eq!(*observed.borrow(), [2]);
    }
}
