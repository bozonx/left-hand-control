//! Supervision of the Spell popup worker: forwarding commands, detecting
//! exits and restarting at most three times per minute with the current
//! theme and language.

use super::{App, post};
use crate::{
    command::{Command, Source},
    i18n::Msg,
    platform::backend,
};
use std::time::{Duration, Instant};

const RESTART_LIMIT: usize = 3;
const RESTART_WINDOW: Duration = Duration::from_secs(60);

impl App {
    /// Forward to the worker. Returns `false` when no worker is attached.
    pub(super) fn send_worker(
        &self,
        command: &Command,
        source: Source,
        start: Instant,
        token: Option<String>,
    ) -> bool {
        let result = self
            .worker
            .borrow()
            .as_ref()
            .map(|worker| worker.send(command, source, start, token));
        if let Some(Err(error)) = &result {
            self.report_worker_error(&error.to_string());
        }
        result.is_some()
    }

    pub(super) fn check_worker(&self) {
        let dead = self
            .worker
            .borrow_mut()
            .as_mut()
            .is_some_and(|worker| !worker.is_alive());
        if dead {
            self.report_worker_error("worker exited");
        } else if self.worker.borrow().is_none() && !self.restart_pending.get() {
            self.restart_worker();
        }
    }

    fn report_worker_error(&self, error: &str) {
        log::error!("Spell popup process unavailable: {error}");
        self.set_error(Msg::WorkerUnavailable(error.into()));
        self.restart_worker();
    }

    fn restart_worker(&self) {
        if self.restart_pending.replace(true) {
            return;
        }
        let now = Instant::now();
        let mut history = self.restart_history.borrow_mut();
        history.retain(|attempt| now.duration_since(*attempt) < RESTART_WINDOW);
        if history.len() >= RESTART_LIMIT {
            self.set_error(Msg::WorkerRestartLimit);
            self.restart_pending.set(false);
            return;
        }
        history.push(now);
        drop(history);
        self.worker.borrow_mut().take();
        std::thread::spawn(|| {
            let result = backend::start().map_err(|error| error.to_string());
            post(move |app| {
                app.restart_pending.set(false);
                match result {
                    Ok(Some(worker)) => {
                        let preferences = Command::Preferences(app.preferences.get());
                        match worker.send(&preferences, Source::Ipc, Instant::now(), None) {
                            Ok(()) => app.set_error(Msg::None),
                            Err(error) => app.set_error(Msg::WorkerError(error.to_string())),
                        }
                        *app.worker.borrow_mut() = Some(worker);
                    }
                    Ok(None) => app.set_error(Msg::WorkerNotStarted),
                    Err(error) => app.set_error(Msg::WorkerError(error)),
                }
            });
        });
    }
}
