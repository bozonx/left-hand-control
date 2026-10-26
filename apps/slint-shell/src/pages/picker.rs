//! Action picker: one dialog that chooses a key, shortcut or action for
//! every page and hands the value to the page that opened it.

use crate::{
    document::Document,
    i18n::Msg,
    ui::{ActionPicker, PickerBehavior, PickerItem, PickerTarget, SettingsWindow},
};
use lhc_core::profile::actions::Action;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

mod capture;
mod catalog;
mod commands;
mod request;
mod session;

pub use capture::capture;
use capture::{CAPTURE, CaptureState};
#[cfg(test)]
use capture::{key_name, physical_key_name};
pub(crate) use catalog::{DESKTOP_SHORTCUTS, action_label, category_for_value};
use catalog::{load_desktop_shortcuts, shortcut_desktop};
#[cfg(test)]
use request::Filter;
pub(super) use request::{PickerMode, PickerRequest};

/// Whether `value` is the `cmd:` action of the command `id`.
fn assigns_command(value: &str, id: &str) -> bool {
    matches!(Action::parse(Some(value)), Action::Command(command) if command == id)
}

/// The open picker: who receives the value and the catalog built on open.
#[derive(Default)]
struct Session {
    target: Option<(PickerTarget, i32)>,
    catalog: Vec<PickerItem>,
    command: Option<lhc_core::profile::model::Command>,
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let session = Rc::new(RefCell::new(Session::default()));
    let picker = ui.global::<ActionPicker>();
    picker.set_shortcut_desktop(shortcut_desktop());
    load_desktop_shortcuts(ui.as_weak());

    let weak = ui.as_weak();
    picker.on_begin_capture(move || {
        CAPTURE.with(|state| *state.borrow_mut() = CaptureState::default());
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(!picker.get_capturing());
            picker.set_capture_keys("".into());
        }
    });

    on!(ui, picker.on_select_behavior => |ui, behavior| {
        let picker = ui.global::<ActionPicker>();
        if (behavior == PickerBehavior::Native && !picker.get_allow_default())
            || (behavior == PickerBehavior::Swallow && !picker.get_allow_swallow())
        {
            return;
        }
        picker.set_behavior(behavior);
        picker.set_ignore_key(behavior == PickerBehavior::Swallow);
        picker.set_capturing(false);
        picker.set_error(Msg::None.to_ui());
        picker.invoke_refresh();
    });

    let weak = ui.as_weak();
    let (doc, state) = (document.clone(), session.clone());
    picker.on_open(move |target, index, value, key_only| {
        if let Some(ui) = weak.upgrade() {
            session::open(&ui, &doc, &state, target, index, value, key_only);
        }
    });

    commands::bind(ui, document, &session);

    on!(ui, picker.on_refresh, doc = document, state = session => |ui| {
        session::refresh(&ui, &doc, &state);
    });

    let weak = ui.as_weak();
    let state = session.clone();
    picker.on_dismiss_picker(move || {
        state.borrow_mut().target = None;
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(false);
            picker.set_opened(false);
        }
    });

    on!(ui, picker.on_apply, doc = document, state = session => |ui| {
        session::apply(&ui, &doc, &state);
    });
}

#[cfg(test)]
#[path = "picker/tests.rs"]
mod tests;
