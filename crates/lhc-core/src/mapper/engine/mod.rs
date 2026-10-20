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
mod model;

pub use self::model::Out;
use self::model::{
    ActionDef, BufferedEvent, HoldMode, LayerTrigger, Pending, Phase, RuleEntry, TapMode,
};
use super::action::Keystroke;
use super::config::GameModeCondition;
use super::system::SysCommand;
use crate::mapper::key_code::KeyCode;
use crate::profile::app_match::any_matches_window;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Macro runs that may wait behind the active one; further runs are dropped.
const MACRO_QUEUE_LIMIT: usize = 3;

pub struct Engine {
    rules: HashMap<KeyCode, Vec<RuleEntry>>,
    chord_rules: Vec<(Vec<KeyCode>, RuleEntry)>,
    physical_down: HashSet<KeyCode>,
    active_chords: HashMap<KeyCode, Vec<KeyCode>>,
    chord_suppressed: HashMap<KeyCode, Vec<(KeyCode, Keystroke)>>,
    chord_consumed: HashSet<KeyCode>,
    long_holds: HashMap<KeyCode, (Instant, ActionDef)>,
    macros: HashMap<String, self::model::MacroDef>,
    commands: HashMap<String, SysCommand>,
    /// layer_id -> (physical_key -> resolved action)
    layer_maps: HashMap<String, HashMap<KeyCode, ActionDef>>,
    default_hold: Duration,
    default_mod_delay: Duration,

    /// Events deferred while a rule key is in `WaitingDecision`
    /// (permissive-hold mode only). Replayed in order once the decision
    /// resolves.
    decision_buffer: Vec<BufferedEvent>,
    /// Events deferred while a non-modifier chord member waits for the rest
    /// of its chord (see `combo_term`). Replayed in order when the chord
    /// cannot complete any more; dropped for members when it fires.
    combo_buffer: Vec<BufferedEvent>,
    /// How long a press of a chord member is held back waiting for the rest
    /// of the chord (QMK's `COMBO_TERM`). A chord whose keys arrive within
    /// this window fires without its members ever reaching the app.
    combo_term: Duration,

    /// Stack of currently active layers (top = highest priority).
    active_layers: Vec<String>,

    /// Per-rule pending state (waiting for release/timeout decision).
    pending: HashMap<KeyCode, Pending>,

    /// Tracks which physical keys are currently pressed AND what keystroke
    /// we emitted for them, so key-up releases the right virtual keys.
    /// Used both for non-rule remaps (keymap lookups) and for rule-keys
    /// whose hold mode is Native/Keystroke (uniform release path).
    emitted: HashMap<KeyCode, Keystroke>,

    /// Physical keys whose press triggered a one-shot macro / literal.
    /// Release just clears the entry — no virtual key release is needed.
    oneshot_consumed: HashSet<KeyCode>,

    /// Currently "held down" virtual modifiers (refcounted).
    mod_refs: HashMap<KeyCode, u32>,

    /// layer_id -> rules that activated it, oldest to newest
    layer_triggers: HashMap<String, Vec<LayerTrigger>>,
    /// Physical keys whose hold is currently suppressed, per isolate key
    /// (so the last isolate-key release can restore the hold).
    isolated_holds: HashMap<KeyCode, Vec<KeyCode>>,

    /// Currently executing macro (if any)
    active_macro: Option<self::model::ActiveMacro>,
    /// Runs waiting for the active macro, oldest first; at most
    /// `MACRO_QUEUE_LIMIT`.
    macro_queue: VecDeque<self::model::MacroDef>,
    /// Longest a single macro run may take before it is aborted.
    macro_timeout: Duration,
    text_pending: bool,
    /// Key presses handled while a layer was active.
    layer_presses: u64,
    layer_hold_id: u64,
    layer_used: bool,
}
impl Engine {
    /// Apply new bindings without releasing keys already held by the user.
    /// Pending decisions retain their original rule until physical release.
    pub fn reconfigure(&mut self, cfg: &super::config::AppConfig, out: &mut Vec<Out>) {
        self.flush_combo_buffer(out);
        self.cancel_macros(out);
        if !cfg.settings.commands_enabled {
            fn revoke(action: &mut ActionDef) {
                match action {
                    ActionDef::Command(_) => *action = ActionDef::Swallow,
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
        self.rules = next.rules;
        self.chord_rules = next.chord_rules;
        self.macros = next.macros;
        self.commands = next.commands;
        self.layer_maps = next.layer_maps;
        self.default_hold = next.default_hold;
        self.default_mod_delay = next.default_mod_delay;
        self.combo_term = next.combo_term;
        self.macro_timeout = next.macro_timeout;
        self.text_pending = false;
    }

    /// The topmost active layer, if any layer is held.
    pub fn top_layer(&self) -> Option<&str> {
        self.active_layers.last().map(String::as_str)
    }

    /// Snapshot for delayed shell notifications; separate presses of the same
    /// trigger remain distinguishable even within one input batch.
    pub fn held_layer_state(&self) -> super::layer_state::HeldLayerState {
        super::layer_state::HeldLayerState {
            layer: self.top_layer().map(str::to_owned),
            hold_id: self.layer_hold_id,
            used: self.top_layer().is_some() && self.layer_used,
        }
    }

    /// Number of key presses handled while a layer was active; changes
    /// whenever a key is pressed inside a held layer.
    pub fn layer_presses(&self) -> u64 {
        self.layer_presses
    }

    /// Time until the nearest pending deadline (hold-decision,
    /// waiting-for-second-press, the next macro step wake-up or the macro
    /// timeout), or None.
    pub fn next_deadline(&self, now: Instant) -> Option<Duration> {
        self.pending
            .values()
            .filter_map(|p| match p.phase {
                Phase::WaitingDecision { deadline } | Phase::WaitingSecond { deadline } => {
                    Some(deadline.saturating_duration_since(now))
                }
                Phase::HoldActive => None,
            })
            .chain(
                self.long_holds
                    .values()
                    .map(|(deadline, _)| deadline.saturating_duration_since(now)),
            )
            .chain(
                self.active_macro
                    .as_ref()
                    .filter(|am| {
                        !self.text_pending || !matches!(am.phase, self::model::MacroPhase::NextStep)
                    })
                    .map(|am| am.next_wake.saturating_duration_since(now)),
            )
            .chain(
                self.active_macro
                    .as_ref()
                    .map(|am| am.deadline.saturating_duration_since(now)),
            )
            .chain(
                // A queued run starts on the next tick.
                (self.active_macro.is_none() && !self.macro_queue.is_empty())
                    .then_some(Duration::ZERO),
            )
            .chain(
                self.combo_deadline()
                    .map(|deadline| deadline.saturating_duration_since(now)),
            )
            .min()
    }

    /// A failed text step must not submit incomplete input with subsequent keys.
    pub fn text_input_failed(&mut self, out: &mut Vec<Out>) {
        self.abort_active_macro(out);
    }

    /// Resume macro timing only after the text worker has completed injection.
    pub fn set_text_pending(&mut self, pending: bool, now: Instant) {
        if self.text_pending
            && !pending
            && let Some(am) = self.active_macro.as_mut()
        {
            let previous = am.current_step.checked_sub(1).and_then(|i| am.steps.get(i));
            if matches!(am.phase, self::model::MacroPhase::NextStep)
                && matches!(
                    previous,
                    Some(crate::mapper::action::MacroStepItem::Literal(_))
                )
            {
                am.next_wake = next_macro_wake(now, am);
            }
        }
        self.text_pending = pending;
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
            .filter(|(_, (deadline, _))| now >= *deadline)
            .map(|(key, _)| *key)
            .collect();
        for key in expired {
            if let Some((_, action)) = self.long_holds.remove(&key) {
                self.commit_hold(key, out);
                self.fire_action(Some(&action), self.default_mod_delay, now, out);
            }
        }
        if self
            .active_macro
            .as_ref()
            .is_some_and(|am| now >= am.deadline)
        {
            log::warn!(
                "[mapper] macro exceeded {} ms, aborting",
                self.macro_timeout.as_millis()
            );
            self.abort_active_macro(out);
        }
        self.start_queued_macro(now);
        if let Some(mut am) = self.active_macro.take() {
            if now >= am.next_wake
                && (!self.text_pending || !matches!(am.phase, self::model::MacroPhase::NextStep))
            {
                use self::model::MacroPhase;
                use crate::mapper::action::MacroStepItem;
                const MAX_MACRO_STEPS: usize = 1000;

                loop {
                    if am.current_step >= am.steps.len() {
                        break;
                    }
                    if am.steps_emitted >= MAX_MACRO_STEPS {
                        log::debug!(
                            "[mapper] macro step limit ({}) exceeded, aborting",
                            MAX_MACRO_STEPS
                        );
                        // Aborting mid-stroke: the stroke's modifiers were
                        // pressed in an earlier phase and must be released
                        // here or they stay stuck down.
                        if let MacroPhase::StrokeModDelayPress(ref ks)
                        | MacroPhase::StrokeModDelayRelease(ref ks) = am.phase
                        {
                            for m in self.take_releasable_mods(&ks.mods).iter().rev() {
                                out.push(Out::KeyRaw {
                                    key: *m,
                                    down: false,
                                });
                            }
                        }
                        break;
                    }
                    match am.phase {
                        MacroPhase::NextStep => {
                            let step = &am.steps[am.current_step];
                            match step {
                                MacroStepItem::Stroke(ks) => {
                                    // Skip modifiers something else already
                                    // holds down (a `hold:` rule, a chord, a
                                    // passthrough key): pressing them again is
                                    // redundant, and releasing them at the end
                                    // of the stroke would break the hold.
                                    let press = self.mods_not_held(&ks.mods);
                                    for m in &ks.mods {
                                        *self.mod_refs.entry(*m).or_insert(0) += 1;
                                    }
                                    if !ks.mods.is_empty() {
                                        for m in &press {
                                            out.push(Out::KeyRaw {
                                                key: *m,
                                                down: true,
                                            });
                                        }
                                        am.phase = MacroPhase::StrokeModDelayPress(Keystroke {
                                            mods: ks.mods.clone(),
                                            key: ks.key,
                                        });
                                        am.next_wake = now + am.mod_delay;
                                        self.active_macro = Some(am);
                                        break;
                                    } else {
                                        self.emit_macro_key(ks.key, out);
                                        am.steps_emitted += 1;
                                        am.current_step += 1;
                                        if am.current_step < am.steps.len() {
                                            am.next_wake = next_macro_wake(now, &am);
                                            self.active_macro = Some(am);
                                            break;
                                        }
                                    }
                                }
                                MacroStepItem::System(action) => {
                                    out.push(Out::RunSystem(action.clone()));
                                    am.steps_emitted += 1;
                                    am.current_step += 1;
                                    if am.current_step < am.steps.len() {
                                        am.next_wake = next_macro_wake(now, &am);
                                        self.active_macro = Some(am);
                                        break;
                                    }
                                }
                                MacroStepItem::Command(command) => {
                                    out.push(Out::RunCommand(command.clone()));
                                    am.steps_emitted += 1;
                                    am.current_step += 1;
                                    if am.current_step < am.steps.len() {
                                        am.next_wake = next_macro_wake(now, &am);
                                        self.active_macro = Some(am);
                                        break;
                                    }
                                }
                                MacroStepItem::Literal(text) => {
                                    out.push(Out::Literal(text.clone()));
                                    self.text_pending = true;
                                    am.steps_emitted += 1;
                                    am.current_step += 1;
                                    if am.current_step < am.steps.len() {
                                        am.next_wake = next_macro_wake(now, &am);
                                        self.active_macro = Some(am);
                                        break;
                                    }
                                }
                                // `run_macro` never starts a run with one.
                                MacroStepItem::Unavailable(_) => {
                                    am.current_step = am.steps.len();
                                }
                                MacroStepItem::Pause(duration) => {
                                    am.steps_emitted += 1;
                                    am.current_step += 1;
                                    if am.current_step < am.steps.len() {
                                        am.next_wake = now + *duration;
                                        self.active_macro = Some(am);
                                        break;
                                    }
                                }
                            }
                        }
                        MacroPhase::StrokeModDelayPress(ref ks) => {
                            let ks = ks.clone();
                            self.emit_macro_key(ks.key, out);
                            am.phase = MacroPhase::StrokeModDelayRelease(ks.clone());
                            am.next_wake = now + am.mod_delay;
                            self.active_macro = Some(am);
                            break;
                        }
                        MacroPhase::StrokeModDelayRelease(ref ks) => {
                            for m in self.take_releasable_mods(&ks.mods).iter().rev() {
                                out.push(Out::KeyRaw {
                                    key: *m,
                                    down: false,
                                });
                            }
                            am.phase = MacroPhase::NextStep;
                            am.steps_emitted += 1;
                            am.current_step += 1;
                            if am.current_step < am.steps.len() {
                                am.next_wake = next_macro_wake(now, &am);
                                self.active_macro = Some(am);
                                break;
                            }
                        }
                    }
                }
            } else {
                self.active_macro = Some(am);
            }
        }
        self.start_queued_macro(now);

        let mut expired_hold: Vec<KeyCode> = Vec::new();
        let mut expired_second: Vec<KeyCode> = Vec::new();
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

    fn emit_macro_key(&self, key: KeyCode, out: &mut Vec<Out>) {
        if !self.is_virtually_held(key) {
            out.push(Out::KeyRaw { key, down: true });
            out.push(Out::KeyRaw { key, down: false });
        }
    }

    /// Abort the in-flight macro (if any). A stroke step presses its
    /// modifiers first and releases them only in a later phase, so an
    /// abort mid-stroke must release them here or they stay stuck down.
    fn abort_active_macro(&mut self, out: &mut Vec<Out>) {
        let Some(am) = self.active_macro.take() else {
            return;
        };
        match am.phase {
            self::model::MacroPhase::StrokeModDelayPress(ks)
            | self::model::MacroPhase::StrokeModDelayRelease(ks) => {
                for m in self.take_releasable_mods(&ks.mods).iter().rev() {
                    out.push(Out::KeyRaw {
                        key: *m,
                        down: false,
                    });
                }
            }
            self::model::MacroPhase::NextStep => {}
        }
    }

    /// Abort the active macro and drop the queued runs.
    fn cancel_macros(&mut self, out: &mut Vec<Out>) {
        self.macro_queue.clear();
        self.abort_active_macro(out);
    }

    /// Start a macro run, or queue it behind the active one: a run that is
    /// cut short leaves the application half-edited.
    fn run_macro(&mut self, md: self::model::MacroDef, now: Instant) {
        if md.steps.is_empty() {
            return;
        }
        // A step this system cannot run would leave the macro half done.
        if let Some(action) = md.steps.iter().find_map(|step| match step {
            crate::mapper::action::MacroStepItem::Unavailable(action) => Some(action),
            _ => None,
        }) {
            report_unavailable(action);
            return;
        }
        if self.active_macro.is_none() {
            self.active_macro = Some(self.begin_macro(md, now));
        } else if self.macro_queue.len() < MACRO_QUEUE_LIMIT {
            self.macro_queue.push_back(md);
        } else {
            log::debug!("[mapper] macro queue is full, dropping the run");
        }
    }

    fn start_queued_macro(&mut self, now: Instant) {
        if self.active_macro.is_none()
            && let Some(md) = self.macro_queue.pop_front()
        {
            self.active_macro = Some(self.begin_macro(md, now));
        }
    }

    fn begin_macro(&self, md: self::model::MacroDef, now: Instant) -> self::model::ActiveMacro {
        self::model::ActiveMacro {
            steps: md.steps,
            step_pause: md.step_pause,
            mod_delay: md.mod_delay,
            current_step: 0,
            steps_emitted: 0,
            phase: self::model::MacroPhase::NextStep,
            next_wake: now,
            deadline: now + self.macro_timeout,
        }
    }

    /// Handle a raw key event from the grabbed device.
    pub fn handle(&mut self, key: KeyCode, down: bool, now: Instant, out: &mut Vec<Out>) {
        if down {
            if !self.physical_down.insert(key) {
                return;
            }
            if !self.active_layers.is_empty() {
                self.layer_used = true;
            }
        } else {
            self.physical_down.remove(&key);
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
        if let Some(ks) = self.emitted.get(&key) {
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
                // (a) Modifier holds (ShiftLeft, CtrlLeft, …) reach uinput
                //     before the physical mouse click arrives at the OS.
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
        }

        if !down
            && !self.pending.contains_key(&key)
            && !self.emitted.contains_key(&key)
            && !self.oneshot_consumed.contains(&key)
        {
            if !self.is_virtually_held(key) {
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
                self.fire_action(dtap.as_ref(), self.default_mod_delay, now, out);
                // The matching release must not emit anything (fire-and-forget).
                self.oneshot_consumed.insert(key);
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

        if !self.active_layers.is_empty() {
            self.layer_presses += 1;
            self.layer_used = true;
        }
        if self.active_layers.is_empty() {
            if let Some(rule) = self.active_rule_for_key(key) {
                self.start_rule(key, rule, now, out);
                return;
            }
            if self.rules.contains_key(&key) {
                out.push(Out::KeyRaw { key, down: true });
                self.emitted.insert(key, Keystroke { mods: vec![], key });
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
                    // Lazy hold: the first whitelisted key press inside the
                    // layer materialises the deferred modifier (e.g. Alt for
                    // the task switcher appears only with Tab, never alone).
                    let materialize = self
                        .layer_triggers
                        .get(&layer_id)
                        .and_then(|v| v.last())
                        .filter(|t| t.hold_ks.is_some() && t.whitelist.contains(&key))
                        .map(|t| (t.key, t.hold_ks.clone().unwrap()));
                    if let Some((owner, ks)) = materialize {
                        self.emit_stroke_press(owner, ks, out);
                        if let Some(t) = self
                            .layer_triggers
                            .get_mut(&layer_id)
                            .and_then(|v| v.last_mut())
                        {
                            t.hold_ks = None;
                        }
                    }

                    if let Some(trigger) = self.layer_triggers.get(&layer_id).and_then(|v| v.last())
                        && trigger.isolate_keys.contains(&key)
                        && let Some(ks) = self.emitted.get(&trigger.key).cloned()
                    {
                        let mut suppressed = Vec::new();
                        for target_key in ks.mods.iter().chain(std::iter::once(&ks.key)) {
                            // Another isolate key may already have
                            // suppressed this target — record the
                            // entry (so release bookkeeping stays
                            // balanced) but don't emit a second up.
                            // Refcounts are left untouched: they keep
                            // tracking chord ownership while the key
                            // is physically up, so unrelated chords
                            // release correctly afterwards.
                            if !self.is_suppressed(*target_key) {
                                out.push(Out::KeyRaw {
                                    key: *target_key,
                                    down: false,
                                });
                            }
                            suppressed.push(*target_key);
                        }
                        if !suppressed.is_empty() {
                            self.isolated_holds.insert(key, suppressed);
                        }
                    }

                    match def {
                        ActionDef::Stroke(ks) => {
                            self.emit_stroke_press(key, ks, out);
                        }
                        ActionDef::Literal(text) => {
                            out.push(Out::Literal(text));
                            self.oneshot_consumed.insert(key);
                        }
                        ActionDef::Macro(md) => {
                            self.run_macro(md, now);
                            self.oneshot_consumed.insert(key);
                        }
                        ActionDef::System(action) => {
                            out.push(Out::RunSystem(action));
                            self.oneshot_consumed.insert(key);
                        }
                        ActionDef::Command(command) => {
                            out.push(Out::RunCommand(command));
                            self.oneshot_consumed.insert(key);
                        }
                        ActionDef::Swallow => {
                            self.oneshot_consumed.insert(key);
                        }
                        ActionDef::Unavailable(action) => {
                            report_unavailable(&action);
                            self.oneshot_consumed.insert(key);
                        }
                    }
                }
                None => {
                    out.push(Out::KeyRaw { key, down: true });
                    self.emitted.insert(key, Keystroke { mods: vec![], key });
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
            self.fire_action(Some(&action), self.default_mod_delay, now, out);
        }
        if self.pending.get(&key).is_some_and(
            |p| matches!(p.phase, Phase::WaitingDecision { deadline } if now >= deadline),
        ) {
            self.commit_hold(key, out);
            self.drain_decision_buffer(out);
        }
        self.long_holds.remove(&key);
        if let Some(suppressed) = self.isolated_holds.remove(&key) {
            for target_key in suppressed {
                // Don't restore while another isolate key still suppresses
                // the same target — its own release will handle it.
                if self.is_suppressed(target_key) {
                    continue;
                }
                let still_held = self
                    .emitted
                    .values()
                    .any(|ks| ks.key == target_key || ks.mods.contains(&target_key));
                if still_held {
                    out.push(Out::KeyRaw {
                        key: target_key,
                        down: true,
                    });
                }
            }
        }

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

        if self.oneshot_consumed.remove(&key) {
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
    fn emit_stroke_press(&mut self, physical: KeyCode, ks: Keystroke, out: &mut Vec<Out>) {
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

    fn release_emitted(&mut self, physical: KeyCode, out: &mut Vec<Out>) {
        let Some(ks) = self.emitted.remove(&physical) else {
            return;
        };
        let mods = self.take_releasable_mods(&ks.mods);
        if self.is_virtually_held(ks.key) || self.is_suppressed(ks.key) {
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
            mod_delay: self.default_mod_delay,
        });
    }

    fn take_releasable_mods(&mut self, mods: &[KeyCode]) -> Vec<KeyCode> {
        if mods.is_empty() {
            return Vec::new();
        }
        for m in mods {
            if let Some(r) = self.mod_refs.get_mut(m) {
                *r = r.saturating_sub(1);
            }
        }
        // A modifier stays down while another chord still references it
        // (refcount) or while some mapping holds it as its *main* key
        // (e.g. a rule with `hold: ControlLeft`). A suppressed modifier
        // (isolate) is already physically up — emitting another release
        // would be spurious; the isolate-key release decides its fate.
        mods.iter()
            .copied()
            .filter(|m| {
                self.mod_refs.get(m).copied().unwrap_or(0) == 0
                    && !self.emitted.values().any(|ks| ks.key == *m)
                    && !self.is_suppressed(*m)
            })
            .collect()
    }

    /// True while any isolate key keeps this key physically released.
    fn is_suppressed(&self, key: KeyCode) -> bool {
        self.isolated_holds.values().any(|v| v.contains(&key))
    }

    /// True when the key is currently held down on the virtual device —
    /// either refcounted as a chord modifier or held as the main key of
    /// an emitted stroke (hold rules, passthrough keys).
    fn is_virtually_held(&self, key: KeyCode) -> bool {
        self.mod_refs.get(&key).copied().unwrap_or(0) > 0
            || self
                .emitted
                .values()
                .any(|ks| ks.key == key || ks.mods.contains(&key))
    }

    /// Subset of `mods` that a fired stroke actually needs to press (and
    /// later release): modifiers already held elsewhere are excluded.
    fn mods_not_held(&self, mods: &[KeyCode]) -> Vec<KeyCode> {
        mods.iter()
            .copied()
            .filter(|m| !self.is_virtually_held(*m))
            .collect()
    }

    fn lookup_mapping(&self, key: KeyCode) -> Option<(String, ActionDef)> {
        for l in self.active_layers.iter().rev() {
            if let Some(m) = self.layer_maps.get(l)
                && let Some(def) = m.get(&key)
            {
                return Some((l.clone(), def.clone()));
            }
        }
        None
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
        self.rules.get(&key).and_then(|rules| {
            rules
                .iter()
                .find(|rule| rule_passes_apps(rule) && rule_passes_conditions(rule))
                .cloned()
        })
    }

    fn should_handle_mouse_button(&self, key: KeyCode, down: bool) -> bool {
        if !down
            && (self.pending.contains_key(&key)
                || self.emitted.contains_key(&key)
                || self.oneshot_consumed.contains(&key))
        {
            return true;
        }

        if self.lookup_mapping(key).is_some() {
            return true;
        }

        !is_primary_mouse_button(key) && self.rules.contains_key(&key)
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
                if self.is_virtually_held(ks.key) {
                    return;
                }
                // Don't press — and above all don't release — modifiers that
                // are already virtually held (hold rules, active chords):
                // the stroke's trailing mod-release would break the hold.
                let ks = Keystroke {
                    mods: self.mods_not_held(&ks.mods),
                    key: ks.key,
                };
                out.push(Out::Stroke { ks, mod_delay });
            }
            Some(ActionDef::Literal(text)) => out.push(Out::Literal(text.clone())),
            Some(ActionDef::Macro(md)) => self.run_macro(md.clone(), now),
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
                    mod_delay: self.default_mod_delay,
                });
            }
            TapMode::NativeChord(keys) => {
                let keys: Vec<_> = keys
                    .iter()
                    .filter(|key| !self.is_virtually_held(**key))
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
                self.fire_action(Some(a), self.default_mod_delay, now, out);
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
        self.cancel_macros(out);
        // HashMap drain order is arbitrary; release plain keys before
        // modifier keys so a chord's main key never outlives its modifier.
        let mut held: Vec<KeyCode> = self.emitted.drain().map(|(_, ks)| ks.key).collect();
        held.sort_by_key(|k| is_modifier_key(*k));
        for key in held {
            out.push(Out::KeyRaw { key, down: false });
        }
        let mods: Vec<KeyCode> = self
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
        // Buffered presses were never emitted to uinput, so there is nothing
        // to release — drop them rather than replaying into a tearing-down
        // engine.
        self.decision_buffer.clear();
        self.combo_buffer.clear();
        self.oneshot_consumed.clear();
        self.layer_triggers.clear();
        self.isolated_holds.clear();
        self.long_holds.clear();
        self.active_chords.clear();
        self.chord_suppressed.clear();
        self.chord_consumed.clear();
        self.physical_down.clear();
    }

    #[allow(dead_code)]
    pub fn default_hold(&self) -> Duration {
        self.default_hold
    }

    pub fn execute_remote(&mut self, action: &str, out: &mut Vec<Out>) {
        let trimmed = action.trim();
        if trimmed.is_empty() {
            return;
        }
        let resolved = if let Some(rest) = trimmed.strip_prefix("macro:") {
            self.macros.get(rest.trim()).cloned().map(ActionDef::Macro)
        } else if let Some(rest) = trimmed.strip_prefix("cmd:") {
            self.commands
                .get(rest.trim())
                .cloned()
                .map(ActionDef::Command)
                .or(Some(ActionDef::Swallow))
        } else if let Some(text) = super::action::explicit_text(action) {
            Some(ActionDef::Literal(text))
        } else {
            super::action::parse_action(trimmed).map(ActionDef::Stroke)
        };
        if let Some(def) = resolved {
            self.fire_action(Some(&def), self.default_mod_delay, Instant::now(), out);
            return;
        }
        let mut available = false;
        if let Some(rest) = action.trim().strip_prefix("sys:") {
            if let Some(sys) = super::system::resolve(rest) {
                out.push(Out::RunSystem(sys));
                available = true;
            }
        } else if let Some(rest) = action.trim().strip_prefix("app:") {
            if let Some(app) = super::system::resolve_app(rest) {
                out.push(Out::RunSystem(app));
                available = true;
            }
        } else if let Some(rest) = action.trim().strip_prefix("kde:")
            && let Some(sys) = super::system::resolve_kde_shortcut(rest)
        {
            out.push(Out::RunSystem(sys));
            available = true;
        }
        if !available {
            report_unavailable(action.trim());
        }
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

fn next_macro_wake(now: Instant, am: &self::model::ActiveMacro) -> Instant {
    use crate::mapper::action::MacroStepItem;

    if matches!(am.steps.get(am.current_step), Some(MacroStepItem::Pause(_))) {
        now
    } else {
        now + am.step_pause
    }
}

fn is_mouse_button(key: KeyCode) -> bool {
    (272..=281).contains(&key.code())
}

#[cfg(test)]
mod tests;
