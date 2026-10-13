//! Layer holds and tap-versus-hold decisions.

use super::*;

impl Engine {
    pub(super) fn push_layer(&mut self, id: String) {
        self.active_layers.push(id);
    }

    pub(super) fn pop_layer(&mut self, id: &str) {
        // Remove the topmost occurrence.
        if let Some(pos) = self.active_layers.iter().rposition(|l| l == id) {
            self.active_layers.remove(pos);
        }
    }

    /// Promote every rule currently in WaitingDecision to HoldActive
    /// (hold gets committed). Called from `on_press` for any new key to
    /// implement "interrupt on other key press".
    pub(super) fn commit_waiting_decisions(&mut self, out: &mut Vec<Out>) {
        let keys: Vec<KeyCode> = self
            .pending
            .iter()
            .filter(|(_, p)| {
                p.rule.long_hold.is_none() && matches!(p.phase, Phase::WaitingDecision { .. })
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
            (p.rule.long_hold.is_none() && matches!(p.phase, Phase::WaitingDecision { .. }))
                .then_some(*k)
        })
    }

    /// Permissive-hold interception. While a rule key is deciding, other key
    /// events are deferred: a press is buffered; a release whose press is in
    /// the buffer (a nested press+release) resolves the decision as a hold
    /// and lets the buffered events — and this release — replay in order.
    ///
    /// Returns true when the event was consumed (buffered) and `handle`
    /// should return; false to continue normal processing.
    pub(super) fn permissive_intercept(
        &mut self,
        key: KeyCode,
        down: bool,
        now: Instant,
        out: &mut Vec<Out>,
    ) -> bool {
        if !matches!(self.decision_mode, DecisionMode::PermissiveHold) {
            return false;
        }
        let Some(deciding) = self.deciding_key() else {
            return false;
        };
        // The deciding key's own events (its release / repeat) flow normally.
        if key == deciding {
            return false;
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
            // Fall through so this release is processed after the replayed
            // buffer (which re-pressed `key`).
            return false;
        }
        // Release of a key held *before* the decision started does not
        // resolve the decision. But if presses are already buffered behind
        // the decision, this release must stay ordered behind them: a key
        // buffered while this one was still physically down has to replay in
        // that same state. With an empty buffer nothing can be reordered, so
        // let the release flow through immediately.
        if !self.decision_buffer.is_empty() {
            self.decision_buffer
                .push(BufferedEvent { key, down, at: now });
            return true;
        }
        false
    }

    /// Commit the current deciding key as a hold, then replay any buffered
    /// events in order.
    pub(super) fn resolve_deciding_as_hold(&mut self, out: &mut Vec<Out>) {
        if let Some(key) = self.deciding_key() {
            self.commit_hold(key, out);
        }
        self.drain_decision_buffer(out);
    }

    /// Replay all deferred presses/releases in order once a decision has
    /// resolved. Each event carries its original timestamp so downstream
    /// deadlines (a replayed rule key opening its own decision) stay
    /// anchored to when the key was physically pressed.
    pub(super) fn drain_decision_buffer(&mut self, out: &mut Vec<Out>) {
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
    pub(super) fn commit_hold(&mut self, key: KeyCode, out: &mut Vec<Out>) {
        let Some(p) = self.pending.get_mut(&key) else {
            return;
        };
        if !matches!(p.phase, Phase::WaitingDecision { .. }) {
            return;
        }
        let rule = p.rule.clone();
        {
            p.phase = Phase::HoldActive;
        }
        self.commit_hold_with(&rule, key, out);
    }

    pub(super) fn commit_hold_with(&mut self, rule: &RuleEntry, key: KeyCode, out: &mut Vec<Out>) {
        // A lazy hold defers its keystroke: the modifier is materialised later,
        // on the first whitelisted key press inside the layer (see `on_press`),
        // so it never reaches the app as a lone tap.
        let lazy_hold_ks = match &rule.hold {
            HoldMode::Keystroke(ks)
                if rule.layer_id.is_some() && !rule.whitelist_keys.is_empty() =>
            {
                Some(ks.clone())
            }
            _ => None,
        };
        if let Some(id) = &rule.layer_id {
            self.push_layer(id.clone());
            self.layer_triggers
                .entry(id.clone())
                .or_default()
                .push(LayerTrigger {
                    key,
                    isolate_keys: rule.isolate_keys.clone(),
                    whitelist: rule.whitelist_keys.clone(),
                    hold_ks: lazy_hold_ks.clone(),
                });
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
                // Eager hold emits now; a lazy hold waits for a whitelist key.
                if lazy_hold_ks.is_none() {
                    self.emit_stroke_press(key, ks.clone(), out);
                }
            }
        }
    }

    /// Undo whatever `commit_hold_with` did for this rule key.
    pub(super) fn release_hold_with(&mut self, rule: &RuleEntry, key: KeyCode, out: &mut Vec<Out>) {
        if let Some(id) = &rule.layer_id {
            if let Some(triggers) = self.layer_triggers.get_mut(id) {
                if let Some(pos) = triggers.iter().rposition(|trigger| trigger.key == key) {
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
}
