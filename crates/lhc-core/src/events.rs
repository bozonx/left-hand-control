//! Core → shell event delivery.
//!
//! The core emits events from background threads (mapper, watchers). Each
//! shell subscribes once at startup and forwards events to its own UI
//! runtime (for example, Slint `invoke_from_event_loop`). This is the
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
    /// Keyboard availability changed while the mapper thread remains alive.
    MapperDeviceChanged,
    /// The topmost held mapper layer changed; `None` when no layer is held.
    ActiveLayerChanged(Option<String>),
    /// A key was pressed inside the held layer.
    LayerKeyPressed,
    /// An `app:` action fired inside the mapper, e.g. `show_quick_menu_1`.
    AppAction(String),
    CommandFinished {
        script: String,
        result: Result<(), String>,
    },
    /// A `text:` action was not typed.
    TextInputFailed(TextInputFailure),
    /// A mapper system action could not be submitted or executed.
    MapperActionFailed(String),
    /// A bound action cannot run on this OS or desktop and was skipped;
    /// the payload is the action string.
    ActionUnavailable(String),
}

/// Why a `text:` action was not typed. Details are untranslated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputFailure {
    /// The active keyboard layout lacks a character of the text.
    MissingCharacters,
    /// The active keyboard layout is unknown, so keys cannot be chosen.
    LayoutUnknown,
    /// The input backend could not start or stopped working.
    BackendUnavailable(String),
    /// The destination lost focus or could not be identified.
    DestinationChanged,
    /// Safe clipboard access or the paste transfer failed.
    Clipboard(String),
    /// The external typing tool failed.
    Tool(String),
}

type Listener = Arc<dyn Fn(&CoreEvent) + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<Mutex<Vec<Listener>>>,
}

impl EventBus {
    /// Register `listener` for every later event. It runs on the emitting
    /// thread, often the mapper thread with the keyboard grabbed, so it must
    /// return quickly and never block (post to a UI loop instead).
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
