//! Interruptible D-Bus watching and reconnect delays for layout backends.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub(super) trait SignalConnection {
    fn interrupt(self);
}

impl SignalConnection for zbus::blocking::Connection {
    fn interrupt(self) {
        if let Err(error) = self.close() {
            log::debug!("[layout] close signal connection: {error}");
        }
    }
}

struct State<C> {
    stopped: bool,
    connection: Option<C>,
}

pub(super) struct SignalWatcher<C = zbus::blocking::Connection> {
    state: Mutex<State<C>>,
    wake: Condvar,
}

impl<C: SignalConnection> SignalWatcher<C> {
    pub(super) const fn new() -> Self {
        Self {
            state: Mutex::new(State {
                stopped: false,
                connection: None,
            }),
            wake: Condvar::new(),
        }
    }

    pub(super) fn reset(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).stopped = false;
    }

    pub(super) fn stop(&self) {
        let connection = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.stopped = true;
            self.wake.notify_all();
            state.connection.take()
        };
        if let Some(connection) = connection {
            connection.interrupt();
        }
    }

    /// Publish before subscribing, so stopping can interrupt a blocking stream.
    /// A connection opened concurrently with stop is closed immediately.
    pub(super) fn connected(&self, connection: C) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.stopped {
            drop(state);
            connection.interrupt();
            false
        } else {
            state.connection = Some(connection);
            true
        }
    }

    fn stopped(&self) -> bool {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).stopped
    }

    fn wait(&self, delay: Duration) {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        drop(
            self.wake
                .wait_timeout_while(state, delay, |state| !state.stopped)
                .unwrap_or_else(|e| e.into_inner()),
        );
    }

    pub(super) fn run(
        &self,
        backend: &str,
        delay: Duration,
        watch: impl FnMut() -> Result<(), String>,
    ) {
        self.run_with_wait(backend, watch, || self.wait(delay));
    }

    fn run_with_wait(
        &self,
        backend: &str,
        mut watch: impl FnMut() -> Result<(), String>,
        mut wait: impl FnMut(),
    ) {
        while !self.stopped() {
            match watch() {
                Ok(()) => break,
                Err(error) => log::debug!("[layout/{backend}] {error}; reconnecting"),
            }
            wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Connection(Arc<AtomicUsize>);
    impl SignalConnection for Connection {
        fn interrupt(self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn stop_interrupts_connections_including_ones_opened_during_stop() {
        let watcher = SignalWatcher::new();
        let closed = Arc::new(AtomicUsize::new(0));
        assert!(watcher.connected(Connection(closed.clone())));
        watcher.stop();
        watcher.stop();
        assert_eq!(closed.load(Ordering::SeqCst), 1);
        assert!(!watcher.connected(Connection(closed.clone())));
        assert_eq!(closed.load(Ordering::SeqCst), 2);
        watcher.reset();
        assert!(watcher.connected(Connection(closed.clone())));
        watcher.stop();
        assert_eq!(closed.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn reconnects_on_failure_and_stops_before_another_attempt_without_sleeping() {
        let watcher = SignalWatcher::<Connection>::new();
        let mut attempts = 0;
        let mut waits = 0;
        watcher.run_with_wait(
            "test",
            || {
                attempts += 1;
                Err("disconnected".into())
            },
            || {
                waits += 1;
                if waits == 2 {
                    watcher.stop();
                }
            },
        );
        assert_eq!(attempts, 2);
        assert_eq!(waits, 2);
        watcher.reset();
        watcher.run_with_wait(
            "test",
            || {
                attempts += 1;
                Ok(())
            },
            || panic!("successful stop retried"),
        );
        assert_eq!(attempts, 3);
    }

    #[test]
    fn stop_wakes_a_reconnect_wait() {
        let watcher = Arc::new(SignalWatcher::<Connection>::new());
        let other = watcher.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            other.wait(Duration::from_secs(60));
            tx.send(()).unwrap();
        });
        watcher.stop();
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        thread.join().unwrap();
    }
}
