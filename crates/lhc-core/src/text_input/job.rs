//! Lifetime, destination and completion accounting for asynchronous text input.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::events::TextInputFailure;

/// How long text input waits for the target window to take the focus.
const TARGET_TIMEOUT: Duration = Duration::from_millis(500);
const TARGET_POLL: Duration = Duration::from_millis(5);

const MAX_QUEUED_TEXTS: usize = 16;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_AGE: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Progress {
    generation: u64,
    active: bool,
    pending: usize,
}

static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    generation: 0,
    active: false,
    pending: 0,
});

/// Completion and cancellation of one macro text step, independent of other input.
#[derive(Clone, Debug, Default)]
pub struct MacroText(Arc<MacroTextState>);

#[derive(Debug, Default)]
struct MacroTextState {
    cancelled: AtomicBool,
    result: Mutex<Option<Result<(), ()>>>,
}

impl MacroText {
    pub(crate) fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn result(&self) -> Option<Result<(), ()>> {
        *self.0.result.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn complete(&self, failed: bool) {
        let mut result = self.0.result.lock().unwrap_or_else(|e| e.into_inner());
        if result.is_none() {
            *result = Some(if failed { Err(()) } else { Ok(()) });
        }
    }
}

#[derive(Clone)]
pub(crate) struct TextRequest {
    macro_job: Option<MacroText>,
    completed: Arc<AtomicBool>,
    generation: u64,
    deadline: Instant,
    target: crate::active_window::TextTarget,
    /// The destination is a terminal, which may need another paste shortcut.
    #[cfg(target_os = "linux")]
    terminal: bool,
}

impl Progress {
    fn reserve(&mut self, text_len: usize) -> Result<u64, TextInputFailure> {
        if !self.active {
            return Err(unavailable("text input session is stopped"));
        }
        if text_len > MAX_TEXT_BYTES || self.pending >= MAX_QUEUED_TEXTS {
            return Err(unavailable("text input queue or text size limit exceeded"));
        }
        self.pending += 1;
        Ok(self.generation)
    }

    fn complete(&mut self, generation: u64) {
        if self.generation == generation {
            self.pending = self.pending.saturating_sub(1);
        }
    }

    fn reset(&mut self, active: bool) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = 0;
        self.active = active;
    }
}

impl TextRequest {
    pub(crate) fn enqueue(text: &str) -> Result<Self, TextInputFailure> {
        Self::enqueue_inner(text, None)
    }

    pub(crate) fn enqueue_macro(text: &str, job: MacroText) -> Result<Self, TextInputFailure> {
        if job.cancelled() {
            return Err(unavailable("macro text input cancelled"));
        }
        let result = Self::enqueue_inner(text, Some(job.clone()));
        if result.is_err() {
            job.complete(true);
        }
        result
    }

    fn enqueue_inner(text: &str, macro_job: Option<MacroText>) -> Result<Self, TextInputFailure> {
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        let generation = progress.reserve(text.len())?;
        let request = Self {
            macro_job,
            completed: Arc::new(AtomicBool::new(false)),
            generation,
            deadline: Instant::now() + MAX_AGE,
            target: crate::active_window::TextTarget::capture(),
            #[cfg(target_os = "linux")]
            terminal: crate::active_window::text_window_is_terminal(),
        };
        Ok(request)
    }

    pub(crate) fn check(&self) -> Result<(), TextInputFailure> {
        self.check_lifetime()?;
        if !self.target.is_current() {
            return Err(TextInputFailure::DestinationChanged);
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn check_progress(&mut self) -> Result<(), TextInputFailure> {
        self.check()?;
        // Native Unicode delivery is paced for WinUI. Bound inactivity and queue
        // age, while allowing a long text that keeps reaching the same target.
        self.deadline = Instant::now() + MAX_AGE;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub(crate) fn wait_for_target(&self) -> Result<(), TextInputFailure> {
        let deadline = Instant::now() + TARGET_TIMEOUT;
        loop {
            self.check_lifetime()?;
            if self.target.is_current() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return self.check();
            }
            std::thread::sleep(TARGET_POLL);
        }
    }

    pub(crate) fn check_lifetime(&self) -> Result<(), TextInputFailure> {
        if self.macro_job.as_ref().is_some_and(MacroText::cancelled) {
            return Err(unavailable("macro text input cancelled"));
        }
        let progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        validate(
            self.generation,
            progress.generation,
            self.deadline,
            Instant::now(),
        )
    }

    pub(crate) fn complete(&self, failed: bool) {
        if self.completed.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        progress.complete(self.generation);
        if let Some(job) = &self.macro_job {
            job.complete(failed);
        }
    }
}

pub(crate) fn cancel() {
    let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
    progress.reset(false);
}

/// Owns one mapper text session, including cancellation on unwinding.
pub(crate) struct Session {
    generation: u64,
}

impl Session {
    pub(crate) fn start() -> Self {
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        progress.reset(true);
        Self {
            generation: progress.generation,
        }
    }

    pub(crate) fn restart(&mut self) {
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        progress.reset(true);
        self.generation = progress.generation;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        if progress.generation == self.generation {
            progress.reset(false);
        }
    }
}

pub(crate) fn pending() -> bool {
    PROGRESS.lock().unwrap_or_else(|e| e.into_inner()).pending != 0
}

fn unavailable(message: &str) -> TextInputFailure {
    TextInputFailure::BackendUnavailable(message.into())
}

fn validate(
    generation: u64,
    current: u64,
    deadline: Instant,
    now: Instant,
) -> Result<(), TextInputFailure> {
    if generation != current || now >= deadline {
        Err(unavailable("text input cancelled or expired"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ownership_survives_restarts_and_cancels_on_unwind() {
        let old = Session::start();
        let mut current = Session::start();
        drop(old);
        assert!(PROGRESS.lock().unwrap().active);
        let job = MacroText::default();
        let request = TextRequest::enqueue_macro("macro text", job.clone()).unwrap();
        let unrelated = TextRequest::enqueue("other text").unwrap();
        job.cancel();
        assert!(request.check_lifetime().is_err());
        assert!(unrelated.check_lifetime().is_ok());
        request.complete(true);
        request.complete(true);
        assert_eq!(PROGRESS.lock().unwrap().pending, 1);
        unrelated.complete(false);
        assert!(!pending());
        assert!(TextRequest::enqueue_macro("cancelled", job).is_err());
        current.restart();
        drop(current);
        assert!(!PROGRESS.lock().unwrap().active);
        assert!(
            std::panic::catch_unwind(|| {
                let _session = Session::start();
                panic!("mapper failed");
            })
            .is_err()
        );
        assert!(!PROGRESS.lock().unwrap().active);
    }

    #[test]
    fn old_completions_cannot_release_new_jobs() {
        let mut progress = Progress {
            active: true,
            ..Progress::default()
        };
        let old = progress.reserve(10).unwrap();
        progress.reset(false);
        assert!(progress.reserve(1).is_err());
        progress.reset(true);
        let new = progress.reserve(10).unwrap();
        progress.complete(old);
        assert_eq!(progress.pending, 1);
        progress.complete(new);
        assert_eq!(progress.pending, 0);
    }

    #[test]
    fn queue_and_payload_limits_fail_without_reserving_extra_jobs() {
        let mut progress = Progress {
            active: true,
            ..Progress::default()
        };
        assert!(progress.reserve(MAX_TEXT_BYTES + 1).is_err());
        assert_eq!(progress.pending, 0);
        for _ in 0..MAX_QUEUED_TEXTS {
            progress.reserve(1).unwrap();
        }
        assert!(progress.reserve(1).is_err());
        assert_eq!(progress.pending, MAX_QUEUED_TEXTS);
    }

    #[test]
    fn stopped_restarted_and_expired_jobs_are_rejected() {
        let now = Instant::now();
        assert!(validate(1, 1, now + Duration::from_secs(1), now).is_ok());
        assert!(validate(1, 2, now + Duration::from_secs(1), now).is_err());
        assert!(validate(1, 1, now, now).is_err());
    }
}
