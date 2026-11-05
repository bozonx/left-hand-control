//! Revision-aware background loading, independent of Slint and worker scheduling.

#[derive(Default)]
pub(super) struct LoadState {
    revision: Option<u64>,
    generation: u64,
    loading: bool,
    loaded: bool,
}

impl LoadState {
    /// Invalidate the cache without starting a second overlapping worker.
    pub fn invalidate(&mut self, revision: u64) -> bool {
        if self.revision == Some(revision) {
            return false;
        }
        self.revision = Some(revision);
        self.generation = self.generation.wrapping_add(1);
        self.loaded = false;
        true
    }

    pub fn begin(&mut self) -> Option<u64> {
        if self.loading || self.loaded {
            return None;
        }
        self.loading = true;
        Some(self.generation)
    }

    /// False means the result belongs to an older document revision.
    pub fn complete(&mut self, generation: u64, success: bool) -> bool {
        self.loading = false;
        if generation != self.generation {
            return false;
        }
        self.loaded = success;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_is_cached_but_failures_can_be_retried() {
        let mut state = LoadState::default();
        assert!(state.invalidate(1));
        let first = state.begin().unwrap();
        assert!(state.begin().is_none());
        assert!(state.complete(first, false));
        let retry = state.begin().unwrap();
        assert!(state.complete(retry, true));
        assert!(!state.invalidate(1));
        assert!(state.begin().is_none());
    }

    #[test]
    fn stale_success_and_failure_are_discarded_and_latest_revision_can_load() {
        for success in [true, false] {
            let mut state = LoadState::default();
            state.invalidate(1);
            let old = state.begin().unwrap();
            state.invalidate(2);
            state.invalidate(3);
            assert!(state.begin().is_none());
            assert!(!state.complete(old, success));
            let latest = state.begin().unwrap();
            assert_ne!(latest, old);
            assert!(state.complete(latest, true));
            assert!(state.begin().is_none());
        }
    }
}
