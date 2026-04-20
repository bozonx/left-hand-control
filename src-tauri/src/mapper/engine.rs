// Tap/hold/double-tap + layer state machine.
//
// Per-rule fields (see `RuleEntry`): layer?, tap?, double_tap?, hold,
// double_tap_window.
//
// Phases of a rule key's pending state (see `Phase`):
//   * WaitingDecision — key was pressed, we don't yet know if it is a
//     tap or a hold. Transitions:
//       - release before deadline, no double_tap configured → fire tap.
//       - release before deadline, double_tap configured  → enter
//         WaitingSecond (delay the tap by double_tap_window to give the
//         user a chance to re-press for the double-tap action).
//       - deadline expires while still held                → push layer,
//         transition to HoldActive.
//   * HoldActive — layer is pushed. On release: pop layer.
//   * WaitingSecond — key already released after a short press, waiting
//     for a possible second press within `double_tap_window`.
//       - same rule key pressed again → fire double_tap immediately,
//         mark the physical key as consumed (the matching release
//         becomes a no-op), exit the phase.
//       - deadline expires                                  → fire tap.
//       - any other physical key gets pressed               → flush the
//         pending tap (commit it immediately) and continue handling the
//         new key. Gives predictable "rolling keys" UX.
//
// Fast path: a rule with only a layer (no tap, no double_tap) skips
// WaitingDecision and pushes the layer on the initial key-down.
//
// Non-rule keys:
//   * On press: look up action for this key in the topmost active layer
//     whose keymap contains it. If none → pass-through the physical key.
//   * The physical key is remembered with the emitted keystroke so we
//     release exactly what we pressed on key-up.

#![cfg(target_os = "linux")]

use super::action::{literal_char, parse_action, Keystroke, MacroStepItem};
use super::config::AppConfig;
use super::keys::code_to_key;
use super::system::{self, SysAction};
use super::system_macros::SYSTEM_MACROS;
use evdev::Key;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Resolved action: single chord, macro, system function, or a
/// literal character delivered through the Wayland virtual-keyboard
/// backend (layout-independent, see `mapper::vkbd`).
#[derive(Clone)]
enum ActionDef {
    Stroke(Keystroke),
    Macro(MacroDef),
    System(SysAction),
    Literal(char),
}

#[derive(Clone)]
struct MacroDef {
    steps: Vec<MacroStepItem>,
    step_pause: Duration,
    mod_delay: Duration,
}

pub struct Engine {
    rules: HashMap<Key, RuleEntry>,
    /// layer_id -> (physical_key -> resolved action)
    layer_maps: HashMap<String, HashMap<Key, ActionDef>>,
    default_hold: Duration,
    default_double_tap: Duration,

    /// Stack of currently active layers (top = highest priority).
    active_layers: Vec<String>,

    /// Per-rule pending state (waiting for release/timeout decision).
    pending: HashMap<Key, Pending>,

    /// Tracks which physical keys are currently pressed AND what keystroke
    /// we emitted for them, so key-up releases the right virtual keys.
    emitted: HashMap<Key, Keystroke>,

    /// Physical keys whose press triggered a one-shot macro. Release just
    /// clears the entry (no virtual key release needed).
    macro_consumed: HashSet<Key>,

    /// Currently "held down" virtual modifiers (refcounted).
    mod_refs: HashMap<Key, u32>,
}

struct RuleEntry {
    layer: Option<String>,
    tap: Option<ActionDef>,
    double_tap: Option<ActionDef>,
    hold: Duration,
    double_tap_window: Duration,
}

enum Phase {
    /// Waiting to decide between tap and hold (key is still down).
    WaitingDecision { deadline: Instant },
    /// Hold has been committed — layer (if any) is active.
    HoldActive,
    /// Short press finished; waiting for a possible second press.
    WaitingSecond { deadline: Instant },
}

struct Pending {
    phase: Phase,
    layer: Option<String>,
    tap: Option<ActionDef>,
    double_tap: Option<ActionDef>,
}

/// Outgoing action the engine asks the I/O layer to perform.
pub enum Out {
    /// Press or release a single key (no modifiers).
    KeyRaw { key: Key, down: bool },
    /// Press/release a keystroke (mods + key), emitted atomically.
    Stroke(Keystroke),
    /// Press modifiers (without the main key) — used to hold mods for remapped keys.
    PressMods(Vec<Key>),
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
    /// Type a single Unicode character via the Wayland virtual-keyboard
    /// backend. Fire-and-forget on key press; no release event is needed.
    Literal(char),
}

impl Engine {
    pub fn new(cfg: &AppConfig) -> Self {
        let default_hold = Duration::from_millis(cfg.settings.default_hold_timeout_ms.max(1));
        let default_double_tap =
            Duration::from_millis(cfg.settings.default_double_tap_timeout_ms.max(1));
        let default_step_pause =
            Duration::from_millis(cfg.settings.default_macro_step_pause_ms);
        let default_mod_delay =
            Duration::from_millis(cfg.settings.default_macro_modifier_delay_ms);

        // Build the macro table first so tap / keymap actions can reference
        // macros by id. System macros are seeded first; user macros with the
        // same id override them.
        let mut macros: HashMap<String, MacroDef> = HashMap::new();

        fn build_steps<'a>(
            id: &str,
            keystrokes: impl Iterator<Item = &'a str>,
        ) -> Vec<MacroStepItem> {
            let mut steps: Vec<MacroStepItem> = Vec::new();
            for (idx, raw) in keystrokes.enumerate() {
                let raw = raw.trim();
                if raw.is_empty() {
                    continue;
                }
                if let Some(rest) = raw.strip_prefix("sys:") {
                    match system::resolve(rest.trim()) {
                        Some(cmd) => steps.push(MacroStepItem::System(cmd)),
                        None => eprintln!(
                            "[mapper] macro {} step #{}: system fn {:?} not available",
                            id,
                            idx + 1,
                            rest.trim()
                        ),
                    }
                    continue;
                }
                if let Some(ch) = literal_char(raw) {
                    steps.push(MacroStepItem::Literal(ch));
                    continue;
                }
                match parse_action(raw) {
                    Some(ks) => steps.push(MacroStepItem::Stroke(ks)),
                    None => eprintln!(
                        "[mapper] macro {} step #{}: unknown keystroke {:?}",
                        id,
                        idx + 1,
                        raw
                    ),
                }
            }
            steps
        }

        for sys in SYSTEM_MACROS {
            let steps = build_steps(sys.id, sys.steps.iter().copied());
            if steps.is_empty() {
                eprintln!(
                    "[mapper] system macro {} has no usable steps — skipped",
                    sys.id
                );
                continue;
            }
            macros.insert(
                sys.id.to_string(),
                MacroDef {
                    steps,
                    step_pause: default_step_pause,
                    mod_delay: default_mod_delay,
                },
            );
        }

        for m in &cfg.macros {
            if m.id.is_empty() {
                eprintln!("[mapper] skipping macro with empty id: {:?}", m.name);
                continue;
            }
            let steps = build_steps(&m.id, m.steps.iter().map(|s| s.keystroke.as_str()));
            if steps.is_empty() {
                eprintln!("[mapper] macro {} has no usable steps — skipped", m.id);
                continue;
            }
            let step_pause = m
                .step_pause_ms
                .map(Duration::from_millis)
                .unwrap_or(default_step_pause);
            let mod_delay = m
                .modifier_delay_ms
                .map(Duration::from_millis)
                .unwrap_or(default_mod_delay);
            macros.insert(
                m.id.clone(),
                MacroDef {
                    steps,
                    step_pause,
                    mod_delay,
                },
            );
        }

        let resolve = |action: &str, where_: &str| -> Option<ActionDef> {
            let trimmed = action.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Some(rest) = trimmed.strip_prefix("macro:") {
                let id = rest.trim();
                if let Some(md) = macros.get(id) {
                    return Some(ActionDef::Macro(md.clone()));
                }
                eprintln!("[mapper] unknown macro ref {:?} ({})", trimmed, where_);
                return None;
            }
            if let Some(rest) = trimmed.strip_prefix("sys:") {
                let name = rest.trim();
                match system::resolve(name) {
                    Some(cmd) => return Some(ActionDef::System(cmd)),
                    None => {
                        eprintln!(
                            "[mapper] system fn {:?} not available on this OS/DE ({})",
                            name, where_
                        );
                        return None;
                    }
                }
            }
            if let Some(ch) = literal_char(trimmed) {
                return Some(ActionDef::Literal(ch));
            }
            match parse_action(trimmed) {
                Some(ks) => Some(ActionDef::Stroke(ks)),
                None => {
                    eprintln!("[mapper] unknown action {:?} ({})", trimmed, where_);
                    None
                }
            }
        };

        let mut rules = HashMap::new();
        for r in &cfg.rules {
            let Some(key) = code_to_key(&r.key) else {
                eprintln!("[mapper] unknown rule key: {}", r.key);
                continue;
            };
            let tap = if r.tap_action.is_empty() {
                None
            } else {
                resolve(&r.tap_action, &format!("tap for {}", r.key))
            };
            let double_tap = if r.double_tap_action.is_empty() {
                None
            } else {
                resolve(
                    &r.double_tap_action,
                    &format!("double-tap for {}", r.key),
                )
            };
            let layer = if r.layer_id.is_empty() {
                None
            } else {
                Some(r.layer_id.clone())
            };
            let hold = r
                .hold_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_hold);
            let double_tap_window = r
                .double_tap_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_double_tap);
            rules.insert(
                key,
                RuleEntry {
                    layer,
                    tap,
                    double_tap,
                    hold,
                    double_tap_window,
                },
            );
        }

        let mut layer_maps: HashMap<String, HashMap<Key, ActionDef>> = HashMap::new();
        for (layer_id, km) in &cfg.layer_keymaps {
            let mut m = HashMap::new();
            for (code, action) in &km.keys {
                let Some(key) = code_to_key(code) else {
                    eprintln!("[mapper] unknown key code in keymap {layer_id}: {code}");
                    continue;
                };
                let Some(def) = resolve(action, &format!("keymap {layer_id}.{code}")) else {
                    continue;
                };
                m.insert(key, def);
            }
            layer_maps.insert(layer_id.clone(), m);
        }

        Self {
            rules,
            layer_maps,
            default_hold,
            default_double_tap,
            active_layers: Vec::new(),
            pending: HashMap::new(),
            emitted: HashMap::new(),
            macro_consumed: HashSet::new(),
            mod_refs: HashMap::new(),
        }
    }

    /// Time until the nearest pending deadline (hold-decision or
    /// waiting-for-second-press), or None.
    pub fn next_deadline(&self, now: Instant) -> Option<Duration> {
        self.pending
            .values()
            .filter_map(|p| match p.phase {
                Phase::WaitingDecision { deadline }
                | Phase::WaitingSecond { deadline } => {
                    Some(deadline.saturating_duration_since(now))
                }
                Phase::HoldActive => None,
            })
            .min()
    }

    /// Tick pending state machines. Emits layer activations for expired
    /// holds and fires delayed taps whose double-tap window elapsed.
    pub fn tick(&mut self, now: Instant, out: &mut Vec<Out>) {
        // Collect keys to process so we don't borrow `self.pending`
        // mutably while mutating `self` through helpers.
        let mut expired_hold: Vec<Key> = Vec::new();
        let mut expired_second: Vec<Key> = Vec::new();
        for (k, p) in &self.pending {
            match p.phase {
                Phase::WaitingDecision { deadline } if deadline <= now => {
                    expired_hold.push(*k);
                }
                Phase::WaitingSecond { deadline } if deadline <= now => {
                    expired_second.push(*k);
                }
                _ => {}
            }
        }
        for k in expired_hold {
            let layer = {
                let p = self.pending.get_mut(&k).unwrap();
                p.phase = Phase::HoldActive;
                p.layer.clone()
            };
            if let Some(layer) = layer {
                eprintln!("[mapper] hold-timeout -> layer+: {layer} (key={:?})", k);
                self.push_layer(layer, out);
            } else {
                eprintln!("[mapper] hold-timeout (no layer) key={:?}", k);
            }
        }
        for k in expired_second {
            let p = self.pending.remove(&k).unwrap();
            eprintln!("[mapper] dtap-window expired -> tap (key={:?})", k);
            fire_action(p.tap.as_ref(), out);
        }
    }

    /// Handle a raw key event from the grabbed device.
    pub fn handle(&mut self, key: Key, down: bool, now: Instant, out: &mut Vec<Out>) {
        eprintln!(
            "[mapper] in {} key={:?} active={:?}",
            if down { "DOWN" } else { " UP " },
            key,
            self.active_layers
        );
        if down {
            self.on_press(key, now, out);
        } else {
            self.on_release(key, now, out);
        }
    }

    fn on_press(&mut self, key: Key, now: Instant, out: &mut Vec<Out>) {
        // Second press of a rule key that is currently waiting for a
        // double-tap → fire it immediately.
        if matches!(
            self.pending.get(&key).map(|p| &p.phase),
            Some(Phase::WaitingSecond { .. })
        ) {
            let p = self.pending.remove(&key).unwrap();
            eprintln!("[mapper] double-tap fired (key={:?})", key);
            fire_action(p.double_tap.as_ref(), out);
            // The matching release must not emit anything (the action
            // was fire-and-forget, like a macro).
            self.macro_consumed.insert(key);
            return;
        }

        if let Some(rule) = self.rules.get(&key) {
            let layer = rule.layer.clone();
            let tap = rule.tap.clone();
            let double_tap = rule.double_tap.clone();
            let hold = rule.hold;
            // Fast path: layer-only rule → activate immediately, no tap wait.
            if layer.is_some() && tap.is_none() && double_tap.is_none() {
                if let Some(l) = layer.clone() {
                    self.push_layer(l, out);
                }
                self.pending.insert(
                    key,
                    Pending {
                        phase: Phase::HoldActive,
                        layer,
                        tap: None,
                        double_tap: None,
                    },
                );
                return;
            }
            // Tap-hold / tap-only / double-tap: wait for release or timeout.
            self.pending.insert(
                key,
                Pending {
                    phase: Phase::WaitingDecision {
                        deadline: now + hold,
                    },
                    layer,
                    tap,
                    double_tap,
                },
            );
            return;
        }

        // Any non-rule key-down must commit any currently deferred taps so
        // the emitted events stay in the natural press order.
        self.flush_waiting_second(out);

        // Non-rule key: consult active layers (top → bottom), fall back to base.
        let mapped = self.lookup_mapping(key);
        match mapped {
            Some(ActionDef::Stroke(ks)) => {
                eprintln!(
                    "[mapper]   press {:?} -> remap mods={:?} key={:?}",
                    key, ks.mods, ks.key
                );
                self.emit_stroke_press(key, ks, out);
            }
            Some(ActionDef::Literal(ch)) => {
                eprintln!("[mapper]   press {:?} -> literal {:?}", key, ch);
                out.push(Out::Literal(ch));
                // Literal output is fire-and-forget, like macros — release
                // event for the physical key must not emit anything.
                self.macro_consumed.insert(key);
            }
            Some(ActionDef::Macro(md)) => {
                eprintln!(
                    "[mapper]   press {:?} -> run macro ({} steps)",
                    key,
                    md.steps.len()
                );
                out.push(Out::RunMacro {
                    steps: md.steps,
                    step_pause: md.step_pause,
                    mod_delay: md.mod_delay,
                });
                self.macro_consumed.insert(key);
            }
            Some(ActionDef::System(action)) => {
                eprintln!("[mapper]   press {:?} -> run system {:?}", key, action);
                out.push(Out::RunSystem(action));
                self.macro_consumed.insert(key);
            }
            None => {
                eprintln!("[mapper]   press {:?} -> passthrough", key);
                out.push(Out::KeyRaw { key, down: true });
                self.emitted.insert(
                    key,
                    Keystroke {
                        mods: vec![],
                        key,
                    },
                );
            }
        }
    }

    fn on_release(&mut self, key: Key, now: Instant, out: &mut Vec<Out>) {
        if let Some(p) = self.pending.get(&key) {
            match p.phase {
                Phase::HoldActive => {
                    let layer = p.layer.clone();
                    self.pending.remove(&key);
                    if let Some(l) = layer {
                        self.pop_layer(&l, out);
                    }
                }
                Phase::WaitingDecision { .. } => {
                    // Short press. If double-tap is configured, delay the
                    // tap by double_tap_window to see if a second press
                    // follows; otherwise fire tap immediately.
                    let has_dtap = p.double_tap.is_some();
                    let rule_window = self
                        .rules
                        .get(&key)
                        .map(|r| r.double_tap_window)
                        .unwrap_or(self.default_double_tap);
                    if has_dtap {
                        let p = self.pending.get_mut(&key).unwrap();
                        p.phase = Phase::WaitingSecond {
                            deadline: now + rule_window,
                        };
                    } else {
                        let p = self.pending.remove(&key).unwrap();
                        fire_action(p.tap.as_ref(), out);
                    }
                }
                Phase::WaitingSecond { .. } => {
                    // Shouldn't happen (we already released once), but
                    // treat as no-op to stay safe.
                }
            }
            return;
        }

        if self.macro_consumed.remove(&key) {
            // Macro was fired on press — nothing to release.
            return;
        }

        if let Some(ks) = self.emitted.remove(&key) {
            out.push(Out::KeyRaw {
                key: ks.key,
                down: false,
            });
            if !ks.mods.is_empty() {
                for m in &ks.mods {
                    if let Some(r) = self.mod_refs.get_mut(m) {
                        *r = r.saturating_sub(1);
                    }
                }
                // Only release modifiers whose refcount dropped to zero.
                let to_release: Vec<Key> = ks
                    .mods
                    .iter()
                    .copied()
                    .filter(|m| self.mod_refs.get(m).copied().unwrap_or(0) == 0)
                    .collect();
                if !to_release.is_empty() {
                    out.push(Out::ReleaseMods(to_release));
                }
            }
        } else {
            // Unknown / orphan release — pass through as release to avoid stuck keys.
            out.push(Out::KeyRaw { key, down: false });
        }
    }

    /// Press a resolved keystroke and remember it so the matching
    /// release event (on physical key up) can emit the correct virtual
    /// key-up + mod-release.
    fn emit_stroke_press(&mut self, physical: Key, ks: Keystroke, out: &mut Vec<Out>) {
        if !ks.mods.is_empty() {
            out.push(Out::PressMods(ks.mods.clone()));
            for m in &ks.mods {
                *self.mod_refs.entry(*m).or_insert(0) += 1;
            }
        }
        out.push(Out::KeyRaw {
            key: ks.key,
            down: true,
        });
        self.emitted.insert(physical, ks);
    }

    fn lookup_mapping(&self, key: Key) -> Option<ActionDef> {
        for l in self.active_layers.iter().rev() {
            if let Some(m) = self.layer_maps.get(l) {
                if let Some(def) = m.get(&key) {
                    return Some(def.clone());
                }
            }
        }
        self.layer_maps
            .get("base")
            .and_then(|m| m.get(&key).cloned())
    }

    fn push_layer(&mut self, id: String, _out: &mut Vec<Out>) {
        self.active_layers.push(id);
    }

    fn pop_layer(&mut self, id: &str, _out: &mut Vec<Out>) {
        // Remove the topmost occurrence.
        if let Some(pos) = self.active_layers.iter().rposition(|l| l == id) {
            self.active_layers.remove(pos);
        }
    }

    /// Fire all pending taps whose double-tap window is still open — used
    /// when another key is pressed, so the deferred tap is committed
    /// before the new key's action. Preserves ordering.
    fn flush_waiting_second(&mut self, out: &mut Vec<Out>) {
        let keys: Vec<Key> = self
            .pending
            .iter()
            .filter(|(_, p)| matches!(p.phase, Phase::WaitingSecond { .. }))
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            let p = self.pending.remove(&k).unwrap();
            eprintln!("[mapper] dtap-window flushed -> tap (key={:?})", k);
            fire_action(p.tap.as_ref(), out);
        }
    }

    /// Generic shutdown helper — release everything that is currently held.
    pub fn shutdown(&mut self, out: &mut Vec<Out>) {
        for (_, ks) in self.emitted.drain() {
            out.push(Out::KeyRaw {
                key: ks.key,
                down: false,
            });
        }
        let mods: Vec<Key> = self
            .mod_refs
            .drain()
            .filter(|(_, r)| *r > 0)
            .map(|(k, _)| k)
            .collect();
        if !mods.is_empty() {
            out.push(Out::ReleaseMods(mods));
        }
        self.active_layers.clear();
        self.pending.clear();
        self.macro_consumed.clear();
    }

    #[allow(dead_code)]
    pub fn default_hold(&self) -> Duration {
        self.default_hold
    }
}

/// Emit a resolved action as the appropriate `Out` event. Shared by the
/// tap / double-tap / deferred-tap paths.
fn fire_action(action: Option<&ActionDef>, out: &mut Vec<Out>) {
    match action {
        Some(ActionDef::Stroke(ks)) => out.push(Out::Stroke(ks.clone())),
        Some(ActionDef::Literal(ch)) => out.push(Out::Literal(*ch)),
        Some(ActionDef::Macro(md)) => out.push(Out::RunMacro {
            steps: md.steps.clone(),
            step_pause: md.step_pause,
            mod_delay: md.mod_delay,
        }),
        Some(ActionDef::System(action)) => out.push(Out::RunSystem(action.clone())),
        None => {}
    }
}
