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

mod builder;
mod model;

pub use self::model::Out;
use self::model::{fire_action, ActionDef, HoldMode, Pending, Phase, RuleEntry, TapMode};
use super::action::Keystroke;
use evdev::Key;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

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

    /// Per-layer list of physical keys that trigger the temporary release of holds.
    layer_isolate_keys: HashMap<String, Vec<Key>>,
    /// layer_id -> physical keys that activated it, oldest to newest
    layer_triggers: HashMap<String, Vec<Key>>,
    /// Tracks isolated holds per physical key (to restore on key release)
    isolated_holds: HashMap<Key, Vec<(Key, u32)>>,
}
impl Engine {
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
        if is_mouse_button(key) && !self.should_handle_mouse_button(key, down) {
            out.push(Out::KeyRaw { key, down });
            return;
        }

        if down {
            let is_active = if let Some(rule) = self.rules.get(&key) {
                rule_passes_apps(rule) && rule_passes_legacy(rule)
            } else {
                true
            };

            if !is_active {
                out.push(Out::KeyRaw { key, down });
                return;
            }
        } else {
            if !self.pending.contains_key(&key)
                && !self.emitted.contains_key(&key)
                && !self.macro_consumed.contains(&key)
            {
                out.push(Out::KeyRaw { key, down });
                return;
            }
        }

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
        // event through. This is what makes Shift+KeyA, MetaLeft(hold=ControlLeft)+KeyC
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
        // participate in resolution.
        {
            let mapped = self.lookup_mapping(key);
            match mapped {
                Some((layer_id, def)) => {
                    if let Some(isolate_keys) = self.layer_isolate_keys.get(&layer_id) {
                        if isolate_keys.contains(&key) {
                            if let Some(trigger_phys) = self
                                .layer_triggers
                                .get(&layer_id)
                                .and_then(|v| v.last())
                                .copied()
                            {
                                if let Some(ks) = self.emitted.get(&trigger_phys).cloned() {
                                    let mut suppressed = Vec::new();
                                    for target_key in ks.mods.iter().chain(std::iter::once(&ks.key))
                                    {
                                        let old_count =
                                            self.mod_refs.get(target_key).copied().unwrap_or(0);
                                        if let Some(count) = self.mod_refs.get_mut(target_key) {
                                            *count = 0;
                                        }
                                        out.push(Out::KeyRaw {
                                            key: *target_key,
                                            down: false,
                                        });
                                        suppressed.push((*target_key, old_count));
                                        eprintln!(
                                            "[mapper] isolate+ {:?} suppress hold {:?}",
                                            key, target_key
                                        );
                                    }
                                    if !suppressed.is_empty() {
                                        self.isolated_holds.insert(key, suppressed);
                                    }
                                }
                            }
                        }
                    }

                    match def {
                        ActionDef::Stroke(ks) => {
                            eprintln!(
                                "[mapper]   press {:?} -> remap mods={:?} key={:?}",
                                key, ks.mods, ks.key
                            );
                            self.emit_stroke_press(key, ks, out);
                        }
                        ActionDef::Literal(text) => {
                            eprintln!("[mapper]   press {:?} -> literal {:?}", key, text);
                            out.push(Out::Literal(text));
                            self.macro_consumed.insert(key);
                        }
                        ActionDef::Macro(md) => {
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
                        ActionDef::System(action) => {
                            eprintln!("[mapper]   press {:?} -> run system {:?}", key, action);
                            out.push(Out::RunSystem(action));
                            self.macro_consumed.insert(key);
                        }
                        ActionDef::Command(command) => {
                            eprintln!("[mapper]   press {:?} -> run command {:?}", key, command);
                            out.push(Out::RunCommand(command));
                            self.macro_consumed.insert(key);
                        }
                        ActionDef::Swallow => {
                            eprintln!("[mapper]   press {:?} -> swallow", key);
                            self.macro_consumed.insert(key);
                        }
                    }
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
        if let Some(suppressed) = self.isolated_holds.remove(&key) {
            for (target_key, old_count) in suppressed {
                let still_held = self
                    .emitted
                    .values()
                    .any(|ks| ks.key == target_key || ks.mods.contains(&target_key));
                if still_held {
                    eprintln!("[mapper] isolate- {:?} restore hold {:?}", key, target_key);
                    if old_count > 0 {
                        *self.mod_refs.entry(target_key).or_insert(0) = old_count;
                    }
                    out.push(Out::KeyRaw {
                        key: target_key,
                        down: true,
                    });
                }
            }
        }

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

    fn lookup_mapping(&self, key: Key) -> Option<(String, ActionDef)> {
        for l in self.active_layers.iter().rev() {
            if let Some(m) = self.layer_maps.get(l) {
                if let Some(def) = m.get(&key) {
                    return Some((l.clone(), def.clone()));
                }
            }
        }
        None
    }

    fn should_handle_mouse_button(&self, key: Key, down: bool) -> bool {
        if !down
            && (self.pending.contains_key(&key)
                || self.emitted.contains_key(&key)
                || self.macro_consumed.contains(&key))
        {
            return true;
        }

        if self.lookup_mapping(key).is_some() {
            return true;
        }

        !is_primary_mouse_button(key) && self.rules.contains_key(&key)
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
            self.layer_triggers.entry(id.clone()).or_default().push(key);
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
        if let Some(id) = &rule.layer_id {
            eprintln!("[mapper] layer- {id} (key={:?})", key);
            if let Some(triggers) = self.layer_triggers.get_mut(id) {
                if let Some(pos) = triggers.iter().rposition(|trigger| *trigger == key) {
                    triggers.remove(pos);
                }
                if triggers.is_empty() {
                    self.layer_triggers.remove(id);
                }
            }
            self.pop_layer(id);
        }
        match &rule.hold {
            HoldMode::Native | HoldMode::Keystroke(_) => {
                self.release_emitted(key, out);
            }
            HoldMode::Swallow => {}
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
        self.layer_triggers.clear();
        self.isolated_holds.clear();
    }

    #[allow(dead_code)]
    pub fn default_hold(&self) -> Duration {
        self.default_hold
    }
}

/// Returns true when a list of substrings matches the active window's
/// title or app id (case-insensitive, OR).
fn matches_active_window(needles: &[String], aw: &crate::active_window::ActiveWindow) -> bool {
    let title = aw.title.to_lowercase();
    let app_id = aw.app_id.to_lowercase();
    needles
        .iter()
        .map(|n| n.trim().to_lowercase())
        .filter(|n| !n.is_empty())
        .any(|n| title.contains(&n) || app_id.contains(&n))
}

/// Evaluates the apps whitelist / blacklist for a rule.
/// Blacklist takes precedence: a match blocks the rule. Whitelist, when
/// non-empty, must match for the rule to fire.
fn rule_passes_apps(rule: &RuleEntry) -> bool {
    let bl = rule
        .condition_apps_blacklist
        .as_deref()
        .filter(|l| !l.is_empty());
    let wl = rule
        .condition_apps_whitelist
        .as_deref()
        .filter(|l| !l.is_empty());
    if bl.is_none() && wl.is_none() {
        return true;
    }
    let aw = crate::active_window::cached_active_window();
    if let Some(bl) = bl {
        if let Some(aw) = &aw {
            if matches_active_window(bl, aw) {
                return false;
            }
        }
    }
    if let Some(wl) = wl {
        match &aw {
            Some(aw) if matches_active_window(wl, aw) => {}
            _ => return false,
        }
    }
    true
}

fn rule_passes_legacy(rule: &RuleEntry) -> bool {
    let has_gm_cond = rule
        .condition_game_mode
        .as_deref()
        .is_some_and(|m| m != "ignore" && !m.is_empty());
    let has_layout_cond = rule
        .condition_layouts
        .as_ref()
        .is_some_and(|l| !l.is_empty());

    if !has_gm_cond && !has_layout_cond {
        return true;
    }
    if has_gm_cond {
        let gm_active = crate::gamemode::cached_status_active();
        if !match rule.condition_game_mode.as_deref() {
            Some("on") => gm_active,
            Some("off") => !gm_active,
            _ => false,
        } {
            return false;
        }
    }
    if has_layout_cond {
        let Some(current) = crate::layout::cached_layout_short() else {
            return false;
        };
        if !rule
            .condition_layouts
            .as_ref()
            .is_some_and(|layouts| layouts.contains(&current))
        {
            return false;
        }
    }
    true
}

fn is_primary_mouse_button(key: Key) -> bool {
    matches!(key, Key::BTN_LEFT | Key::BTN_RIGHT | Key::BTN_MIDDLE)
}

fn is_mouse_button(key: Key) -> bool {
    (272..=281).contains(&key.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::config::{ActionSpec, AppConfig, ExtraKey, LayerKeymap, Rule, Settings};
    use evdev::Key;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    fn empty_cfg() -> AppConfig {
        AppConfig {
            rules: Vec::new(),
            layer_keymaps: HashMap::new(),
            macros: Vec::new(),
            commands: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn sel_layer_q_emits_ctrl_key_z() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
            ..Default::default()
        };
        sel.keys.insert("KeyQ".into(), Some("Ctrl+KeyZ".into()));
        cfg.layer_keymaps.insert("sel".into(), sel);
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
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
            ..Default::default()
        };
        space.keys.insert("Tab".into(), Some("Escape".into()));
        cfg.layer_keymaps.insert("space".into(), space);
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
    fn unmapped_key_in_active_layer_passthroughs() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            id: "r_space".into(),
            key: "Space".into(),
            layer_id: "space".into(),
            tap_action: ActionSpec::Native,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        cfg.layer_keymaps.insert(
            "space".into(),
            LayerKeymap {
                keys: HashMap::new(),
                ..Default::default()
            },
        );
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_SPACE, true, now, &mut out);
        engine.handle(Key::KEY_A, true, now + Duration::from_millis(10), &mut out);
        engine.handle(Key::KEY_A, false, now + Duration::from_millis(11), &mut out);

        assert!(matches!(
            out.as_slice(),
            [
                Out::KeyRaw {
                    key: Key::KEY_A,
                    down: true
                },
                Out::KeyRaw {
                    key: Key::KEY_A,
                    down: false
                }
            ]
        ));
    }

    #[test]
    fn explicit_null_in_layer_keymap_swallows_key() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            id: "r_space".into(),
            key: "Space".into(),
            layer_id: "space".into(),
            tap_action: ActionSpec::Native,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut space = LayerKeymap {
            keys: HashMap::new(),
            ..Default::default()
        };
        space.keys.insert("KeyA".into(), None);
        cfg.layer_keymaps.insert("space".into(), space);
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_SPACE, true, now, &mut out);
        engine.handle(Key::KEY_A, true, now + Duration::from_millis(10), &mut out);
        engine.handle(Key::KEY_A, false, now + Duration::from_millis(11), &mut out);

        assert!(out.is_empty());
    }

    #[test]
    fn hold_can_activate_layer_and_modifier_together() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
            ..Default::default()
        };
        win.keys.insert("Tab".into(), Some("Tab".into()));
        cfg.layer_keymaps.insert("win".into(), win);
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
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
            ..Default::default()
        };
        win.keys.insert("Tab".into(), Some("Escape".into()));
        cfg.layer_keymaps.insert("win".into(), win);
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

    // Serialise all tests that mutate the shared `active_window::CACHED`
    // so they don't race when cargo runs the suite in parallel.
    static APPS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn rule_with_apps(whitelist: Option<Vec<String>>, blacklist: Option<Vec<String>>) -> RuleEntry {
        RuleEntry {
            tap: TapMode::Native,
            layer_id: None,
            hold: HoldMode::Swallow,
            double_tap: None,
            hold_timeout: Duration::from_millis(200),
            double_tap_window: Duration::from_millis(200),
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: whitelist,
            condition_apps_blacklist: blacklist,
        }
    }

    #[test]
    fn apps_whitelist_blocks_when_no_active_window() {
        let _g = APPS_TEST_LOCK.lock().unwrap();
        crate::active_window::set_cached_for_test(None);
        let rule = rule_with_apps(Some(vec!["firefox".into()]), None);
        assert!(!rule_passes_apps(&rule));
    }

    #[test]
    fn apps_whitelist_passes_when_title_matches() {
        let _g = APPS_TEST_LOCK.lock().unwrap();
        crate::active_window::set_cached_for_test(Some(crate::active_window::ActiveWindow {
            title: "Mozilla Firefox".into(),
            app_id: "navigator".into(),
        }));
        let rule = rule_with_apps(Some(vec!["firefox".into()]), None);
        assert!(rule_passes_apps(&rule));
        crate::active_window::set_cached_for_test(None);
    }

    #[test]
    fn apps_whitelist_passes_when_app_id_matches_case_insensitive() {
        let _g = APPS_TEST_LOCK.lock().unwrap();
        crate::active_window::set_cached_for_test(Some(crate::active_window::ActiveWindow {
            title: "Library".into(),
            app_id: "Steam".into(),
        }));
        let rule = rule_with_apps(Some(vec!["steam".into()]), None);
        assert!(rule_passes_apps(&rule));
        crate::active_window::set_cached_for_test(None);
    }

    #[test]
    fn apps_blacklist_blocks_even_when_whitelist_passes() {
        let _g = APPS_TEST_LOCK.lock().unwrap();
        crate::active_window::set_cached_for_test(Some(crate::active_window::ActiveWindow {
            title: "Editor — secret".into(),
            app_id: "editor".into(),
        }));
        let rule = rule_with_apps(Some(vec!["editor".into()]), Some(vec!["secret".into()]));
        assert!(!rule_passes_apps(&rule));
        crate::active_window::set_cached_for_test(None);
    }

    #[test]
    fn apps_no_lists_always_passes() {
        let _g = APPS_TEST_LOCK.lock().unwrap();
        crate::active_window::set_cached_for_test(None);
        let rule = rule_with_apps(None, None);
        assert!(rule_passes_apps(&rule));
    }

    #[test]
    fn mouse_button_does_not_interrupt_pending_hold() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            id: "r_shift".into(),
            key: "ShiftLeft".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("Escape".into()),
            hold_action: ActionSpec::Action("ControlLeft".into()),
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_LEFTSHIFT, true, now, &mut out);
        // No hold committed yet — Shift is still in WaitingDecision.
        assert!(out.is_empty());

        // Mouse button should pass through without committing the pending hold.
        engine.handle(
            Key::BTN_LEFT,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );
        engine.handle(
            Key::BTN_LEFT,
            false,
            now + Duration::from_millis(11),
            &mut out,
        );

        // Releasing Shift after mouse click should fire the tap (Escape),
        // proving the mouse click did NOT promote the pending rule to hold.
        engine.handle(
            Key::KEY_LEFTSHIFT,
            false,
            now + Duration::from_millis(20),
            &mut out,
        );
        assert!(matches!(
            out.as_slice(),
            [
                Out::KeyRaw { key: k1, down: true },
                Out::KeyRaw { key: k2, down: false },
                Out::Stroke { ks, .. },
            ] if *k1 == Key::BTN_LEFT && *k2 == Key::BTN_LEFT && ks.key == Key::KEY_ESC
        ));
    }

    #[test]
    fn unmapped_extra_mouse_button_does_not_interrupt_pending_hold() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            id: "r_shift".into(),
            key: "ShiftLeft".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("Escape".into()),
            hold_action: ActionSpec::Action("ControlLeft".into()),
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_LEFTSHIFT, true, now, &mut out);
        engine.handle(
            Key::BTN_SIDE,
            true,
            now + Duration::from_millis(10),
            &mut out,
        );
        engine.handle(
            Key::BTN_SIDE,
            false,
            now + Duration::from_millis(11),
            &mut out,
        );
        engine.handle(
            Key::KEY_LEFTSHIFT,
            false,
            now + Duration::from_millis(20),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [
                Out::KeyRaw { key: k1, down: true },
                Out::KeyRaw { key: k2, down: false },
                Out::Stroke { ks, .. },
            ] if *k1 == Key::BTN_SIDE && *k2 == Key::BTN_SIDE && ks.key == Key::KEY_ESC
        ));
    }

    #[test]
    fn extra_mouse_button_can_be_rule_trigger() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            id: "r_mouse_side".into(),
            key: "MouseSide".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("BrowserBack".into()),
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::BTN_SIDE, true, now, &mut out);
        engine.handle(
            Key::BTN_SIDE,
            false,
            now + Duration::from_millis(10),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [Out::Stroke { ks, .. }] if ks.mods.is_empty() && ks.key == Key::KEY_BACK
        ));
    }

    #[test]
    fn primary_mouse_button_can_be_layer_mapping() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            id: "r_space".into(),
            key: "Space".into(),
            layer_id: "mouse".into(),
            tap_action: ActionSpec::Native,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut mouse = LayerKeymap {
            keys: HashMap::new(),
            ..Default::default()
        };
        mouse.extras.push(ExtraKey {
            id: "left".into(),
            key: "MouseLeft".into(),
            action: "Escape".into(),
        });
        cfg.layer_keymaps.insert("mouse".into(), mouse);
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_SPACE, true, now, &mut out);
        engine.tick(now + Duration::from_millis(260), &mut out);
        engine.handle(
            Key::BTN_LEFT,
            true,
            now + Duration::from_millis(270),
            &mut out,
        );
        engine.handle(
            Key::BTN_LEFT,
            false,
            now + Duration::from_millis(271),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [
                Out::ChordPress { ks: press, .. },
                Out::KeyRaw {
                    key: Key::KEY_ESC,
                    down: false,
                }
            ] if press.mods.is_empty() && press.key == Key::KEY_ESC
        ));
    }

    #[test]
    fn isolate_suppresses_and_restores_layer_trigger_modifier() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
            isolate: vec!["KeyW".into()],
            ..Default::default()
        };
        win.keys.insert("KeyW".into(), Some("Ctrl+KeyA".into()));
        cfg.layer_keymaps.insert("win".into(), win);
        let mut engine = Engine::new(&cfg);
        let mut out = Vec::new();
        let now = Instant::now();

        engine.handle(Key::KEY_LEFTALT, true, now, &mut out);
        engine.handle(Key::KEY_W, true, now + Duration::from_millis(10), &mut out);
        engine.handle(Key::KEY_W, false, now + Duration::from_millis(11), &mut out);
        engine.handle(
            Key::KEY_LEFTALT,
            false,
            now + Duration::from_millis(20),
            &mut out,
        );

        assert!(matches!(
            out.as_slice(),
            [
                Out::ChordPress { ks: alt_hold, .. },
                Out::KeyRaw { key: release_alt_1, down: false },
                Out::ChordPress { ks: ctrl_a, .. },
                Out::KeyRaw { key: restore_alt, down: true },
                Out::ChordRelease { key: key_a, mods: ctrl_release, .. },
                Out::KeyRaw { key: release_alt_2, down: false },
            ] if alt_hold.mods.is_empty()
                && alt_hold.key == Key::KEY_LEFTALT
                && *release_alt_1 == Key::KEY_LEFTALT
                && ctrl_a.mods.as_slice() == [Key::KEY_LEFTCTRL]
                && ctrl_a.key == Key::KEY_A
                && *restore_alt == Key::KEY_LEFTALT
                && *key_a == Key::KEY_A
                && ctrl_release.as_slice() == [Key::KEY_LEFTCTRL]
                && *release_alt_2 == Key::KEY_LEFTALT
        ));
    }
}
