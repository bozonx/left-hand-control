//! Supervision of the Spell popup worker: starting it off the UI thread,
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

#[derive(Default)]
pub(super) struct Supervisor {
    /// Popups run in the worker (layer-shell available and requested).
    pub(super) enabled: bool,
    worker: Option<Worker>,
    starting: bool,
    /// The restart limit was hit; wait for the user to ask again.
    gave_up: bool,
    policy: RestartPolicy,
}

impl Supervisor {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }
}

impl App {
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
            let dead = supervisor.worker.as_mut().is_some_and(|worker| !worker.is_alive());
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
            if !supervisor.enabled || supervisor.starting || supervisor.worker.is_some() {
                return;
            }
            if supervisor.gave_up && !user {
                return;
            }
            if !supervisor.policy.allow(Instant::now()) {
                supervisor.gave_up = true;
                drop(supervisor);
                self.set_error(Msg::WorkerRestartLimit);
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
                let preferences = Command::Preferences(self.preferences.get());
                match worker.send(&preferences, Source::Ipc, Instant::now(), None) {
                    Ok(()) => self.set_error(Msg::None),
                    Err(error) => self.set_error(Msg::WorkerError(error.to_string())),
                }
                self.supervisor.borrow_mut().worker = Some(worker);
                self.menus_sent.borrow_mut().take();
            }
            Err(error) => {
                log::error!("Spell worker: {error}");
                self.set_error(Msg::WorkerError(error));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
