//! State groups the engine is composed of.

use super::model::{ActionDef, ActiveMacro, BufferedEvent, LayerTrigger, MacroDef, Out, RuleEntry};
use crate::mapper::action::Keystroke;
use crate::mapper::key_code::KeyCode;
use crate::mapper::system::SysCommand;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Bindings compiled from the configuration. Immutable between
/// reconfigurations.
pub(super) struct CompiledLayout {
    pub(super) rules: HashMap<KeyCode, Vec<RuleEntry>>,
    pub(super) chord_rules: Vec<(Vec<KeyCode>, RuleEntry)>,
    pub(super) macros: HashMap<String, MacroDef>,
    pub(super) commands: HashMap<String, SysCommand>,
    /// layer_id -> (physical_key -> resolved action)
    pub(super) layer_maps: HashMap<String, HashMap<KeyCode, ActionDef>>,
    /// layer_id -> keys whose keystroke modifiers stay pressed until the
    /// layer's trigger is released.
    pub(super) layer_hold_mods: HashMap<String, HashSet<KeyCode>>,
    pub(super) default_hold: Duration,
    pub(super) default_mod_delay: Duration,
    /// How long a press of a chord member is held back waiting for the rest
    /// of the chord (QMK's `COMBO_TERM`). A chord whose keys arrive within
    /// this window fires without its members ever reaching the app.
    pub(super) combo_term: Duration,
}

/// Chords in progress.
#[derive(Default)]
pub(super) struct ChordState {
    /// Chord owner key -> member keys of the fired chord.
    pub(super) active: HashMap<KeyCode, Vec<KeyCode>>,
    /// Chord owner -> member strokes suppressed when the chord fired;
    /// restored for members still held when it ends.
    pub(super) suppressed: HashMap<KeyCode, Vec<(KeyCode, Keystroke)>>,
    /// Members of a fired chord whose events are swallowed until release.
    pub(super) consumed: HashSet<KeyCode>,
    /// Events deferred while a non-modifier chord member waits for the rest
    /// of its chord (see `combo_term`). Replayed in order when the chord
    /// cannot complete any more; dropped for members when it fires.
    pub(super) buffer: Vec<BufferedEvent>,
}

/// Held layers and the statistics the shell shows for them.
#[derive(Default)]
pub(super) struct LayerStack {
    /// Currently active layers (top = highest priority).
    pub(super) active: Vec<String>,
    /// layer_id -> rules that activated it, oldest to newest
    pub(super) triggers: HashMap<String, Vec<LayerTrigger>>,
    /// Key presses handled while a layer was active.
    pub(super) presses: u64,
    pub(super) hold_id: u64,
    pub(super) used: bool,
}

impl ChordState {
    /// When the buffered combo stops waiting for the rest of its chord.
    pub(super) fn deadline(&self, combo_term: Duration) -> Option<Instant> {
        self.buffer.first().map(|ev| ev.at + combo_term)
    }
}

impl LayerStack {
    pub(super) fn push(&mut self, id: String) {
        if self.active.is_empty() {
            self.hold_id = crate::mapper::layer_state::next_hold_id();
            self.used = false;
        }
        self.active.push(id);
    }

    pub(super) fn pop(&mut self, id: &str) {
        // Remove the topmost occurrence.
        if let Some(pos) = self.active.iter().rposition(|l| l == id) {
            self.active.remove(pos);
        }
    }

    /// The topmost active layer, if any layer is held.
    pub(super) fn top(&self) -> Option<&str> {
        self.active.last().map(String::as_str)
    }

    /// The action the topmost active layer that maps `key` binds to it.
    pub(super) fn lookup(
        &self,
        maps: &HashMap<String, HashMap<KeyCode, ActionDef>>,
        key: KeyCode,
    ) -> Option<(String, ActionDef)> {
        self.active.iter().rev().find_map(|l| {
            maps.get(l)
                .and_then(|m| m.get(&key))
                .map(|def| (l.clone(), def.clone()))
        })
    }
}

/// What the engine holds down on the virtual device.
#[derive(Default)]
pub(super) struct VirtualKeys {
    /// Tracks which physical keys are currently pressed AND what keystroke
    /// we emitted for them, so key-up releases the right virtual keys.
    /// Used both for non-rule remaps (keymap lookups) and for rule-keys
    /// whose hold mode is Native/Keystroke (uniform release path).
    pub(super) emitted: HashMap<KeyCode, Keystroke>,
    /// Physical keys whose press triggered a one-shot macro / literal.
    /// Release just clears the entry — no virtual key release is needed.
    pub(super) oneshot: HashSet<KeyCode>,
    /// Currently "held down" virtual modifiers (refcounted).
    pub(super) mod_refs: HashMap<KeyCode, u32>,
}

impl VirtualKeys {
    pub(super) fn take_releasable_mods(&mut self, mods: &[KeyCode]) -> Vec<KeyCode> {
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
        // (e.g. a rule with `hold: ControlLeft`).
        mods.iter()
            .copied()
            .filter(|m| {
                self.mod_refs.get(m).copied().unwrap_or(0) == 0
                    && !self.emitted.values().any(|ks| ks.key == *m)
            })
            .collect()
    }

    /// True when the key is currently held down on the virtual device —
    /// either refcounted as a chord modifier or held as the main key of
    /// an emitted stroke (hold rules, passthrough keys).
    pub(super) fn is_held(&self, key: KeyCode) -> bool {
        self.mod_refs.get(&key).copied().unwrap_or(0) > 0
            || self
                .emitted
                .values()
                .any(|ks| ks.key == key || ks.mods.contains(&key))
    }

    /// Subset of `mods` that a fired stroke actually needs to press (and
    /// later release): modifiers already held elsewhere are excluded.
    pub(super) fn mods_not_held(&self, mods: &[KeyCode]) -> Vec<KeyCode> {
        mods.iter().copied().filter(|m| !self.is_held(*m)).collect()
    }

    /// Tap `key` unless it is already held down.
    pub(super) fn tap_unless_held(&self, key: KeyCode, out: &mut Vec<Out>) {
        if !self.is_held(key) {
            out.push(Out::KeyRaw { key, down: true });
            out.push(Out::KeyRaw { key, down: false });
        }
    }
}

/// The running macro and the runs queued behind it.
pub(super) struct MacroRunner {
    pub(super) active: Option<ActiveMacro>,
    /// Runs waiting for the active macro, oldest first; at most
    /// `MACRO_QUEUE_LIMIT`.
    pub(super) queue: VecDeque<MacroDef>,
    /// Longest a single macro run may take before it is aborted.
    pub(super) timeout: Duration,
    /// A text step is being injected; the next step waits for it.
    pub(super) text_pending: bool,
}

impl MacroRunner {
    pub(super) fn new(timeout: Duration) -> Self {
        Self {
            active: None,
            queue: VecDeque::new(),
            timeout,
            text_pending: false,
        }
    }
}
