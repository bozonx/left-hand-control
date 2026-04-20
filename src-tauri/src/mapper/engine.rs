// Tap / hold / double-tap + layer state machine.
//
// Per-rule configuration (see `RuleEntry`):
//   * `tap`        — TapMode::Native | Swallow | Action
//   * `hold`       — HoldMode::Native | Swallow | Layer | Keystroke
//   * `double_tap` — Option<ActionDef>
//   * `hold_timeout`, `double_tap_window`
//
// Three-state "tap" / "hold" semantics:
//   Native   — pass the physical key through on that event.
//              For `tap`: emit a short press+release of the physical key.
//              For `hold`: press physical key down on commit, release on
//                          physical key up.
//   Swallow  — do nothing on that event.
//   Action   — user-defined action string (tap fires an action; hold
//              presses the resolved keystroke down on commit and releases
//              it on physical key up, so e.g. `hold: ControlLeft` makes
//              the physical key behave like a held Ctrl modifier).
//
// Rules where `tap == Native`, `hold == Native` and no layer/keystroke
// hold is configured are completely transparent — they are not
// registered with the engine at all so the key passes through the kernel
// → uinput path unchanged.
//
// Phases of a rule key's pending state:
//   * WaitingDecision — key was pressed, we don't yet know if it is a
//     tap or a hold. Transitions:
//       - release before deadline, no double_tap configured → fire tap.
//       - release before deadline, double_tap configured  → enter
//         WaitingSecond (delay the tap by double_tap_window).
//       - deadline expires while still held                → commit hold.
//       - another key is pressed while we're still in this phase
//         ("interrupt on other key press") → commit hold immediately and
//         then let the new key flow through; this is required for
//         modifier-tap rules (Shift-tap-letter, MetaLeft hold=Ctrl, …)
//         to feel natural.
//   * HoldActive — hold has been committed (layer pushed, native key or
//     keystroke held). On release: undo the hold.
//   * WaitingSecond — short press finished; waiting for a possible
//     second press within `double_tap_window`.
//
// Fast path: a rule with `tap == Swallow` and no `double_tap` skips
// WaitingDecision and commits the hold immediately on key-down.

#![cfg(target_os = "linux")]

use super::action::{literal_text, parse_action, Keystroke, MacroStepItem};
use super::config::{ActionSpec, AppConfig};
use super::keys::code_to_key;
use super::system::{self, SysAction};
use super::system_macros::SYSTEM_MACROS;
use evdev::Key;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Resolved action: single chord, macro, system function, or a
/// layout-independent Unicode character.
#[derive(Clone)]
enum ActionDef {
    Stroke(Keystroke),
    Macro(MacroDef),
    System(SysAction),
    Literal(String),
}

#[derive(Clone)]
struct MacroDef {
    steps: Vec<MacroStepItem>,
    step_pause: Duration,
    mod_delay: Duration,
}

#[derive(Clone)]
enum TapMode {
    Native,
    Swallow,
    Action(ActionDef),
}

#[derive(Clone)]
enum HoldMode {
    Native,
    Swallow,
    Keystroke(Keystroke),
}

pub struct Engine {
    rules: HashMap<Key, RuleEntry>,
    /// layer_id -> (physical_key -> resolved action)
    layer_maps: HashMap<String, HashMap<Key, ActionDef>>,
    default_hold: Duration,
    default_double_tap: Duration,
    default_mod_delay: Duration,

    /// Stack of currently active layers (top = highest priority).
    active_layers: Vec<String>,

    /// Per-rule pending state (waiting for release/timeout decision).
    pending: HashMap<Key, Pending>,

    /// Tracks which physical keys are currently pressed AND what keystroke
    /// we emitted for them, so key-up releases the right virtual keys.
    /// Used both for non-rule remaps (keymap lookups) and for rule-keys
    /// whose hold mode is Native/Keystroke (uniform release path).
    emitted: HashMap<Key, Keystroke>,

    /// Physical keys whose press triggered a one-shot macro / literal.
    /// Release just clears the entry — no virtual key release is needed.
    macro_consumed: HashSet<Key>,

    /// Currently "held down" virtual modifiers (refcounted).
    mod_refs: HashMap<Key, u32>,
}

#[derive(Clone)]
struct RuleEntry {
    tap: TapMode,
    layer_id: Option<String>,
    hold: HoldMode,
    double_tap: Option<ActionDef>,
    hold_timeout: Duration,
    double_tap_window: Duration,
}

enum Phase {
    /// Waiting to decide between tap and hold (key is still down).
    WaitingDecision { deadline: Instant },
    /// Hold has been committed — layer (if any) is active, native /
    /// keystroke key is held down.
    HoldActive,
    /// Short press finished; waiting for a possible second press.
    WaitingSecond { deadline: Instant },
}

struct Pending {
    phase: Phase,
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
    /// Type a single Unicode character via the Wayland virtual-keyboard
    /// backend. Fire-and-forget on key press; no release event is needed.
    Literal(String),
}

impl Engine {
    pub fn new(cfg: &AppConfig) -> Self {
        let default_hold = Duration::from_millis(cfg.settings.default_hold_timeout_ms.max(1));
        let default_double_tap =
            Duration::from_millis(cfg.settings.default_double_tap_timeout_ms.max(1));
        let default_step_pause = Duration::from_millis(cfg.settings.default_macro_step_pause_ms);
        let default_mod_delay = Duration::from_millis(cfg.settings.default_macro_modifier_delay_ms);

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
                if let Some(text) = literal_text(raw) {
                    steps.push(MacroStepItem::Literal(text));
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
            if let Some(text) = literal_text(trimmed) {
                return Some(ActionDef::Literal(text));
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

            let tap = match &r.tap_action {
                ActionSpec::Native => TapMode::Native,
                ActionSpec::Swallow => TapMode::Swallow,
                ActionSpec::Action(s) => match resolve(s, &format!("tap for {}", r.key)) {
                    Some(a) => TapMode::Action(a),
                    None => TapMode::Swallow,
                },
            };

            let layer_id = if r.layer_id.is_empty() {
                None
            } else {
                Some(r.layer_id.clone())
            };
            let hold = match &r.hold_action {
                ActionSpec::Native if layer_id.is_some() => HoldMode::Swallow,
                ActionSpec::Native => HoldMode::Native,
                ActionSpec::Swallow => HoldMode::Swallow,
                ActionSpec::Action(s) => match parse_action(s) {
                    Some(ks) => HoldMode::Keystroke(ks),
                    None => {
                        eprintln!(
                            "[mapper] rule {:?}: unknown hold keystroke {:?} — falling back to native hold",
                            r.key, s
                        );
                        HoldMode::Native
                    }
                },
            };

            let double_tap = match &r.double_tap_action.as_str() {
                &"" => None,
                s => resolve(s, &format!("double-tap for {}", r.key)),
            };

            // Fully transparent rule — skip registration so the key passes
            // straight through the kernel → uinput grab.
            if layer_id.is_none()
                && matches!(tap, TapMode::Native)
                && matches!(hold, HoldMode::Native)
                && double_tap.is_none()
            {
                eprintln!(
                    "[mapper] rule {:?}: tap=native, hold=native, no double-tap — skipped (passthrough)",
                    r.key
                );
                continue;
            }

            let hold_timeout = r
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
                    tap,
                    layer_id,
                    hold,
                    double_tap,
                    hold_timeout,
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
            default_mod_delay,
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
                Phase::WaitingDecision { deadline } | Phase::WaitingSecond { deadline } => {
                    Some(deadline.saturating_duration_since(now))
                }
                Phase::HoldActive => None,
            })
            .min()
    }

    /// Tick pending state machines. Commits holds whose decision window
    /// elapsed and fires taps whose double-tap window elapsed.
    pub fn tick(&mut self, now: Instant, out: &mut Vec<Out>) {
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
            eprintln!("[mapper] hold-timeout -> commit hold (key={:?})", k);
            self.commit_hold(k, out);
        }
        for k in expired_second {
            self.pending.remove(&k);
            eprintln!("[mapper] dtap-window expired -> tap (key={:?})", k);
            let tap = self.rules.get(&k).map(|r| r.tap.clone());
            if let Some(tap) = tap {
                self.fire_tap(k, &tap, out);
            }
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
            self.pending.remove(&key);
            eprintln!("[mapper] double-tap fired (key={:?})", key);
            let dtap = self.rules.get(&key).and_then(|r| r.double_tap.clone());
            fire_action(dtap.as_ref(), self.default_mod_delay, out);
            // The matching release must not emit anything (fire-and-forget).
            self.macro_consumed.insert(key);
            return;
        }

        // Interrupt-on-other-key-press: any pending rule still in
        // WaitingDecision must resolve as hold before we let this new
        // event through. This is what makes Shift+A, MetaLeft(hold=Ctrl)+C
        // and similar mod-tap patterns feel natural.
        self.commit_waiting_decisions(out);

        // Any new key-down must also commit any currently deferred taps so
        // emitted events stay in natural press order.
        self.flush_waiting_second(out);

        if self.active_layers.is_empty() {
            if let Some(rule) = self.rules.get(&key).cloned() {
                // Fast path: no tap action, no double-tap → commit hold
                // immediately without the decision wait.
                if matches!(rule.tap, TapMode::Swallow) && rule.double_tap.is_none() {
                    self.pending.insert(
                        key,
                        Pending {
                            phase: Phase::HoldActive,
                        },
                    );
                    self.commit_hold_with(&rule, key, out);
                    return;
                }
                // Otherwise wait to decide between tap and hold.
                self.pending.insert(
                    key,
                    Pending {
                        phase: Phase::WaitingDecision {
                            deadline: now + rule.hold_timeout,
                        },
                    },
                );
                return;
            }
        }

        // When at least one layer is active, rule keys must behave like
        // normal keys inside that layer. Only the active-layer keymaps
        // (and the base keymap fallback) participate in resolution.
        {
            let mapped = self.lookup_mapping(key);
            match mapped {
                Some(ActionDef::Stroke(ks)) => {
                    eprintln!(
                        "[mapper]   press {:?} -> remap mods={:?} key={:?}",
                        key, ks.mods, ks.key
                    );
                    self.emit_stroke_press(key, ks, out);
                }
                Some(ActionDef::Literal(text)) => {
                    eprintln!("[mapper]   press {:?} -> literal {:?}", key, text);
                    out.push(Out::Literal(text));
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
                    self.emitted.insert(key, Keystroke { mods: vec![], key });
                }
            }
        }
    }

    fn on_release(&mut self, key: Key, now: Instant, out: &mut Vec<Out>) {
        if let Some(pending) = self.pending.get(&key) {
            match pending.phase {
                Phase::HoldActive => {
                    self.pending.remove(&key);
                    self.release_hold(key, out);
                }
                Phase::WaitingDecision { .. } => {
                    // Short press. If double-tap is configured, delay the
                    // tap by double_tap_window to see if a second press
                    // follows; otherwise fire tap immediately.
                    let rule_opt = self.rules.get(&key).cloned();
                    let has_dtap = rule_opt
                        .as_ref()
                        .and_then(|r| r.double_tap.as_ref())
                        .is_some();
                    let rule_window = rule_opt
                        .as_ref()
                        .map(|r| r.double_tap_window)
                        .unwrap_or(self.default_double_tap);
                    if has_dtap {
                        if let Some(p) = self.pending.get_mut(&key) {
                            p.phase = Phase::WaitingSecond {
                                deadline: now + rule_window,
                            };
                        }
                    } else {
                        self.pending.remove(&key);
                        if let Some(rule) = rule_opt {
                            self.fire_tap(key, &rule.tap, out);
                        }
                    }
                }
                Phase::WaitingSecond { .. } => {
                    // Shouldn't happen (we already released once); stay safe.
                }
            }
            return;
        }

        if self.macro_consumed.remove(&key) {
            // Macro / literal / double-tap fired on press — nothing to release.
            return;
        }

        if self.emitted.contains_key(&key) {
            self.release_emitted(key, out);
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
            for m in &ks.mods {
                *self.mod_refs.entry(*m).or_insert(0) += 1;
            }
        }
        out.push(Out::ChordPress {
            ks: ks.clone(),
            mod_delay: self.default_mod_delay,
        });
        self.emitted.insert(physical, ks);
    }

    fn release_emitted(&mut self, physical: Key, out: &mut Vec<Out>) {
        let Some(ks) = self.emitted.remove(&physical) else {
            return;
        };
        let mods = self.take_releaseable_mods(&ks.mods);
        if ks.mods.is_empty() {
            out.push(Out::KeyRaw {
                key: ks.key,
                down: false,
            });
            return;
        }
        out.push(Out::ChordRelease {
            key: ks.key,
            mods,
            mod_delay: self.default_mod_delay,
        });
    }

    fn take_releaseable_mods(&mut self, mods: &[Key]) -> Vec<Key> {
        if mods.is_empty() {
            return Vec::new();
        }
        for m in mods {
            if let Some(r) = self.mod_refs.get_mut(m) {
                *r = r.saturating_sub(1);
            }
        }
        mods.iter()
            .copied()
            .filter(|m| self.mod_refs.get(m).copied().unwrap_or(0) == 0)
            .collect()
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

    fn push_layer(&mut self, id: String) {
        self.active_layers.push(id);
    }

    fn pop_layer(&mut self, id: &str) {
        // Remove the topmost occurrence.
        if let Some(pos) = self.active_layers.iter().rposition(|l| l == id) {
            self.active_layers.remove(pos);
        }
    }

    /// Promote every rule currently in WaitingDecision to HoldActive
    /// (hold gets committed). Called from `on_press` for any new key to
    /// implement "interrupt on other key press".
    fn commit_waiting_decisions(&mut self, out: &mut Vec<Out>) {
        let keys: Vec<Key> = self
            .pending
            .iter()
            .filter(|(_, p)| matches!(p.phase, Phase::WaitingDecision { .. }))
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            eprintln!("[mapper] interrupt -> commit hold (key={:?})", k);
            self.commit_hold(k, out);
        }
    }

    /// Commit a pending rule to HoldActive and emit the corresponding
    /// press side-effect (layer push, native key-down, keystroke-down).
    fn commit_hold(&mut self, key: Key, out: &mut Vec<Out>) {
        let Some(rule) = self.rules.get(&key).cloned() else {
            self.pending.remove(&key);
            return;
        };
        if let Some(p) = self.pending.get_mut(&key) {
            p.phase = Phase::HoldActive;
        } else {
            // Shouldn't happen — commit is always from a pending entry.
            return;
        }
        self.commit_hold_with(&rule, key, out);
    }

    fn commit_hold_with(&mut self, rule: &RuleEntry, key: Key, out: &mut Vec<Out>) {
        if let Some(id) = &rule.layer_id {
            eprintln!("[mapper] layer+ {id} (key={:?})", key);
            self.push_layer(id.clone());
        }
        match &rule.hold {
            HoldMode::Native => {
                out.push(Out::KeyRaw { key, down: true });
                self.emitted.insert(key, Keystroke { mods: vec![], key });
            }
            HoldMode::Swallow => {
                // Nothing — physical key is eaten.
            }
            HoldMode::Keystroke(ks) => {
                self.emit_stroke_press(key, ks.clone(), out);
            }
        }
    }

    /// Undo whatever `commit_hold_with` did for this rule key.
    fn release_hold(&mut self, key: Key, out: &mut Vec<Out>) {
        let Some(rule) = self.rules.get(&key).cloned() else {
            return;
        };
        match &rule.hold {
            HoldMode::Native | HoldMode::Keystroke(_) => {
                self.release_emitted(key, out);
            }
            HoldMode::Swallow => {}
        }
        if let Some(id) = &rule.layer_id {
            eprintln!("[mapper] layer- {id} (key={:?})", key);
            self.pop_layer(id);
        }
    }

    fn fire_tap(&mut self, key: Key, tap: &TapMode, out: &mut Vec<Out>) {
        match tap {
            TapMode::Native => {
                // Short native press+release of the physical key.
                out.push(Out::Stroke {
                    ks: Keystroke { mods: vec![], key },
                    mod_delay: self.default_mod_delay,
                });
            }
            TapMode::Swallow => {}
            TapMode::Action(a) => {
                fire_action(Some(a), self.default_mod_delay, out);
            }
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
            self.pending.remove(&k);
            eprintln!("[mapper] dtap-window flushed -> tap (key={:?})", k);
            let tap = self.rules.get(&k).map(|r| r.tap.clone());
            if let Some(tap) = tap {
                self.fire_tap(k, &tap, out);
            }
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
fn fire_action(action: Option<&ActionDef>, mod_delay: Duration, out: &mut Vec<Out>) {
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
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::config::{AppConfig, LayerKeymap, Rule, Settings};
    use evdev::Key;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    fn empty_cfg() -> AppConfig {
        AppConfig {
            rules: Vec::new(),
            layer_keymaps: HashMap::new(),
            macros: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn sel_layer_q_emits_ctrl_z() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            id: "r_tab".into(),
            key: "Tab".into(),
            layer_id: "sel".into(),
            tap_action: ActionSpec::Native,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let mut sel = LayerKeymap {
            keys: HashMap::new(),
        };
        sel.keys.insert("KeyQ".into(), "Ctrl+Z".into());
        cfg.layer_keymaps.insert("sel".into(), sel);
        cfg.layer_keymaps.insert(
            "base".into(),
            LayerKeymap {
                keys: HashMap::new(),
            },
        );

        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_TAB, true, now, &mut out);
        engine.handle(Key::KEY_Q, true, now + Duration::from_millis(10), &mut out);

        assert!(matches!(
            out.as_slice(),
            [Out::ChordPress { ks, .. }]
                if ks.mods == vec![Key::KEY_LEFTCTRL] && ks.key == Key::KEY_Z
        ));
    }

    #[test]
    fn active_layer_bypasses_rule_for_same_key() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            id: "r_space".into(),
            key: "Space".into(),
            layer_id: "space".into(),
            tap_action: ActionSpec::Native,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        cfg.rules.push(Rule {
            id: "r_tab".into(),
            key: "Tab".into(),
            layer_id: "sel".into(),
            tap_action: ActionSpec::Native,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let mut space = LayerKeymap {
            keys: HashMap::new(),
        };
        space.keys.insert("Tab".into(), "Escape".into());
        cfg.layer_keymaps.insert("space".into(), space);
        cfg.layer_keymaps.insert(
            "base".into(),
            LayerKeymap {
                keys: HashMap::new(),
            },
        );

        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_SPACE, true, now, &mut out);
        engine.handle(
            Key::KEY_TAB,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [Out::ChordPress { ks, .. }]
                if ks.mods.is_empty() && ks.key == Key::KEY_ESC
        ));
    }

    #[test]
    fn hold_can_activate_layer_and_modifier_together() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            id: "r_alt".into(),
            key: "AltLeft".into(),
            layer_id: "win".into(),
            tap_action: ActionSpec::Action("Enter".into()),
            hold_action: ActionSpec::Action("AltLeft".into()),
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let mut win = LayerKeymap {
            keys: HashMap::new(),
        };
        win.keys.insert("Tab".into(), "Tab".into());
        cfg.layer_keymaps.insert("win".into(), win);
        cfg.layer_keymaps.insert(
            "base".into(),
            LayerKeymap {
                keys: HashMap::new(),
            },
        );

        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_LEFTALT, true, now, &mut out);
        engine.handle(
            Key::KEY_TAB,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [
                Out::ChordPress { ks: alt, .. },
                Out::ChordPress { ks: tab, .. },
            ] if alt.mods.is_empty()
                && alt.key == Key::KEY_LEFTALT
                && tab.mods.is_empty()
                && tab.key == Key::KEY_TAB
        ));
    }

    #[test]
    fn layer_only_rule_is_not_treated_as_passthrough() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            id: "r_alt".into(),
            key: "AltLeft".into(),
            layer_id: "win".into(),
            tap_action: ActionSpec::Action("Enter".into()),
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let mut win = LayerKeymap {
            keys: HashMap::new(),
        };
        win.keys.insert("Tab".into(), "Escape".into());
        cfg.layer_keymaps.insert("win".into(), win);
        cfg.layer_keymaps.insert(
            "base".into(),
            LayerKeymap {
                keys: HashMap::new(),
            },
        );

        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_LEFTALT, true, now, &mut out);
        engine.handle(
            Key::KEY_TAB,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [Out::ChordPress { ks, .. }]
                if ks.mods.is_empty() && ks.key == Key::KEY_ESC
        ));
    }
}
