//! Lifetime of the system watchers (keyboard layout, game mode, active
//! window) that feed rule conditions and layout auto-switching.

use crate::storage::StoragePaths;

/// Running system watchers; dropping it stops them all.
///
/// The watchers publish into process-wide caches, so keep a single guard
/// per process.
#[must_use = "watchers stop when the guard is dropped"]
pub struct Watchers {
    _private: (),
}

impl Watchers {
    /// Start every watcher. `storage` lets game mode read its persisted
    /// settings; `ignore_own_windows` hides this process's windows from
    /// app conditions (settings and popups must not change them while open).
    pub fn start(storage: Option<StoragePaths>, ignore_own_windows: bool) -> Self {
        crate::layout::start_watcher();
        crate::gamemode::start_watcher(storage);
        crate::active_window::ignore_own_windows(ignore_own_windows);
        crate::active_window::start_watcher();
        Self { _private: () }
    }
}

impl Drop for Watchers {
    fn drop(&mut self) {
        crate::layout::stop_watcher();
        crate::gamemode::stop_watcher();
        crate::active_window::stop_watcher();
    }
}
