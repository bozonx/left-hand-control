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

    /// Commit the undecided rules a mouse click needs: a held modifier or
    /// keystroke (Shift+click), or a layer that maps `button`.
    pub(super) fn commit_decisions_for_click(&mut self, button: KeyCode, out: &mut Vec<EngineOutput>) {
        let keys: Vec<KeyCode> = self
            .pending
            .iter()
            .filter(|(_, p)| {
                p.rule.long_hold.is_none()
                    && matches!(p.phase, RulePhase::WaitingDecision { .. })
                    && (!matches!(p.rule.hold, HoldOutputMode::Swallow)
                        || p.rule.layer_id.as_ref().is_some_and(|layer| {
                            self.keymap
                                .layer_maps
                                .get(layer)
                                .is_some_and(|keys| keys.contains_key(&button))
                        }))
            })
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            self.commit_hold(k, out);
        }
    }

    /// Resolve every undecided long-hold rule when another key is pressed.
    /// Such rules do not buffer other keys, so waiting for the release
    /// would let the new key overtake the rule key's tap ("b " instead of
    /// " b"). The rule key was pressed first, so its tap — or its long hold,
    /// if that was already due at `now` — is emitted before the new key.
    pub(super) fn resolve_long_hold_decisions(
        &mut self,
        pressed: KeyCode,
        now: Instant,
        out: &mut Vec<EngineOutput>,
    ) {
        let keys: Vec<KeyCode> = self
            .pending
            .iter()
            .filter(|(k, p)| {
                **k != pressed
                    && p.rule.long_hold.is_some()
                    && matches!(p.phase, RulePhase::WaitingDecision { .. })
            })
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            if self
                .long_holds
                .get(&key)
                .is_some_and(|(deadline, _)| now >= *deadline)
                && let Some((_, action)) = self.long_holds.remove(&key)
            {
                self.commit_hold(key, out);
                self.fire_action(Some(&action), self.keymap.default_mod_delay, now, out);
                continue;
            }
            self.long_holds.remove(&key);
            if self.chords.active.contains_key(&key)
                && self
                    .pending
                    .get(&key)
                    .is_some_and(|p| matches!(p.rule.tap, TapMode::NativeChord(_)))
            {
                self.dissolve_chord(key, out);
                continue;
            }
            if let Some(pending) = self.pending.remove(&key) {
                self.fire_tap(key, &pending.rule.tap, now, out);
                // The tap is done; the physical release must emit nothing.
                self.virt.oneshot.insert(key);
            }
        }
        // A long hold whose key went straight to its (swallowed) hold — no
        // tap to decide — is cancelled by typing the same way, unless due.
        let armed: Vec<KeyCode> = self
            .long_holds
            .keys()
            .copied()
            .filter(|k| {
                *k != pressed
                    && self
                        .pending
                        .get(k)
                        .is_some_and(|p| matches!(p.phase, RulePhase::HoldActive))
            })
            .collect();
        for key in armed {
            if let Some((deadline, action)) = self.long_holds.remove(&key)
                && now >= deadline
            {
                self.fire_action(Some(&action), self.keymap.default_mod_delay, now, out);
            }
        }
        // Modifier releases deferred behind the resolved taps.
        if self.deciding_key().is_none() && !self.long_hold_undecided(pressed) {
            self.drain_decision_buffer(out);
        }
    }

    /// True while a long-hold rule key other than `key` is undecided.
    pub(super) fn long_hold_undecided(&self, key: KeyCode) -> bool {
        self.pending.iter().any(|(k, p)| {
            *k != key
                && p.rule.long_hold.is_some()
                && matches!(p.phase, RulePhase::WaitingDecision { .. })
        })
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
            // An undecided long-hold key taps at its release; like any tap it
            // must still see the modifiers held when it was pressed.
            if !down && self.releases_modifier(key) && self.long_hold_undecided(key) {
                return self.defer_release(key, now);
            }
            return false;
        };
        // The deciding key's own events (its release / repeat) flow normally.
        if key == deciding {
            return false;
        }
        if !self.pending.get(&deciding).is_some_and(|p| {
            p.rule.tap_decision == crate::mapper::config::TapDecision::PermissiveHold
        }) {
            // Hold on other key press: a modifier press is deferred, as it
            // only prepares the next key (Space, Shift, A is a space and "A").
            // Any other press commits the hold in `on_press`, after the
            // events deferred before it.
            if down && is_modifier_key(key) {
                self.decision_buffer
                    .push(BufferedEvent { key, down, at: now });
                return true;
            }
            if down {
                if !self.decision_buffer.is_empty() {
                    self.resolve_deciding_as_hold(out);
                    return self.permissive_intercept(key, down, now, out);
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
            // A key the layer leaves out behaves the same with or without the
            // layer: it does not make the press a hold. A modifier only
            // prepares the next key; anything else means the user was typing.
            if is_modifier_key(key)
                && self.layer_ignores(deciding, key)
                && self.decision_buffer.iter().any(|e| {
                    e.key == key && e.down && self.before_deadline(deciding, e.at)
                })
            {
                return self.defer_release(key, now);
            }
            if self.timeout_taps(deciding) {
                self.resolve_deciding_as_tap(deciding, now, out);
                return self.permissive_intercept(key, down, now, out);
            }
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

    /// True when `key` does nothing different inside the layer held by the
    /// undecided layer-only rule `deciding`: the layer does not map it and it
    /// is no rule or chord key of its own (which could stack another layer).
    pub(super) fn layer_ignores(&self, deciding: KeyCode, key: KeyCode) -> bool {
        let Some(pending) = self.pending.get(&deciding) else {
            return false;
        };
        let Some(layer) = &pending.rule.layer_id else {
            return false;
        };
        matches!(pending.rule.hold, HoldOutputMode::Swallow)
            && !self
                .keymap
                .layer_maps
                .get(layer)
                .is_some_and(|keys| keys.contains_key(&key))
            && self.active_rule_for_key(key).is_none()
            && !self
                .keymap
                .chord_rules
                .iter()
                .any(|(keys, rule)| keys.contains(&key) && self.rule_allowed(rule))
    }

    /// True when the hold timeout of the deciding layer key should resolve as
    /// a tap: the first ordinary key buffered behind it is one the layer
    /// ignores, so the press was typing that crossed the timeout.
    pub(super) fn timeout_taps(&self, key: KeyCode) -> bool {
        self.deciding_key() == Some(key)
            && self
                .decision_buffer
                .iter()
                .find(|event| event.down && !is_modifier_key(event.key))
                .is_some_and(|event| {
                    self.before_deadline(key, event.at) && self.layer_ignores(key, event.key)
                })
    }

    /// True when `at` falls before the hold deadline of the undecided `key`:
    /// a press after it arrived when the key was already a hold.
    fn before_deadline(&self, key: KeyCode, at: Instant) -> bool {
        self.pending.get(&key).is_some_and(
            |p| matches!(p.phase, RulePhase::WaitingDecision { deadline } if at < deadline),
        )
    }

    /// Resolve the still-held deciding key as a tap (or the double-tap of a
    /// second press), then replay the buffered events. Its later release
    /// emits nothing.
    pub(super) fn resolve_deciding_as_tap(
        &mut self,
        key: KeyCode,
        now: Instant,
        out: &mut Vec<EngineOutput>,
    ) {
        let Some(pending) = self.pending.remove(&key) else {
            return;
        };
        self.long_holds.remove(&key);
        if pending.second_press {
            self.fire_action(
                pending.rule.double_tap.as_ref(),
                self.keymap.default_mod_delay,
                now,
                out,
            );
        } else {
            self.fire_tap(key, &pending.rule.tap, now, out);
        }
        self.virt.oneshot.insert(key);
        self.drain_decision_buffer(out);
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
        let RulePhase::WaitingDecision { deadline } = p.phase else {
            return;
        };
        let rule = p.rule.clone();
        // A layer key tapped and then held within the double-tap window:
        // the first tap was typing (a space before navigating), not part of
        // the hold, so it is emitted before the layer starts. It happened at
        // the second press, `hold_timeout` before the deadline.
        let first_tap = (p.second_press && rule.layer_id.is_some())
            .then(|| deadline.checked_sub(rule.hold_timeout).unwrap_or(deadline));
        p.phase = RulePhase::HoldActive;
        if let Some(at) = first_tap {
            self.fire_tap(key, &rule.tap, at, out);
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
