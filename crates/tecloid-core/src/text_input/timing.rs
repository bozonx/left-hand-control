//! Shared queue, preparation and progress deadlines for text backends.

use std::time::{Duration, Instant};

pub(super) const MAX_TEXT_BYTES: usize = 1024 * 1024;
const QUEUE_TIMEOUT: Duration = Duration::from_secs(15);
const PREPARE_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Queued,
    Preparing,
    Typing,
    #[cfg(target_os = "linux")]
    Tool,
}

/// Deadline policy shared by mapper text and native popup insertion.
/// Checking a deadline never extends it; only successful output is progress.
pub struct TextTiming {
    phase: Phase,
    deadline: Instant,
}

impl TextTiming {
    /// Reserve the queue lifetime and reject oversized text before copying it.
    pub fn new(text: &str) -> Result<Self, String> {
        if text.len() > MAX_TEXT_BYTES {
            return Err("text size limit exceeded".into());
        }
        Ok(Self::queued(Instant::now()))
    }

    fn queued(now: Instant) -> Self {
        Self {
            phase: Phase::Queued,
            deadline: now + QUEUE_TIMEOUT,
        }
    }

    /// Start setup once the worker takes the job, without reviving expired work.
    pub fn prepare(&mut self) -> Result<(), String> {
        self.transition(
            Phase::Queued,
            Phase::Preparing,
            PREPARE_TIMEOUT,
            Instant::now(),
        )
    }

    /// Start character delivery after setup and modifier waits have completed.
    pub fn start_typing(&mut self) -> Result<(), String> {
        self.transition(
            Phase::Preparing,
            Phase::Typing,
            IDLE_TIMEOUT,
            Instant::now(),
        )
    }

    fn transition(
        &mut self,
        from: Phase,
        to: Phase,
        timeout: Duration,
        now: Instant,
    ) -> Result<(), String> {
        self.check_at(now)?;
        if self.phase == from {
            self.phase = to;
            self.deadline = now + timeout;
        }
        Ok(())
    }

    /// External tools cannot report character progress; use their bounded budget.
    #[cfg(target_os = "linux")]
    pub(crate) fn start_tool(&mut self, timeout: Duration) -> Result<(), String> {
        self.transition(Phase::Preparing, Phase::Tool, timeout, Instant::now())
    }

    /// Check queue age, setup time or inactivity, depending on the current phase.
    pub fn check(&self) -> Result<(), String> {
        self.check_at(Instant::now())
    }

    fn check_at(&self, now: Instant) -> Result<(), String> {
        if now >= self.deadline {
            Err(match self.phase {
                Phase::Queued => "text input expired in queue",
                Phase::Preparing => "text input preparation timed out",
                Phase::Typing => "text input made no progress",
                #[cfg(target_os = "linux")]
                Phase::Tool => "text input tool timed out",
            }
            .into())
        } else {
            Ok(())
        }
    }

    /// Call only after a character/unit was successfully sent to the backend.
    pub fn mark_progress(&mut self) {
        self.progress_at(Instant::now());
    }

    fn progress_at(&mut self, now: Instant) {
        if self.phase == Phase::Typing && now < self.deadline {
            self.deadline = now + IDLE_TIMEOUT;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_limit_counts_utf8_bytes() {
        assert!(TextTiming::new(&"x".repeat(MAX_TEXT_BYTES)).is_ok());
        assert!(TextTiming::new(&"я".repeat(MAX_TEXT_BYTES / 2 + 1)).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn external_tool_budget_is_bounded_and_cannot_be_renewed_by_polling() {
        let mut timing = TextTiming::new("text").unwrap();
        timing.prepare().unwrap();
        timing.start_tool(Duration::from_secs(60)).unwrap();
        let deadline = timing.deadline;
        timing.start_tool(Duration::from_secs(120)).unwrap();
        timing.progress_at(deadline - Duration::from_secs(1));
        assert_eq!(timing.deadline, deadline);
        assert!(timing.check_at(deadline).is_err());
    }

    #[test]
    fn expired_queue_cannot_be_started_or_revived() {
        let now = Instant::now();
        let mut timing = TextTiming::queued(now);
        let expired = now + QUEUE_TIMEOUT;
        assert!(
            timing
                .transition(Phase::Queued, Phase::Preparing, PREPARE_TIMEOUT, expired)
                .is_err()
        );
        timing.progress_at(expired);
        assert!(timing.check_at(expired).is_err());
    }

    #[test]
    fn successful_typing_can_outlive_queue_age_but_checks_do_not_extend_it() {
        let now = Instant::now();
        let mut timing = TextTiming::queued(now);
        let setup = now + Duration::from_secs(14);
        timing
            .transition(Phase::Queued, Phase::Preparing, PREPARE_TIMEOUT, setup)
            .unwrap();
        let start = setup + Duration::from_secs(14);
        timing
            .transition(Phase::Preparing, Phase::Typing, IDLE_TIMEOUT, start)
            .unwrap();
        for seconds in 1..100 {
            let at = start + Duration::from_secs(seconds);
            assert!(timing.check_at(at).is_ok());
            timing.progress_at(at);
        }
        let deadline = timing.deadline;
        assert!(timing.check_at(deadline - Duration::from_millis(1)).is_ok());
        assert!(timing.check_at(deadline).is_err());
        timing.progress_at(deadline);
        assert!(timing.check_at(deadline).is_err());
    }

    #[test]
    fn repeated_setup_and_non_typing_progress_do_not_extend_deadlines() {
        let now = Instant::now();
        let mut timing = TextTiming::queued(now);
        timing
            .transition(Phase::Queued, Phase::Preparing, PREPARE_TIMEOUT, now)
            .unwrap();
        let deadline = timing.deadline;
        timing
            .transition(
                Phase::Queued,
                Phase::Preparing,
                PREPARE_TIMEOUT,
                now + Duration::from_secs(10),
            )
            .unwrap();
        timing.progress_at(now + Duration::from_secs(10));
        assert_eq!(timing.deadline, deadline);
        assert!(timing.check_at(deadline).is_err());
    }
}
