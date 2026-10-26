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
//       - another key is pressed while we're still in this phase — behaviour
//         depends on the rule’s `tap_decision`:
//           * PermissiveHold (default, QMK-style): the interrupting key is
//             buffered (see `permissive_intercept`). The hold commits only
//             if that key is *released* before the rule key (nested
//             press+release) or the hold timeout elapses; the buffered
//             events then replay in order. If the rule key is released
//             first, it is a tap and the buffer replays after it. This lets
//             fast rolling overlaps during typing resolve as taps instead of
//             swallowing them into a hold.
//           * HoldOnOtherKeyPress: any other key press commits the
//             hold immediately, then the new key flows through.
//   * HoldActive — hold has been committed (layer pushed, native key or
//     keystroke held). On release: undo the hold.
//   * WaitingSecond — short press finished; waiting for a possible
//     second press within `double_tap_window`.
//
// Fast path: a rule with `tap == Swallow` and no `double_tap` skips
// WaitingDecision and commits the hold immediately on key-down.

mod builder;
mod chords;
mod holds;
mod macros;
mod model;
mod state;

pub use self::model::Out;
use self::model::{
    ActionDef, BufferedEvent, HoldMode, LayerTrigger, Pending, Phase, RuleEntry, TapMode,
};
use self::state::{ChordState, CompiledLayout, LayerStack, MacroRunner, VirtualKeys};
use super::action::Keystroke;
use super::config::GameModeCondition;
use crate::mapper::key_code::KeyCode;
use crate::profile::app_match::any_matches_window;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub struct Engine {
    /// Bindings compiled from the configuration; replaced as a whole by
    /// `reconfigure`.
    layout: CompiledLayout,
    physical_down: HashSet<KeyCode>,
    /// When each physical key in `physical_down` was pressed.
    press_at: HashMap<KeyCode, Instant>,
    long_holds: HashMap<KeyCode, (Instant, ActionDef)>,
    /// Events deferred while a rule key is in `WaitingDecision`
    /// (permissive-hold mode only). Replayed in order once the decision
    /// resolves.
    decision_buffer: Vec<BufferedEvent>,
    /// Per-rule pending state (waiting for release/timeout decision).
    pending: HashMap<KeyCode, Pending>,
    chords: ChordState,
    layers: LayerStack,
    virt: VirtualKeys,
    runner: MacroRunner,
}
impl Engine {
    /// Apply new bindings without releasing keys already held by the user.
    /// Pending decisions retain their original rule until physical release.
    pub fn reconfigure(&mut self, cfg: &super::config::AppConfig, out: &mut Vec<Out>) {
        self.flush_combo_buffer(out);
        self.runner.cancel(&mut self.virt, out);
        if !cfg.settings.commands_enabled {
            fn revoke(action: &mut ActionDef) {
                match action {
                    // Reported without an id: the command is off as a whole.
                    ActionDef::Command(_) => {
                        *action = ActionDef::Unavailable(
                            crate::profile::actions::Action::Command(String::new())
                                .format()
                                .unwrap_or_default(),
                        )
                    }
                    ActionDef::Macro(m) => m.steps.retain(|step| {
                        !matches!(step, crate::mapper::action::MacroStepItem::Command(_))
                    }),
                    _ => {}
                }
            }
            for pending in self.pending.values_mut() {
                if let TapMode::Action(action) = &mut pending.rule.tap {
                    revoke(action);
                }
                for action in [&mut pending.rule.double_tap, &mut pending.rule.long_hold]
                    .into_iter()
                    .flatten()
                {
                    revoke(action);
                }
            }
            for (_, action) in self.long_holds.values_mut() {
                revoke(action);
            }
        }
        let next = Self::new(cfg);
        self.layout = next.layout;
        self.runner.timeout = next.runner.timeout;
        self.runner.text_pending = false;
    }

    /// The topmost active layer, if any layer is held.
    pub fn top_layer(&self) -> Option<&str> {
        self.layers.top()
    }

    /// Snapshot for delayed shell notifications; separate presses of the same
    /// trigger remain distinguishable even within one input batch.
    pub fn held_layer_state(&self) -> super::layer_state::HeldLayerState {
        super::layer_state::HeldLayerState {
            layer: self.top_layer().map(str::to_owned),
            hold_id: self.layers.hold_id,
            used: self.top_layer().is_some() && self.layers.used,
        }
    }

    /// Number of key presses handled while a layer was active; changes
    /// whenever a key is pressed inside a held layer.
    pub fn layer_presses(&self) -> u64 {
        self.layers.presses
    }

    /// Time until the nearest pending deadline (hold-decision,
    /// waiting-for-second-press, the next macro step wake-up or the macro
    /// timeout), or None.
    pub fn next_deadline(&self, now: Instant) -> Option<Duration> {
        self.pending
            .iter()
            .filter_map(|(key, p)| match p.phase {
                Phase::WaitingDecision { deadline } | Phase::WaitingSecond { deadline }
                    if !self.timer_blocked(*key, deadline) =>
                {
                    Some(deadline.saturating_duration_since(now))
                }
                _ => None,
            })
            .chain(
                self.long_holds
                    .iter()
                    .filter(|(key, (deadline, _))| !self.timer_blocked(**key, *deadline))
                    .map(|(_, (deadline, _))| deadline.saturating_duration_since(now)),
            )
            .chain(self.runner.next_deadline(now))
            .chain(
                self.combo_deadline()
                    .map(|deadline| deadline.saturating_duration_since(now)),
            )
            .min()
    }

    /// A failed text step must not submit incomplete input with subsequent keys.
    pub fn text_input_failed(&mut self, out: &mut Vec<Out>) {
        self.runner.abort(&mut self.virt, out);
    }

    /// Resume macro timing only after the text worker has completed injection.
    pub fn set_text_pending(&mut self, pending: bool, now: Instant) {
        self.runner.set_text_pending(pending, now);
    }

    /// Tick pending state machines. Commits holds whose decision window
    /// elapsed and fires taps whose double-tap window elapsed.
    pub fn tick(&mut self, now: Instant, out: &mut Vec<Out>) {
        if self
            .combo_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            self.flush_combo_buffer(out);
        }
        let expired: Vec<_> = self
            .long_holds
            .iter()
            .filter(|(key, (deadline, _))| {
                now >= *deadline && !self.timer_blocked(**key, *deadline)
            })
            .map(|(key, _)| *key)
            .collect();
        for key in expired {
            if let Some((_, action)) = self.long_holds.remove(&key) {
                self.commit_hold(key, out);
                self.fire_action(Some(&action), self.layout.default_mod_delay, now, out);
            }
        }
        self.runner.tick(&mut self.virt, now, out);

        let mut expired_hold: Vec<KeyCode> = Vec::new();
        let mut expired_second: Vec<KeyCode> = Vec::new();
        for (k, p) in &self.pending {
            match p.phase {
                Phase::WaitingDecision { deadline }
                    if deadline <= now && !self.timer_blocked(*k, deadline) =>
                {
                    expired_hold.push(*k);
                }
                Phase::WaitingSecond { deadline }
                    if deadline <= now && !self.timer_blocked(*k, deadline) =>
                {
                    expired_second.push(*k);
                }
                _ => {}
            }
        }
        for k in expired_hold {
            self.commit_hold(k, out);
            // A hold that timed out while presses were buffered behind it
            // (permissive-hold mode) must now replay them in the held context.
            self.drain_decision_buffer(out);
        }
        for k in expired_second {
            let pending = self.pending.remove(&k);
            if let Some(pending) = pending {
                self.fire_tap(k, &pending.rule.tap, now, out);
            }
        }
    }

    /// Handle a raw key event from the grabbed device.
    pub fn handle(&mut self, key: KeyCode, down: bool, now: Instant, out: &mut Vec<Out>) {
        if down {
            if !self.physical_down.insert(key) {
                return;
            }
            self.press_at.insert(key, now);
            if !self.layers.active.is_empty() {
                self.layers.used = true;
            }
        } else {
            self.physical_down.remove(&key);
            self.press_at.remove(&key);
        }
        // A press arriving after the combo window but before the next tick
        // must not extend the stale combo.
        if self
            .combo_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            self.flush_combo_buffer(out);
        }
        if self.handle_chord(key, down, now, out) {
            return;
        }
        if self.combo_intercept(key, down, now, out) {
            return;
        }
        self.handle_event(key, down, now, out);
    }

    /// Auto-repeat of a held physical key, for backends whose OS does not
    /// repeat injected keys itself (Windows). Only a key that currently
    /// holds a virtual key down repeats it; undecided taps, layer triggers
    /// and one-shot actions do not.
    pub fn repeat(&self, key: KeyCode, out: &mut Vec<Out>) {
        if self
            .pending
            .get(&key)
            .is_some_and(|pending| !matches!(pending.phase, Phase::HoldActive))
        {
            return;
        }
        if let Some(ks) = self.virt.emitted.get(&key) {
            out.push(Out::KeyRaw {
                key: ks.key,
                down: true,
            });
        }
    }

    fn handle_event(&mut self, key: KeyCode, down: bool, now: Instant, out: &mut Vec<Out>) {
        if is_mouse_button(key) {
            if down {
                // Commit all pending tap/hold decisions before processing the
                // mouse event. Two goals:
                // (a) Modifier holds (ShiftLeft, CtrlLeft, …) are down for
                //     the release and later clicks. The ungrabbed click
                //     itself reaches the compositor first (KDE VM: 0.3–0.8 ms
                //     ahead), so it arrives without the modifier.
                // (b) Layer triggers activate so that layer keymaps are
                //     consulted when should_handle_mouse_button runs below.
                //
                // A mouse click never buffers (permissive hold applies to keys
                // only), so any deferred presses are flushed here too, before
                // the click, preserving press order.
                self.flush_waiting_second(now, out);
                self.commit_waiting_decisions(out);
                self.drain_decision_buffer(out);
            }
            if !self.should_handle_mouse_button(key, down) {
                out.push(Out::KeyRaw { key, down });
                return;
            }
            // Fall through to normal key handling below.
        } else if self.permissive_intercept(key, down, now, out) {
            return;
        } else if !down && self.releases_modifier(key) {
            // A deferred tap was typed while this modifier was down; firing
            // it after the release would drop the modifier (Ctrl+A -> "a").
            self.flush_waiting_second(now, out);
        }

        if !down
            && !self.pending.contains_key(&key)
            && !self.virt.emitted.contains_key(&key)
            && !self.virt.oneshot.contains(&key)
        {
            if !self.virt.is_held(key) {
                out.push(Out::KeyRaw { key, down });
            }
            return;
        }

        if down {
            self.on_press(key, now, out);
        } else {
            self.on_release(key, now, out);
        }
    }

    fn on_press(&mut self, key: KeyCode, now: Instant, out: &mut Vec<Out>) {
        // Second press of a rule key that is currently waiting for a
        // double-tap. Compare the *event* time against the window deadline
        // rather than trusting the phase alone: a live press can arrive
        // after the deadline but before the next `tick` fires the tap, and a
        // permissive-hold replay carries an old timestamp. Only a press that
        // actually lands inside the window is a double-tap.
        let waiting_second_deadline = match self.pending.get(&key).map(|p| &p.phase) {
            Some(Phase::WaitingSecond { deadline })
                if self.pending[&key].rule.trigger_keys.len() <= 1 =>
            {
                Some(*deadline)
            }
            _ => None,
        };
        if let Some(deadline) = waiting_second_deadline {
            if now <= deadline {
                let pending = self.pending.remove(&key);
                // Other keys' deferred taps happened physically before this
                // second press — flush them first so output stays in order.
                self.flush_waiting_second(now, out);
                // Permissive rules already intercepted this event above.
                self.commit_waiting_decisions(out);
                let dtap = pending.and_then(|p| p.rule.double_tap);
                self.fire_action(dtap.as_ref(), self.layout.default_mod_delay, now, out);
                // The matching release must not emit anything (fire-and-forget).
                self.virt.oneshot.insert(key);
                return;
            }
            // Window elapsed: emit the deferred first tap now, then fall
            // through and treat this key-down as a brand-new press (which may
            // itself open a fresh tap/hold/double-tap sequence).
            if let Some(pending) = self.pending.remove(&key) {
                self.fire_tap(key, &pending.rule.tap, now, out);
            }
        }

        // Any new key-down first commits currently deferred taps so
        // emitted events stay in natural press order (the deferred tap's
        // press happened before everything below).
        self.flush_waiting_second(now, out);

        // Interrupt-on-other-key-press: any pending rule still in
        // WaitingDecision must resolve as hold before we let this new
        // event through. This is what makes Shift+KeyA, MetaLeft(hold=ControlLeft)+KeyC
        // and similar mod-tap patterns feel natural.
        //
        // In permissive-hold mode this is a no-op: interrupting presses are
        // buffered in `handle` before ever reaching `on_press`, so no rule is
        // in WaitingDecision here.
        self.commit_waiting_decisions(out);

        if !self.layers.active.is_empty() {
            self.layers.presses += 1;
            self.layers.used = true;
        }
        if self.layers.active.is_empty() {
            if let Some(rule) = self.active_rule_for_key(key) {
                self.start_rule(key, rule, now, out);
                return;
            }
            if self.layout.rules.contains_key(&key) {
                out.push(Out::KeyRaw { key, down: true });
                self.virt
                    .emitted
                    .insert(key, Keystroke { mods: vec![], key });
                return;
            }
        }

        // When at least one layer is active, rule keys must behave like
        // normal keys inside that layer. Only the active-layer keymaps
        // participate in resolution.
        {
            let mapped = self.lookup_mapping(key);
            match mapped {
                Some((layer_id, def)) => match def {
                    ActionDef::Stroke(ks)
                        if !ks.mods.is_empty()
                            && self
                                .layout
                                .layer_hold_mods
                                .get(&layer_id)
                                .is_some_and(|keys| keys.contains(&key)) =>
                    {
                        self.press_holding_mods(&layer_id, key, ks, out);
                    }
                    ActionDef::Stroke(ks) => {
                        self.emit_stroke_press(key, ks, out);
                    }
                    ActionDef::Literal(text) => {
                        out.push(Out::Literal(text));
                        self.virt.oneshot.insert(key);
                    }
                    ActionDef::Macro(md) => {
                        self.runner.run(md, now);
                        self.virt.oneshot.insert(key);
                    }
                    ActionDef::System(action) => {
                        out.push(Out::RunSystem(action));
                        self.virt.oneshot.insert(key);
                    }
                    ActionDef::Command(command) => {
                        out.push(Out::RunCommand(command));
                        self.virt.oneshot.insert(key);
                    }
                    ActionDef::Swallow => {
                        self.virt.oneshot.insert(key);
                    }
                    ActionDef::Unavailable(action) => {
                        report_unavailable(&action);
                        self.virt.oneshot.insert(key);
                    }
                },
                None => {
                    out.push(Out::KeyRaw { key, down: true });
                    self.virt
                        .emitted
                        .insert(key, Keystroke { mods: vec![], key });
                }
            }
        }
    }

    fn on_release(&mut self, key: KeyCode, now: Instant, out: &mut Vec<Out>) {
        // Releases can arrive through replay or a chord owner, bypassing
        // `handle`. Resolve only this key's timer at the event timestamp.
        if self
            .long_holds
            .get(&key)
            .is_some_and(|(deadline, _)| now >= *deadline)
            && let Some((_, action)) = self.long_holds.remove(&key)
        {
            self.commit_hold(key, out);
            self.fire_action(Some(&action), self.layout.default_mod_delay, now, out);
        }
        if self.pending.get(&key).is_some_and(
            |p| matches!(p.phase, Phase::WaitingDecision { deadline } if now >= deadline),
        ) {
            self.commit_hold(key, out);
            self.drain_decision_buffer(out);
        }
        self.long_holds.remove(&key);
        if let Some(pending) = self.pending.get(&key).cloned() {
            match pending.phase {
                Phase::HoldActive => {
                    self.pending.remove(&key);
                    self.release_hold_with(&pending.rule, key, out);
                }
                Phase::WaitingDecision { .. } => {
                    // Short press. If double-tap is configured, delay the
                    // tap by double_tap_window to see if a second press
                    // follows; otherwise fire tap immediately.
                    let has_dtap = pending.rule.double_tap.is_some();
                    let rule_window = pending.rule.double_tap_window;
                    if has_dtap {
                        if let Some(p) = self.pending.get_mut(&key) {
                            p.phase = Phase::WaitingSecond {
                                deadline: now + rule_window,
                            };
                        }
                    } else {
                        self.pending.remove(&key);
                        self.fire_tap(key, &pending.rule.tap, now, out);
                    }
                }
                Phase::WaitingSecond { .. } => {
                    self.release_emitted(key, out);
                }
            }
            // The deciding key resolved as a tap (permissive-hold mode) —
            // replay any presses that were deferred behind its decision.
            // No-op in eager mode and for HoldActive/WaitingSecond releases.
            self.drain_decision_buffer(out);
            return;
        }

        if self.virt.oneshot.remove(&key) {
            // Macro / literal / double-tap fired on press — nothing to release.
            return;
        }

        if self.virt.emitted.contains_key(&key) {
            self.release_emitted(key, out);
        } else {
            // Unknown / orphan release — pass through as release to avoid stuck keys.
            out.push(Out::KeyRaw { key, down: false });
        }
    }

    /// Press a resolved keystroke and remember it so the matching
    /// release event (on physical key up) can emit the correct virtual
    /// key-up + mod-release.
    fn emit_stroke_press(&mut self, physical: KeyCode, ks: Keystroke, out: &mut Vec<Out>) {
        if !ks.mods.is_empty() {
            for m in &ks.mods {
                *self.virt.mod_refs.entry(*m).or_insert(0) += 1;
            }
        }
        out.push(Out::ChordPress {
            ks: ks.clone(),
            mod_delay: self.layout.default_mod_delay,
        });
        self.virt.emitted.insert(physical, ks);
    }

    /// Press `ks` for a hold-modifiers layer key. Its modifiers stay down
    /// until the layer's trigger is released; only the main key follows the
    /// physical key. Modifiers left by an earlier such key and not used by
    /// this one are released, so Alt+Shift+Tab followed by Alt+Tab keeps Alt
    /// but drops Shift.
    fn press_holding_mods(
        &mut self,
        layer_id: &str,
        physical: KeyCode,
        ks: Keystroke,
        out: &mut Vec<Out>,
    ) {
        let Some(trigger) = self
            .layers
            .triggers
            .get_mut(layer_id)
            .and_then(|triggers| triggers.last_mut())
        else {
            self.emit_stroke_press(physical, ks, out);
            return;
        };
        let previous = std::mem::replace(&mut trigger.held_mods, ks.mods.clone());
        // Modifiers already down (Alt from the previous Tab) are not pressed
        // again, so repeated presses skip the modifier delay.
        let fresh = self.virt.mods_not_held(&ks.mods);
        for m in ks.mods.iter().filter(|m| !previous.contains(m)) {
            *self.virt.mod_refs.entry(*m).or_insert(0) += 1;
        }
        let dropped: Vec<KeyCode> = previous
            .into_iter()
            .filter(|m| !ks.mods.contains(m))
            .collect();
        let released = self.virt.take_releasable_mods(&dropped);
        if !released.is_empty() {
            out.push(Out::ReleaseMods(released));
        }
        out.push(Out::ChordPress {
            ks: Keystroke {
                mods: fresh,
                key: ks.key,
            },
            mod_delay: self.layout.default_mod_delay,
        });
        self.virt.emitted.insert(
            physical,
            Keystroke {
                mods: Vec::new(),
                key: ks.key,
            },
        );
    }

    fn release_emitted(&mut self, physical: KeyCode, out: &mut Vec<Out>) {
        let Some(ks) = self.virt.emitted.remove(&physical) else {
            return;
        };
        let mods = self.virt.take_releasable_mods(&ks.mods);
        if self.virt.is_held(ks.key) {
            if !mods.is_empty() {
                out.push(Out::ReleaseMods(mods));
            }
            return;
        }
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
            mod_delay: self.layout.default_mod_delay,
        });
    }

    /// True while an event of `key` at or before `deadline` still waits in
    /// the combo or decision buffer. The key's timer must not fire before
    /// that event replays: a buffered release or second press decides it.
    fn timer_blocked(&self, key: KeyCode, deadline: Instant) -> bool {
        self.chords
            .buffer
            .iter()
            .chain(&self.decision_buffer)
            .any(|event| event.key == key && event.at <= deadline)
    }

    /// True when releasing `key` releases a modifier: a physical modifier
    /// or a key that currently holds one (e.g. `hold: ControlLeft`).
    fn releases_modifier(&self, key: KeyCode) -> bool {
        is_modifier_key(key)
            || self
                .virt
                .emitted
                .get(&key)
                .is_some_and(|ks| is_modifier_key(ks.key))
    }

    fn lookup_mapping(&self, key: KeyCode) -> Option<(String, ActionDef)> {
        self.layers.lookup(&self.layout.layer_maps, key)
    }

    fn start_rule(&mut self, key: KeyCode, rule: RuleEntry, now: Instant, out: &mut Vec<Out>) {
        if let Some(action) = rule.long_hold.clone() {
            self.long_holds
                .insert(key, (now + rule.long_hold_timeout, action));
        }
        if matches!(rule.tap, TapMode::Swallow) && rule.double_tap.is_none() {
            self.pending.insert(
                key,
                Pending {
                    rule: rule.clone(),
                    phase: Phase::HoldActive,
                },
            );
            self.commit_hold_with(&rule, key, out);
        } else {
            let deadline = now + rule.hold_timeout;
            self.pending.insert(
                key,
                Pending {
                    rule,
                    phase: Phase::WaitingDecision { deadline },
                },
            );
        }
    }

    fn active_rule_for_key(&self, key: KeyCode) -> Option<RuleEntry> {
        self.layout.rules.get(&key).and_then(|rules| {
            rules
                .iter()
                .find(|rule| rule_passes_apps(rule) && rule_passes_conditions(rule))
                .cloned()
        })
    }

    fn should_handle_mouse_button(&self, key: KeyCode, down: bool) -> bool {
        if !down
            && (self.pending.contains_key(&key)
                || self.virt.emitted.contains_key(&key)
                || self.virt.oneshot.contains(&key))
        {
            return true;
        }

        if self.lookup_mapping(key).is_some() {
            return true;
        }

        !is_primary_mouse_button(key) && self.layout.rules.contains_key(&key)
    }

    fn fire_action(
        &mut self,
        action: Option<&ActionDef>,
        mod_delay: Duration,
        now: Instant,
        out: &mut Vec<Out>,
    ) {
        match action {
            Some(ActionDef::Stroke(ks)) => {
                if self.virt.is_held(ks.key) {
                    return;
                }
                // Don't press — and above all don't release — modifiers that
                // are already virtually held (hold rules, active chords):
                // the stroke's trailing mod-release would break the hold.
                let ks = Keystroke {
                    mods: self.virt.mods_not_held(&ks.mods),
                    key: ks.key,
                };
                out.push(Out::Stroke { ks, mod_delay });
            }
            Some(ActionDef::Literal(text)) => out.push(Out::Literal(text.clone())),
            Some(ActionDef::Macro(md)) => self.runner.run(md.clone(), now),
            Some(ActionDef::System(action)) => out.push(Out::RunSystem(action.clone())),
            Some(ActionDef::Command(command)) => out.push(Out::RunCommand(command.clone())),
            Some(ActionDef::Swallow) => {}
            Some(ActionDef::Unavailable(action)) => report_unavailable(action),
            None => {}
        }
    }

    fn fire_tap(&mut self, physical: KeyCode, tap: &TapMode, now: Instant, out: &mut Vec<Out>) {
        match tap {
            TapMode::Native => {
                // Short native press+release of the physical key.
                out.push(Out::Stroke {
                    ks: Keystroke {
                        mods: vec![],
                        key: physical,
                    },
                    mod_delay: self.layout.default_mod_delay,
                });
            }
            TapMode::NativeChord(keys) => {
                let keys: Vec<_> = keys
                    .iter()
                    .filter(|key| !self.virt.is_held(**key))
                    .copied()
                    .collect();
                for key in &keys {
                    out.push(Out::KeyRaw {
                        key: *key,
                        down: true,
                    });
                }
                for key in keys.iter().rev() {
                    out.push(Out::KeyRaw {
                        key: *key,
                        down: false,
                    });
                }
            }
            TapMode::Swallow => {}
            TapMode::Action(a) => {
                self.fire_action(Some(a), self.layout.default_mod_delay, now, out);
            }
        }
    }

    /// Fire all pending taps whose double-tap window is still open — used
    /// when another key is pressed, so the deferred tap is committed
    /// before the new key's action. Preserves ordering.
    fn flush_waiting_second(&mut self, now: Instant, out: &mut Vec<Out>) {
        let keys: Vec<KeyCode> = self
            .pending
            .iter()
            .filter(|(_, p)| matches!(p.phase, Phase::WaitingSecond { deadline } if deadline <= now || p.rule.trigger_keys.len() <= 1 || !p.rule.trigger_keys.iter().any(|key| self.physical_down.contains(key))))
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            let pending = self.pending.remove(&k);
            if let Some(pending) = pending {
                self.fire_tap(k, &pending.rule.tap, now, out);
            }
        }
    }

    /// Generic shutdown helper — release everything that is currently held.
    pub fn shutdown(&mut self, out: &mut Vec<Out>) {
        self.runner.cancel(&mut self.virt, out);
        // HashMap drain order is arbitrary; release plain keys before
        // modifier keys so a chord's main key never outlives its modifier.
        let mut held: Vec<KeyCode> = self.virt.emitted.drain().map(|(_, ks)| ks.key).collect();
        held.sort_by_key(|k| is_modifier_key(*k));
        for key in held {
            out.push(Out::KeyRaw { key, down: false });
        }
        let mods: Vec<KeyCode> = self
            .virt
            .mod_refs
            .drain()
            .filter(|(_, r)| *r > 0)
            .map(|(k, _)| k)
            .collect();
        if !mods.is_empty() {
            out.push(Out::ReleaseMods(mods));
        }
        self.layers.active.clear();
        self.pending.clear();
        // Buffered presses were never emitted to uinput, so there is nothing
        // to release — drop them rather than replaying into a tearing-down
        // engine.
        self.decision_buffer.clear();
        self.chords.buffer.clear();
        self.virt.oneshot.clear();
        self.layers.triggers.clear();
        self.long_holds.clear();
        self.chords.active.clear();
        self.chords.suppressed.clear();
        self.chords.consumed.clear();
        self.physical_down.clear();
        self.press_at.clear();
    }

    #[allow(dead_code)]
    pub fn default_hold(&self) -> Duration {
        self.layout.default_hold
    }

    pub fn execute_remote(&mut self, action: &str, out: &mut Vec<Out>) {
        let resolved =
            builder::resolve_action(action, &self.layout.macros, &self.layout.commands, "remote");
        self.fire_action(
            resolved.as_ref(),
            self.layout.default_mod_delay,
            Instant::now(),
            out,
        );
    }
}

fn report_unavailable(action: &str) {
    log::debug!("[mapper] {action:?} is unavailable on this system");
    crate::events::emit(crate::events::CoreEvent::ActionUnavailable(action.into()));
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
    let aw = crate::runtime_state::active_window();
    if aw.is_none() {
        // Without active-window detection (e.g. KDE Wayland missing
        // `kdotool`) app-conditioned rules silently never fire (whitelist)
        // or are always blocked (blacklist) — say so once, loudly.
        use std::sync::atomic::{AtomicBool, Ordering};
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "[mapper] rules have app conditions but the active window is unknown \
                 (active-window detection unavailable on this desktop?); \
                 whitelist rules will not fire and blacklist rules stay blocked"
            );
        }
    }
    if let Some(bl) = bl {
        match &aw {
            Some(aw) if any_matches_window(bl, aw) => return false,
            Some(_) => {}
            None => return false, // fail-closed: unknown window + blacklist -> block
        }
    }
    if let Some(wl) = wl {
        match &aw {
            Some(aw) if any_matches_window(wl, aw) => {}
            _ => return false,
        }
    }
    true
}

fn rule_passes_conditions(rule: &RuleEntry) -> bool {
    let has_gm_cond = rule
        .condition_game_mode
        .as_ref()
        .is_some_and(|m| *m != GameModeCondition::Ignore);
    let has_layout_cond = rule
        .condition_layouts
        .as_ref()
        .is_some_and(|l| !l.is_empty());

    if !has_gm_cond && !has_layout_cond {
        return true;
    }
    if has_gm_cond {
        // Undetectable game mode counts as off, so desktop rules keep working.
        let gm_active = crate::runtime_state::game_mode_active();
        if !match rule.condition_game_mode {
            Some(GameModeCondition::On) => gm_active,
            Some(GameModeCondition::Off) => !gm_active,
            _ => false,
        } {
            return false;
        }
    }
    if has_layout_cond {
        let Some(current) = crate::runtime_state::layout_short() else {
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

fn is_primary_mouse_button(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::BTN_LEFT | KeyCode::BTN_RIGHT | KeyCode::BTN_MIDDLE
    )
}

fn is_modifier_key(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
    )
}

fn is_mouse_button(key: KeyCode) -> bool {
    (272..=281).contains(&key.code())
}

#[cfg(test)]
mod tests;
