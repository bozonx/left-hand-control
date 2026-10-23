//! Lifetime, destination and completion accounting for asynchronous text input.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::events::TextInputFailure;

const MAX_QUEUED_TEXTS: usize = 16;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_AGE: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Progress {
    generation: u64,
    active: bool,
    pending: usize,
    failed: bool,
}

static PROGRESS: Mutex<Progress> = Mutex::new(Progress {
    generation: 0,
    active: false,
    pending: 0,
    failed: false,
});

#[derive(Clone)]
pub(super) struct TextRequest {
    generation: u64,
    deadline: Instant,
    target: crate::active_window::TextTarget,
}

impl Progress {
    fn reserve(&mut self, text_len: usize) -> Result<u64, TextInputFailure> {
        if !self.active {
            return Err(unavailable("text input session is stopped"));
        }
        if text_len > MAX_TEXT_BYTES || self.pending >= MAX_QUEUED_TEXTS {
            self.failed = true;
            return Err(unavailable("text input queue or text size limit exceeded"));
        }
        self.pending += 1;
        Ok(self.generation)
    }

    fn complete(&mut self, generation: u64, failed: bool) {
        if self.generation == generation {
            self.pending = self.pending.saturating_sub(1);
            self.failed |= failed;
        }
    }

    fn reset(&mut self, active: bool) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = 0;
        self.failed = false;
        self.active = active;
    }
}

impl TextRequest {
    pub(super) fn enqueue(text: &str) -> Result<Self, TextInputFailure> {
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        let generation = progress.reserve(text.len())?;
        let request = Self {
            generation,
            deadline: Instant::now() + MAX_AGE,
            target: crate::active_window::TextTarget::capture(),
        };
        Ok(request)
    }

    pub(super) fn check(&self) -> Result<(), TextInputFailure> {
        self.check_lifetime()?;
        if !self.target.is_current() {
            return Err(unavailable(
                "text input cancelled: destination window changed or is unknown",
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub(super) fn check_progress(&mut self) -> Result<(), TextInputFailure> {
        self.check()?;
        // Native Unicode delivery is paced for WinUI. Bound inactivity and queue
        // age, while allowing a long text that keeps reaching the same target.
        self.deadline = Instant::now() + MAX_AGE;
        Ok(())
    }

    pub(super) fn wait_for_target(&self) -> Result<(), TextInputFailure> {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            self.check_lifetime()?;
            if self.target.is_current() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return self.check();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn check_lifetime(&self) -> Result<(), TextInputFailure> {
        let progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        validate(
            self.generation,
            progress.generation,
            self.deadline,
            Instant::now(),
        )
    }

    pub(super) fn complete(&self, failed: bool) {
        let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
        progress.complete(self.generation, failed);
    }
}

pub(super) fn cancel() {
    let mut progress = PROGRESS.lock().unwrap_or_else(|e| e.into_inner());
    progress.reset(false);
}

pub(super) fn start() {
    PROGRESS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .reset(true);
}

pub(super) fn pending() -> bool {
    PROGRESS.lock().unwrap_or_else(|e| e.into_inner()).pending != 0
}

pub(super) fn take_failure() -> bool {
    std::mem::take(&mut PROGRESS.lock().unwrap_or_else(|e| e.into_inner()).failed)
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
    fn old_completions_cannot_release_new_jobs_or_abort_new_macros() {
        let mut progress = Progress {
            active: true,
            ..Progress::default()
        };
        let old = progress.reserve(10).unwrap();
        progress.reset(false);
        assert!(progress.reserve(1).is_err());
        progress.reset(true);
        let new = progress.reserve(10).unwrap();
        progress.complete(old, true);
        assert_eq!(progress.pending, 1);
        assert!(!progress.failed);
        progress.complete(new, true);
        assert_eq!(progress.pending, 0);
        assert!(progress.failed);
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
