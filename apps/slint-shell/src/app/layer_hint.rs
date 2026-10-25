//! Layer hint: while a mapper layer is held, the Spell worker shows its keys
//! above all windows without taking focus.

use super::App;
use crate::{
    command::{Command, Source},
    pages,
};
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

/// How long a layer must stay held before its hint appears, so quick layer
/// use by someone who knows the layout does not flash it.
const HINT_DELAY: Duration = Duration::from_secs(2);
const RETRY_DELAY: Duration = Duration::from_millis(100);

/// A used layer stays suppressed until all layer triggers are released.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum HintSession {
    #[default]
    Idle,
    Ready,
    Used,
}

impl HintSession {
    fn layer_changed(self, held: bool) -> Self {
        match (self, held) {
            (_, false) => Self::Idle,
            (Self::Idle, true) => Self::Ready,
            (session, true) => session,
        }
    }

    fn key_pressed(self) -> Self {
        match self {
            Self::Idle => Self::Idle,
            _ => Self::Used,
        }
    }
}

/// State of the layer hint.
#[derive(Default)]
pub(super) struct LayerHint {
    /// Topmost layer the mapper holds now.
    held: RefCell<Option<String>>,
    /// The hint is up, or due as soon as the worker starts.
    shown: Cell<bool>,
    session: Cell<HintSession>,
    hold_id: Cell<u64>,
    /// Delays the hint after a layer is taken.
    timer: slint::Timer,
}

impl App {
    /// Apply the saved hint setting: start or stop the worker it needs.
    pub(super) fn sync_layer_hint_setting(&self) {
        let enabled = self
            .document()
            .is_some_and(|document| document.read().settings().layer_hint);
        {
            let mut supervisor = self.supervisor.borrow_mut();
            if supervisor.hint == enabled {
                return;
            }
            supervisor.hint = enabled;
        }
        if enabled {
            self.start_worker(true);
            if self.layer_hint.held.borrow().is_some() {
                self.schedule_layer_hint();
            }
        } else {
            self.hide_layer_hint();
            self.supervisor.borrow_mut().stop_unwanted();
        }
    }

    /// Queued core events are wakeups: read the latest snapshot so an old
    /// event cannot revive a released or already used hold.
    pub(super) fn sync_held_layer(&self) {
        let state = lhc_core::mapper::layer_state::current();
        let new_hold = self.layer_hint.hold_id.replace(state.hold_id) != state.hold_id;
        if new_hold {
            self.hide_layer_hint();
            self.layer_hint.session.set(HintSession::Idle);
        }
        if new_hold || *self.layer_hint.held.borrow() != state.layer {
            self.layer_changed(state.layer);
        }
        if state.used {
            self.layer_key_pressed();
        }
    }

    /// The mapper's topmost held layer changed.
    pub(super) fn layer_changed(&self, layer: Option<String>) {
        self.layer_hint.timer.stop();
        let held = layer.is_some();
        self.layer_hint
            .session
            .set(self.layer_hint.session.get().layer_changed(held));
        *self.layer_hint.held.borrow_mut() = layer;
        if self.layer_hint.session.get() != HintSession::Ready {
            self.hide_layer_hint();
        } else if self.layer_hint.shown.get() {
            // Another layer took over while the hint is up.
            self.show_layer_hint();
        } else {
            self.start_worker(true);
            self.schedule_layer_hint();
        }
    }

    /// Cancel both a visible hint and its pending timer for this hold.
    pub(super) fn layer_key_pressed(&self) {
        self.layer_hint
            .session
            .set(self.layer_hint.session.get().key_pressed());
        self.hide_layer_hint();
    }

    fn schedule_layer_hint(&self) {
        if self.layer_hint.session.get() != HintSession::Ready {
            return;
        }
        self.layer_hint
            .timer
            .start(slint::TimerMode::SingleShot, HINT_DELAY, || {
                super::with_app(|app| app.show_layer_hint())
            });
    }

    fn show_layer_hint(&self) {
        let Some(id) = self.layer_hint.held.borrow().clone() else {
            return;
        };
        if self.layer_hint.session.get() != HintSession::Ready || !self.supervisor.borrow().hint {
            return;
        }
        if !lhc_core::mapper::layer_state::current().allows_hint(&id, self.layer_hint.hold_id.get())
        {
            self.hide_layer_hint();
            return;
        }
        let Some(document) = self.document() else {
            return;
        };
        let layout = self.menus();
        let hint = pages::layers::hint(&self.settings, &document.read(), &layout.layout, &id);
        let Some(hint) = hint else {
            self.hide_layer_hint();
            return;
        };
        // The mapper can handle a release while captions are being built.
        if !lhc_core::mapper::layer_state::current().allows_hint(&id, self.layer_hint.hold_id.get())
        {
            self.hide_layer_hint();
            return;
        }
        // Marked before sending: a send that fails after the worker took
        // the command must not leave the hint up with nothing to hide it.
        // A worker that is still starting gets the hint once it is up.
        self.layer_hint.shown.set(true);
        if !self.supervisor.borrow().running() {
            return;
        }
        if let Err(error) = self.send_worker(
            &Command::LayerHint(Some(hint)),
            Source::Mapper,
            Instant::now(),
            None,
        ) {
            log::warn!("layer hint: {error:?}");
            self.layer_hint
                .timer
                .start(slint::TimerMode::SingleShot, RETRY_DELAY, || {
                    super::with_app(|app| app.show_layer_hint())
                });
        }
    }

    /// Send the hint again to a worker that just started.
    pub(super) fn resend_layer_hint(&self) {
        if self.layer_hint.shown.get() {
            self.show_layer_hint();
        }
    }

    pub(super) fn hide_layer_hint(&self) {
        self.layer_hint.timer.stop();
        self.layer_hint.shown.set(false);
        let running = self.supervisor.borrow().running();
        if running
            && let Err(error) = self.send_worker(
                &Command::LayerHint(None),
                Source::Mapper,
                Instant::now(),
                None,
            )
        {
            log::warn!("hide layer hint: {error:?}");
            self.layer_hint
                .timer
                .start(slint::TimerMode::SingleShot, RETRY_DELAY, || {
                    super::with_app(|app| app.hide_layer_hint())
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HintSession;

    #[test]
    fn pressing_a_layer_key_blocks_pending_and_visible_hints_until_release() {
        let held = HintSession::default().layer_changed(true);
        assert_eq!(held, HintSession::Ready);
        let used = held.key_pressed();
        assert_eq!(used, HintSession::Used);
        // Neither another layer taking over nor returning to the lower
        // held layer may re-enable a timer or a worker resend.
        assert_eq!(used.layer_changed(true), HintSession::Used);
        assert_eq!(used.key_pressed(), HintSession::Used);
        let released = used.layer_changed(false);
        assert_eq!(released, HintSession::Idle);
        assert_eq!(released.layer_changed(true), HintSession::Ready);
    }

    #[test]
    fn presses_without_a_held_layer_do_not_suppress_the_next_hold() {
        assert_eq!(
            HintSession::Idle.key_pressed().layer_changed(true),
            HintSession::Ready
        );
    }
}
