//! Supervision of the Spell worker: starting it off the UI thread,
//! forwarding commands and restarting it after a crash, at most three times
//! a minute. Past that limit it is restarted only when the user asks for a
//! popup again.

use super::{App, post};
use crate::{
    command::{Command, Source},
    i18n::Msg,
    platform::backend::{self, Worker},
};
use std::time::{Duration, Instant};

const RESTART_LIMIT: usize = 3;
const RESTART_WINDOW: Duration = Duration::from_secs(60);
/// Pause before retrying a worker that failed to start.
const RETRY_DELAY: Duration = Duration::from_secs(1);

/// Allows at most `RESTART_LIMIT` starts within `RESTART_WINDOW`.
#[derive(Default)]
pub(super) struct RestartPolicy {
    history: Vec<Instant>,
}

impl RestartPolicy {
    /// Record a start at `now` if the limit allows it.
    pub(super) fn allow(&mut self, now: Instant) -> bool {
        self.history
            .retain(|attempt| now.duration_since(*attempt) < RESTART_WINDOW);
        if self.history.len() >= RESTART_LIMIT {
            return false;
        }
        self.history.push(now);
        true
    }
}

#[derive(Clone)]
pub(super) struct PendingShow {
    pub(super) command: Command,
    pub(super) source: Source,
    pub(super) start: Instant,
    pub(super) token: Option<String>,
}

#[derive(Default)]
pub(super) struct Supervisor {
    pub(super) pending_show: Option<PendingShow>,
    /// Popups run in the worker (layer-shell available and requested).
    pub(super) enabled: bool,
    /// The worker can run: layer-shell is available in a Spell build.
    pub(super) layer_shell: bool,
    /// The layer hint is turned on; it needs the worker even when the
    /// popups are winit windows.
    pub(super) hint: bool,
    worker: Option<Worker>,
    starting: bool,
    /// The restart limit was hit; wait for the user to ask again.
    gave_up: bool,
    /// A start failure was already shown; later ones only go to the log
    /// until a start succeeds.
    start_error_shown: bool,
    policy: RestartPolicy,
}

impl Supervisor {
    pub(super) fn cancel_blocked_show(&mut self, game_active: bool) {
        if self
            .pending_show
            .as_ref()
            .is_some_and(|pending| pending.source.popup_blocked(game_active))
        {
            self.pending_show.take();
        }
    }

    pub(super) fn new(enabled: bool, layer_shell: bool) -> Self {
        Self {
            enabled,
            layer_shell: enabled || layer_shell,
            ..Self::default()
        }
    }

    /// Whether the worker should be running.
    pub(super) fn wanted(&self) -> bool {
        self.enabled || (self.layer_shell && self.hint)
    }

    pub(super) fn running(&self) -> bool {
        self.worker.is_some()
    }

    /// Stop a worker that is no longer wanted.
    pub(super) fn stop_unwanted(&mut self) {
        if !self.wanted() {
            self.shutdown();
            self.worker = None;
        }
    }

    pub(super) fn shutdown(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(worker) = self.worker.as_mut() {
            worker.shutdown();
        }
    }
}

impl App {
    pub(super) fn flush_worker_show(&self) {
        let pending = {
            let mut supervisor = self.supervisor.borrow_mut();
            supervisor.cancel_blocked_show(lhc_core::runtime_state::game_mode_active());
            if supervisor.worker.is_none() {
                return;
            }
            supervisor.pending_show.clone()
        };
        let Some(pending) = pending else { return };
        let result = self
            .sync_worker_menus(pending.source, pending.start)
            .and_then(|()| {
                self.send_worker(
                    &pending.command,
                    pending.source,
                    pending.start,
                    pending.token,
                )
            });
        match result {
            Ok(()) => {
                self.supervisor.borrow_mut().pending_show.take();
            }
            Err(error) => self.set_error(error),
        }
    }

    /// Forward `command` to the worker.
    pub(super) fn send_worker(
        &self,
        command: &Command,
        source: Source,
        start: Instant,
        token: Option<String>,
    ) -> Result<(), Msg> {
        let result = {
            let supervisor = self.supervisor.borrow();
            match &supervisor.worker {
                Some(worker) => worker
                    .send(command, source, start, token)
                    .map_err(|error| error.to_string()),
                None if supervisor.starting => return Err(Msg::WorkerRestarting),
                None => return Err(Msg::WorkerNotStarted),
            }
        };
        result.map_err(|error| {
            log::error!("Spell worker: {error}");
            // A failed send of a live worker is transient; a dead one restarts.
            self.check_worker();
            Msg::WorkerError(error)
        })
    }

    /// Restart a worker that exited.
    pub(super) fn check_worker(&self) {
        let dead = {
            let mut supervisor = self.supervisor.borrow_mut();
            let dead = supervisor
                .worker
                .as_mut()
                .is_some_and(|worker| !worker.is_alive());
            if dead {
                supervisor.worker = None;
            }
            dead
        };
        if dead {
            log::error!("Spell popup process exited");
            self.set_error(Msg::WorkerUnavailable("worker exited".into()));
            self.start_worker(false);
        }
    }

    /// Start the worker in the background. `user` restarts are allowed
    /// even after the restart limit was hit.
    pub(super) fn start_worker(&self, user: bool) {
        {
            let mut supervisor = self.supervisor.borrow_mut();
            if !supervisor.wanted() || supervisor.starting || supervisor.worker.is_some() {
                return;
            }
            if supervisor.gave_up && !user {
                return;
            }
            if !supervisor.policy.allow(Instant::now()) {
                let first = !std::mem::replace(&mut supervisor.gave_up, true);
                drop(supervisor);
                log::error!("Spell worker: restart limit reached");
                if first {
                    self.set_error(Msg::WorkerRestartLimit);
                }
                return;
            }
            supervisor.gave_up = false;
            supervisor.starting = true;
        }
        std::thread::spawn(|| {
            let result = backend::spawn().map_err(|error| error.to_string());
            post(move |app| app.worker_started(result));
        });
    }

    fn worker_started(&self, result: Result<Worker, String>) {
        self.supervisor.borrow_mut().starting = false;
        match result {
            Ok(worker) => {
                self.supervisor.borrow_mut().start_error_shown = false;
                let preferences = Command::Preferences(self.preferences.get());
                if let Err(error) = worker.send(&preferences, Source::Ipc, Instant::now(), None) {
                    self.set_error(Msg::WorkerError(error.to_string()));
                }
                self.supervisor.borrow_mut().worker = Some(worker);
                self.popups.reset_worker();
                self.flush_worker_show();
                self.resend_layer_hint();
            }
            Err(error) => {
                log::error!("Spell worker: {error}");
                let shown =
                    std::mem::replace(&mut self.supervisor.borrow_mut().start_error_shown, true);
                if !shown {
                    self.set_error(Msg::WorkerError(error));
                }
                // A start that failed counts against the restart limit too.
                slint::Timer::single_shot(RETRY_DELAY, || {
                    super::with_app(|app| app.start_worker(false))
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_mode_cancels_shortcuts_waiting_for_worker_start_or_restart() {
        for source in [Source::Hotkey, Source::Evdev, Source::Mapper, Source::Ipc] {
            let mut supervisor = Supervisor::new(true, true);
            supervisor.pending_show = Some(PendingShow {
                command: Command::Show(crate::command::Window::EMOJI),
                source,
                start: Instant::now(),
                token: None,
            });
            supervisor.cancel_blocked_show(false);
            assert!(supervisor.pending_show.is_some());
            supervisor.cancel_blocked_show(true);
            assert_eq!(
                supervisor.pending_show.is_none(),
                matches!(source, Source::Hotkey | Source::Evdev)
            );
            supervisor.cancel_blocked_show(false);
            assert_eq!(
                supervisor.pending_show.is_none(),
                matches!(source, Source::Hotkey | Source::Evdev)
            );
        }
    }

    #[test]
    fn restarts_are_limited_per_minute() {
        let mut policy = RestartPolicy::default();
        let now = Instant::now();
        assert!(policy.allow(now));
        assert!(policy.allow(now + Duration::from_secs(1)));
        assert!(policy.allow(now + Duration::from_secs(2)));
        assert!(!policy.allow(now + Duration::from_secs(3)));
        assert!(policy.allow(now + Duration::from_secs(61)));
    }
}
