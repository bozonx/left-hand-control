//! Timing and cancellation of held-layer hints, independent of a shell or worker.

use super::layer_state::HeldLayerState;
use std::time::{Duration, Instant};

/// Delay from layer activation to its hint, avoiding flashes during normal typing.
pub const HINT_DELAY: Duration = Duration::from_secs(2);

/// Desired hint visibility for a continuous mapper hold.
#[derive(Default)]
pub struct LayerHintState {
    held: HeldLayerState,
    enabled: bool,
    deadline: Option<Instant>,
}

impl LayerHintState {
    /// Observe the latest mapper snapshot. Queued events must not supply old snapshots.
    pub fn observe(&mut self, held: HeldLayerState, enabled: bool, now: Instant) {
        let new_hold = self.held.hold_id != held.hold_id;
        let new_layer = self.held.layer != held.layer;
        if !enabled || held.layer.is_none() || held.used {
            self.deadline = None;
        } else if new_hold || (!self.enabled && enabled) {
            self.deadline = Some(now + HINT_DELAY);
        } else if new_layer && !self.deadline.is_some_and(|deadline| deadline <= now) {
            // Keep an already visible hint visible when a lower layer takes over.
            self.deadline = Some(now + HINT_DELAY);
        }
        self.held = held;
        self.enabled = enabled;
    }

    /// Cancel this display, for example when an interactive popup opens.
    pub fn dismiss(&mut self) {
        self.deadline = None;
    }

    /// Layer to display now. Worker readiness and content changes do not restart the delay.
    pub fn target(&self, now: Instant) -> Option<&str> {
        self.deadline
            .filter(|deadline| *deadline <= now)
            .and(self.held.layer.as_deref())
    }

    /// Time until the hint is due; an already due or cancelled hint has no timer.
    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline
            .filter(|deadline| *deadline > now)
            .map(|deadline| deadline.duration_since(now))
    }

    /// Identity used to reject a release or re-press while the shell builds captions.
    pub fn hold_id(&self) -> u64 {
        self.held.hold_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(id: u64) -> HeldLayerState {
        HeldLayerState {
            layer: Some("nav".into()),
            hold_id: id,
            used: false,
        }
    }

    #[test]
    fn delay_survives_refreshes_and_worker_start_but_not_release_or_repress() {
        let mut hint = LayerHintState::default();
        let now = Instant::now();
        hint.observe(held(1), true, now);
        assert_eq!(hint.target(now), None);
        assert_eq!(hint.remaining(now), Some(HINT_DELAY));
        hint.observe(held(1), true, now + Duration::from_secs(1));
        assert_eq!(
            hint.remaining(now + Duration::from_secs(1)),
            Some(Duration::from_secs(1))
        );
        assert_eq!(hint.target(now + HINT_DELAY), Some("nav"));
        hint.observe(HeldLayerState::default(), true, now + HINT_DELAY);
        assert_eq!(hint.target(now + HINT_DELAY), None);
        hint.observe(held(2), true, now + HINT_DELAY);
        assert_eq!(hint.target(now + HINT_DELAY), None);
        assert_eq!(hint.hold_id(), 2);
    }

    #[test]
    fn usage_cancels_pending_and_visible_hints_until_a_new_hold() {
        for elapsed in [Duration::ZERO, HINT_DELAY] {
            let mut hint = LayerHintState::default();
            let now = Instant::now();
            hint.observe(held(1), true, now);
            hint.observe(
                HeldLayerState {
                    used: true,
                    ..held(1)
                },
                true,
                now + elapsed,
            );
            assert_eq!(hint.remaining(now + elapsed), None);
            assert_eq!(hint.target(now + HINT_DELAY), None);
            hint.observe(
                HeldLayerState {
                    layer: Some("other".into()),
                    used: true,
                    ..held(1)
                },
                true,
                now + elapsed,
            );
            assert_eq!(hint.target(now + HINT_DELAY), None);
            hint.observe(held(2), true, now + HINT_DELAY);
            assert_eq!(hint.target(now + HINT_DELAY * 2), Some("nav"));
        }
    }

    #[test]
    fn disabling_and_dismissing_cancel_resends_and_content_refreshes() {
        let mut hint = LayerHintState::default();
        let now = Instant::now();
        hint.observe(held(1), true, now);
        hint.observe(held(1), false, now + HINT_DELAY);
        assert_eq!(hint.target(now + HINT_DELAY), None);
        hint.observe(held(1), true, now + HINT_DELAY);
        assert_eq!(hint.target(now + HINT_DELAY), None);
        assert_eq!(hint.target(now + HINT_DELAY * 2), Some("nav"));
        hint.dismiss();
        hint.observe(held(1), true, now + HINT_DELAY * 3);
        assert_eq!(hint.target(now + HINT_DELAY * 3), None);
        assert_eq!(hint.remaining(now), None);
    }

    #[test]
    fn changing_the_top_layer_restarts_only_a_pending_delay() {
        let now = Instant::now();
        let mut hint = LayerHintState::default();
        hint.observe(held(1), true, now);
        hint.observe(
            HeldLayerState {
                layer: Some("symbols".into()),
                ..held(1)
            },
            true,
            now + Duration::from_secs(1),
        );
        assert_eq!(hint.target(now + HINT_DELAY), None);
        assert_eq!(
            hint.target(now + HINT_DELAY + Duration::from_secs(1)),
            Some("symbols")
        );
        hint.observe(held(1), true, now + HINT_DELAY * 2);
        assert_eq!(hint.target(now + HINT_DELAY * 2), Some("nav"));
    }
}
