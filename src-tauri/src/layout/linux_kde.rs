// KDE Plasma keyboard layout backend (Wayland + X11).
//
// Current product support is Linux/KDE, so this backend stays fully native:
// it talks to DBus directly through `zbus`, which the project already ships
// for the portal backend. Other desktops remain explicit skeletons for now.

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use zbus::blocking::{Connection, Proxy};

use super::LayoutInfo;

const SERVICE: &str = "org.kde.keyboard";
const OBJECT: &str = "/Layouts";
const IFACE: &str = "org.kde.KeyboardLayouts";

pub fn current() -> Result<Option<LayoutInfo>, String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    current_with_conn(&conn)
}

pub fn start_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut last = None;
        loop {
            match current() {
                Ok(Some(info)) => {
                    if last.as_ref() != Some(&info) {
                        let _ = app.emit("layout-changed", info.clone());
                        last = Some(info);
                    }
                }
                Ok(None) => {}
                Err(e) => eprintln!("[layout/kde] poll error: {e}"),
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn current_with_conn(conn: &Connection) -> Result<Option<LayoutInfo>, String> {
    let proxy = Proxy::new(conn, SERVICE, OBJECT, IFACE)
        .map_err(|e| format!("create keyboard proxy: {e}"))?;
    let list = match call_list(&proxy)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let idx = call_index(&proxy)?.unwrap_or(0);
    let entry = list
        .get(idx as usize)
        .or_else(|| list.first())
        .cloned()
        .ok_or_else(|| "no layouts configured".to_string())?;
    Ok(Some(LayoutInfo {
        short: entry.0,
        display: entry.1,
        long: entry.2,
        index: idx,
        backend: "linux-kde",
    }))
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

fn call_list(proxy: &Proxy<'_>) -> Result<Option<Vec<(String, String, String)>>, String> {
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
