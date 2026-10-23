//! Shared timing contracts for persisted settings, rules and macros.

use super::model::{AppSettings, LayoutPreset};

#[derive(Clone, Copy)]
pub struct Range {
    pub min: u64,
    pub max: u64,
}

impl Range {
    pub fn contains(self, value: u64) -> bool {
        (self.min..=self.max).contains(&value)
    }

    pub fn check(self, value: u64, path: &str) -> Result<(), String> {
        if self.contains(value) {
            Ok(())
        } else {
            Err(format!(
                "{path}: expected {}..={} milliseconds, got {value}",
                self.min, self.max
            ))
        }
    }
}

pub const TAP_HOLD: Range = Range { min: 50, max: 1000 };
pub const LONG_HOLD: Range = Range {
    min: 100,
    max: 60_000,
};
pub const COMBO: Range = Range { min: 10, max: 500 };
pub const MACRO_DELAY: Range = Range { min: 0, max: 2000 };
pub const PAUSE: Range = Range {
    min: 1,
    max: 10_000,
};
pub const MACRO_TIMEOUT: Range = Range {
    min: 1000,
    max: 600_000,
};

pub fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if !matches!(
        settings.tap_decision.as_str(),
        "permissiveHold" | "holdOnOtherKeyPress"
    ) {
        return Err("settings.timing.tapDecision: unknown value".into());
    }
    for (path, value, range) in [
        ("holdMs", settings.default_hold_timeout_ms, TAP_HOLD),
        (
            "doubleTapMs",
            settings.default_double_tap_timeout_ms,
            TAP_HOLD,
        ),
        (
            "longHoldMs",
            settings.default_long_hold_timeout_ms,
            LONG_HOLD,
        ),
        ("comboMs", settings.combo_timeout_ms, COMBO),
        (
            "macroStepPauseMs",
            settings.default_macro_step_pause_ms,
            MACRO_DELAY,
        ),
        (
            "macroModifierDelayMs",
            settings.default_macro_modifier_delay_ms,
            MACRO_DELAY,
        ),
        ("macroTimeoutMs", settings.macro_timeout_ms, MACRO_TIMEOUT),
    ] {
        range.check(value, &format!("settings.timing.{path}"))?;
    }
    Ok(())
}

pub fn validate_layout(layout: &LayoutPreset) -> Result<(), String> {
    for (index, rule) in layout.rules.iter().enumerate() {
        for (field, value, range) in [
            ("hold.timeoutMs", rule.hold_timeout_ms, TAP_HOLD),
            ("doubleTap.timeoutMs", rule.double_tap_timeout_ms, TAP_HOLD),
            ("longHold.timeoutMs", rule.long_hold_timeout_ms, LONG_HOLD),
        ] {
            if let Some(value) = value {
                range.check(value, &format!("rules[{index}].{field}"))?;
            }
        }
    }
    for (index, item) in layout.macros.iter().enumerate() {
        for (field, value) in [
            ("stepPauseMs", item.step_pause_ms),
            ("modifierDelayMs", item.modifier_delay_ms),
        ] {
            if let Some(value) = value {
                MACRO_DELAY.check(value, &format!("macros[{index}].{field}"))?;
            }
        }
    }
    Ok(())
}
