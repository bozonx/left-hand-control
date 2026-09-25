use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    LayoutChanged,
    GameModeChanged,
    MapperStatusChanged,
    MapperError(String),
}

type Listener = Arc<dyn Fn(CoreEvent) + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<Mutex<Vec<Listener>>>,
}

impl EventBus {
    pub fn subscribe(&self, listener: impl Fn(CoreEvent) + Send + Sync + 'static) {
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
            listener(event.clone());
        }
    }
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
        events.subscribe(move |event| target.lock().unwrap().push(event));
        events.emit(CoreEvent::MapperStatusChanged);
        assert_eq!(
            *received.lock().unwrap(),
            vec![CoreEvent::MapperStatusChanged]
        );
    }
}
