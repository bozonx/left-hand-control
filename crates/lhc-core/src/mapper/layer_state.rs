//! Current held-layer state, shared with shells independently of UI event queues.

use std::sync::{Mutex, OnceLock};

/// Identity and use of the currently held mapper layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldLayerState {
    pub layer: Option<String>,
    /// Unique across releases, re-presses and mapper restarts.
    pub hold_id: u64,
    /// A key was used during this continuous hold, including lower layers.
    pub used: bool,
}

impl HeldLayerState {
    /// Whether a delayed display request still belongs to an unused hold.
    pub fn allows_hint(&self, layer: &str, hold_id: u64) -> bool {
        !self.used && self.hold_id == hold_id && self.layer.as_deref() == Some(layer)
    }
}

fn state() -> &'static Mutex<HeldLayerState> {
    static STATE: OnceLock<Mutex<HeldLayerState>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

/// Read the latest mapper state without waiting for a shell event queue.
pub fn current() -> HeldLayerState {
    state()
        .lock()
        .map(|state| state.clone())
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
pub(crate) fn next_hold_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(target_os = "linux")]
pub(crate) fn publish(next: HeldLayerState) {
    let previous = {
        let Ok(mut state) = state().lock() else {
            return;
        };
        if *state == next {
            return;
        }
        std::mem::replace(&mut *state, next.clone())
    };
    if previous.hold_id != next.hold_id || previous.layer != next.layer {
        super::notify_active_layer(next.layer.clone());
    }
    if next.used && (!previous.used || previous.hold_id != next.hold_id) {
        super::notify_layer_key_pressed();
    }
}

#[cfg(test)]
mod tests {
    use super::HeldLayerState;

    #[test]
    fn delayed_hint_is_rejected_after_release_usage_or_repress_of_the_same_layer() {
        let held = HeldLayerState {
            layer: Some("nav".into()),
            hold_id: 1,
            used: false,
        };
        assert!(held.allows_hint("nav", 1));
        assert!(
            !HeldLayerState {
                used: true,
                ..held.clone()
            }
            .allows_hint("nav", 1)
        );
        assert!(
            !HeldLayerState {
                layer: None,
                ..held.clone()
            }
            .allows_hint("nav", 1)
        );
        assert!(
            !HeldLayerState {
                hold_id: 2,
                ..held.clone()
            }
            .allows_hint("nav", 1)
        );
        assert!(!held.allows_hint("symbols", 1));
    }
}
