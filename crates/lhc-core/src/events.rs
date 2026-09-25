//! Core → shell event delivery.
//!
//! The core emits events from background threads (mapper, watchers). Each
//! shell subscribes once at startup and forwards events to its own UI
//! runtime (Tauri `emit`, Slint `invoke_from_event_loop`, …). This is the
//! only notification channel out of the core.

use crate::active_window::ActiveWindow;
use crate::gamemode::GameModeStatus;
use crate::layout::LayoutInfo;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// The OS keyboard layout changed.
    LayoutChanged(LayoutInfo),
    /// Game-mode detection produced a new status.
    GameModeChanged(GameModeStatus),
    /// The focused window changed (`None` when it cannot be detected).
    ActiveWindowChanged(Option<ActiveWindow>),
    /// The mapper thread stopped on its own; the payload is the reason.
    MapperStopped(String),
    /// An `app:` action fired inside the mapper, e.g. `show_quick_menu_1`.
    AppAction(String),
}

type Listener = Arc<dyn Fn(&CoreEvent) + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<Mutex<Vec<Listener>>>,
}

impl EventBus {
    pub fn subscribe(&self, listener: impl Fn(&CoreEvent) + Send + Sync + 'static) {
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push(Arc::new(listener));
        }
    }

    pub fn emit(&self, event: CoreEvent) {
        let listeners = self
            .listeners
            .lock()
            .map(|listeners| listeners.clone())
            .unwrap_or_default();
        for listener in listeners {
            listener(&event);
        }
    }
}

/// Process-wide bus used by the core. Shells subscribe to it at startup.
pub fn bus() -> &'static EventBus {
    static BUS: OnceLock<EventBus> = OnceLock::new();
    BUS.get_or_init(EventBus::default)
}

pub fn emit(event: CoreEvent) {
    bus().emit(event);
}

#[cfg(test)]
mod tests {
    use super::{CoreEvent, EventBus};
    use std::sync::{Arc, Mutex};

    #[test]
    fn delivers_events_to_subscribers() {
        let events = EventBus::default();
        let received = Arc::new(Mutex::new(Vec::new()));
        let target = received.clone();
        events.subscribe(move |event| target.lock().unwrap().push(event.clone()));
        events.emit(CoreEvent::MapperStopped("boom".into()));
        assert_eq!(
            *received.lock().unwrap(),
            vec![CoreEvent::MapperStopped("boom".into())]
        );
    }
}
