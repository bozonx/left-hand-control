//! Transport-independent paste progress and clipboard restoration policy.

use std::time::{Duration, Instant};

// Data-control cannot identify the reader; keep serving briefly after a
// transfer so a background reader cannot restore ahead of the paste target.
pub(super) const DATA_CONTROL_GRACE: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PasteFailure {
    Cancelled,
    Timeout,
}

/// Reads before arming cannot acknowledge delivery to the paste destination.
#[derive(Default)]
pub(super) struct PasteProgress {
    armed: bool,
    transferred: bool,
    finished: Option<(bool, Instant)>,
}

impl PasteProgress {
    pub(super) fn arm(&mut self) {
        self.armed = true;
        self.transferred = false;
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }

    pub(super) fn transferred(&mut self, text: bool) {
        self.transferred |= self.armed && text;
    }

    pub(super) fn finish(&mut self, injected: bool, now: Instant) {
        self.finished = Some((injected, now));
    }

    pub(super) fn is_finished(&self) -> bool {
        self.finished.is_some()
    }

    /// Data-control also waits out a grace interval and outstanding writes;
    /// the portal can acknowledge consumption immediately after a transfer.
    pub(super) fn outcome(
        &self,
        now: Instant,
        deadline: Instant,
        grace: Duration,
        in_flight: usize,
    ) -> Option<Result<(), PasteFailure>> {
        if let Some((injected, finished)) = self.finished {
            if !injected {
                return Some(Err(PasteFailure::Cancelled));
            }
            if self.transferred
                && in_flight == 0
                && now.saturating_duration_since(finished) >= grace
            {
                return Some(Ok(()));
            }
        }
        (now >= deadline).then_some(Err(PasteFailure::Timeout))
    }
}

/// Saved MIME data belongs to one transaction. Restoration is always conditional
/// on continued ownership; cancellation never overwrites a newer user copy.
pub(super) struct SavedSelection<T>(T);

impl<T> SavedSelection<T> {
    pub(super) fn new(contents: T) -> Self {
        Self(contents)
    }

    pub(super) fn restore(
        self,
        still_owned: bool,
        write: impl FnOnce(T) -> Result<(), String>,
    ) -> Result<(), String> {
        if !still_owned {
            return Err("clipboard ownership changed during paste".into());
        }
        write(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn both_policies_preserve_formats_on_success_cancellation_and_timeout() {
        for grace in [Duration::ZERO, DATA_CONTROL_GRACE] {
            for finish in [Some(true), Some(false), None] {
                let now = Instant::now();
                let deadline = now + Duration::from_secs(2);
                let original = HashMap::from([
                    ("image/png", vec![0, 255, 1]),
                    ("text/html", b"<b>x</b>".to_vec()),
                ]);
                let saved = SavedSelection::new(original.clone());
                let mut clipboard = HashMap::from([("text/plain", b"temporary".to_vec())]);
                let mut progress = PasteProgress::default();
                progress.arm();
                if let Some(injected) = finish {
                    progress.finish(injected, now);
                }
                if finish == Some(true) {
                    progress.transferred(true);
                }
                let result = progress.outcome(deadline, deadline, grace, 0).unwrap();
                assert_eq!(
                    result,
                    match finish {
                        Some(true) => Ok(()),
                        Some(false) => Err(PasteFailure::Cancelled),
                        None => Err(PasteFailure::Timeout),
                    }
                );
                saved
                    .restore(true, |contents| {
                        clipboard = contents;
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(clipboard, original);
            }
        }
    }

    #[test]
    fn new_owner_wins_even_when_cancelled_or_transfer_completed() {
        for grace in [Duration::ZERO, DATA_CONTROL_GRACE] {
            for injected in [true, false] {
                let now = Instant::now();
                let mut progress = PasteProgress::default();
                progress.arm();
                progress.transferred(true);
                progress.finish(injected, now);
                assert!(
                    progress
                        .outcome(now + grace, now + Duration::from_secs(2), grace, 0)
                        .is_some()
                );
                let saved = SavedSelection::new(b"old copy");
                assert!(
                    saved
                        .restore(false, |_| panic!("overwrote new user copy"))
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn pre_arm_reads_non_text_and_incomplete_writes_do_not_finish_a_paste() {
        for grace in [Duration::ZERO, DATA_CONTROL_GRACE] {
            let now = Instant::now();
            let deadline = now + Duration::from_secs(2);
            let mut progress = PasteProgress::default();
            progress.transferred(true);
            progress.arm();
            progress.finish(true, now);
            assert_eq!(progress.outcome(now + grace, deadline, grace, 0), None);
            progress.transferred(false);
            assert_eq!(progress.outcome(now + grace, deadline, grace, 0), None);
            progress.transferred(true);
            assert_eq!(progress.outcome(now + grace, deadline, grace, 1), None);
            assert_eq!(
                progress.outcome(now + grace, deadline, grace, 0),
                Some(Ok(()))
            );
            progress.arm();
            assert_eq!(progress.outcome(now + grace, deadline, grace, 0), None);
        }
    }

    #[test]
    fn data_control_keeps_serving_until_grace_elapses_and_never_past_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(2);
        let mut progress = PasteProgress::default();
        progress.arm();
        progress.transferred(true);
        progress.finish(true, now);
        assert_eq!(progress.outcome(now, deadline, DATA_CONTROL_GRACE, 0), None);
        assert_eq!(
            progress.outcome(now + DATA_CONTROL_GRACE, deadline, DATA_CONTROL_GRACE, 0),
            Some(Ok(()))
        );
        assert_eq!(
            progress.outcome(deadline, deadline, DATA_CONTROL_GRACE, 1),
            Some(Err(PasteFailure::Timeout))
        );
    }
}
