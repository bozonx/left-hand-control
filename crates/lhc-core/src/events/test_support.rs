//! Thread-local event routing for isolated core unit tests.

use super::{CoreEvent, EventBus};
use std::{cell::RefCell, marker::PhantomData, ops::Deref, rc::Rc};

thread_local! {
    static OVERRIDE: RefCell<Option<EventBus>> = const { RefCell::new(None) };
}

/// Restore the previous route when a test scope ends, including on panic.
pub(crate) struct IsolatedBus {
    bus: EventBus,
    previous: Option<EventBus>,
    _thread: PhantomData<Rc<()>>,
}

pub(crate) fn isolated_bus() -> IsolatedBus {
    let bus = EventBus::default();
    let previous = OVERRIDE.with(|slot| slot.replace(Some(bus.clone())));
    IsolatedBus {
        bus,
        previous,
        _thread: PhantomData,
    }
}

impl Deref for IsolatedBus {
    type Target = EventBus;

    fn deref(&self) -> &EventBus {
        &self.bus
    }
}

impl Drop for IsolatedBus {
    fn drop(&mut self) {
        OVERRIDE.with(|slot| slot.replace(self.previous.take()));
    }
}

pub(super) fn route(event: &CoreEvent) -> bool {
    OVERRIDE.with_borrow(|bus| {
        if let Some(bus) = bus {
            bus.emit(event.clone());
            true
        } else {
            false
        }
    })
}

fn subscribe(bus: &EventBus) -> (super::Subscription, std::sync::mpsc::Receiver<String>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let subscription = bus
        .subscribe(move |event| {
            if let CoreEvent::ActionUnavailable(action) = event {
                sender.send(action.clone()).unwrap();
            }
        })
        .unwrap();
    (subscription, receiver)
}

fn expect(receiver: &std::sync::mpsc::Receiver<String>, expected: &str) {
    assert_eq!(
        receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap(),
        expected
    );
}

#[test]
fn nested_scope_restores_the_previous_route() {
    let outer = isolated_bus();
    let (_subscription, received) = subscribe(&outer);
    super::emit(CoreEvent::ActionUnavailable("before".into()));
    expect(&received, "before");
    {
        let inner = isolated_bus();
        let (_subscription, received) = subscribe(&inner);
        super::emit(CoreEvent::ActionUnavailable("inner".into()));
        expect(&received, "inner");
    }
    super::emit(CoreEvent::ActionUnavailable("after".into()));
    expect(&received, "after");
}

#[test]
fn concurrent_scopes_keep_their_own_events() {
    let bus = isolated_bus();
    let (_subscription, received) = subscribe(&bus);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let ready = barrier.clone();
    let worker = std::thread::spawn(move || {
        let bus = isolated_bus();
        let (_subscription, received) = subscribe(&bus);
        ready.wait();
        super::emit(CoreEvent::ActionUnavailable("worker".into()));
        expect(&received, "worker");
    });
    barrier.wait();
    super::emit(CoreEvent::ActionUnavailable("main".into()));
    expect(&received, "main");
    worker.join().unwrap();
}
