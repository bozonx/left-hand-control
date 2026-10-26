//! State of the physical keyboards: Caps Lock and held modifiers.

use super::*;

/// Portal keycode mode has no modifier feed: require readable keyboard LEDs
/// rather than silently assuming Caps Lock is off.
pub(super) fn caps_lock_state() -> Result<bool, TextInputFailure> {
    HardwareState::capture().caps_lock_state()
}

/// Input devices with whether the mapper grabbed each one.
pub(super) struct HardwareState(Vec<(bool, evdev::Device)>);

impl HardwareState {
    pub(super) fn capture() -> Self {
        use std::os::unix::fs::MetadataExt;
        Self(
            evdev::enumerate()
                .map(|(path, device)| {
                    let grabbed = std::fs::metadata(path)
                        .is_ok_and(|meta| crate::mapper::linux::is_grabbed_keyboard(meta.rdev()));
                    (grabbed, device)
                })
                .collect(),
        )
    }

    pub(super) fn caps_lock_state(&self) -> Result<bool, TextInputFailure> {
        let mut caps = None;
        for (_, device) in &self.0 {
            if device
                .supported_leds()
                .is_some_and(|leds| leds.contains(evdev::LedCode::LED_CAPSL))
            {
                let leds = device.get_led_state().map_err(|e| {
                    TextInputFailure::BackendUnavailable(format!("read Caps Lock state: {e}"))
                })?;
                let value = leds.contains(evdev::LedCode::LED_CAPSL);
                if caps.is_some_and(|previous| previous != value) {
                    return Err(TextInputFailure::BackendUnavailable(
                        "keyboard Caps Lock states disagree".into(),
                    ));
                }
                caps = Some(value);
            }
        }
        caps.ok_or_else(|| {
            TextInputFailure::BackendUnavailable(
                "Caps Lock state is unavailable; use the libei backend".into(),
            )
        })
    }

    pub(super) fn modifiers_released(&self) -> Result<(), TextInputFailure> {
        use evdev::KeyCode as K;
        let modifiers = [
            K::KEY_LEFTCTRL,
            K::KEY_RIGHTCTRL,
            K::KEY_LEFTSHIFT,
            K::KEY_RIGHTSHIFT,
            K::KEY_LEFTALT,
            K::KEY_RIGHTALT,
            K::KEY_LEFTMETA,
            K::KEY_RIGHTMETA,
        ];
        // What a grabbed keyboard passes on is checked on the virtual device.
        for (_, device) in self.0.iter().filter(|(grabbed, _)| !grabbed) {
            if device
                .supported_keys()
                .is_some_and(|keys| modifiers.iter().any(|key| keys.contains(*key)))
            {
                let keys = device.get_key_state().map_err(|e| {
                    TextInputFailure::BackendUnavailable(format!("read held modifiers: {e}"))
                })?;
                if modifiers.iter().any(|key| keys.contains(*key)) {
                    return Err(TextInputFailure::BackendUnavailable(
                        "release keyboard modifiers before text input".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(super) fn modifiers_released() -> Result<(), TextInputFailure> {
    HardwareState::capture().modifiers_released()
}
