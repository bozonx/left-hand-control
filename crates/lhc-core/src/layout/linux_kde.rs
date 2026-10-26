//! KDE Plasma keyboard layout backend (Wayland + X11).
//!
//! Current product support is Linux/KDE, so this backend stays fully native:
//! it talks to DBus directly through `zbus`, which the project already ships
//! for the portal backend. Other desktops remain explicit skeletons for now.
//! Every successful read goes through `super::publish`, which updates the
//! mapper cache and emits `LayoutChanged` only on real changes.
//!
//! The watcher listens to the interface's signals (`layoutChanged`,
//! `layoutListChanged`) and re-reads the layout on each, so nothing is
//! polled. Switching the active layout goes through the same DBus interface
//! (`setLayout(uint)`).

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};

use super::LayoutInfo;

/// Pause before the signal watcher reconnects.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

type LayoutList = Vec<(String, String, String)>;

use crate::platform::dbus::{
    KEYBOARD_LAYOUTS_INTERFACE as IFACE, KEYBOARD_LAYOUTS_PATH as OBJECT,
    KEYBOARD_SERVICE as SERVICE, SERVICE_UNKNOWN_ERROR,
};

/// Session-bus connection for one-shot calls, opened once: layout
/// actions call in from the key path.
fn session_bus() -> Result<&'static Connection, String> {
    static BUS: OnceLock<Connection> = OnceLock::new();
    if let Some(bus) = BUS.get() {
        return Ok(bus);
    }
    let bus = Connection::session().map_err(|e| format!("Cannot connect to session bus: {e}"))?;
    Ok(BUS.get_or_init(|| bus))
}

fn keyboard_proxy(conn: &Connection) -> Result<Proxy<'static>, String> {
    Proxy::new(conn, SERVICE, OBJECT, IFACE).map_err(|e| format!("create keyboard proxy: {e}"))
}

pub fn current() -> Result<Option<LayoutInfo>, String> {
    current_with_proxy(&keyboard_proxy(session_bus()?)?)
}

pub fn refresh_cache() -> Result<Option<LayoutInfo>, String> {
    current()
}

pub fn available_layouts() -> Result<Vec<LayoutInfo>, String> {
    let proxy = keyboard_proxy(session_bus()?)?;
    let list = match call_list(&proxy)? {
        Some(v) => v,
        None => return Ok(vec![]),
    };
    let variants = configured_variants(&list);
    Ok(list
        .into_iter()
        .enumerate()
        .map(|(idx, entry)| LayoutInfo {
            short: entry.0,
            display: entry.1,
            long: entry.2,
            variant: variants.get(idx).cloned().unwrap_or_default(),
            index: idx as u32,
            backend: "linux-kde",
        })
        .collect())
}

/// Switch the active layout to the given zero-based index.
///
/// Backed by `org.kde.KeyboardLayouts.setLayout(uint) -> bool`. The
/// boolean return is `false` when the index is out of range; we surface
/// that as an error so the shell can fall back to refreshing the
/// list and retrying.
pub fn set_layout(index: u32) -> Result<(), String> {
    let proxy = keyboard_proxy(session_bus()?)?;
    let msg = proxy
        .call_method("setLayout", &(index,))
        .map_err(|e| format!("Failed to switch layout to {index}: {e}"))?;
    let ok: bool = msg
        .body()
        .deserialize()
        .map_err(|e| format!("Failed to decode setLayout response: {e}"))?;
    if !ok {
        return Err(format!("Layout switch to {index} was rejected by KDE"));
    }
    let _ = current_with_proxy(&proxy);
    Ok(())
}

pub fn start_watcher() {
    super::register_watcher(
        thread::Builder::new()
            .name("layout-kde-signal".into())
            .spawn(run_signal_watcher),
    );
}

fn run_signal_watcher() {
    while !super::watcher_stop_requested() {
        match signal_watch_once() {
            Ok(()) => return,
            Err(e) => {
                log::debug!(
                    "[layout/kde] signal watcher error: {e}; retrying in {RECONNECT_DELAY:?}"
                );
                thread::sleep(RECONNECT_DELAY);
            }
        }
    }
}

/// Connection the signal watcher blocks on; closed to wake it for stop.
static SIGNAL_CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

/// End the signal watcher's wait so it notices the stop request.
pub(super) fn interrupt_signal_watcher() {
    let connection = SIGNAL_CONNECTION
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(connection) = connection
        && let Err(e) = connection.close()
    {
        log::debug!("[layout/kde] close signal connection: {e}");
    }
}

fn signal_watch_once() -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    if let Ok(mut slot) = SIGNAL_CONNECTION.lock() {
        *slot = Some(conn.clone());
    }
    // A stop requested before the connection was published.
    if super::watcher_stop_requested() {
        return Ok(());
    }
    let proxy = keyboard_proxy(&conn)?;
    // Both a switch and an edited layout list can change the result.
    let signals = proxy
        .receive_all_signals()
        .map_err(|e| format!("subscribe to keyboard signals: {e}"))?;
    // Read once after subscribing, so a change in between is not lost.
    emit_current(&proxy);
    for _msg in signals {
        if super::watcher_stop_requested() {
            return Ok(());
        }
        emit_current(&proxy);
    }
    if super::watcher_stop_requested() {
        return Ok(());
    }
    Err("keyboard signal stream ended".to_string())
}

fn emit_current(proxy: &Proxy<'_>) {
    if let Err(e) = current_with_proxy(proxy) {
        log::debug!("[layout/kde] read error: {e}");
    }
}

fn current_with_proxy(proxy: &Proxy<'_>) -> Result<Option<LayoutInfo>, String> {
    let list = match call_list(proxy)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let idx = call_index(proxy)?.unwrap_or(0);
    let position = if (idx as usize) < list.len() {
        idx as usize
    } else {
        0
    };
    let entry = list
        .get(position)
        .cloned()
        .ok_or_else(|| "No keyboard layouts configured".to_string())?;
    let variant = configured_variants(&list)
        .get(position)
        .cloned()
        .unwrap_or_default();
    let info = LayoutInfo {
        short: entry.0,
        display: entry.1,
        long: entry.2,
        variant,
        index: idx,
        backend: "linux-kde",
    };
    super::publish(&info);
    Ok(Some(info))
}

/// XKB variants of the configured layouts, by index. The DBus API only
/// gives layout codes and user labels, so they come from `kxkbrc`, and only
/// when its layout list matches `list`; otherwise none are known.
fn configured_variants(list: &LayoutList) -> Vec<String> {
    let Some(path) = dirs::config_dir().map(|dir| dir.join("kxkbrc")) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    variants_from_kxkbrc(&text, list)
}

fn variants_from_kxkbrc(text: &str, list: &LayoutList) -> Vec<String> {
    let mut in_layout = false;
    let mut layouts = None;
    let mut variants = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
            continue;
        }
        if !in_layout {
            continue;
        }
        if let Some(value) = line.strip_prefix("LayoutList=") {
            layouts = Some(value);
        } else if let Some(value) = line.strip_prefix("VariantList=") {
            variants = Some(value);
        }
    }
    let split =
        |value: &str| -> Vec<String> { value.split(',').map(|v| v.trim().to_owned()).collect() };
    let (Some(layouts), Some(variants)) = (layouts.map(split), variants.map(split)) else {
        return Vec::new();
    };
    let matches = layouts.len() == list.len()
        && layouts
            .iter()
            .zip(list)
            .all(|(layout, entry)| *layout == entry.0);
    if !matches {
        return Vec::new();
    }
    (0..list.len())
        .map(|idx| variants.get(idx).cloned().unwrap_or_default())
        .collect()
}

fn call_index(proxy: &Proxy<'_>) -> Result<Option<u32>, String> {
    let msg = match proxy.call_method("getLayout", &()) {
        Ok(msg) => msg,
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == SERVICE_UNKNOWN_ERROR => {
            return Ok(None);
        }
        Err(e) => return Err(format!("Failed to get current layout: {e}")),
    };
    let idx: u32 = msg
        .body()
        .deserialize()
        .map_err(|e| format!("Failed to decode getLayout response: {e}"))?;
    Ok(Some(idx))
}

fn call_list(proxy: &Proxy<'_>) -> Result<Option<LayoutList>, String> {
    let msg = match proxy.call_method("getLayoutsList", &()) {
        Ok(msg) => msg,
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == SERVICE_UNKNOWN_ERROR => {
            return Ok(None);
        }
        Err(e) => return Err(format!("Failed to get layout list: {e}")),
    };
    let list: Vec<(String, String, String)> = msg
        .body()
        .deserialize()
        .map_err(|e| format!("Failed to decode getLayoutsList response: {e}"))?;
    Ok(Some(list))
}

pub(super) fn xkb_options() -> String {
    dirs::config_dir()
        .and_then(|dir| std::fs::read_to_string(dir.join("kxkbrc")).ok())
        .map(|text| options_from_kxkbrc(&text))
        .unwrap_or_default()
}

fn options_from_kxkbrc(text: &str) -> String {
    let mut in_layout = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
        } else if in_layout && let Some(options) = line.strip_prefix("Options=") {
            return options.trim().into();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(codes: &[&str]) -> LayoutList {
        codes
            .iter()
            .map(|code| (code.to_string(), String::new(), String::new()))
            .collect()
    }

    #[test]
    fn variants_come_from_matching_kxkbrc() {
        let text =
            "[Layout]\nDisplayNames=,lat\nLayoutList=us,latam\nVariantList=dvorak,\nUse=true\n";
        assert_eq!(
            variants_from_kxkbrc(text, &list(&["us", "latam"])),
            ["dvorak", ""]
        );
    }

    #[test]
    fn variants_are_unknown_when_kxkbrc_differs() {
        let text = "[Layout]\nLayoutList=us,ru\nVariantList=dvorak,\n";
        assert!(variants_from_kxkbrc(text, &list(&["ru", "us"])).is_empty());
        let other_section = "[Other]\nLayoutList=us\nVariantList=dvorak\n";
        assert!(variants_from_kxkbrc(other_section, &list(&["us"])).is_empty());
    }
}
