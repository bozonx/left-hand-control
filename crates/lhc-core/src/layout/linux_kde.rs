// KDE Plasma keyboard layout backend (Wayland + X11).
//
// Current product support is Linux/KDE, so this backend stays fully native:
// it talks to DBus directly through `zbus`, which the project already ships
// for the portal backend. Other desktops remain explicit skeletons for now.
// Every successful read goes through `super::publish`, which updates the
// mapper cache and emits `LayoutChanged` only on real changes.
//
// The watcher polls the DBus interface at a short interval so it can stop
// promptly during app shutdown. Switching the active layout goes through
// the same DBus interface (`setLayout(uint)`).

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};

use super::LayoutInfo;

type LayoutList = Vec<(String, String, String)>;

const SERVICE: &str = "org.kde.keyboard";
const OBJECT: &str = "/Layouts";
const IFACE: &str = "org.kde.KeyboardLayouts";

pub fn current() -> Result<Option<LayoutInfo>, String> {
    let conn = Connection::session().map_err(|e| format!("Cannot connect to session bus: {e}"))?;
    current_with_conn(&conn)
}

pub fn refresh_cache() -> Result<Option<LayoutInfo>, String> {
    current()
}

pub fn available_layouts() -> Result<Vec<LayoutInfo>, String> {
    let conn = Connection::session().map_err(|e| format!("Cannot connect to session bus: {e}"))?;
    let proxy = Proxy::new(&conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("Cannot create keyboard proxy: {e}"))?;
    let list = match call_list(&proxy)? {
        Some(v) => v,
        None => return Ok(vec![]),
    };
    Ok(list
        .into_iter()
        .enumerate()
        .map(|(idx, entry)| LayoutInfo {
            short: entry.0,
            display: entry.1,
            long: entry.2,
            index: idx as u32,
            backend: "linux-kde",
        })
        .collect())
}

/// Switch the active layout to the given zero-based index.
///
/// Backed by `org.kde.KeyboardLayouts.setLayout(uint) -> bool`. The
/// boolean return is `false` when the index is out of range; we surface
/// that as an error so the frontend can fall back to refreshing the
/// list and retrying.
pub fn set_layout(index: u32) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let proxy = Proxy::new(&conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;
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
    let _ = current_with_conn(&conn);
    Ok(())
}

pub fn start_watcher() {
    super::register_watcher(
        thread::Builder::new()
            .name("layout-kde-watcher".into())
            .spawn(run_watcher),
    );
    super::register_watcher(
        thread::Builder::new()
            .name("layout-kde-signal".into())
            .spawn(run_signal_watcher),
    );
}

fn run_watcher() {
    while !super::watcher_stop_requested() {
        match watch_once() {
            Ok(()) => return,
            Err(e) => {
                log::debug!("[layout/kde] watcher error: {e}; retrying in 2s");
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

fn run_signal_watcher() {
    while !super::watcher_stop_requested() {
        match signal_watch_once() {
            Ok(()) => return,
            Err(e) => {
                log::debug!("[layout/kde] signal watcher error: {e}; retrying in 2s");
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

// The 500 ms poll loop leaves a window where key presses right after a
// layout switch are evaluated against the previous layout, so rules with a
// layout condition fire (or stay silent) wrongly. Subscribing to KDE's
// `layoutChanged` signal refreshes the cache the moment the layout changes;
// the poll loop stays as a fallback.
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
    let proxy = Proxy::new(&conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;
    let signals = proxy
        .receive_signal("layoutChanged")
        .map_err(|e| format!("subscribe to layoutChanged: {e}"))?;
    for _msg in signals {
        if super::watcher_stop_requested() {
            return Ok(());
        }
        emit_current(&proxy);
    }
    if super::watcher_stop_requested() {
        return Ok(());
    }
    Err("layoutChanged signal stream ended".to_string())
}

fn watch_once() -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let proxy = Proxy::new(&conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;

    while !super::watcher_stop_requested() {
        emit_current(&proxy);
        thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

fn emit_current(proxy: &Proxy<'_>) {
    if let Err(e) = current_with_proxy(proxy) {
        log::debug!("[layout/kde] poll error: {e}");
    }
}

fn current_with_conn(conn: &Connection) -> Result<Option<LayoutInfo>, String> {
    let proxy = Proxy::new(conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;
    current_with_proxy(&proxy)
}

fn current_with_proxy(proxy: &Proxy<'_>) -> Result<Option<LayoutInfo>, String> {
    let list = match call_list(proxy)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let idx = call_index(proxy)?.unwrap_or(0);
    let entry = list
        .get(idx as usize)
        .or_else(|| list.first())
        .cloned()
        .ok_or_else(|| "No keyboard layouts configured".to_string())?;
    let info = LayoutInfo {
        short: entry.0,
        display: entry.1,
        long: entry.2,
        index: idx,
        backend: "linux-kde",
    };
    super::publish(&info);
    Ok(Some(info))
}

fn call_index(proxy: &Proxy<'_>) -> Result<Option<u32>, String> {
    let msg = match proxy.call_method("getLayout", &()) {
        Ok(msg) => msg,
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
        {
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
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
        {
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
