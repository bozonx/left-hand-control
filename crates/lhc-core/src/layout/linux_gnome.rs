//! GNOME input sources, including the actual per-window source, from the Shell bridge.

use std::time::Duration;

use super::LayoutInfo;
use crate::platform::gnome;

fn info(source: gnome::InputSource) -> LayoutInfo {
    LayoutInfo {
        short: source.short,
        variant: source.variant,
        display: source.display,
        long: source.long,
        index: source.index,
        backend: "linux-gnome",
    }
}

pub fn current() -> Result<Option<LayoutInfo>, String> {
    let snapshot = match gnome::snapshot(false) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            clear_layout();
            return Err(error);
        }
    };
    let current = snapshot.current;
    let layout = snapshot
        .layouts
        .into_iter()
        .find(|source| Some(source.index) == current)
        .map(info);
    if let Some(layout) = &layout {
        super::publish(layout);
    } else {
        clear_layout();
    }
    Ok(layout)
}

fn clear_layout() {
    crate::runtime_state::set_layout(None);
    if let Ok(mut last) = super::LAST_PUBLISHED.lock() {
        *last = None;
    }
}

pub fn available_layouts() -> Result<Vec<LayoutInfo>, String> {
    Ok(gnome::snapshot(false)?
        .layouts
        .into_iter()
        .map(info)
        .collect())
}

pub fn set_layout(index: u32) -> Result<(), String> {
    if index >= 10 {
        return Err("GNOME input source index is out of range".into());
    }
    gnome::action(&format!("switchLayout{}", index + 1))?;
    let _ = current();
    Ok(())
}

static SIGNAL_CONNECTION: std::sync::Mutex<Option<zbus::blocking::Connection>> =
    std::sync::Mutex::new(None);

pub(super) fn interrupt_signal_watcher() {
    if let Some(connection) = SIGNAL_CONNECTION
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
    {
        let _ = connection.close();
    }
}

const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const STOP_POLL: Duration = Duration::from_millis(100);

pub fn start_watcher() {
    super::register_watcher(
        std::thread::Builder::new()
            .name("layout-gnome".into())
            .spawn(|| {
                while !super::watcher_stop_requested() {
                    if let Err(error) = watch_once() {
                        log::debug!("[layout/gnome] {error}");
                    }
                    // Wait before reconnecting, checking for a stop meanwhile.
                    for _ in 0..RECONNECT_DELAY.as_millis() / STOP_POLL.as_millis() {
                        if super::watcher_stop_requested() {
                            return;
                        }
                        std::thread::sleep(STOP_POLL);
                    }
                }
            }),
    );
}

fn watch_once() -> Result<(), String> {
    let connection = zbus::blocking::Connection::session().map_err(|error| error.to_string())?;
    if let Ok(mut slot) = SIGNAL_CONNECTION.lock() {
        *slot = Some(connection.clone());
    }
    if super::watcher_stop_requested() {
        return Ok(());
    }
    let proxy =
        zbus::blocking::Proxy::new(&connection, gnome::SERVICE, gnome::PATH, gnome::INTERFACE)
            .map_err(|error| error.to_string())?;
    let signals = proxy
        .receive_all_signals()
        .map_err(|error| error.to_string())?;
    let _ = current();
    for _ in signals {
        if super::watcher_stop_requested() {
            return Ok(());
        }
        crate::active_window::wake();
        let _ = current();
    }
    Ok(())
}
