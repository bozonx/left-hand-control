//! Physical keyboard capture for the action picker.

use crate::ui::{ActionPicker, SettingsWindow};
use slint::ComponentHandle;
use std::cell::RefCell;

/// Keyboard capture for the picker; returns `true` to withhold the event from Slint.
pub fn capture(ui: &SettingsWindow, event: &slint::winit_030::winit::event::WindowEvent) -> bool {
    use slint::winit_030::winit::event::WindowEvent;
    let picker = ui.global::<ActionPicker>();
    if !picker.get_opened() || !picker.get_capturing() {
        return false;
    }
    if let WindowEvent::Focused(false) = event {
        picker.set_capturing(false);
        return false;
    }
    let WindowEvent::KeyboardInput { event, .. } = event else {
        return false;
    };
    let Some(key) = physical_key_name(event.physical_key) else {
        return true;
    };
    if event.repeat {
        return true;
    }
    let value = CAPTURE.with(|state| {
        let mut state = state.borrow_mut();
        let value = state.update(
            &key,
            event.state,
            picker.get_trigger_chords(),
            picker.get_key_only(),
        );
        picker.set_capture_keys(state.held.join("+").into());
        value
    });
    if let Some(value) = value {
        picker.set_touched(true);
        picker.set_value(value.into());
        picker.set_capturing(false);
        picker.invoke_refresh();
    }
    // Slint tracks modifiers from key events, not ModifiersChanged. Swallowing
    // a release here can leave Ctrl/Shift stuck when capture ends and block typing.
    lhc_core::profile::key_catalog::modifier_key(&key).is_none()
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
                .and_then(lhc_core::mapper::keys::evdev_to_code)
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

#[derive(Default)]
pub(super) struct CaptureState {
    pub(super) held: Vec<String>,
    pub(super) keys: Vec<String>,
}

impl CaptureState {
    pub(super) fn update(
        &mut self,
        key: &str,
        state: slint::winit_030::winit::event::ElementState,
        trigger: bool,
        key_only: bool,
    ) -> Option<String> {
        use slint::winit_030::winit::event::ElementState;
        let known = if trigger {
            lhc_core::profile::key_catalog::valid_trigger(key)
        } else {
            lhc_core::profile::key_catalog::valid_key(key)
        };
        if !known {
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
        let keys = std::mem::take(&mut self.keys);
        if trigger || key_only || keys.len() == 1 {
            return Some(keys.join("+"));
        }
        let mut modifiers = Vec::new();
        let mut ordinary = Vec::new();
        for key in &keys {
            let modifier = match key.as_str() {
                "ControlLeft" | "ControlRight" => Some("Ctrl"),
                "AltLeft" | "AltRight" => Some("Alt"),
                "ShiftLeft" | "ShiftRight" => Some("Shift"),
                "MetaLeft" | "MetaRight" => Some("Meta"),
                _ => None,
            };
            if let Some(modifier) = modifier {
                if !modifiers.contains(&modifier) {
                    modifiers.push(modifier);
                }
            } else {
                ordinary.push(key.as_str());
            }
        }
        Some(
            ["Ctrl", "Alt", "Shift", "Meta"]
                .into_iter()
                .filter(|m| modifiers.contains(m))
                .chain(ordinary)
                .collect::<Vec<_>>()
                .join("+"),
        )
    }
}

thread_local! { pub(super) static CAPTURE: RefCell<CaptureState> = const { RefCell::new(CaptureState { held: Vec::new(), keys: Vec::new() }) }; }
