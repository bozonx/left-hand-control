//! Chord recognition and deferred combo events.

use super::*;

impl Engine {
    pub(super) fn handle_chord(
        &mut self,
        key: KeyCode,
        down: bool,
        now: Instant,
        out: &mut Vec<Out>,
    ) -> bool {
        if !down {
            let owner = self
                .active_chords
                .iter()
                .find(|(_, keys)| keys.contains(&key))
                .map(|(owner, _)| *owner);
            if let Some(owner) = owner {
                self.active_chords.remove(&owner);
                self.on_release(owner, now, out);
                for (member, stroke) in self.chord_suppressed.remove(&owner).unwrap_or_default() {
                    if self.physical_down.contains(&member) {
                        self.emit_stroke_press(member, stroke, out);
                    }
                }
                self.release_emitted(key, out);
                self.chord_consumed.remove(&key);
                return true;
            }
            if self.chord_consumed.remove(&key) {
                self.release_emitted(key, out);
                return true;
            }
            return false;
        }
        if self.chord_consumed.contains(&key) {
            return true;
        }
        let matching = self
            .chord_rules
            .iter()
            .rev()
            .filter(|(keys, rule)| {
                keys.contains(&key)
                    && self.chord_eligible(keys, rule)
                    && keys.iter().all(|member| {
                        self.physical_down.contains(member)
                            && (*member == key || !self.chord_member_leaked(*member))
                    })
            })
            .max_by_key(|(keys, _)| keys.len())
            .cloned();
        let Some((keys, rule)) = matching else {
            return false;
        };
        // Buffered member presses never reached the app — drop them. Anything
        // else in the buffer (releases of unrelated keys) happened before
        // this press, so it replays first.
        for ev in std::mem::take(&mut self.combo_buffer) {
            if !keys.contains(&ev.key) {
                self.handle_event(ev.key, ev.down, ev.at, out);
            }
        }
        let owner = keys[0];
        let previous = self.pending.remove(&owner);
        let mut suppressed = Vec::new();
        self.decision_buffer
            .retain(|event| !keys.contains(&event.key));
        for member in &keys {
            self.long_holds.remove(member);
            let pending = if *member == owner {
                previous.clone()
            } else {
                self.pending.remove(member)
            };
            if let Some(pending) = pending
                && matches!(pending.phase, Phase::HoldActive)
            {
                self.release_hold_with(&pending.rule, *member, out);
            }
            if let Some(stroke) = self.emitted.get(member).cloned() {
                suppressed.push((*member, stroke));
            }
            self.release_emitted(*member, out);
            self.oneshot_consumed.remove(member);
            self.chord_consumed.insert(*member);
        }
        self.active_chords.insert(owner, keys);
        self.chord_suppressed.insert(owner, suppressed);
        if let Some(previous) = previous
            && let Phase::WaitingSecond { deadline } = previous.phase
        {
            if now <= deadline {
                self.fire_action(
                    previous.rule.double_tap.as_ref(),
                    self.default_mod_delay,
                    now,
                    out,
                );
                self.oneshot_consumed.insert(owner);
                return true;
            }
            self.fire_tap(owner, &previous.rule.tap, now, out);
        }
        self.start_rule(owner, rule, now, out);
        self.drain_decision_buffer(out);
        true
    }

    /// Chords, like single-key rules, act only on the base level (no layer
    /// active), never reuse a key owned by an active chord and obey the
    /// rule's conditions.
    pub(super) fn chord_eligible(&self, keys: &[KeyCode], rule: &RuleEntry) -> bool {
        self.active_layers.is_empty()
            && !keys
                .iter()
                .any(|member| self.chord_consumed.contains(member))
            && rule_passes_apps(rule)
            && rule_passes_conditions(rule)
    }

    /// True when a held chord member already produced visible output (a
    /// passed-through character, a fired action), so the chord can no longer
    /// take it back. Held modifiers are not output: the chord suppresses and
    /// later restores them.
    pub(super) fn chord_member_leaked(&self, member: KeyCode) -> bool {
        !is_modifier_key(member)
            && (self.oneshot_consumed.contains(&member)
                || self
                    .emitted
                    .get(&member)
                    .is_some_and(|ks| !is_modifier_key(ks.key)))
    }

    pub(super) fn combo_deadline(&self) -> Option<Instant> {
        self.combo_buffer.first().map(|ev| ev.at + self.combo_term)
    }

    /// A non-modifier key that belongs to an eligible chord starts a combo
    /// window. Modifiers pass through as before: a held modifier prints
    /// nothing and the chord suppresses it if it completes.
    pub(super) fn starts_combo(&self, key: KeyCode) -> bool {
        !is_modifier_key(key)
            && self
                .chord_rules
                .iter()
                .any(|(keys, rule)| keys.contains(&key) && self.chord_eligible(keys, rule))
    }

    /// True when `key` together with every buffered press is still part of
    /// some eligible chord.
    pub(super) fn extends_combo(&self, key: KeyCode) -> bool {
        self.chord_rules.iter().any(|(keys, rule)| {
            keys.contains(&key)
                && self
                    .combo_buffer
                    .iter()
                    .filter(|ev| ev.down)
                    .all(|ev| keys.contains(&ev.key))
                && self.chord_eligible(keys, rule)
        })
    }

    /// Combo interception, run after `handle_chord` found no complete chord.
    /// Presses that may still grow into a chord are buffered; anything that
    /// rules the chord out replays the buffer first so output keeps the
    /// physical order.
    ///
    /// Returns true when the event was consumed (buffered).
    pub(super) fn combo_intercept(
        &mut self,
        key: KeyCode,
        down: bool,
        now: Instant,
        out: &mut Vec<Out>,
    ) -> bool {
        if !self.combo_buffer.is_empty() {
            if down && self.extends_combo(key) {
                self.combo_buffer.push(BufferedEvent { key, down, at: now });
                return true;
            }
            if down || self.combo_buffer.iter().any(|ev| ev.key == key) {
                // A key outside the chord, or a buffered member released
                // before the chord completed: no chord here.
                self.flush_combo_buffer(out);
            } else {
                // Release of a key held before the combo started: keep it
                // ordered behind the buffered presses.
                self.combo_buffer.push(BufferedEvent { key, down, at: now });
                return true;
            }
        }
        if down && self.starts_combo(key) {
            self.combo_buffer.push(BufferedEvent { key, down, at: now });
            return true;
        }
        false
    }

    /// Replay the combo buffer as ordinary key events with their original
    /// timestamps, so tap/hold deadlines stay anchored to the physical press.
    pub(super) fn flush_combo_buffer(&mut self, out: &mut Vec<Out>) {
        for ev in std::mem::take(&mut self.combo_buffer) {
            self.handle_event(ev.key, ev.down, ev.at, out);
        }
    }
}
