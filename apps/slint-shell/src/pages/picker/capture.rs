//! Physical keyboard capture for the action picker.

use crate::ui::{ActionPicker, CaptureStatus, SettingsWindow};
use slint::ComponentHandle;
use std::cell::RefCell;

/// Keyboard capture for the picker; returns `true` to withhold the event from Slint.
pub(in crate::pages) fn capture(
    ui: &SettingsWindow,
    event: &slint::winit_030::winit::event::WindowEvent,
    capture: &RefCell<CaptureState>,
) -> bool {
    use slint::winit_030::winit::event::WindowEvent;
    let picker = ui.global::<ActionPicker>();
    if !picker.get_opened() || !picker.get_capturing() {
        return false;
    }
    if let WindowEvent::Focused(false) = event {
        // Releases while unfocused never arrive: drop the half-pressed keys.
        abort(&picker, capture);
        return false;
    }
    let WindowEvent::KeyboardInput { event, .. } = event else {
        return false;
    };
    if event.repeat {
        return true;
    }
    let pressed = event.state == slint::winit_030::winit::event::ElementState::Pressed;
    let Some(key) = physical_key_name(event.physical_key) else {
        if pressed {
            capture.borrow().cancel_timer.stop();
            picker.set_cancel_armed(false);
            picker.set_capture_unsupported(true);
        }
        return true;
    };
    if key == "Escape" && pressed {
        arm_cancel(ui, capture);
    } else if pressed || key == "Escape" {
        // Another key or an early Escape release: this is not a cancel.
        capture.borrow().cancel_timer.stop();
    }
    let (known, value) = {
        let mut state = capture.borrow_mut();
        let known = state.known(&key, picker.get_trigger_chords());
        let value = state.update(
            &key,
            event.state,
            picker.get_trigger_chords(),
            picker.get_key_only(),
        );
        picker.set_capture_keys(state.held.join("+").into());
        (known, value)
    };
    if event.state == slint::winit_030::winit::event::ElementState::Pressed {
        picker.set_capture_unsupported(!known);
    }
    picker.set_cancel_armed(capture.borrow().cancel_timer.running());
    if let Some(value) = value {
        finish(&picker, value);
    }
    // Slint tracks modifiers from key events, not ModifiersChanged. Swallowing
    // a release here can leave Ctrl/Shift stuck when capture ends and block typing.
    tecloid_core::config::key_catalog::modifier_key(&key).is_none()
}

/// How long Escape must be held alone to cancel capture. Escape itself is
/// assignable, so a plain press cannot cancel; without this a keyboard-only
/// user could leave capture only by recording some key.
pub(super) const CANCEL_HOLD: std::time::Duration = std::time::Duration::from_secs(2);

/// Start the cancel timer when Escape is the only key of this capture.
pub(super) fn arm_cancel(ui: &SettingsWindow, capture: &RefCell<CaptureState>) {
    let state = capture.borrow();
    if state.held.iter().chain(&state.keys).any(|k| k != "Escape") {
        state.cancel_timer.stop();
        return;
    }
    let weak = ui.as_weak();
    state
        .cancel_timer
        .start(slint::TimerMode::SingleShot, CANCEL_HOLD, move || {
            let Some(ui) = weak.upgrade() else { return };
            cancel(&ui.global::<ActionPicker>());
        });
}

/// Leave capture without changing the value. The keys pressed so far are
/// dropped by the next `begin-capture`, which resets the state.
pub(super) fn cancel(picker: &ActionPicker) {
    if !picker.get_capturing() {
        return;
    }
    picker.set_capturing(false);
    picker.set_cancel_armed(false);
    picker.set_capture_unsupported(false);
    picker.set_capture_keys("".into());
    picker.set_capture_status(CaptureStatus::Cancelled);
}

/// Leave capture because the window lost focus: forget the keys pressed
/// so far and report the capture as cancelled.
pub(super) fn abort(picker: &ActionPicker, capture: &RefCell<CaptureState>) {
    capture.borrow_mut().reset();
    cancel(picker);
}

/// Config key code of a physical key. Keys winit cannot name on Linux
/// (most media, launch and laptop keys) arrive with their evdev code.
pub(super) fn physical_key_name(
    key: slint::winit_030::winit::keyboard::PhysicalKey,
) -> Option<String> {
    use slint::winit_030::winit::keyboard::PhysicalKey;
    match key {
        PhysicalKey::Code(code) => Some(key_name(code)),
        #[cfg(target_os = "linux")]
        PhysicalKey::Unidentified(slint::winit_030::winit::keyboard::NativeKeyCode::Xkb(code)) => {
            u16::try_from(code)
                .ok()
                .and_then(tecloid_core::mapper::keys::evdev_to_code)
                .map(str::to_owned)
        }
        PhysicalKey::Unidentified(_) => None,
    }
}

/// Config key code of a winit key code. winit names its `KeyCode` variants
/// after the W3C `KeyboardEvent.code` values the config uses, so the
/// variant name is the code except for the Meta keys.
pub(super) fn key_name(code: slint::winit_030::winit::keyboard::KeyCode) -> String {
    let raw = format!("{code:?}");
    match raw.as_str() {
        "SuperLeft" => "MetaLeft".into(),
        "SuperRight" => "MetaRight".into(),
        _ => raw,
    }
}

/// Put a captured value into the picker and stop capturing.
pub(super) fn finish(picker: &ActionPicker, value: String) {
    picker.set_touched(true);
    picker.set_value(value.into());
    picker.set_capturing(false);
    picker.set_cancel_armed(false);
    picker.set_capture_unsupported(false);
    picker.set_capture_status(CaptureStatus::Captured);
    picker.invoke_refresh();
}

#[derive(Default)]
pub(in crate::pages) struct CaptureState {
    pub(super) held: Vec<String>,
    pub(super) keys: Vec<String>,
    /// Fires when Escape is held alone for [`CANCEL_HOLD`].
    cancel_timer: slint::Timer,
}

impl CaptureState {
    /// Forget the pressed keys and stop the cancel timer.
    pub(in crate::pages) fn reset(&mut self) {
        self.cancel_timer.stop();
        self.held.clear();
        self.keys.clear();
    }

    /// Whether the Escape-hold cancel timer is armed.
    #[cfg(test)]
    pub(super) fn cancel_armed(&self) -> bool {
        self.cancel_timer.running()
    }

    /// Whether capture accepts `key`.
    pub(super) fn known(&self, key: &str, trigger: bool) -> bool {
        if trigger {
            tecloid_core::config::key_catalog::valid_trigger(key)
        } else {
            tecloid_core::config::key_catalog::valid_key(key)
        }
    }

    pub(super) fn update(
        &mut self,
        key: &str,
        state: slint::winit_030::winit::event::ElementState,
        trigger: bool,
        key_only: bool,
    ) -> Option<String> {
        use slint::winit_030::winit::event::ElementState;
        if !self.known(key, trigger) {
            return None;
        }
        if state == ElementState::Pressed {
            if !self.held.iter().any(|k| k == key) {
                self.held.push(key.to_owned());
            }
            if !self.keys.iter().any(|k| k == key) {
                self.keys.push(key.to_owned());
            }
            return None;
        }
        let index = self.held.iter().position(|k| k == key)?;
        self.held.remove(index);
        if !self.held.is_empty() {
            return None;
        }
        self.finish(trigger, key_only)
    }

    /// Value of the keys pressed so far; ends the capture.
    pub(in crate::pages) fn finish(&mut self, trigger: bool, key_only: bool) -> Option<String> {
        self.cancel_timer.stop();
        self.held.clear();
        let keys = std::mem::take(&mut self.keys);
        if keys.is_empty() {
            return None;
        }
        if trigger || keys.len() == 1 {
            return Some(keys.join("+"));
        }
        // A plain key slot takes one key: the last one pressed.
        if key_only {
            return keys.last().cloned();
        }
        let mut modifiers = Vec::new();
        let mut ordinary = Vec::new();
        for key in &keys {
            let modifier = tecloid_core::config::key_catalog::modifier_name(key);
            if let Some(modifier) = modifier {
                if !modifiers.contains(&modifier) {
                    modifiers.push(modifier);
                }
            } else {
                ordinary.push(key.as_str());
            }
        }
        // Only modifiers: keep the physical keys, the last one is the main key.
        if ordinary.is_empty() {
            return Some(keys.join("+"));
        }
        Some(
            tecloid_core::config::key_catalog::MODIFIER_ORDER
                .into_iter()
                .filter(|m| modifiers.contains(m))
                .chain(ordinary)
                .collect::<Vec<_>>()
                .join("+"),
        )
    }
}
