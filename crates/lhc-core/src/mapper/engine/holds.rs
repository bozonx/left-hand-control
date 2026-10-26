//! Layer holds and tap-versus-hold decisions.

use super::*;

impl Engine {
    /// Promote every rule currently in WaitingDecision to HoldActive
    /// (hold gets committed). Called from `on_press` for any new key to
    /// implement "interrupt on other key press".
    pub(super) fn commit_waiting_decisions(&mut self, out: &mut Vec<EngineOutput>) {
        let keys: Vec<KeyCode> = self
            .pending
            .iter()
            .filter(|(_, p)| {
                p.rule.long_hold.is_none() && matches!(p.phase, RulePhase::WaitingDecision { .. })
            })
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            self.commit_hold(k, out);
        }
    }

    /// The rule key currently in `WaitingDecision`, if any. In permissive
    /// mode there is at most one (further rule keys are buffered).
    pub(super) fn deciding_key(&self) -> Option<KeyCode> {
        self.pending.iter().find_map(|(k, p)| {
            (p.rule.long_hold.is_none() && matches!(p.phase, RulePhase::WaitingDecision { .. }))
                .then_some(*k)
        })
    }

    /// Interception while a rule key is deciding. In permissive-hold mode
    /// other key events are deferred: a press is buffered; a release whose
    /// press is in the buffer (a nested press+release) resolves the decision
    /// as a hold and lets the buffered events — and this release — replay in
    /// order. In both modes modifier releases wait for the decision (see
    /// `defer_release`).
    ///
    /// Returns true when the event was consumed (buffered) and `handle`
    /// should return; false to continue normal processing.
    pub(super) fn permissive_intercept(
        &mut self,
        key: KeyCode,
        down: bool,
        now: Instant,
        out: &mut Vec<EngineOutput>,
    ) -> bool {
        let Some(deciding) = self.deciding_key() else {
            return false;
        };
        // The deciding key's own events (its release / repeat) flow normally.
        if key == deciding {
            return false;
        }
        if !self.pending.get(&deciding).is_some_and(|p| {
            p.rule.tap_decision == crate::mapper::config::TapDecision::PermissiveHold
        }) {
            // Hold on other key press: a press commits the hold in
            // `on_press`, after the releases deferred before it.
            if down {
                if !self.decision_buffer.is_empty() {
                    self.resolve_deciding_as_hold(out);
                }
                return false;
            }
            return self.defer_release(key, now);
        }
        if down {
            self.decision_buffer
                .push(BufferedEvent { key, down, at: now });
            return true;
        }
        // Release of a key pressed *after* the deciding key → nested
        // press+release → the deciding key is a hold.
        if self.decision_buffer.iter().any(|e| e.key == key && e.down) {
            self.resolve_deciding_as_hold(out);
            // Replay may start another decision and buffer this key's press
            // again. Resolve that decision before allowing its release through.
            return self.permissive_intercept(key, down, now, out);
        }
        // Release of a key held *before* the decision started does not
        // resolve the decision.
        self.defer_release(key, now)
    }

    /// Defer the release of a key held before the decision started, or let
    /// it through. It stays ordered behind presses already buffered: a key
    /// buffered while this one was still physically down has to replay in
    /// that same state. A modifier release waits for the decision too: a
    /// tap fires at the deciding key's release and must still get the
    /// modifier that was down when it was pressed (Shift+A, not "a").
    /// Anything else with an empty buffer cannot be reordered and flows
    /// through immediately.
    fn defer_release(&mut self, key: KeyCode, now: Instant) -> bool {
        if self.decision_buffer.is_empty() && !self.releases_modifier(key) {
            return false;
        }
        self.decision_buffer.push(BufferedEvent {
            key,
            down: false,
            at: now,
        });
        true
    }

    /// Commit the current deciding key as a hold, then replay any buffered
    /// events in order.
    pub(super) fn resolve_deciding_as_hold(&mut self, out: &mut Vec<EngineOutput>) {
        if let Some(key) = self.deciding_key() {
            self.commit_hold(key, out);
        }
        self.drain_decision_buffer(out);
    }

    /// Replay all deferred presses/releases in order once a decision has
    /// resolved. Each event carries its original timestamp so downstream
    /// deadlines (a replayed rule key opening its own decision) stay
    /// anchored to when the key was physically pressed.
    pub(super) fn drain_decision_buffer(&mut self, out: &mut Vec<EngineOutput>) {
        if self.decision_buffer.is_empty() {
            return;
        }
        let buffered = std::mem::take(&mut self.decision_buffer);
        for ev in buffered {
            self.handle_event(ev.key, ev.down, ev.at, out);
        }
    }

    /// Commit a pending rule to HoldActive and emit the corresponding
    /// press side-effect (layer push, native key-down, keystroke-down).
    pub(super) fn commit_hold(&mut self, key: KeyCode, out: &mut Vec<EngineOutput>) {
        let Some(p) = self.pending.get_mut(&key) else {
            return;
        };
        if !matches!(p.phase, RulePhase::WaitingDecision { .. }) {
            return;
        }
        let rule = p.rule.clone();
        {
            p.phase = RulePhase::HoldActive;
        }
        self.commit_hold_with(&rule, key, out);
    }

    pub(super) fn commit_hold_with(
        &mut self,
        rule: &RuleEntry,
        key: KeyCode,
        out: &mut Vec<EngineOutput>,
    ) {
        if let Some(id) = &rule.layer_id {
            self.layers.push(id.clone());
            self.layers
                .triggers
                .entry(id.clone())
                .or_default()
                .push(LayerTrigger {
                    key,
                    held_mods: Vec::new(),
                });
        }
        match &rule.hold {
            HoldOutputMode::Native => {
                out.push(EngineOutput::KeyRaw { key, down: true });
                self.virt
                    .emitted
                    .insert(key, Keystroke { mods: vec![], key });
            }
            HoldOutputMode::Swallow => {
                // Nothing — physical key is eaten.
            }
            HoldOutputMode::Keystroke(ks) => {
                self.emit_stroke_press(key, ks.clone(), out);
            }
        }
    }

    /// Undo whatever `commit_hold_with` did for this rule key.
    pub(super) fn release_hold_with(
        &mut self,
        rule: &RuleEntry,
        key: KeyCode,
        out: &mut Vec<EngineOutput>,
    ) {
        if let Some(id) = &rule.layer_id {
            let mut held_mods = Vec::new();
            if let Some(triggers) = self.layers.triggers.get_mut(id) {
                if let Some(pos) = triggers.iter().rposition(|trigger| trigger.key == key) {
                    held_mods = triggers.remove(pos).held_mods;
                }
                if triggers.is_empty() {
                    self.layers.triggers.remove(id);
                }
            }
            self.layers.pop(id);
            let mods = self.virt.take_releasable_mods(&held_mods);
            if !mods.is_empty() {
                out.push(EngineOutput::ReleaseMods(mods));
            }
        }
        match &rule.hold {
            HoldOutputMode::Native | HoldOutputMode::Keystroke(_) => {
                self.release_emitted(key, out);
            }
            HoldOutputMode::Swallow => {}
        }
    }
}
