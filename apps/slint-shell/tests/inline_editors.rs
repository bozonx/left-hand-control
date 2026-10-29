//! Inline drafts survive rejected persistence and explicit flushes are synchronous.

use slint::{
    ComponentHandle,
    platform::{Platform, WindowAdapter, software_renderer::MinimalSoftwareWindow},
};
use std::{cell::Cell, rc::Rc};

slint::slint! {
    import { InlineEditors as EditorState, InlineTextField, InlineTextArea } from "../ui/controls.slint";
    export { InlineEditors } from "../ui/controls.slint";
    export component InlineFixture inherits Window {
        width: 400px; height: 300px;
        in-out property <string> value: "Saved";
        in-out property <bool> multiline;
        out property <bool> editing: root.multiline ? area.editing : field.editing;
        out property <string> draft: root.multiline ? area.draft : field.draft;
        callback persist(string) -> bool;
        public function edit(value: string) {
            if root.multiline { area.begin(); area.draft = value; }
            else { field.begin(); field.draft = value; }
        }
        public function flush() -> bool { return EditorState.flush(); }
        VerticalLayout {
            field := InlineTextField {
                visible: !root.multiline;
                text: root.value;
                saved(value) => { return root.persist(value); }
            }
            area := InlineTextArea {
                visible: root.multiline;
                text: root.value;
                saved(value) => { return root.persist(value); }
            }
        }
    }
}

struct TestPlatform;
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(MinimalSoftwareWindow::new(Default::default()))
    }
}

#[test]
fn rejected_save_keeps_both_editor_drafts_and_blocks_flush_until_retry() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let ui = InlineFixture::new().unwrap();
    ui.global::<InlineEditors>()
        .on_nonempty(|value| !value.trim().is_empty());
    ui.global::<InlineEditors>()
        .on_synchronize(i_slint_core::properties::ChangeTracker::run_change_handlers);
    let accept = Rc::new(Cell::new(false));
    let accepted = accept.clone();
    let weak = ui.as_weak();
    ui.on_persist(move |value| {
        if !accepted.get() {
            return false;
        }
        weak.upgrade().unwrap().set_value(value);
        true
    });
    ui.show().unwrap();
    slint::platform::update_timers_and_animations();
    for multiline in [false, true] {
        ui.set_multiline(multiline);
        accept.set(false);
        let draft = if multiline {
            "Multiline draft\nRetry me"
        } else {
            "Field draft"
        };
        ui.invoke_edit(draft.into());
        assert!(!ui.invoke_flush());
        assert!(ui.get_editing());
        assert_eq!(ui.get_draft(), draft);
        accept.set(true);
        assert!(ui.invoke_flush());
        assert!(!ui.get_editing());
        assert_eq!(ui.get_value(), draft);
    }
}
