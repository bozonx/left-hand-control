// KDE Plasma keyboard layout backend (Wayland + X11).
//
// Current product support is Linux/KDE, so this backend stays fully native:
// it talks to DBus directly through `zbus`, which the project already ships
// for the portal backend. Other desktops remain explicit skeletons for now.
//
// The watcher polls the DBus interface at a short interval so it can stop
// promptly during app shutdown. Switching the active layout goes through
// the same DBus interface (`setLayout(uint)`).

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use zbus::blocking::{Connection, Proxy};

use super::LayoutInfo;

type LayoutList = Vec<(String, String, String)>;

const SERVICE: &str = "org.kde.keyboard";
const OBJECT: &str = "/Layouts";
const IFACE: &str = "org.kde.KeyboardLayouts";

use std::sync::Mutex;

static CACHED_LAYOUT: Mutex<Option<String>> = Mutex::new(None);
static LAST_EMITTED_LAYOUT: Mutex<Option<String>> = Mutex::new(None);

pub fn cached_layout_short() -> Option<String> {
    CACHED_LAYOUT.lock().ok().and_then(|g| g.clone())
}

fn update_cached_layout(info: &LayoutInfo) {
    if let Ok(mut g) = CACHED_LAYOUT.lock() {
        *g = Some(info.short.clone());
    }
}

fn should_emit_layout(info: &LayoutInfo) -> bool {
    match LAST_EMITTED_LAYOUT.lock() {
        Ok(mut g) => {
            if g.as_deref() == Some(info.short.as_str()) {
                return false;
            }
            *g = Some(info.short.clone());
            true
        }
        Err(_) => true,
    }
}

pub fn current() -> Result<Option<LayoutInfo>, String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    current_with_conn(&conn)
}

pub fn refresh_cache() -> Result<Option<LayoutInfo>, String> {
    current()
}

pub fn available_layouts() -> Result<Vec<LayoutInfo>, String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let proxy = Proxy::new(&conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;
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
        .map_err(|e| format!("setLayout({index}) failed: {e}"))?;
    let ok: bool = msg
        .body()
        .deserialize()
        .map_err(|e| format!("decode setLayout response: {e}"))?;
    if !ok {
        return Err(format!("setLayout({index}) rejected by KDE"));
    }
    let _ = current_with_conn(&conn);
    Ok(())
}

pub fn start_watcher(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("layout-kde-watcher".into())
        .spawn(move || run_watcher(app));
}

fn run_watcher(app: AppHandle) {
    while !super::watcher_stop_requested() {
        match watch_once(&app) {
            Ok(()) => return,
            Err(e) => {
                eprintln!("[layout/kde] watcher error: {e}; retrying in 2s");
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

fn watch_once(app: &AppHandle) -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let proxy = Proxy::new(&conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;

    while !super::watcher_stop_requested() {
        emit_current(app, &proxy);
        thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

fn emit_current(app: &AppHandle, proxy: &Proxy<'_>) {
    let info = match current_with_proxy(proxy) {
        Ok(Some(info)) => info,
        Ok(None) => return,
        Err(e) => {
            eprintln!("[layout/kde] poll error: {e}");
            return;
        }
    };
    if !should_emit_layout(&info) {
        return;
    }
    if let Err(e) = app.emit("layout-changed", info) {
        eprintln!("[layout/kde] emit error: {e}");
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
        .ok_or_else(|| "no layouts configured".to_string())?;
    let info = LayoutInfo {
        short: entry.0,
        display: entry.1,
        long: entry.2,
        index: idx,
        backend: "linux-kde",
    };
    update_cached_layout(&info);
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
        Err(e) => return Err(format!("getLayout failed: {e}")),
    };
    let idx: u32 = msg
        .body()
        .deserialize()
        .map_err(|e| format!("decode getLayout response: {e}"))?;
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
        Err(e) => return Err(format!("getLayoutsList failed: {e}")),
    };
    let list: Vec<(String, String, String)> = msg
        .body()
        .deserialize()
        .map_err(|e| format!("decode getLayoutsList response: {e}"))?;
    Ok(Some(list))
}
