//! GNOME input sources, including the actual per-window source, from the Shell bridge.

use std::time::Duration;

use super::KeyboardLayoutInfo;
use crate::platform::gnome;

fn info(source: gnome::InputSource) -> KeyboardLayoutInfo {
    KeyboardLayoutInfo {
        code: source.short,
        variant: source.variant,
        display_label: source.display,
        name: source.long,
        index: source.index,
        backend: "linux-gnome",
    }
}

pub fn current() -> Result<Option<KeyboardLayoutInfo>, String> {
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
    crate::runtime_state::set_keyboard_layout(None);
    if let Ok(mut last) = super::LAST_PUBLISHED.lock() {
        last.last = None;
    }
}

pub fn available_layouts() -> Result<Vec<KeyboardLayoutInfo>, String> {
    Ok(gnome::snapshot(false)?
        .layouts
        .into_iter()
        .map(info)
        .collect())
}

pub fn set_layout(index: u32) -> Result<(), String> {
    if index >= crate::profile::actions::SYSTEM_NUMBERED_COUNT {
        return Err("GNOME input source index is out of range".into());
    }
    gnome::action(&format!("switchLayout{}", index + 1))?;
    let _ = current();
    Ok(())
}

static SIGNAL_WATCHER: super::signal_watcher::SignalWatcher =
    super::signal_watcher::SignalWatcher::new();

pub(super) fn interrupt_signal_watcher() {
    SIGNAL_WATCHER.stop();
}

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

pub fn start_watcher() {
    SIGNAL_WATCHER.reset();
    super::register_watcher(
        std::thread::Builder::new()
            .name("layout-gnome".into())
            .spawn(|| SIGNAL_WATCHER.run("gnome", RECONNECT_DELAY, watch_once)),
    );
}

fn watch_once() -> Result<(), String> {
    let connection = zbus::blocking::Connection::session().map_err(|error| error.to_string())?;
    if !SIGNAL_WATCHER.connected(connection.clone()) {
        return Ok(());
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
    if super::watcher_stop_requested() {
        return Ok(());
    }
    Err("keyboard signal stream ended".into())
}
