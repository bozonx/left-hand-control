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

pub(super) enum Phase {
    /// Waiting to decide between tap and hold (key is still down).
    WaitingDecision { deadline: Instant },
    /// Hold has been committed — layer (if any) is active, native /
    /// keystroke key is held down.
    HoldActive,
    /// Short press finished; waiting for a possible second press.
    WaitingSecond { deadline: Instant },
}

pub(super) struct Pending {
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
    /// Run a macro: sequence of keystrokes / system calls with inter-step
    /// pauses and an intra-chord delay between modifiers and the main key.
    RunMacro {
        steps: Vec<MacroStepItem>,
        step_pause: Duration,
        mod_delay: Duration,
    },
    /// Execute a system function (DBus call or fire-and-forget spawn).
    RunSystem(SysAction),
    /// Execute a shell command defined by the current layout.
    RunCommand(SysCommand),
    /// Type a single Unicode character via the Wayland virtual-keyboard
    /// backend. Fire-and-forget on key press; no release event is needed.
    Literal(String),
}

/// Emit a resolved action as the appropriate `Out` event. Shared by the
/// tap / double-tap / deferred-tap paths.
pub(super) fn fire_action(action: Option<&ActionDef>, mod_delay: Duration, out: &mut Vec<Out>) {
    match action {
        Some(ActionDef::Stroke(ks)) => out.push(Out::Stroke {
            ks: ks.clone(),
            mod_delay,
        }),
        Some(ActionDef::Literal(text)) => out.push(Out::Literal(text.clone())),
        Some(ActionDef::Macro(md)) => out.push(Out::RunMacro {
            steps: md.steps.clone(),
            step_pause: md.step_pause,
            mod_delay: md.mod_delay,
        }),
        Some(ActionDef::System(action)) => out.push(Out::RunSystem(action.clone())),
        Some(ActionDef::Command(command)) => out.push(Out::RunCommand(command.clone())),
        Some(ActionDef::Swallow) => {}
        None => {}
    }
}
