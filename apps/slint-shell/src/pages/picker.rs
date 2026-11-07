//! Action picker: one dialog that chooses a key, shortcut or action for
//! every page and hands the value to the page that opened it.

use super::WindowInput;
use crate::{
    document::Document,
    i18n::Msg,
    ui::{ActionPicker, CaptureStatus, PickerBehavior, PickerItem, PickerTarget, SettingsWindow},
};
use lhc_core::profile::actions::Action;
use slint::ComponentHandle;
use slint::SharedString;
use std::{cell::RefCell, rc::Rc};

mod capture;
mod catalog;
mod commands;
mod request;
mod session;

pub(super) use capture::{CaptureState, capture};
#[cfg(test)]
use capture::{key_name, physical_key_name};
use catalog::shortcut_desktop;
pub(crate) use catalog::{DESKTOP_SHORTCUTS, action_label, category_for_value};
#[cfg(test)]
use request::Filter;
pub(super) use request::{PickerMode, PickerRequest};

/// Catalog label of an existing menu action, including staged commands.
pub(super) fn menu_label(
    ui: &SettingsWindow,
    config: &lhc_core::profile::model::ResolvedAppConfig,
    value: &str,
) -> SharedString {
    session::label_for(value, &catalog::catalog(ui, config), None, &config.commands)
}

/// Whether `value` is the `cmd:` action of the command `id`.
fn assigns_command(value: &str, id: &str) -> bool {
    matches!(Action::parse(Some(value)), Action::Command(command) if command == id)
}

/// The open picker: who receives the value and the catalog built on open.
#[derive(Default)]
struct Session {
    /// Changes on every open and close, so late background results of a
    /// previous session are ignored.
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    target: Option<(PickerTarget, i32)>,
    /// Value the picker opened with.
    initial: SharedString,
    catalog: Vec<PickerItem>,
    /// [`catalog::SHORTCUTS_GENERATION`] the catalog was built with.
    catalog_shortcuts: u64,
    /// Value an apply error refers to; editing the value clears the error.
    error_value: Option<SharedString>,
    command: Option<lhc_core::profile::model::Command>,
    layer_target: Option<super::layers::EditTarget>,
}

impl Session {
    /// Start a new generation; returns the shared counter for the next session.
    fn next_generation(&self) -> std::sync::Arc<std::sync::atomic::AtomicU64> {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.generation.clone()
    }

    fn current_generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub(super) struct Drafts {
    pub macros: Rc<RefCell<super::macros::State>>,
    pub menus: Rc<RefCell<super::menus::State>>,
}

impl Drafts {
    fn config(
        &self,
        document: &Document,
        target: Option<PickerTarget>,
    ) -> lhc_core::profile::model::ResolvedAppConfig {
        if target == Some(PickerTarget::MacroStep) {
            super::macros::picker_config(document, &self.macros)
        } else {
            document.read().config()
        }
    }

    fn command(&self, id: &str) -> Option<lhc_core::profile::model::Command> {
        super::macros::command(&self.macros, id).or_else(|| super::menus::command(&self.menus, id))
    }
}

pub(super) fn bind(
    ui: &SettingsWindow,
    document: &Rc<Document>,
    input: &Rc<WindowInput>,
    drafts: Drafts,
) {
    let drafts = Rc::new(drafts);
    let session = Rc::new(RefCell::new(Session::default()));
    let picker = ui.global::<ActionPicker>();
    picker.set_shortcut_desktop(shortcut_desktop());

    let weak = ui.as_weak();
    let input = input.clone();
    picker.on_begin_capture(move || {
        let Some(ui) = weak.upgrade() else { return };
        let picker = ui.global::<ActionPicker>();
        picker.set_capture_keys("".into());
        picker.set_capture_unsupported(false);
        picker.set_capture_status(CaptureStatus::None);
        picker.set_cancel_armed(false);
        if !picker.get_capturing() {
            input.reset_capture();
            picker.set_capturing(true);
            return;
        }
        // Stop keeps the keys pressed so far instead of dropping them.
        match input.stop_capture(picker.get_trigger_chords(), picker.get_key_only()) {
            Some(value) => capture::finish(&picker, value),
            None => picker.set_capturing(false),
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
        picker.set_capturing(false);
        picker.set_error(Msg::None.to_ui());
        picker.invoke_refresh();
    });

    on!(ui, picker.on_select_category, doc = document => |ui, category| {
        session::select_category(&ui, &doc, category);
    });

    on!(ui, picker.on_step_selection => |ui, delta| {
        session::step_selection(&ui, delta);
    });

    let weak = ui.as_weak();
    let (doc, state, pending) = (document.clone(), session.clone(), drafts.clone());
    picker.on_open(move |target, index, value, key_only| {
        if let Some(ui) = weak.upgrade() {
            session::open(
                &ui,
                &doc,
                &state,
                &pending,
                (target, index),
                value,
                key_only,
            );
        }
    });

    commands::bind(ui, document, &session, &drafts);

    on!(ui, picker.on_refresh, doc = document, state = session, drafts = drafts => |ui| {
        session::refresh(&ui, &doc, &state, &drafts);
    });

    let weak = ui.as_weak();
    let state = session.clone();
    picker.on_dismiss_picker(move || {
        {
            let mut state = state.borrow_mut();
            let generation = state.next_generation();
            *state = Session {
                generation,
                ..Session::default()
            };
        }
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(false);
            picker.set_choosing_file(false);
            picker.set_opened(false);
        }
    });

    on!(ui, picker.on_apply, doc = document, state = session, drafts = drafts => |ui| {
        session::apply(&ui, &doc, &state, &drafts);
    });
}

#[cfg(test)]
#[path = "picker/tests.rs"]
mod tests;
