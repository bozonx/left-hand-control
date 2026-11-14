//! Compiled engine data: rule entries, actions, macros and the `EngineOutput` events the
//! engine emits.

use super::super::action::{Keystroke, MacroStepItem};
use super::super::config::GameModeCondition;
use super::super::system::{ProcessInvocation, SystemAction};
use crate::mapper::key_code::KeyCode;
use std::time::{Duration, Instant};

/// Resolved action: single chord, macro, system function, or a
/// keyboard-layout-independent Unicode character.
#[derive(Clone)]
pub(super) enum CompiledAction {
    Stroke(Keystroke),
    Macro(CompiledMacro),
    System(SystemAction),
    Command(ProcessInvocation),
    Literal(String),
    Swallow,
    /// Cannot run on this OS or desktop; firing it reports the action.
    Unavailable(String),
}

#[derive(Clone)]
pub(super) struct CompiledMacro {
    pub(super) steps: Vec<CompiledMacroStep>,
}

#[derive(Clone)]
pub(super) struct CompiledMacroStep {
    pub(super) source: String,
    pub(super) item: MacroStepItem,
    pub(super) step_pause: Duration,
    pub(super) mod_delay: Duration,
}

#[derive(Clone)]
pub(super) enum TapMode {
    Native,
    NativeChord(Vec<KeyCode>),
    Swallow,
    Action(CompiledAction),
}

#[derive(Clone)]
pub(super) enum HoldOutputMode {
    Native,
    Swallow,
    Keystroke(Keystroke),
}

#[derive(Clone)]
pub(super) struct RuleEntry {
    pub(super) tap_decision: crate::mapper::config::TapDecision,
    pub(super) tap: TapMode,
    pub(super) layer_id: Option<String>,
    pub(super) hold: HoldOutputMode,
    pub(super) long_hold: Option<CompiledAction>,
    pub(super) long_hold_timeout: Duration,
    pub(super) double_tap: Option<CompiledAction>,
    pub(super) hold_timeout: Duration,
    /// Layer hold released within this time of the press, with no other
    /// input meanwhile, still fires the tap.
    pub(super) retro_tap: Option<Duration>,
    pub(super) double_tap_window: Duration,
    pub(super) condition_game_mode: Option<GameModeCondition>,
    pub(super) condition_keyboard_layouts: Option<Vec<String>>,
    pub(super) condition_window_patterns: Option<Vec<String>>,
    pub(super) condition_excluded_window_patterns: Option<Vec<String>>,
}

#[derive(Clone)]
pub(super) struct LayerTrigger {
    pub(super) key: KeyCode,
    /// Modifiers a hold-modifiers layer key left pressed; released together
    /// with this trigger (e.g. Alt of Alt+Tab while the switcher is open).
    pub(super) held_mods: Vec<KeyCode>,
}

#[derive(Clone)]
pub(super) struct ActiveMacro {
    pub(super) steps: Vec<CompiledMacroStep>,
    pub(super) mod_delay: Duration,
    pub(super) current_step: usize,
    pub(super) steps_emitted: usize,
    pub(super) phase: MacroPhase,
    pub(super) next_wake: Instant,
    /// The run is aborted and counts as finished at this time.
    pub(super) deadline: Instant,
}

#[derive(Clone)]
pub(super) enum MacroPhase {
    NextStep,
    FinalPause,
    Command(crate::mapper::commands::MacroCommand),
    Text(crate::text_input::MacroText),
    StrokeModDelayPress(Keystroke),
    StrokeModDelayRelease(Keystroke),
}

#[derive(Clone)]
pub(super) enum RulePhase {
    /// Waiting to decide between tap and hold (key is still down).
    WaitingDecision { deadline: Instant },
    /// Hold has been committed — layer (if any) is active, native /
    /// keystroke key is held down.
    HoldActive,
    /// Short press finished; waiting for a possible second press.
    WaitingSecond { deadline: Instant },
}

#[derive(Clone)]
pub(super) struct RuleExecution {
    pub(super) rule: RuleEntry,
    pub(super) phase: RulePhase,
    /// Second press inside the double-tap window of a rule with a hold:
    /// a short press fires the double-tap, a long one the hold.
    pub(super) second_press: bool,
    /// When the rule key was pressed (the event time, also for replays).
    pub(super) pressed_at: Instant,
}

impl RuleEntry {
    /// True when holding the key does something: a layer, a held keystroke
    /// or the native key itself (a Shift with a tap action is still a Shift
    /// when held). Only a swallowed hold with no layer does nothing.
    pub(super) fn has_hold(&self) -> bool {
        self.layer_id.is_some() || !matches!(self.hold, HoldOutputMode::Swallow)
    }

    /// True when the tap only types: the key itself, a text or a key with at
    /// most Shift. Typing heuristics may resolve such a press as a tap; a tap
    /// that runs a shortcut or a macro (Ctrl+V) must not replace a layer hold.
    pub(super) fn tap_types(&self) -> bool {
        match &self.tap {
            TapMode::Native | TapMode::NativeChord(_) => true,
            TapMode::Action(CompiledAction::Literal(_)) => true,
            TapMode::Action(CompiledAction::Stroke(ks)) => ks
                .mods
                .iter()
                .all(|m| matches!(*m, KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT)),
            _ => false,
        }
    }

    /// True for a chord rule with nothing to do on hold: its tap fires as
    /// soon as the chord completes.
    pub(super) fn taps_on_press(&self) -> bool {
        !self.has_hold()
            && self.long_hold.is_none()
            && self.double_tap.is_none()
            && !matches!(self.tap, TapMode::Swallow)
    }
}

/// A key event deferred while a rule key is in `WaitingDecision`
/// (permissive-hold mode). Replayed in order once the decision resolves,
/// carrying its original timestamp so downstream deadlines stay accurate.
#[derive(Clone)]
pub(super) struct BufferedEvent {
    pub(super) key: KeyCode,
    pub(super) down: bool,
    pub(super) at: Instant,
}

/// Outgoing action the engine asks the I/O layer to perform.
pub enum EngineOutput {
    /// Press or release a single key (no modifiers).
    KeyRaw { key: KeyCode, down: bool },
    /// Press/release a keystroke (mods + key), emitted atomically.
    Stroke { ks: Keystroke, mod_delay: Duration },
    /// Press a keystroke but keep the main key held until physical key-up.
    ChordPress { ks: Keystroke, mod_delay: Duration },
    /// Release a previously held keystroke.
    ChordRelease {
        key: KeyCode,
        mods: Vec<KeyCode>,
        mod_delay: Duration,
    },
    /// Release modifiers.
    ReleaseMods(Vec<KeyCode>),
    /// Execute a system function (DBus call or fire-and-forget spawn).
    RunSystem(SystemAction),
    /// Execute a shell command defined by the current keymap.
    RunCommand(ProcessInvocation),
    /// A macro command whose completion gates subsequent steps.
    RunMacroCommand(ProcessInvocation, crate::mapper::commands::MacroCommand),
    /// Type a single Unicode character via the Wayland virtual-keyboard
    /// backend. Fire-and-forget on key press; no release event is needed.
    Literal(String),
    /// A text step owned by one macro run; cancellation revokes queued delivery.
    RunMacroText(String, crate::text_input::MacroText),
}
