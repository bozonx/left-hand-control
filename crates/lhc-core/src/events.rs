//! Core → shell event delivery.
//!
//! The core emits events from background threads (mapper, watchers). Each
//! shell subscribes once at startup and forwards events to its own UI
//! runtime (for example, Slint `invoke_from_event_loop`). This is the
//! only notification channel out of the core.

use crate::active_window::ActiveWindow;
use crate::gamemode::GameModeStatus;
use crate::keyboard_layout::KeyboardLayoutInfo;
use std::sync::{
    Arc, Mutex, OnceLock, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// The OS keyboard layout changed.
    LayoutChanged(KeyboardLayoutInfo),
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

/// Each subscription buffers at most this many events. A full subscriber
/// drops new events and logs the loss; producers never wait for callbacks.
const SUBSCRIBER_QUEUE: usize = 1024;

type Listeners = Mutex<Vec<Arc<Listener>>>;

struct Listener {
    tx: mpsc::SyncSender<CoreEvent>,
    active: Arc<AtomicBool>,
}

/// Keeps a callback subscribed. Dropping it revokes queued delivery without
/// waiting for a callback that may already be running.
#[must_use = "dropping the subscription stops event delivery"]
pub struct Subscription {
    listener: Arc<Listener>,
    listeners: Weak<Listeners>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.listener.active.store(false, Ordering::Release);
        if let Some(listeners) = self.listeners.upgrade() {
            listeners
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|listener| !Arc::ptr_eq(listener, &self.listener));
        }
    }
}

#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<Listeners>,
}

impl EventBus {
    /// Deliver events in order on this subscription's worker thread. Slow or
    /// panicking callbacks cannot block or unwind through an emitting thread.
    pub fn subscribe(
        &self,
        listener: impl Fn(&CoreEvent) + Send + 'static,
    ) -> Result<Subscription, String> {
        let (tx, rx) = mpsc::sync_channel(SUBSCRIBER_QUEUE);
        let active = Arc::new(AtomicBool::new(true));
        let running = active.clone();
        std::thread::Builder::new()
            .name("lhc-events".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    if !running.load(Ordering::Acquire) {
                        break;
                    }
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(&event)))
                        .is_err()
                    {
                        log::error!("core event subscriber panicked");
                    }
                }
            })
            .map_err(|error| format!("start event subscriber: {error}"))?;
        let listener = Arc::new(Listener { tx, active });
        self.listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(listener.clone());
        Ok(Subscription {
            listener,
            listeners: Arc::downgrade(&self.listeners),
        })
    }

    pub fn emit(&self, event: CoreEvent) {
        let listeners = self
            .listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        for listener in listeners {
            if listener.active.load(Ordering::Acquire) {
                match listener.tx.try_send(event.clone()) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        log::error!("core event subscriber queue full; event dropped")
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        log::error!("core event subscriber stopped")
                    }
                }
            }
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
    use super::*;
    use std::time::Duration;

    #[test]
    fn delivers_in_order_and_unsubscribes_on_drop() {
        let bus = EventBus::default();
        let (tx, rx) = mpsc::channel();
        let subscription = bus
            .subscribe(move |event| tx.send(event.clone()).unwrap())
            .unwrap();
        bus.emit(CoreEvent::MapperStopped("first".into()));
        bus.emit(CoreEvent::MapperStopped("second".into()));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            CoreEvent::MapperStopped("first".into())
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            CoreEvent::MapperStopped("second".into())
        );
        drop(subscription);
        assert!(bus.listeners.lock().unwrap().is_empty());
        bus.emit(CoreEvent::LayerKeyPressed);
        assert!(rx.recv_timeout(Duration::from_secs(1)).is_err());
    }

    #[test]
    fn blocked_subscriber_cannot_block_producers_or_other_subscribers() {
        let bus = EventBus::default();
        let (entered, entry) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let slow = bus
            .subscribe(move |_| {
                entered.send(()).unwrap();
                wait.recv().unwrap();
            })
            .unwrap();
        bus.emit(CoreEvent::LayerKeyPressed);
        entry.recv_timeout(Duration::from_secs(1)).unwrap();
        // Even filling the slow subscriber's queue must return.
        for _ in 0..SUBSCRIBER_QUEUE + 1 {
            bus.emit(CoreEvent::LayerKeyPressed);
        }
        let (tx, rx) = mpsc::channel();
        let _fast = bus
            .subscribe(move |_| {
                tx.send(()).unwrap();
            })
            .unwrap();
        bus.emit(CoreEvent::LayerKeyPressed);
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(slow);
        release.send(()).unwrap();
    }

    #[test]
    fn panicking_callback_does_not_stop_later_delivery() {
        let bus = EventBus::default();
        let (tx, rx) = mpsc::channel();
        let _subscription = bus
            .subscribe(move |event| {
                if matches!(event, CoreEvent::LayerKeyPressed) {
                    panic!("subscriber failure");
                }
                tx.send(()).unwrap();
            })
            .unwrap();
        bus.emit(CoreEvent::LayerKeyPressed);
        bus.emit(CoreEvent::MapperDeviceChanged);
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
    }
}
