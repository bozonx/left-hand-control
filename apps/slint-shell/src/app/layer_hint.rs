//! Render the current held-layer hint and keep the Spell worker in sync.

use super::App;
use crate::{
    command::{Source, WorkerCommand},
    layer_hint_model::LayerHint as Hint,
};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
use tecloid_core::mapper::{layer_hint::LayerHintState, layer_state};

const RETRY_DELAY: Duration = Duration::from_millis(100);

#[derive(Default)]
pub(super) struct LayerHint {
    state: RefCell<LayerHintState>,
    /// Last command accepted by this worker's ordered sender, not an acknowledgement.
    sent: RefCell<Option<Option<Hint>>>,
    timer: slint::Timer,
}

impl LayerHint {
    /// A failed enqueue leaves the previous state intact, so the next attempt
    /// submits the latest desired state rather than replaying a stale command.
    fn submit(
        &self,
        hint: Option<Hint>,
        send: impl FnOnce(WorkerCommand) -> Result<(), crate::i18n::Msg>,
    ) -> Result<(), crate::i18n::Msg> {
        if self.sent.borrow().as_ref() == Some(&hint) {
            return Ok(());
        }
        send(WorkerCommand::LayerHint(hint.clone()))?;
        *self.sent.borrow_mut() = Some(hint);
        Ok(())
    }
}

impl App {
    /// Reconcile settings, current hold and content without extending a pending delay.
    pub(super) fn sync_layer_hint_setting(&self) {
        let enabled = self
            .document()
            .is_some_and(|document| document.read().settings().layer_hint);
        let changed = self.supervisor.borrow().hint != enabled;
        self.supervisor.borrow_mut().hint = enabled;
        if changed && enabled {
            self.start_worker(true);
        }
        self.sync_held_layer();
        if !enabled {
            self.supervisor.borrow_mut().stop_unwanted();
        }
    }

    /// Events are wakeups only: stale queued events cannot revive an old hold.
    pub(super) fn sync_held_layer(&self) {
        let now = Instant::now();
        let held = layer_state::current();
        let enabled = self.supervisor.borrow().hint;
        let new_hold =
            held.layer.is_some() && self.layer_hint.state.borrow().hold_id() != held.hold_id;
        self.layer_hint
            .state
            .borrow_mut()
            .observe(held, enabled, now);
        if enabled && new_hold {
            self.start_worker(true);
        }
        self.update_layer_hint();
    }

    fn update_layer_hint(&self) {
        self.layer_hint.timer.stop();
        let now = Instant::now();
        let target = self
            .layer_hint
            .state
            .borrow()
            .target(now)
            .map(str::to_owned);
        let hint = target.and_then(|id| {
            let hold_id = self.layer_hint.state.borrow().hold_id();
            if !layer_state::current().allows_hint(&id, hold_id) {
                return None;
            }
            let document = self.document()?;
            // A failed context switch keeps the old mapper running. Never describe
            // the newly selected profile as though it had been applied.
            let config = document.read();
            let selected = config
                .active_profile_id(&tecloid_core::config::auto_switch::AutoSwitchContext::current())
                .map_err(|error| log::warn!("layer hint profile: {error}"))
                .ok()?;
            if document.applied_profile_id()? != selected {
                return None;
            }
            let profile = self.menus();
            let hint =
                crate::layer_key_model::hint(&self.settings, &config, &profile.profile, &id)?;
            // Caption construction can race with release, usage or re-press.
            layer_state::current()
                .allows_hint(&id, hold_id)
                .then_some(hint)
        });
        let running = self.supervisor.borrow().running();
        let retry = running
            && self
                .layer_hint
                .submit(hint, |command| {
                    self.send_worker(&command, Source::Mapper, now, None)
                })
                .map_err(|error| log::warn!("layer hint: {error:?}"))
                .is_err();
        let remaining = self.layer_hint.state.borrow().remaining(now);
        let delay = match (remaining, retry) {
            (Some(delay), true) => Some(delay.min(RETRY_DELAY)),
            (delay, false) => delay,
            (None, true) => Some(RETRY_DELAY),
        };
        if let Some(delay) = delay {
            self.layer_hint
                .timer
                .start(slint::TimerMode::SingleShot, delay, || {
                    super::with_app(|app| app.sync_held_layer())
                });
        }
    }

    /// A new worker starts hidden and must receive the latest desired state.
    pub(super) fn resend_layer_hint(&self) {
        self.layer_hint.sent.borrow_mut().take();
        self.sync_held_layer();
    }

    pub(super) fn hide_layer_hint(&self) {
        self.layer_hint.state.borrow_mut().dismiss();
        self.update_layer_hint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hint(name: &str) -> Option<Hint> {
        Some(Hint {
            name: name.into(),
            left: vec![],
            right: vec![],
        })
    }

    #[test]
    fn failed_enqueue_retries_latest_content_and_release_instead_of_stale_show() {
        let delivery = LayerHint::default();
        assert!(
            delivery
                .submit(hint("Old"), |_| Err(crate::i18n::Msg::WorkerRestarting))
                .is_err()
        );
        let mut commands = vec![];
        delivery
            .submit(None, |command| {
                commands.push(command);
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            commands.as_slice(),
            [WorkerCommand::LayerHint(None)]
        ));
        delivery.submit(hint("Old"), |_| Ok(())).unwrap();
        assert!(
            delivery
                .submit(hint("New"), |_| Err(crate::i18n::Msg::WorkerRestarting))
                .is_err()
        );
        delivery
            .submit(hint("Newest"), |command| {
                commands.push(command);
                Ok(())
            })
            .unwrap();
        assert!(
            matches!(commands.last(), Some(WorkerCommand::LayerHint(Some(hint))) if hint.name == "Newest")
        );
        delivery
            .submit(hint("Newest"), |_| panic!("unchanged content"))
            .unwrap();
    }

    #[test]
    fn failed_hide_is_retried_and_new_worker_receives_the_current_state() {
        let delivery = LayerHint::default();
        delivery.submit(hint("Nav"), |_| Ok(())).unwrap();
        assert!(
            delivery
                .submit(None, |_| Err(crate::i18n::Msg::WorkerRestarting))
                .is_err()
        );
        delivery.submit(None, |_| Ok(())).unwrap();
        delivery.sent.borrow_mut().take();
        let mut sent = false;
        delivery
            .submit(None, |command| {
                assert!(matches!(command, WorkerCommand::LayerHint(None)));
                sent = true;
                Ok(())
            })
            .unwrap();
        assert!(sent);
    }
}
