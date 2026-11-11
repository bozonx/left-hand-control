//! Macro playback: the running macro and the runs queued behind it.

use super::model::{ActiveMacro, CompiledMacro, EngineOutput, MacroPhase};
use super::report_unavailable;
use super::state::{MacroRunner, VirtualKeys};
use crate::mapper::action::Keystroke;
use std::time::{Duration, Instant};

/// Macro runs that may wait behind the active one; further runs are dropped.
const MACRO_QUEUE_LIMIT: usize = 3;

impl MacroRunner {
    /// Advance the active macro and start a queued one when it finishes.
    pub(super) fn tick(
        &mut self,
        virt: &mut VirtualKeys,
        now: Instant,
        out: &mut Vec<EngineOutput>,
    ) {
        if self.active.as_ref().is_some_and(|am| now >= am.deadline) {
            log::warn!(
                "[mapper] macro exceeded {} ms, aborting",
                self.timeout.as_millis()
            );
            crate::events::emit(crate::events::CoreEvent::MacroRunFailed(
                crate::events::MacroRunFailure::Timeout,
            ));
            self.abort(virt, out);
        }
        if let Some(MacroPhase::Command(job)) = self.active.as_ref().map(|am| &am.phase) {
            match job.result() {
                None => return,
                Some(Err(error)) => {
                    if let Some(am) = &self.active {
                        log::warn!("[mapper] {}: {error}", am.steps[am.current_step].source);
                    }
                    crate::events::emit(crate::events::CoreEvent::MacroRunFailed(
                        crate::events::MacroRunFailure::Command,
                    ));
                    self.cancel(virt, out);
                    return;
                }
                Some(Ok(())) => {
                    if let Some(am) = self.active.as_mut() {
                        am.phase = MacroPhase::NextStep;
                        am.current_step += 1;
                        am.next_wake = next_macro_wake(now, am);
                    }
                }
            }
        }
        if let Some(MacroPhase::Text(job)) = self.active.as_ref().map(|am| &am.phase) {
            match job.result() {
                None => return,
                Some(Err(())) => self.abort(virt, out),
                Some(Ok(())) => {
                    if let Some(am) = self.active.as_mut() {
                        am.phase = MacroPhase::NextStep;
                        am.current_step += 1;
                        am.next_wake = next_macro_wake(now, am);
                    }
                }
            }
        }
        self.start_queued(now);
        if let Some(mut am) = self.active.take() {
            if now >= am.next_wake
                && (!self.text_pending || !matches!(am.phase, MacroPhase::NextStep))
            {
                use crate::mapper::action::MacroStepItem;
                use crate::config::actions::MAX_MACRO_STEPS;
                use MacroPhase;

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
                            for m in virt.take_releasable_mods(&ks.mods).iter().rev() {
                                out.push(EngineOutput::KeyRaw {
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
                            am.mod_delay = step.mod_delay;
                            match &step.item {
                                MacroStepItem::Stroke(ks) => {
                                    #[cfg(target_os = "linux")]
                                    let contextual;
                                    #[cfg(target_os = "linux")]
                                    let ks = if step.source.starts_with("builtin:") {
                                        contextual = crate::text_input::portal::builtin_clipboard_stroke(ks);
                                        &contextual
                                    } else { ks };
                                    // Skip modifiers something else already
                                    // holds down (a `hold:` rule, a chord, a
                                    // passthrough key): pressing them again is
                                    // redundant, and releasing them at the end
                                    // of the stroke would break the hold.
                                    let press = virt.mods_not_held(&ks.mods);
                                    for m in &ks.mods {
                                        *virt.mod_refs.entry(*m).or_insert(0) += 1;
                                    }
                                    if !ks.mods.is_empty() {
                                        for m in &press {
                                            out.push(EngineOutput::KeyRaw {
                                                key: *m,
                                                down: true,
                                            });
                                        }
                                        am.phase = MacroPhase::StrokeModDelayPress(Keystroke {
                                            mods: ks.mods.clone(),
                                            key: ks.key,
                                        });
                                        am.next_wake = now + am.mod_delay;
                                        self.active = Some(am);
                                        break;
                                    } else {
                                        virt.tap_unless_held(ks.key, out);
                                        am.steps_emitted += 1;
                                        am.current_step += 1;
                                        if am.current_step < am.steps.len() {
                                            am.next_wake = next_macro_wake(now, &am);
                                            self.active = Some(am);
                                            break;
                                        }
                                    }
                                }
                                MacroStepItem::System(action) => {
                                    out.push(EngineOutput::RunSystem(action.clone()));
                                    am.steps_emitted += 1;
                                    am.current_step += 1;
                                    if am.current_step < am.steps.len() {
                                        am.next_wake = next_macro_wake(now, &am);
                                        self.active = Some(am);
                                        break;
                                    }
                                }
                                MacroStepItem::Command(command) => {
                                    let job = crate::mapper::commands::MacroCommand::default();
                                    out.push(EngineOutput::RunMacroCommand(
                                        command.clone(),
                                        job.clone(),
                                    ));
                                    am.phase = MacroPhase::Command(job);
                                    am.steps_emitted += 1;
                                    self.active = Some(am);
                                    break;
                                }
                                MacroStepItem::Literal(text) => {
                                    let job = crate::text_input::MacroText::default();
                                    out.push(EngineOutput::RunMacroText(text.clone(), job.clone()));
                                    am.phase = MacroPhase::Text(job);
                                    am.steps_emitted += 1;
                                    self.active = Some(am);
                                    break;
                                }
                                // `run_macro` never starts a run with one.
                                MacroStepItem::Unavailable(_) => {
                                    am.current_step = am.steps.len();
                                }
                                MacroStepItem::Pause(duration) => {
                                    am.steps_emitted += 1;
                                    am.current_step += 1;
                                    am.next_wake = now + *duration;
                                    if am.current_step == am.steps.len() {
                                        am.phase = MacroPhase::FinalPause;
                                    }
                                    self.active = Some(am);
                                    break;
                                }
                            }
                        }
                        MacroPhase::FinalPause | MacroPhase::Command(_) | MacroPhase::Text(_) => break,
                        MacroPhase::StrokeModDelayPress(ref ks) => {
                            let ks = ks.clone();
                            virt.tap_unless_held(ks.key, out);
                            am.phase = MacroPhase::StrokeModDelayRelease(ks.clone());
                            am.next_wake = now + am.mod_delay;
                            self.active = Some(am);
                            break;
                        }
                        MacroPhase::StrokeModDelayRelease(ref ks) => {
                            if matches!(
                                &am.steps[am.current_step].item,
                                crate::mapper::action::MacroStepItem::Stroke(original)
                                    if crate::mapper::action::is_copy_stroke(original)
                            ) {
                                crate::events::emit(crate::events::CoreEvent::CopyCommandSent);
                            }
                            for m in virt.take_releasable_mods(&ks.mods).iter().rev() {
                                out.push(EngineOutput::KeyRaw {
                                    key: *m,
                                    down: false,
                                });
                            }
                            am.phase = MacroPhase::NextStep;
                            am.steps_emitted += 1;
                            am.current_step += 1;
                            if am.current_step < am.steps.len() {
                                am.next_wake = next_macro_wake(now, &am);
                                self.active = Some(am);
                                break;
                            }
                        }
                    }
                }
            } else {
                self.active = Some(am);
            }
        }
        self.start_queued(now);
    }

    /// Time until the next macro step, the macro timeout or the start of a
    /// queued run.
    pub(super) fn next_deadline(&self, now: Instant) -> Option<Duration> {
        let wake = self
            .active
            .as_ref()
            .filter(|am| !self.text_pending || !matches!(am.phase, MacroPhase::NextStep))
            .map(|am| {
                if matches!(am.phase, MacroPhase::Command(_) | MacroPhase::Text(_)) {
                    Duration::from_millis(5)
                } else {
                    am.next_wake.saturating_duration_since(now)
                }
            });
        let timeout = self
            .active
            .as_ref()
            .map(|am| am.deadline.saturating_duration_since(now));
        // A queued run starts on the next tick.
        let queued = (self.active.is_none() && !self.queue.is_empty()).then_some(Duration::ZERO);
        [wake, timeout, queued].into_iter().flatten().min()
    }

    /// Resume macro timing only after the text worker has completed injection.
    pub(super) fn set_text_pending(&mut self, pending: bool, _now: Instant) {
        self.text_pending = pending;
    }

    /// Abort the in-flight macro (if any). A stroke step presses its
    /// modifiers first and releases them only in a later phase, so an
    /// abort mid-stroke must release them here or they stay stuck down.
    pub(super) fn abort(&mut self, virt: &mut VirtualKeys, out: &mut Vec<EngineOutput>) {
        let Some(am) = self.active.take() else {
            return;
        };
        match am.phase {
            MacroPhase::StrokeModDelayPress(ks) | MacroPhase::StrokeModDelayRelease(ks) => {
                for m in virt.take_releasable_mods(&ks.mods).iter().rev() {
                    out.push(EngineOutput::KeyRaw {
                        key: *m,
                        down: false,
                    });
                }
            }
            MacroPhase::Command(job) => job.cancel(),
            MacroPhase::Text(job) => job.cancel(),
            MacroPhase::NextStep | MacroPhase::FinalPause => {}
        }
    }

    /// Abort the active macro and drop the queued runs.
    pub(super) fn cancel(&mut self, virt: &mut VirtualKeys, out: &mut Vec<EngineOutput>) {
        self.queue.clear();
        self.abort(virt, out);
    }

    /// Start a macro run, or queue it behind the active one: a run that is
    /// cut short leaves the application half-edited.
    pub(super) fn run(&mut self, md: CompiledMacro, now: Instant) {
        if md.steps.is_empty() {
            return;
        }
        // A step this system cannot run would leave the macro half done.
        if let Some(action) = md.steps.iter().find_map(|step| match &step.item {
            crate::mapper::action::MacroStepItem::Unavailable(action) => Some(action),
            _ => None,
        }) {
            report_unavailable(action);
            return;
        }
        if self.active.is_none() {
            self.active = Some(self.begin(md, now));
        } else if self.queue.len() < MACRO_QUEUE_LIMIT {
            self.queue.push_back(md);
        } else {
            crate::events::emit(crate::events::CoreEvent::MacroRunFailed(
                crate::events::MacroRunFailure::QueueFull,
            ));
        }
    }

    fn start_queued(&mut self, now: Instant) {
        if self.active.is_none()
            && let Some(md) = self.queue.pop_front()
        {
            self.active = Some(self.begin(md, now));
        }
    }

    fn begin(&self, md: CompiledMacro, now: Instant) -> ActiveMacro {
        ActiveMacro {
            steps: md.steps,
            mod_delay: Duration::ZERO,
            current_step: 0,
            steps_emitted: 0,
            phase: MacroPhase::NextStep,
            next_wake: now,
            deadline: now + self.timeout,
        }
    }
}

fn next_macro_wake(now: Instant, am: &ActiveMacro) -> Instant {
    match am.steps.get(am.current_step) {
        Some(step) if !matches!(step.item, crate::mapper::action::MacroStepItem::Pause(_)) => {
            now + step.step_pause
        }
        _ => now,
    }
}
