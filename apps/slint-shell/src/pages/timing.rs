//! Timing bounds shared by every numeric editor.

use crate::ui::{SettingsWindow, Timing, TimingRange};
use slint::ComponentHandle;
use tecloid_core::config::timing::{self, MillisecondsRange};

fn range(value: MillisecondsRange) -> TimingRange {
    TimingRange {
        minimum: value.min as f32,
        maximum: value.max as f32,
    }
}

pub(super) fn bind(ui: &SettingsWindow) {
    let timing = ui.global::<Timing>();
    timing.set_tap_hold(range(timing::TAP_HOLD));
    timing.set_long_hold(range(timing::LONG_HOLD));
    timing.set_retro_tap(range(timing::RETRO_TAP));
    timing.set_trigger_chord(range(timing::TRIGGER_CHORD));
    timing.set_macro_delay(range(timing::MACRO_DELAY));
    timing.set_pause(range(timing::PAUSE));
    timing.set_macro_timeout(range(timing::MACRO_TIMEOUT));
}
