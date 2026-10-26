//! Platform-independent half of the mapper loop: gating output behind
//! popups and text, turning engine output into key transitions, and one
//! loop iteration over an abstract input driver. Each OS backend supplies
//! an [`EventSink`], [`SideEffects`] and a [`LoopDriver`].

use super::action::Keystroke;
use super::engine::{Engine, EngineOutput};
use super::key_code::KeyCode;
use super::system::{ProcessInvocation, SystemAction};
use std::time::{Duration, Instant};

/// Gap inserted between a synthetic key-down and key-up so the receiving
/// app sees a non-zero hold duration. A bare down+up pair in a single
/// `EV_SYN` frame is treated as zero-duration by some apps and dropped.
const KEY_TAP_GAP: Duration = Duration::from_millis(1);

/// Longest modifier delay a keystroke outside a macro may wait for. Such
/// keystrokes are emitted synchronously, so the delay blocks all input;
/// macros pace their own steps without blocking and use the full setting.
const MAX_BLOCKING_MOD_DELAY: Duration = Duration::from_millis(50);

/// Longest time output stays held after an action opened a popup when the
/// shell never reports that the popup took the keyboard focus.
const POPUP_HOLD_MAX: Duration = Duration::from_millis(1000);

/// Longest time output stays held behind text that is still being typed;
/// a slower backend then lets the keys through rather than lose them.
const TEXT_HOLD_MAX: Duration = Duration::from_millis(2000);

/// How often the mapper loop checks whether held text has been typed.
const TEXT_HOLD_POLL: Duration = Duration::from_millis(5);

/// Output that follows a popup-opening action, held until the popup has the
/// keyboard focus. Keys typed right after the hotkey would otherwise reach
/// the window the popup is about to cover.
///
/// Output that follows text is held until the text worker has typed it, so
/// keys typed meanwhile land after the text instead of before it.
#[derive(Default)]
pub(super) struct OutputHold {
    held: Vec<EngineOutput>,
    until: Option<Instant>,
    released: bool,
    after_text: Vec<EngineOutput>,
    text_until: Option<Instant>,
}

impl OutputHold {
    /// The popup took the focus: replay held output on the next iteration.
    pub(super) fn release(&mut self) {
        self.released = true;
    }

    /// Stop holding and put held output in front of `buf`.
    pub(super) fn take(&mut self, buf: &mut Vec<EngineOutput>) {
        self.until = None;
        if !self.held.is_empty() {
            let mut held = std::mem::take(&mut self.held);
            held.append(buf);
            *buf = held;
        }
    }

    /// Shutdown must not replay input or actions waiting for popup focus.
    pub(super) fn discard(&mut self) {
        self.held.clear();
        self.until = None;
        self.released = false;
        self.after_text.clear();
        self.text_until = None;
    }

    /// Config changes revoke pending side effects, while key transitions
    /// must remain balanced with presses already emitted before the popup.
    pub(super) fn cancel_actions(&mut self, buf: &mut Vec<EngineOutput>) {
        let key_transition = |out: &EngineOutput| {
            !matches!(
                out,
                EngineOutput::Literal(_) | EngineOutput::RunCommand(_) | EngineOutput::RunSystem(_)
            )
        };
        self.held.retain(key_transition);
        self.after_text.retain(key_transition);
        buf.retain(key_transition);
    }

    fn deadline(&self, now: Instant) -> Option<Instant> {
        // Text completion does not wake the loop, so poll for it.
        let text = self.text_until.map(|until| until.min(now + TEXT_HOLD_POLL));
        match (self.until, text) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Leave in `buf` only what may be emitted now. `text_pending` tells
    /// whether text sent to the text worker is still being typed.
    pub(super) fn gate(&mut self, buf: &mut Vec<EngineOutput>, now: Instant, text_pending: bool) {
        self.gate_popup(buf, now);
        self.gate_text(buf, now, text_pending);
    }

    fn gate_popup(&mut self, buf: &mut Vec<EngineOutput>, now: Instant) {
        if self
            .until
            .is_some_and(|until| self.released || now >= until)
        {
            self.take(buf);
        }
        // A release that arrives while nothing is held belongs to no popup.
        self.released = false;
        let rest = if self.until.is_some() {
            std::mem::take(buf)
        } else if let Some(index) = buf
            .iter()
            .position(|out| matches!(out, EngineOutput::RunSystem(action) if action.opens_popup()))
        {
            self.until = Some(now + POPUP_HOLD_MAX);
            buf.split_off(index + 1)
        } else {
            return;
        };
        for out in rest {
            // As for text: releases of keys pressed before the popup pass,
            // so the popup does not open with a modifier still down.
            if is_release(&out) && !releases_held_press(&self.held, &out) {
                buf.push(out);
            } else {
                self.held.push(out);
            }
        }
    }

    fn gate_text(&mut self, buf: &mut Vec<EngineOutput>, now: Instant, text_pending: bool) {
        if self
            .text_until
            .is_some_and(|until| !text_pending || now >= until)
        {
            self.text_until = None;
            let mut held = std::mem::take(&mut self.after_text);
            held.append(buf);
            *buf = held;
        }
        let rest = if self.text_until.is_some() {
            std::mem::take(buf)
        } else if let Some(index) = buf
            .iter()
            .position(|out| matches!(out, EngineOutput::Literal(_)))
        {
            self.text_until = Some(now + TEXT_HOLD_MAX);
            buf.split_off(index + 1)
        } else {
            return;
        };
        for out in rest {
            // Releases of keys pressed before the text pass: a modifier
            // still down in the compositor would make the text fail.
            if is_release(&out) && !releases_held_press(&self.after_text, &out) {
                buf.push(out);
            } else {
                self.after_text.push(out);
            }
        }
    }
}

fn is_release(out: &EngineOutput) -> bool {
    matches!(
        out,
        EngineOutput::KeyRaw { down: false, .. }
            | EngineOutput::ChordRelease { .. }
            | EngineOutput::ReleaseMods(_)
    )
}

/// Whether `release` belongs to a press that is itself held in `held`.
fn releases_held_press(held: &[EngineOutput], release: &EngineOutput) -> bool {
    let pressed = |key: KeyCode| {
        held.iter().any(|out| match out {
            EngineOutput::KeyRaw { key: k, down: true } => *k == key,
            EngineOutput::ChordPress { ks, .. } => ks.key == key || ks.mods.contains(&key),
            _ => false,
        })
    };
    match release {
        EngineOutput::KeyRaw { key, .. } => pressed(*key),
        EngineOutput::ChordRelease { key, .. } => pressed(*key),
        EngineOutput::ReleaseMods(mods) => mods.iter().any(|key| pressed(*key)),
        _ => false,
    }
}

/// Receives key transitions (`true` = press) that must reach the OS
/// together, in order.
pub(super) trait EventSink {
    fn emit(&mut self, events: &[(KeyCode, bool)]) -> Result<(), String>;
}

pub(super) trait SideEffects {
    fn held_layer_changed(&mut self, _state: super::layer_state::HeldLayerState) {}
    fn sleep(&mut self, duration: Duration);
    fn type_text(&mut self, text: &str);
    fn text_failed(&mut self) -> bool {
        false
    }
    fn text_pending(&self) -> bool {
        false
    }
    fn run_system(&mut self, action: &SystemAction);
    fn run_command(&mut self, command: &ProcessInvocation);
}

/// One input from a [`LoopDriver`], in arrival order.
pub(super) enum DriverInput {
    Key {
        key: KeyCode,
        down: bool,
        at: Instant,
    },
    /// Auto-repeat of a held key, for an OS that does not repeat injected
    /// keys itself.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Repeat(KeyCode),
}

pub(super) trait LoopDriver {
    fn now(&mut self) -> Instant;
    fn wait(&mut self, timeout: Duration) -> Result<bool, String>;
    fn fetch_key_events(&mut self) -> Result<Vec<(KeyCode, bool)>, String>;
    fn fetch_timed_key_events(&mut self) -> Result<Vec<(KeyCode, bool, Instant)>, String> {
        let events = self.fetch_key_events()?;
        Ok(events
            .into_iter()
            .map(|(key, down)| (key, down, self.now()))
            .collect())
    }
    /// Intercept a platform hotkey before it reaches the engine and OS.
    /// `Some(None)` consumes a release; `Some(Some(action))` runs an app action.
    fn app_hotkey(&mut self, _key: KeyCode, _down: bool) -> Option<Option<&'static str>> {
        None
    }
    /// Everything received since the last fetch, repeats in their place
    /// among the transitions.
    fn fetch_inputs(&mut self) -> Result<Vec<DriverInput>, String> {
        Ok(self
            .fetch_timed_key_events()?
            .into_iter()
            .map(|(key, down, at)| DriverInput::Key { key, down, at })
            .collect())
    }
}

pub(super) fn process_iteration_with<D: LoopDriver, S: EventSink, E: SideEffects>(
    driver: &mut D,
    engine: &mut Engine,
    hold: &mut OutputHold,
    sink: &mut S,
    effects: &mut E,
    out_buf: &mut Vec<EngineOutput>,
) -> Result<(), String> {
    const MAX_WAIT: Duration = Duration::from_millis(100);

    let now = driver.now();
    let pending = effects.text_pending();
    if effects.text_failed() {
        engine.text_input_failed(out_buf);
    }
    engine.set_text_pending(pending, now);
    let mut wait = engine
        .next_deadline(now)
        .map(|d| d.min(MAX_WAIT))
        .unwrap_or(MAX_WAIT);
    if let Some(until) = hold.deadline(now) {
        wait = wait.min(until.saturating_duration_since(now));
    }

    if driver.wait(wait)? {
        for input in driver.fetch_inputs()? {
            match input {
                DriverInput::Key { key, down, at } => {
                    match driver.app_hotkey(key, down) {
                        Some(Some(action)) => engine.execute_remote(action, out_buf),
                        Some(None) => {}
                        None => engine.handle_key_event(key, down, at, out_buf),
                    }
                    effects.held_layer_changed(engine.held_layer_state());
                }
                DriverInput::Repeat(key) => engine.repeat(key, out_buf),
            }
        }
    }

    let now = driver.now();
    engine.tick(now, out_buf);
    effects.held_layer_changed(engine.held_layer_state());
    hold.gate(out_buf, now, effects.text_pending());
    flush_out_with(sink, effects, out_buf)?;
    Ok(())
}

fn emit_stroke_tap<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    ks: &Keystroke,
    mod_delay: Duration,
) -> Result<(), String> {
    let mod_delay = mod_delay.min(MAX_BLOCKING_MOD_DELAY);
    if !ks.mods.is_empty() {
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in &ks.mods {
            ev.push((*m, true));
        }
        sink.emit(&ev)?;
        effects.sleep(mod_delay);
    }

    // Down and up go in separate `EV_SYN` frames (with a tiny gap) so the
    // tap isn't seen as a zero-duration event and dropped.
    sink.emit(&[(ks.key, true)])?;
    effects.sleep(KEY_TAP_GAP);
    sink.emit(&[(ks.key, false)])?;

    if !ks.mods.is_empty() {
        effects.sleep(mod_delay);
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in ks.mods.iter().rev() {
            ev.push((*m, false));
        }
        sink.emit(&ev)?;
    }

    Ok(())
}

fn emit_chord_press<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    ks: &Keystroke,
    mod_delay: Duration,
) -> Result<(), String> {
    let mod_delay = mod_delay.min(MAX_BLOCKING_MOD_DELAY);
    if !ks.mods.is_empty() {
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in &ks.mods {
            ev.push((*m, true));
        }
        sink.emit(&ev)?;
        effects.sleep(mod_delay);
    }
    let down = [(ks.key, true)];
    sink.emit(&down)?;
    Ok(())
}

fn emit_chord_release<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    key: KeyCode,
    mods: &[KeyCode],
    mod_delay: Duration,
) -> Result<(), String> {
    let mod_delay = mod_delay.min(MAX_BLOCKING_MOD_DELAY);
    let up = [(key, false)];
    sink.emit(&up)?;

    if !mods.is_empty() {
        effects.sleep(mod_delay);
        let mut ev = Vec::with_capacity(mods.len());
        for m in mods.iter().rev() {
            ev.push((*m, false));
        }
        sink.emit(&ev)?;
    }
    Ok(())
}

pub(super) fn flush_out_with<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    buf: &mut Vec<EngineOutput>,
) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }
    // We emit in chunks: most EngineOutput variants go into a single atomic batch, while
    // chords, literals, and side effects need ordered flush boundaries.
    let mut events: Vec<(KeyCode, bool)> = Vec::with_capacity(buf.len() * 3);

    fn flush_events<S: EventSink>(
        sink: &mut S,
        events: &mut Vec<(KeyCode, bool)>,
    ) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        sink.emit(events)?;
        events.clear();
        Ok(())
    }

    for out in buf.drain(..) {
        match out {
            EngineOutput::KeyRaw { key, down } => {
                // Mouse buttons are not grabbed, so they already reach the OS.
                // Re-emitting them would create duplicates.
                let code = key.code();
                if (272..=281).contains(&code) {
                    continue;
                }
                // Two transitions of the same key must not share one
                // `EV_SYN` frame — some apps coalesce them and lose the
                // first transition (a press+release becomes a no-op).
                if events.iter().any(|ev| ev.0.code() == code) {
                    flush_events(sink, &mut events)?;
                }
                events.push((key, down));
            }
            EngineOutput::Stroke { ks, mod_delay } => {
                flush_events(sink, &mut events)?;
                emit_stroke_tap(sink, effects, &ks, mod_delay)?;
            }
            EngineOutput::ChordPress { ks, mod_delay } => {
                flush_events(sink, &mut events)?;
                emit_chord_press(sink, effects, &ks, mod_delay)?;
            }
            EngineOutput::ChordRelease {
                key,
                mods,
                mod_delay,
            } => {
                flush_events(sink, &mut events)?;
                emit_chord_release(sink, effects, key, &mods, mod_delay)?;
            }
            EngineOutput::ReleaseMods(mods) => {
                for m in mods {
                    let code = m.code();
                    // Same rule as KeyRaw: two transitions of one key must
                    // not share an `EV_SYN` frame or they may coalesce.
                    if events.iter().any(|ev| ev.0.code() == code) {
                        flush_events(sink, &mut events)?;
                    }
                    events.push((m, false));
                }
            }
            EngineOutput::RunSystem(action) => {
                flush_events(sink, &mut events)?;
                effects.run_system(&action);
            }
            EngineOutput::RunCommand(command) => {
                flush_events(sink, &mut events)?;
                effects.run_command(&command);
            }
            EngineOutput::Literal(text) => {
                flush_events(sink, &mut events)?;
                effects.type_text(&text);
            }
        }
    }
    flush_events(sink, &mut events)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::config::{ActionSpec, MapperConfig, Rule, Settings};
    use std::collections::{HashMap, VecDeque};
    #[derive(Default)]
    struct FakeSink {
        batches: Vec<Vec<(u16, i32)>>,
    }

    impl EventSink for FakeSink {
        fn emit(&mut self, events: &[(KeyCode, bool)]) -> Result<(), String> {
            self.batches.push(
                events
                    .iter()
                    .map(|&(key, down)| (key.code(), i32::from(down)))
                    .collect(),
            );
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeEffects {
        sleeps: Vec<Duration>,
        texts: Vec<String>,
        systems: Vec<String>,
        commands: Vec<String>,
        held_layers: Vec<crate::mapper::layer_state::HeldLayerState>,
        text_pending: bool,
    }

    impl SideEffects for FakeEffects {
        fn text_pending(&self) -> bool {
            self.text_pending
        }

        fn held_layer_changed(&mut self, state: crate::mapper::layer_state::HeldLayerState) {
            self.held_layers.push(state);
        }

        fn sleep(&mut self, duration: Duration) {
            if !duration.is_zero() {
                self.sleeps.push(duration);
            }
        }

        fn type_text(&mut self, text: &str) {
            self.texts.push(text.to_string());
        }

        fn run_system(&mut self, action: &SystemAction) {
            let label = match action {
                SystemAction::Spawn(ProcessInvocation { program, args, .. }) => {
                    format!("spawn:{program}:{args:?}")
                }
                #[cfg(target_os = "linux")]
                SystemAction::Dbus(call) => format!("dbus:{}:{}", call.destination, call.method),
                #[cfg(target_os = "windows")]
                SystemAction::Windows(action) => format!("windows:{action:?}"),
                SystemAction::AppEvent(event) => format!("app:{event}"),
            };
            self.systems.push(label);
        }

        fn run_command(&mut self, command: &ProcessInvocation) {
            self.commands
                .push(format!("cmd:{}:{:?}", command.program, command.args));
        }
    }

    fn key_events(batch: &[(u16, i32)]) -> Vec<(u16, i32)> {
        batch.to_vec()
    }

    struct FakeDriver {
        now_values: VecDeque<Instant>,
        waits: VecDeque<Result<bool, String>>,
        fetches: VecDeque<Result<Vec<(KeyCode, bool)>, String>>,
        seen_waits: Vec<Duration>,
    }

    impl FakeDriver {
        fn new(
            now_values: Vec<Instant>,
            waits: Vec<Result<bool, String>>,
            fetches: Vec<Result<Vec<(KeyCode, bool)>, String>>,
        ) -> Self {
            Self {
                now_values: now_values.into(),
                waits: waits.into(),
                fetches: fetches.into(),
                seen_waits: Vec::new(),
            }
        }
    }

    impl LoopDriver for FakeDriver {
        fn now(&mut self) -> Instant {
            self.now_values.pop_front().expect("now value")
        }

        fn wait(&mut self, timeout: Duration) -> Result<bool, String> {
            self.seen_waits.push(timeout);
            self.waits.pop_front().expect("wait result")
        }

        fn fetch_key_events(&mut self) -> Result<Vec<(KeyCode, bool)>, String> {
            self.fetches.pop_front().expect("fetch result")
        }
    }

    fn empty_cfg() -> MapperConfig {
        MapperConfig {
            rules: Vec::new(),
            layer_keymaps: HashMap::new(),
            macros: Vec::new(),
            commands: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn flush_out_routes_key_batches_literals_and_systems() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut buf = vec![
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_A,
                down: true,
            },
            EngineOutput::ReleaseMods(vec![KeyCode::KEY_LEFTSHIFT]),
            EngineOutput::Stroke {
                ks: Keystroke {
                    mods: vec![KeyCode::KEY_LEFTCTRL],
                    key: KeyCode::KEY_C,
                },
                mod_delay: Duration::from_millis(2),
            },
            EngineOutput::Literal("tail".into()),
            EngineOutput::RunSystem(SystemAction::Spawn(ProcessInvocation {
                program: "fire".into(),
                args: vec!["now".into()],
                working_directory: None,
            })),
            EngineOutput::RunCommand(ProcessInvocation {
                program: "sh".into(),
                working_directory: None,
                args: vec!["-lc".into(), "notify-send done".into()],
            }),
        ];

        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");

        assert!(buf.is_empty());
        assert_eq!(sink.batches.len(), 5);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![
                (KeyCode::KEY_A.code(), 1),
                (KeyCode::KEY_LEFTSHIFT.code(), 0)
            ]
        );
        assert_eq!(
            key_events(&sink.batches[1]),
            vec![(KeyCode::KEY_LEFTCTRL.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[2]),
            vec![(KeyCode::KEY_C.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[3]),
            vec![(KeyCode::KEY_C.code(), 0)]
        );
        assert_eq!(
            key_events(&sink.batches[4]),
            vec![(KeyCode::KEY_LEFTCTRL.code(), 0)]
        );
        assert_eq!(
            effects.sleeps,
            vec![
                Duration::from_millis(2),
                Duration::from_millis(1),
                Duration::from_millis(2)
            ]
        );
        assert_eq!(effects.texts, vec!["tail".to_string()]);
        assert_eq!(effects.systems, vec!["spawn:fire:[\"now\"]".to_string()]);
        assert_eq!(
            effects.commands,
            vec!["cmd:sh:[\"-lc\", \"notify-send done\"]".to_string()]
        );
    }

    #[test]
    fn flush_out_preserves_chord_press_and_release_order() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut buf = vec![
            EngineOutput::ChordPress {
                ks: Keystroke {
                    mods: vec![KeyCode::KEY_LEFTALT],
                    key: KeyCode::KEY_TAB,
                },
                mod_delay: Duration::from_millis(3),
            },
            EngineOutput::ChordRelease {
                key: KeyCode::KEY_TAB,
                mods: vec![KeyCode::KEY_LEFTALT],
                mod_delay: Duration::from_millis(3),
            },
        ];

        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");

        assert_eq!(sink.batches.len(), 4);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(KeyCode::KEY_LEFTALT.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[1]),
            vec![(KeyCode::KEY_TAB.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[2]),
            vec![(KeyCode::KEY_TAB.code(), 0)]
        );
        assert_eq!(
            key_events(&sink.batches[3]),
            vec![(KeyCode::KEY_LEFTALT.code(), 0)]
        );
        assert_eq!(
            effects.sleeps,
            vec![Duration::from_millis(3), Duration::from_millis(3)]
        );
    }

    /// Delivers one batch of inputs, repeats included.
    struct InputDriver(Vec<DriverInput>);

    impl LoopDriver for InputDriver {
        fn now(&mut self) -> Instant {
            Instant::now()
        }

        fn wait(&mut self, _timeout: Duration) -> Result<bool, String> {
            Ok(!self.0.is_empty())
        }

        fn fetch_key_events(&mut self) -> Result<Vec<(KeyCode, bool)>, String> {
            unreachable!("fetch_inputs is overridden")
        }

        fn fetch_inputs(&mut self) -> Result<Vec<DriverInput>, String> {
            Ok(std::mem::take(&mut self.0))
        }
    }

    #[test]
    fn repeat_is_handled_in_its_place_among_transitions() {
        let at = Instant::now();
        let key = |key, down| DriverInput::Key { key, down, at };
        // A repeat of A that arrived before its release repeats A; a repeat
        // handled after the whole batch would come after the release.
        let mut driver = InputDriver(vec![
            key(KeyCode::KEY_A, true),
            DriverInput::Repeat(KeyCode::KEY_A),
            key(KeyCode::KEY_A, false),
            key(KeyCode::KEY_B, true),
            key(KeyCode::KEY_B, false),
        ]);
        let mut engine = Engine::new(&empty_cfg());
        let mut sink = FakeSink::default();
        process_iteration_with(
            &mut driver,
            &mut engine,
            &mut OutputHold::default(),
            &mut sink,
            &mut FakeEffects::default(),
            &mut Vec::new(),
        )
        .expect("iteration");
        let events: Vec<_> = sink.batches.iter().flat_map(|b| key_events(b)).collect();
        let (a, b) = (KeyCode::KEY_A.code(), KeyCode::KEY_B.code());
        assert_eq!(events, vec![(a, 1), (a, 1), (a, 0), (b, 1), (b, 0)]);
    }

    #[test]
    fn process_iteration_filters_repeats_at_driver_boundary_and_flushes_press_release() {
        let start = Instant::now();
        let mut driver = FakeDriver::new(
            vec![start, start, start, start],
            vec![Ok(true)],
            vec![Ok(vec![(KeyCode::KEY_A, true), (KeyCode::KEY_A, false)])],
        );
        let mut engine = Engine::new(&empty_cfg());
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut out_buf = Vec::new();

        process_iteration_with(
            &mut driver,
            &mut engine,
            &mut OutputHold::default(),
            &mut sink,
            &mut effects,
            &mut out_buf,
        )
        .expect("iteration");

        assert_eq!(driver.seen_waits, vec![Duration::from_millis(100)]);
        assert_eq!(sink.batches.len(), 2);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(KeyCode::KEY_A.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[1]),
            vec![(KeyCode::KEY_A.code(), 0)]
        );
        assert!(effects.texts.is_empty());
        assert!(out_buf.is_empty());
    }

    #[test]
    fn output_after_popup_action_waits_for_focus_or_timeout() {
        let start = Instant::now();
        let key_a = |down| EngineOutput::KeyRaw {
            key: KeyCode::KEY_A,
            down,
        };
        let mut hold = OutputHold::default();
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        fn run(
            hold: &mut OutputHold,
            sink: &mut FakeSink,
            effects: &mut FakeEffects,
            now: Instant,
            mut out_buf: Vec<EngineOutput>,
        ) -> Duration {
            let mut driver = FakeDriver::new(vec![now, now], vec![Ok(false)], Vec::new());
            let mut engine = Engine::new(&empty_cfg());
            process_iteration_with(&mut driver, &mut engine, hold, sink, effects, &mut out_buf)
                .expect("iteration");
            driver.seen_waits[0]
        }
        macro_rules! iterate {
            ($hold:expr, $now:expr, $out:expr) => {
                run($hold, &mut sink, &mut effects, $now, $out)
            };
        }

        let popup = EngineOutput::RunSystem(SystemAction::AppEvent("show_quick_menu_1".into()));
        iterate!(&mut hold, start, vec![popup, key_a(true)]);
        let wait = iterate!(&mut hold, start, vec![key_a(false)]);
        assert_eq!(effects.systems, vec!["app:show_quick_menu_1"]);
        assert!(sink.batches.is_empty(), "keys wait for the popup");
        assert_eq!(wait, Duration::from_millis(100));

        hold.release();
        iterate!(&mut hold, start + Duration::from_millis(5), Vec::new());
        assert_eq!(
            sink.batches,
            vec![
                vec![(KeyCode::KEY_A.code(), 1)],
                vec![(KeyCode::KEY_A.code(), 0)]
            ]
        );

        let popup = EngineOutput::RunSystem(SystemAction::AppEvent("show_emoji_menu_last".into()));
        iterate!(&mut hold, start, vec![popup, key_a(true)]);
        assert_eq!(sink.batches.len(), 2);
        iterate!(&mut hold, start + Duration::from_secs(1), Vec::new());
        assert_eq!(sink.batches.len(), 3, "released after the timeout");

        // Other app actions and releases with nothing held change nothing.
        hold.release();
        let settings = EngineOutput::RunSystem(SystemAction::AppEvent("show_settings".into()));
        iterate!(&mut hold, start, vec![settings, key_a(false)]);
        assert_eq!(sink.batches.len(), 4);
    }

    #[test]
    fn popup_hold_lets_releases_of_earlier_presses_through() {
        let key = |key, down| EngineOutput::KeyRaw { key, down };
        let start = Instant::now();
        let mut hold = OutputHold::default();
        let popup = EngineOutput::RunSystem(SystemAction::AppEvent("show_quick_menu_1".into()));
        let mut buf = vec![
            popup,
            key(KeyCode::KEY_B, true),
            key(KeyCode::KEY_LEFTCTRL, false),
            EngineOutput::ReleaseMods(vec![KeyCode::KEY_LEFTALT]),
        ];
        hold.gate(&mut buf, start, false);
        assert!(matches!(
            buf.as_slice(),
            [
                EngineOutput::RunSystem(_),
                EngineOutput::KeyRaw {
                    key: KeyCode::KEY_LEFTCTRL,
                    down: false
                },
                EngineOutput::ReleaseMods(_)
            ]
        ));
        // A release of a press that is itself held stays behind it.
        let mut buf = vec![
            key(KeyCode::KEY_B, false),
            key(KeyCode::KEY_LEFTSHIFT, false),
        ];
        hold.gate(&mut buf, start, false);
        assert!(matches!(
            buf.as_slice(),
            [EngineOutput::KeyRaw {
                key: KeyCode::KEY_LEFTSHIFT,
                down: false
            }]
        ));
        hold.release();
        let mut buf = Vec::new();
        hold.gate(&mut buf, start, false);
        assert_eq!(
            transitions_of(&buf),
            [(KeyCode::KEY_B, true), (KeyCode::KEY_B, false)]
        );
    }

    fn transitions_of(buf: &[EngineOutput]) -> Vec<(KeyCode, bool)> {
        buf.iter()
            .filter_map(|out| match out {
                EngineOutput::KeyRaw { key, down } => Some((*key, *down)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn modifier_delay_outside_macros_never_blocks_input_for_long() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let ks = Keystroke {
            mods: vec![KeyCode::KEY_LEFTCTRL],
            key: KeyCode::KEY_C,
        };
        let long = Duration::from_millis(2000);
        let mut buf = vec![
            EngineOutput::Stroke {
                ks: ks.clone(),
                mod_delay: long,
            },
            EngineOutput::ChordPress {
                ks,
                mod_delay: long,
            },
            EngineOutput::ChordRelease {
                key: KeyCode::KEY_C,
                mods: vec![KeyCode::KEY_LEFTCTRL],
                mod_delay: long,
            },
        ];
        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");
        assert!(!effects.sleeps.is_empty());
        assert!(effects.sleeps.iter().all(|d| *d <= MAX_BLOCKING_MOD_DELAY));
    }

    #[test]
    fn output_after_text_waits_until_it_is_typed() {
        let start = Instant::now();
        let key = |key, down| EngineOutput::KeyRaw { key, down };
        let mut hold = OutputHold::default();
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        fn run(
            hold: &mut OutputHold,
            sink: &mut FakeSink,
            effects: &mut FakeEffects,
            now: Instant,
            mut out_buf: Vec<EngineOutput>,
        ) -> Duration {
            let mut driver = FakeDriver::new(vec![now, now], vec![Ok(false)], Vec::new());
            let mut engine = Engine::new(&empty_cfg());
            process_iteration_with(&mut driver, &mut engine, hold, sink, effects, &mut out_buf)
                .expect("iteration");
            driver.seen_waits[0]
        }
        macro_rules! iterate {
            ($now:expr, $out:expr $(,)?) => {
                run(&mut hold, &mut sink, &mut effects, $now, $out)
            };
        }

        // Shift was pressed before the text, A after it.
        iterate!(
            start,
            vec![
                EngineOutput::Literal("🙂".into()),
                key(KeyCode::KEY_A, true),
                key(KeyCode::KEY_LEFTSHIFT, false),
            ],
        );
        effects.text_pending = true;
        let wait = iterate!(start, vec![key(KeyCode::KEY_A, false)],);
        assert_eq!(effects.texts, vec!["🙂"]);
        assert_eq!(
            sink.batches,
            vec![vec![(KeyCode::KEY_LEFTSHIFT.code(), 0)]],
            "only the earlier press is released"
        );
        assert_eq!(wait, Duration::from_millis(5), "polls for the text");

        effects.text_pending = false;
        iterate!(start, Vec::new());
        assert_eq!(
            sink.batches[1..],
            [
                vec![(KeyCode::KEY_A.code(), 1)],
                vec![(KeyCode::KEY_A.code(), 0)]
            ]
        );

        effects.text_pending = true;
        iterate!(
            start,
            vec![EngineOutput::Literal("x".into()), key(KeyCode::KEY_B, true)],
        );
        iterate!(start, Vec::new());
        assert_eq!(sink.batches.len(), 3);
        iterate!(start + Duration::from_secs(2), Vec::new(),);
        assert_eq!(sink.batches.len(), 4, "released after the timeout");
    }

    #[test]
    fn flush_out_filters_mouse_buttons() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut buf = vec![
            EngineOutput::KeyRaw {
                key: KeyCode::BTN_LEFT,
                down: true,
            },
            EngineOutput::KeyRaw {
                key: KeyCode::BTN_RIGHT,
                down: true,
            },
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_A,
                down: true,
            },
        ];

        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");

        // Mouse buttons in range 272..=281 are filtered out
        assert_eq!(sink.batches.len(), 1);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(KeyCode::KEY_A.code(), 1)]
        );
    }

    #[test]
    fn flush_out_splits_same_key_code_to_avoid_coalesce() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut buf = vec![
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_A,
                down: true,
            },
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_A,
                down: false,
            },
        ];

        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");

        assert_eq!(sink.batches.len(), 2);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(KeyCode::KEY_A.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[1]),
            vec![(KeyCode::KEY_A.code(), 0)]
        );
    }

    #[test]
    fn process_iteration_uses_tick_to_commit_hold_without_input_ready() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "AltLeft".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("Escape".into()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Action("ControlLeft".into()),
            hold_timeout_ms: Some(10),
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut engine = Engine::new(&cfg);
        let mut pending = Vec::new();
        let start = Instant::now();
        engine.handle_key_event(KeyCode::KEY_LEFTALT, true, start, &mut pending);

        let mut driver = FakeDriver::new(
            vec![start, start + Duration::from_millis(20)],
            vec![Ok(false)],
            Vec::new(),
        );
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();

        process_iteration_with(
            &mut driver,
            &mut engine,
            &mut OutputHold::default(),
            &mut sink,
            &mut effects,
            &mut pending,
        )
        .expect("iteration");

        assert_eq!(driver.seen_waits, vec![Duration::from_millis(10)]);
        assert_eq!(sink.batches.len(), 1);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(KeyCode::KEY_LEFTCTRL.code(), 1)]
        );
    }

    #[test]
    fn one_input_batch_reports_release_repress_and_usage_separately() {
        let cfg: MapperConfig = serde_json::from_value(serde_json::json!({"rules": [
            {"key": "Space", "layerId": "nav", "tapAction": null}
        ]}))
        .unwrap();
        let now = Instant::now();
        let mut driver = FakeDriver::new(
            vec![now; 6],
            vec![Ok(true)],
            vec![Ok(vec![
                (KeyCode::KEY_SPACE, true),
                (KeyCode::KEY_SPACE, false),
                (KeyCode::KEY_SPACE, true),
                (KeyCode::KEY_J, true),
            ])],
        );
        let mut engine = Engine::new(&cfg);
        let mut effects = FakeEffects::default();
        process_iteration_with(
            &mut driver,
            &mut engine,
            &mut OutputHold::default(),
            &mut FakeSink::default(),
            &mut effects,
            &mut Vec::new(),
        )
        .unwrap();
        let states = &effects.held_layers;
        assert_eq!(states.len(), 5);
        assert_eq!(states[0].layer.as_deref(), Some("nav"));
        assert!(states[1].layer.is_none());
        assert_eq!(states[2].layer, states[0].layer);
        assert_ne!(states[2].hold_id, states[0].hold_id);
        assert!(!states[2].used);
        assert!(states[3].used);
        assert!(states[4].used);
    }

    #[test]
    fn config_changes_revoke_buffered_actions_without_losing_key_releases() {
        let command = || {
            EngineOutput::RunCommand(ProcessInvocation {
                program: "sh".into(),
                args: vec!["-c".into(), "true".into()],
                working_directory: None,
            })
        };
        let mut hold = OutputHold::default();
        let mut out = vec![
            EngineOutput::RunSystem(SystemAction::AppEvent("show_quick_menu_1".into())),
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_B,
                down: true,
            },
            EngineOutput::KeyRaw {
                key: KeyCode::KEY_LEFTCTRL,
                down: false,
            },
            EngineOutput::Literal("stale".into()),
            command(),
            EngineOutput::RunSystem(SystemAction::AppEvent("game_mode_toggle".into())),
        ];
        let now = Instant::now();
        hold.gate(&mut out, now, false);
        // The release of an earlier press is not held for the popup.
        assert!(matches!(
            out.as_slice(),
            [
                EngineOutput::RunSystem(_),
                EngineOutput::KeyRaw {
                    key: KeyCode::KEY_LEFTCTRL,
                    down: false
                }
            ]
        ));
        out.clear();
        out.push(command());
        hold.cancel_actions(&mut out);
        hold.release();
        hold.gate(&mut out, now, false);
        assert!(matches!(
            out.as_slice(),
            [EngineOutput::KeyRaw {
                key: KeyCode::KEY_B,
                down: true
            }]
        ));
    }

    #[test]
    fn shutdown_discards_actions_waiting_for_popup_focus() {
        let mut hold = OutputHold::default();
        let mut out = vec![
            EngineOutput::RunSystem(SystemAction::AppEvent("show_quick_menu_1".into())),
            EngineOutput::Literal("must not be typed on shutdown".into()),
        ];
        hold.gate(&mut out, Instant::now(), false);
        out.clear();
        hold.discard();
        hold.take(&mut out);
        assert!(out.is_empty());
    }
}
