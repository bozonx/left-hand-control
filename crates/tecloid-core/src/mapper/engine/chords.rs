//! Chord recognition and deferred trigger-chord events.

use super::*;

impl Engine {
    pub(super) fn handle_chord(
        &mut self,
        key: KeyCode,
        down: bool,
        now: Instant,
        out: &mut Vec<EngineOutput>,
    ) -> bool {
        if !down {
            let owner = self
                .chords
                .active
                .iter()
                .find(|(_, keys)| keys.contains(&key))
                .map(|(owner, _)| *owner);
            if let Some(owner) = owner {
                self.chords.active.remove(&owner);
                self.on_release(owner, now, out);
                for (member, stroke) in self.chords.suppressed.remove(&owner).unwrap_or_default() {
                    if self.physical_down.contains(&member) {
                        self.emit_stroke_press(member, stroke, out);
                    }
                }
                self.release_emitted(key, out);
                self.chords.consumed.remove(&key);
                return true;
            }
            if self.chords.consumed.remove(&key) {
                self.release_emitted(key, out);
                return true;
            }
            return false;
        }
        if self.chords.consumed.contains(&key) {
            return true;
        }
        let buffered = self.buffered_chord_presses();
        let Some((keys, rule)) = self.complete_chord(Some(key), now, &buffered) else {
            return false;
        };
        // A longer chord that contains this one may still complete: wait for
        // it inside the combo window instead of firing the shorter one now
        // (A+S must not pre-empt A+S+D pressed in that order).
        if self.longer_chord_possible(&keys, now) {
            self.chords
                .buffer
                .push(BufferedEvent { key, down, at: now });
            return true;
        }
        self.fire_chord(keys, rule, now, out);
        true
    }

    /// Keys whose presses wait in the combo buffer.
    fn buffered_chord_presses(&self) -> HashSet<KeyCode> {
        self.chords
            .buffer
            .iter()
            .filter(|ev| ev.down)
            .map(|ev| ev.key)
            .collect()
    }

    /// The chord whose members are all down and pressed together at `at`:
    /// `key` (the press being handled), buffered presses, or held keys that
    /// produced no output and were pressed within the combo window.
    fn complete_chord(
        &self,
        key: Option<KeyCode>,
        at: Instant,
        buffered: &HashSet<KeyCode>,
    ) -> Option<(Vec<KeyCode>, RuleEntry)> {
        self.profile
            .chord_rules
            .iter()
            .rev()
            .filter(|(keys, rule)| {
                key.is_none_or(|key| keys.contains(&key))
                    && self.chord_eligible(keys, rule)
                    && keys.iter().all(|member| {
                        Some(*member) == key
                            || buffered.contains(member)
                            || self.chord_member_ready(keys, *member, at)
                    })
                    && (!is_shortcut_chord(keys) || self.only_chord_modifiers_held(keys))
            })
            .max_by_key(|(keys, _)| keys.len())
            .cloned()
    }

    /// A held key that may still join a chord completed at `at`: pressed by
    /// then, within the combo window, and it printed nothing the chord cannot
    /// take back.
    /// A modifier of a shortcut chord counts whenever it was pressed, as in
    /// an ordinary shortcut.
    fn chord_member_ready(&self, keys: &[KeyCode], member: KeyCode, at: Instant) -> bool {
        self.physical_down.contains(&member)
            && self.press_at.get(&member).is_some_and(|pressed| *pressed <= at)
            && !self.chord_member_leaked(member)
            && ((is_modifier_key(member) && is_shortcut_chord(keys))
                || !self.chord_member_late(member, at))
    }

    /// A shortcut chord fires only with exactly its modifiers held, so
    /// `ControlLeft+KeyC` leaves Ctrl+Shift+C to the app.
    fn only_chord_modifiers_held(&self, keys: &[KeyCode]) -> bool {
        self.physical_down
            .iter()
            .all(|key| !is_modifier_key(*key) || keys.contains(key))
    }

    /// True when an eligible chord longer than `keys` contains all of them
    /// and its missing members can still arrive inside the combo window.
    fn longer_chord_possible(&self, keys: &[KeyCode], now: Instant) -> bool {
        if self
            .trigger_chord_deadline()
            .is_some_and(|deadline| now >= deadline)
        {
            return false;
        }
        let buffered = self.buffered_chord_presses();
        let has_main_key = keys.iter().any(|key| !is_modifier_key(*key));
        self.profile.chord_rules.iter().any(|(longer, rule)| {
            // A shortcut's modifiers come before its main key, never after.
            !(has_main_key && is_shortcut_chord(longer))
                && longer.len() > keys.len()
                && keys.iter().all(|key| longer.contains(key))
                && self.chord_eligible(longer, rule)
                && longer.iter().all(|member| {
                    keys.contains(member)
                        || buffered.contains(member)
                        || !self.physical_down.contains(member)
                        || self.chord_member_ready(longer, *member, now)
                })
        })
    }

    /// Claim the members of a completed chord and start its rule. `at` is
    /// when the chord completed.
    fn fire_chord(
        &mut self,
        keys: Vec<KeyCode>,
        rule: RuleEntry,
        at: Instant,
        out: &mut Vec<EngineOutput>,
    ) {
        // Buffered member presses never reached the app — drop them. Other
        // events replay around the chord in their physical order.
        let (before, after): (Vec<_>, Vec<_>) = std::mem::take(&mut self.chords.buffer)
            .into_iter()
            .filter(|ev| !keys.contains(&ev.key))
            .partition(|ev| ev.at <= at);
        for ev in before {
            self.handle_event(ev.key, ev.down, ev.at, out);
        }
        // A completed chord is a different trigger from any member's
        // pending single tap, even inside that member's double-tap window.
        self.flush_waiting_second(at, out);
        let owner = keys[0];
        let mut suppressed = Vec::new();
        self.decision_buffer
            .retain(|event| !keys.contains(&event.key));
        for member in &keys {
            self.long_holds.remove(member);
            let pending = self.pending.remove(member);
            if let Some(pending) = pending
                && matches!(pending.phase, RulePhase::HoldActive)
            {
                self.release_hold_with(&pending.rule, *member, out);
            }
            if let Some(stroke) = self.virt.emitted.get(member).cloned() {
                suppressed.push((*member, stroke));
            }
            self.release_emitted(*member, out);
            self.virt.oneshot.remove(member);
            self.chords.consumed.insert(*member);
        }
        self.chords.active.insert(owner, keys);
        self.chords.suppressed.insert(owner, suppressed);
        self.start_rule(owner, rule, at, out);
        self.drain_decision_buffer(out);
        for ev in after {
            self.handle_event(ev.key, ev.down, ev.at, out);
        }
    }

    /// End a native-tap chord interrupted by another key before it did
    /// anything: its members were simply held together (Shift+ControlRight
    /// for Ctrl+Shift+X), so they are pressed again as themselves, in press
    /// order, and later released through the consumed-member path.
    pub(super) fn dissolve_chord(&mut self, owner: KeyCode, out: &mut Vec<EngineOutput>) {
        self.pending.remove(&owner);
        self.long_holds.remove(&owner);
        let mut members = self.chords.active.remove(&owner).unwrap_or_default();
        let suppressed = self.chords.suppressed.remove(&owner).unwrap_or_default();
        members.retain(|member| self.physical_down.contains(member));
        members.sort_by_key(|member| self.press_at.get(member).copied());
        for member in members {
            let stroke = suppressed
                .iter()
                .find(|(key, _)| *key == member)
                .map(|(_, stroke)| stroke.clone())
                .unwrap_or(Keystroke {
                    mods: Vec::new(),
                    key: member,
                });
            self.emit_stroke_press(member, stroke, out);
        }
    }

    /// Chords, like single-key rules, are transparent through held layers:
    /// they act unless an active layer maps one of their members. They never
    /// reuse a key owned by an active chord and obey the rule's conditions.
    pub(super) fn chord_eligible(&self, keys: &[KeyCode], rule: &RuleEntry) -> bool {
        keys.iter().all(|member| self.lookup_mapping(*member).is_none())
            && !keys
                .iter()
                .any(|member| self.chords.consumed.contains(member))
            // An unrelated deferred press must reach the old context first.
            // Claiming a chord now would overtake it or replay it in the new
            // layer. Keep typing buffered rather than force a tap/hold choice.
            && !self
                .decision_buffer
                .iter()
                .any(|event| event.down && !keys.contains(&event.key))
            // A tap-hold key outside the chord is still undecided: its
            // decision owns the following presses (a nested chord makes it a
            // hold), and firing the chord now would overtake its tap.
            && self.deciding_key().is_none_or(|key| keys.contains(&key))
            && self.rule_allowed(rule)
    }

    /// True when a held chord member already produced visible output (a
    /// passed-through character, a fired action), so the chord can no longer
    /// take it back. Held modifiers are not output: the chord suppresses and
    /// later restores them.
    pub(super) fn chord_member_leaked(&self, member: KeyCode) -> bool {
        !is_modifier_key(member)
            && (self.virt.oneshot.contains(&member)
                || self
                    .virt
                    .emitted
                    .get(&member)
                    .is_some_and(|ks| !is_modifier_key(ks.key)))
    }

    /// True when a held member was pressed at least the trigger chord window
    /// before `at`. The keys were not pressed together: a modifier held for
    /// typing is part of a shortcut (Shift held, then ControlRight for
    /// Ctrl+Shift+X), and an undecided tap-hold key owns the later key as its
    /// own tap or hold (e.g. a layer), as for keys that already leaked. The
    /// boundary matches the combo buffer, which expires at the same instant.
    pub(super) fn chord_member_late(&self, member: KeyCode, at: Instant) -> bool {
        self.press_at.get(&member).is_some_and(|pressed| {
            at.saturating_duration_since(*pressed) >= self.profile.trigger_chord_timeout
        })
    }

    pub(super) fn trigger_chord_deadline(&self) -> Option<Instant> {
        self.chords.deadline(self.profile.trigger_chord_timeout)
    }

    /// A non-modifier key that belongs to an eligible chord starts a combo
    /// window. Modifiers pass through as before: a held modifier prints
    /// nothing and the chord suppresses it if it completes. Shortcut chords
    /// never wait: their main key either completes them at once or types.
    pub(super) fn starts_trigger_chord(&self, key: KeyCode) -> bool {
        !is_modifier_key(key)
            && self.profile.chord_rules.iter().any(|(keys, rule)| {
                keys.contains(&key) && !is_shortcut_chord(keys) && self.chord_eligible(keys, rule)
            })
    }

    /// True when `key` together with every buffered press is still part of
    /// some eligible chord.
    pub(super) fn extends_trigger_chord(&self, key: KeyCode) -> bool {
        self.profile.chord_rules.iter().any(|(keys, rule)| {
            keys.contains(&key)
                && self
                    .chords
                    .buffer
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
    pub(super) fn intercept_trigger_chord(
        &mut self,
        key: KeyCode,
        down: bool,
        now: Instant,
        out: &mut Vec<EngineOutput>,
    ) -> bool {
        if !self.chords.buffer.is_empty() {
            if down && self.extends_trigger_chord(key) {
                self.chords
                    .buffer
                    .push(BufferedEvent { key, down, at: now });
                return true;
            }
            if down || self.chords.buffer.iter().any(|ev| ev.key == key) {
                // A key outside the chord, or a buffered member released
                // before a longer chord completed: the buffer resolves now.
                // A chord it completed owns this member's release.
                if self.flush_trigger_chord_buffer(out) && !down {
                    return self.handle_chord(key, down, now, out);
                }
            } else {
                // Release of a key held before the combo started: keep it
                // ordered behind the buffered presses.
                self.chords
                    .buffer
                    .push(BufferedEvent { key, down, at: now });
                return true;
            }
        }
        if down && self.starts_trigger_chord(key) {
            self.chords
                .buffer
                .push(BufferedEvent { key, down, at: now });
            return true;
        }
        false
    }

    /// Resolve the trigger chord buffer: fire the chord its presses complete
    /// (one that waited for a longer chord), or replay it as ordinary key
    /// events with their original timestamps, so tap/hold deadlines stay
    /// anchored to the physical press. Returns true when a chord fired.
    pub(super) fn flush_trigger_chord_buffer(&mut self, out: &mut Vec<EngineOutput>) -> bool {
        let buffered = self.buffered_chord_presses();
        if let Some(completed) = self
            .chords
            .buffer
            .iter()
            .filter(|ev| ev.down)
            .map(|ev| ev.at)
            .max()
            && let Some((keys, rule)) = self.complete_chord(None, completed, &buffered)
            && keys.iter().any(|key| buffered.contains(key))
        {
            self.fire_chord(keys, rule, completed, out);
            return true;
        }
        for ev in std::mem::take(&mut self.chords.buffer) {
            self.handle_event(ev.key, ev.down, ev.at, out);
        }
        false
    }
}

/// Modifiers plus exactly one other key (`ControlLeft+KeyC`): pressed like an
/// ordinary shortcut, modifiers first, with no combo window.
fn is_shortcut_chord(keys: &[KeyCode]) -> bool {
    let main_keys = keys.iter().filter(|key| !is_modifier_key(**key)).count();
    main_keys == 1 && keys.len() > 1
}
