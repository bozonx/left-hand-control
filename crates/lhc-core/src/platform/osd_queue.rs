//! Ordered OSD delivery with one replaceable pending message and one sender.

use std::sync::{Arc, Mutex, OnceLock};

#[derive(Default)]
struct Pending {
    text: Option<String>,
    running: bool,
}

#[derive(Default)]
struct Queue {
    pending: Mutex<Pending>,
}

impl Queue {
    fn submit(
        self: &Arc<Self>,
        text: String,
        show: impl Fn(&str) -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
        pending.text = Some(text);
        if pending.running {
            return Ok(());
        }
        pending.running = true;
        let queue = self.clone();
        // Keep the lock until spawn succeeds; another submit must not get a
        // successful result for a sender that failed to start.
        if let Err(error) = std::thread::Builder::new()
            .name("lhc-osd".into())
            .spawn(move || {
                loop {
                    let text = {
                        let Ok(mut pending) = queue.pending.lock() else {
                            return;
                        };
                        let Some(text) = pending.text.take() else {
                            pending.running = false;
                            return;
                        };
                        text
                    };
                    if let Err(error) = show(&text) {
                        log::warn!("[osd] {error}");
                    }
                }
            })
        {
            pending.running = false;
            pending.text = None;
            return Err(format!("start OSD sender: {error}"));
        }
        Ok(())
    }
}

/// Enqueue an OSD without blocking the caller. New messages replace any waiting
/// message; an in-flight D-Bus call finishes before the next message is sent.
pub fn enqueue(text: String) -> Result<(), String> {
    static QUEUE: OnceLock<Arc<Queue>> = OnceLock::new();
    QUEUE
        .get_or_init(Arc::default)
        .submit(text, super::osd::show)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn slow_or_failed_delivery_preserves_order_and_coalesces_waiting_messages() {
        for fail in [false, true] {
            let queue = Arc::new(Queue::default());
            let (shown_tx, shown_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            queue
                .submit("A".into(), move |text| {
                    shown_tx.send(text.to_owned()).unwrap();
                    if text == "A" {
                        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                        if fail {
                            return Err("D-Bus unavailable".into());
                        }
                    }
                    Ok(())
                })
                .unwrap();
            assert_eq!(shown_rx.recv_timeout(Duration::from_secs(5)).unwrap(), "A");
            // These closures must never run: the existing sender owns delivery.
            queue
                .submit("B".into(), |_| panic!("second sender"))
                .unwrap();
            queue
                .submit("C".into(), |_| panic!("third sender"))
                .unwrap();
            release_tx.send(()).unwrap();
            assert_eq!(shown_rx.recv_timeout(Duration::from_secs(5)).unwrap(), "C");
        }
    }
}
