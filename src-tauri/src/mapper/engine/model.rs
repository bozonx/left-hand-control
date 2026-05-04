use super::super::action::{Keystroke, MacroStepItem};
use super::super::system::{SysAction, SysCommand};
use evdev::Key;
use std::time::{Duration, Instant};

/// Resolved action: single chord, macro, system function, or a
/// layout-independent Unicode character.
#[derive(Clone)]
pub(super) enum ActionDef {
    Stroke(Keystroke),
    Macro(MacroDef),
    System(SysAction),
    Command(SysCommand),
    Literal(String),
    Swallow,
}

#[derive(Clone)]
pub(super) struct MacroDef {
    pub(super) steps: Vec<MacroStepItem>,
    pub(super) step_pause: Duration,
    pub(super) mod_delay: Duration,
}

#[derive(Clone)]
pub(super) enum TapMode {
    Native,
    Swallow,
    Action(ActionDef),
}

#[derive(Clone)]
pub(super) enum HoldMode {
    Native,
    Swallow,
    Keystroke(Keystroke),
}

#[derive(Clone)]
pub(super) struct RuleEntry {
    pub(super) tap: TapMode,
    pub(super) layer_id: Option<String>,
    pub(super) hold: HoldMode,
    pub(super) double_tap: Option<ActionDef>,
    pub(super) hold_timeout: Duration,
    pub(super) double_tap_window: Duration,
    pub(super) condition_game_mode: Option<String>,
    pub(super) condition_layouts: Option<Vec<String>>,
    pub(super) condition_apps_whitelist: Option<Vec<String>>,
    pub(super) condition_apps_blacklist: Option<Vec<String>>,
}

#[derive(Clone)]
pub(super) struct ActiveMacro {
    pub(super) steps: Vec<MacroStepItem>,
    pub(super) step_pause: Duration,
    pub(super) mod_delay: Duration,
    pub(super) current_step: usize,
    pub(super) phase: MacroPhase,
    pub(super) next_wake: Instant,
}

#[derive(Clone)]
pub(super) enum MacroPhase {
    NextStep,
    StrokeModDelayPress(Keystroke),
    StrokeModDelayRelease(Keystroke),
}

#[derive(Clone)]
pub(super) enum Phase {
    /// Waiting to decide between tap and hold (key is still down).
    WaitingDecision { deadline: Instant },
    /// Hold has been committed — layer (if any) is active, native /
    /// keystroke key is held down.
    HoldActive,
    /// Short press finished; waiting for a possible second press.
    WaitingSecond { deadline: Instant },
}

#[derive(Clone)]
pub(super) struct Pending {
    pub(super) rule: RuleEntry,
    pub(super) phase: Phase,
}

/// Outgoing action the engine asks the I/O layer to perform.
pub enum Out {
    /// Press or release a single key (no modifiers).
    KeyRaw { key: Key, down: bool },
    /// Press/release a keystroke (mods + key), emitted atomically.
    Stroke { ks: Keystroke, mod_delay: Duration },
    /// Press a keystroke but keep the main key held until physical key-up.
    ChordPress { ks: Keystroke, mod_delay: Duration },
    /// Release a previously held keystroke.
    ChordRelease {
        key: Key,
        mods: Vec<Key>,
        mod_delay: Duration,
    },
    /// Release modifiers.
    ReleaseMods(Vec<Key>),
    /// Execute a system function (DBus call or fire-and-forget spawn).
    RunSystem(SysAction),
    /// Execute a shell command defined by the current layout.
    RunCommand(SysCommand),
    /// Type a single Unicode character via the Wayland virtual-keyboard
    /// backend. Fire-and-forget on key press; no release event is needed.
    Literal(String),
}
