//! Short on-screen display messages, such as the name of the layout that
//! just became active. KDE Plasma shows them in its own OSD; other
//! desktops get a transient desktop notification.

pub use super::osd_queue::enqueue;

use super::dbus;
use super::linux::{self, Desktop};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

/// How long a notification stays on screen.
const NOTIFICATION_TIMEOUT_MS: i32 = 1500;
/// A stalled desktop service must not hold up newer OSD messages for seconds.
const METHOD_TIMEOUT: Duration = Duration::from_millis(750);

/// Notification the next message replaces, so quick switches do not
/// stack up.
static LAST_NOTIFICATION: Mutex<u32> = Mutex::new(0);

/// Show `text` on screen. Blocks on D-Bus; call it off the UI thread.
pub fn show(text: &str) -> Result<(), String> {
    let conn = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.method_timeout(METHOD_TIMEOUT).build())
        .map_err(|e| format!("session bus: {e}"))?;
    if matches!(linux::detect().desktop, Desktop::Kde) {
        match show_plasma(&conn, text) {
            Ok(()) => return Ok(()),
            Err(error) => {
                log::debug!("[osd] Plasma OSD unavailable, using a notification: {error}")
            }
        }
    }
    notify(&conn, text)
}

fn show_plasma(conn: &Connection, text: &str) -> Result<(), String> {
    Proxy::new(
        conn,
        dbus::PLASMA_SHELL_SERVICE,
        dbus::PLASMA_OSD_PATH,
        dbus::PLASMA_OSD_INTERFACE,
    )
    .and_then(|proxy| proxy.call::<_, _, ()>("showText", &("input-keyboard", text)))
    .map_err(|e| format!("osdService showText: {e}"))
}

fn notify(conn: &Connection, text: &str) -> Result<(), String> {
    let proxy = Proxy::new(
        conn,
        dbus::NOTIFICATIONS_SERVICE,
        dbus::NOTIFICATIONS_PATH,
        dbus::NOTIFICATIONS_INTERFACE,
    )
    .map_err(|e| format!("notifications: {e}"))?;
    let mut last = LAST_NOTIFICATION.lock().map_err(|e| e.to_string())?;
    let hints = HashMap::from([("transient", Value::from(true))]);
    *last = proxy
        .call(
            "Notify",
            &(
                crate::storage::APP_NAME,
                *last,
                "input-keyboard",
                text,
                "",
                Vec::<&str>::new(),
                hints,
                NOTIFICATION_TIMEOUT_MS,
            ),
        )
        .map_err(|e| format!("notifications Notify: {e}"))?;
    Ok(())
}
