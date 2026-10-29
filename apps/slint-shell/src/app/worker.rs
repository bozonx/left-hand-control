//! Supervision of the Spell worker: starting it off the UI thread,
//! forwarding commands and restarting it after a crash, at most three times
//! a minute. Past that limit it is restarted only when the user asks for a
//! popup again.

use super::{App, post};
use crate::{
    command::{Source, WorkerCommand},
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
    pub(super) command: WorkerCommand,
    pub(super) source: Source,
    pub(super) start: Instant,
    pub(super) token: Option<String>,
    pub(super) queued: bool,
    id: u64,
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
    generation: u64,
    next_show: u64,
    starting: bool,
    /// The restart limit was hit; wait for the user to ask again.
    gave_up: bool,
    /// A start failure was already shown; later ones only go to the log
    /// until a start succeeds.
    start_error_shown: bool,
    policy: RestartPolicy,
}

impl Supervisor {
    pub(super) fn request_show(
        &mut self,
        command: WorkerCommand,
        source: Source,
        start: Instant,
        token: Option<String>,
    ) {
        self.next_show = self.next_show.wrapping_add(1);
        self.pending_show = Some(PendingShow {
            command,
            source,
            start,
            token,
            queued: false,
            id: self.next_show,
        });
    }

    fn acknowledge_show(&mut self, id: u64) {
        if self
            .pending_show
            .as_ref()
            .is_some_and(|pending| pending.id == id)
        {
            self.pending_show.take();
        }
    }

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
            supervisor
                .pending_show
                .clone()
                .filter(|pending| !pending.queued)
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
                if let Some(pending) = self.supervisor.borrow_mut().pending_show.as_mut() {
                    pending.queued = true;
                }
            }
            Err(error) => self.set_error(error),
        }
    }

    /// Forward `command` to the worker.
    pub(super) fn send_worker(
        &self,
        command: &WorkerCommand,
        source: Source,
        start: Instant,
        token: Option<String>,
    ) -> Result<(), Msg> {
        let result = {
            let supervisor = self.supervisor.borrow();
            match &supervisor.worker {
                Some(worker) => worker.send(command, source, start, token, {
                    let generation = supervisor.generation;
                    let show_id = matches!(
                        command,
                        WorkerCommand::Show(_) | WorkerCommand::ShowPage(_, _)
                    )
                    .then(|| supervisor.pending_show.as_ref().map(|pending| pending.id))
                    .flatten();
                    move |result| {
                        post(move |app| {
                            if app.supervisor.borrow().generation != generation {
                                return;
                            }
                            match result {
                                Ok(()) => {
                                    if let Some(id) = show_id {
                                        app.supervisor.borrow_mut().acknowledge_show(id);
                                    }
                                }
                                Err(error) => {
                                    if let Some(pending) =
                                        app.supervisor.borrow_mut().pending_show.as_mut()
                                    {
                                        pending.queued = false;
                                    }
                                    app.popups.reset_worker();
                                    app.set_error(Msg::WorkerError(error));
                                    app.check_worker();
                                }
                            }
                        });
                    }
                }),
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
                if let Some(pending) = supervisor.pending_show.as_mut() {
                    pending.queued = false;
                }
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
                {
                    let mut supervisor = self.supervisor.borrow_mut();
                    supervisor.start_error_shown = false;
                    supervisor.generation = supervisor.generation.wrapping_add(1);
                    supervisor.worker = Some(worker);
                }
                let preferences = WorkerCommand::Preferences(self.preferences.get());
                if let Err(error) =
                    self.send_worker(&preferences, Source::Ipc, Instant::now(), None)
                {
                    self.set_error(error);
                }
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
                command: WorkerCommand::Show(crate::command::Popup::Emoji),
                source,
                start: Instant::now(),
                token: None,
                queued: false,
                id: 1,
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
    #[test]
    fn old_acknowledgement_cannot_clear_a_new_show_with_the_same_trigger_time() {
        let mut supervisor = Supervisor::new(true, true);
        let start = Instant::now();
        let show = WorkerCommand::Show(crate::command::Popup::Quick);
        supervisor.request_show(show.clone(), Source::Ipc, start, None);
        let first = supervisor.pending_show.as_ref().unwrap().id;
        supervisor.request_show(show, Source::Ipc, start, None);
        let second = supervisor.pending_show.as_ref().unwrap().id;
        supervisor.acknowledge_show(first);
        assert_eq!(supervisor.pending_show.as_ref().unwrap().id, second);
        supervisor.acknowledge_show(second);
        assert!(supervisor.pending_show.is_none());
    }
}
