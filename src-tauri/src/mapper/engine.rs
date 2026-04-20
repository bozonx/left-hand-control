// Tap/hold + layer state machine.
//
// Design (MVP, simple timeout-based):
//   * Every rule binds a physical key to (layer?, tap_action?, hold_ms?).
//   * On press of a "rule key":
//       - if rule has layer      → pending (wait for release or timeout)
//       - if rule has only tap   → pending (wait for release; tap on release)
//       - if rule has only layer → activate immediately (fastest UX)
//   * On timeout while still held → activate layer (if any), mark decided=hold.
//   * On release:
//       - before timeout & not yet decided → emit tap action (if any)
//       - after timeout → deactivate layer (if it was activated)
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
use super::symbols::{resolve_literal, SymbolResolver};
use super::system::{self, SysCommand};
use evdev::Key;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Resolved action: single chord, macro, system function, or a
/// layout-dependent character literal resolved at press time.
#[derive(Clone)]
enum ActionDef {
    Stroke(Keystroke),
    Macro(MacroDef),
    System(SysCommand),
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
    hold: Duration,
}

struct Pending {
    deadline: Instant,
    decided_hold: bool,
    layer: Option<String>,
    tap: Option<ActionDef>,
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
    /// Spawn a system command (fire-and-forget).
    RunSystem(SysCommand),
}

impl Engine {
    pub fn new(cfg: &AppConfig) -> Self {
        let default_hold = Duration::from_millis(cfg.settings.default_hold_timeout_ms.max(1));
        let default_step_pause =
            Duration::from_millis(cfg.settings.default_macro_step_pause_ms);
        let default_mod_delay =
            Duration::from_millis(cfg.settings.default_macro_modifier_delay_ms);

        // Build the macro table first so tap / keymap actions can reference
        // macros by id.
        let mut macros: HashMap<String, MacroDef> = HashMap::new();
        for m in &cfg.macros {
            if m.id.is_empty() {
                eprintln!("[mapper] skipping macro with empty id: {:?}", m.name);
                continue;
            }
            let mut steps: Vec<MacroStepItem> = Vec::with_capacity(m.steps.len());
            for (idx, s) in m.steps.iter().enumerate() {
                let raw = s.keystroke.trim();
                if raw.is_empty() {
                    continue;
                }
                if let Some(rest) = raw.strip_prefix("sys:") {
                    match system::resolve(rest.trim()) {
                        Some(cmd) => steps.push(MacroStepItem::System(cmd)),
                        None => eprintln!(
                            "[mapper] macro {} step #{}: system fn {:?} not available",
                            m.id,
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
                        m.id,
                        idx + 1,
                        raw
                    ),
                }
            }
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
            let layer = if r.layer_id.is_empty() {
                None
            } else {
                Some(r.layer_id.clone())
            };
            let hold = r
                .hold_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_hold);
            rules.insert(key, RuleEntry { layer, tap, hold });
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
            active_layers: Vec::new(),
            pending: HashMap::new(),
            emitted: HashMap::new(),
            macro_consumed: HashSet::new(),
            mod_refs: HashMap::new(),
        }
    }

    /// Time until the nearest pending-hold deadline, or None.
    pub fn next_deadline(&self, now: Instant) -> Option<Duration> {
        self.pending
            .values()
            .filter(|p| !p.decided_hold)
            .map(|p| p.deadline.saturating_duration_since(now))
            .min()
    }

    /// Tick pending state machines. Emits layer activations for expired holds.
    pub fn tick(&mut self, now: Instant, out: &mut Vec<Out>) {
        let expired: Vec<Key> = self
            .pending
            .iter()
            .filter(|(_, p)| !p.decided_hold && p.deadline <= now)
            .map(|(k, _)| *k)
            .collect();
        for k in expired {
            let p = self.pending.get_mut(&k).unwrap();
            p.decided_hold = true;
            if let Some(layer) = p.layer.clone() {
                eprintln!("[mapper] hold-timeout -> layer+: {layer} (key={:?})", k);
                self.push_layer(layer, out);
            } else {
                eprintln!("[mapper] hold-timeout (no layer) key={:?}", k);
            }
        }
    }

    /// Handle a raw key event from the grabbed device.
    pub fn handle(
        &mut self,
        key: Key,
        down: bool,
        now: Instant,
        resolver: Option<&mut SymbolResolver>,
        out: &mut Vec<Out>,
    ) {
        eprintln!(
            "[mapper] in {} key={:?} active={:?}",
            if down { "DOWN" } else { " UP " },
            key,
            self.active_layers
        );
        if down {
            self.on_press(key, now, resolver, out);
        } else {
            self.on_release(key, now, resolver, out);
        }
    }

    fn on_press(
        &mut self,
        key: Key,
        now: Instant,
        mut resolver: Option<&mut SymbolResolver>,
        out: &mut Vec<Out>,
    ) {
        if let Some(rule) = self.rules.get(&key) {
            let layer = rule.layer.clone();
            let tap = rule.tap.clone();
            let hold = rule.hold;
            // Fast path: layer-only rule → activate immediately, no tap wait.
            if layer.is_some() && tap.is_none() {
                if let Some(l) = layer.clone() {
                    self.push_layer(l, out);
                }
                self.pending.insert(
                    key,
                    Pending {
                        deadline: now,
                        decided_hold: true,
                        layer,
                        tap: None,
                    },
                );
                return;
            }
            // Tap-hold or tap-only: wait for release or timeout.
            self.pending.insert(
                key,
                Pending {
                    deadline: now + hold,
                    decided_hold: false,
                    layer,
                    tap,
                },
            );
            return;
        }

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
                match resolve_literal(resolver.as_deref_mut(), ch) {
                    Some(ks) => {
                        eprintln!(
                            "[mapper]   press {:?} -> literal {:?} mods={:?} key={:?}",
                            key, ch, ks.mods, ks.key
                        );
                        self.emit_stroke_press(key, ks, out);
                    }
                    None => {
                        eprintln!(
                            "[mapper]   press {:?} -> literal {:?} unresolved, ignored",
                            key, ch
                        );
                        // Nothing emitted; remember so release is a no-op.
                        self.macro_consumed.insert(key);
                    }
                }
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
            Some(ActionDef::System(cmd)) => {
                eprintln!(
                    "[mapper]   press {:?} -> run system {} {:?}",
                    key, cmd.program, cmd.args
                );
                out.push(Out::RunSystem(cmd));
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

    fn on_release(
        &mut self,
        key: Key,
        _now: Instant,
        mut resolver: Option<&mut SymbolResolver>,
        out: &mut Vec<Out>,
    ) {
        if let Some(p) = self.pending.remove(&key) {
            if p.decided_hold {
                // Hold was active — deactivate layer if we activated one.
                if let Some(l) = p.layer {
                    self.pop_layer(&l, out);
                }
            } else {
                // Release before timeout → emit tap action.
                match p.tap {
                    Some(ActionDef::Stroke(ks)) => out.push(Out::Stroke(ks)),
                    Some(ActionDef::Literal(ch)) => {
                        if let Some(ks) = resolve_literal(resolver.as_deref_mut(), ch) {
                            out.push(Out::Stroke(ks));
                        } else {
                            eprintln!(
                                "[mapper]   tap literal {:?} unresolved, ignored",
                                ch
                            );
                        }
                    }
                    Some(ActionDef::Macro(md)) => out.push(Out::RunMacro {
                        steps: md.steps,
                        step_pause: md.step_pause,
                        mod_delay: md.mod_delay,
                    }),
                    Some(ActionDef::System(cmd)) => out.push(Out::RunSystem(cmd)),
                    None => {}
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
