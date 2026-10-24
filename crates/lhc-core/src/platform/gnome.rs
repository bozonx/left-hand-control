//! Narrow GNOME Shell extension API, served by the authenticated Shell bus owner.

use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use zbus::blocking::{Connection, Proxy};

/// Reply timeout of GNOME Shell calls.
const METHOD_TIMEOUT: Duration = Duration::from_millis(750);

pub(crate) const SERVICE: &str = "org.gnome.Shell";
pub(crate) const PATH: &str = "/dev/bozonx/LeftHandControl/Gnome";
pub(crate) const INTERFACE: &str = "dev.bozonx.LeftHandControl.Gnome";
/// Activation tries while the window is not mapped yet, and their pause.
const ACTIVATION_ATTEMPTS: u32 = 5;
const ACTIVATION_RETRY: Duration = Duration::from_millis(100);

#[derive(Deserialize)]
pub(crate) struct Snapshot {
    version: u32,
    pub window: Option<Value>,
    #[serde(rename = "externalWindow")]
    pub external_window: Option<Value>,
    pub layouts: Vec<InputSource>,
    pub current: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct InputSource {
    pub short: String,
    pub variant: String,
    pub display: String,
    pub long: String,
    pub index: u32,
}

fn connection() -> Result<&'static Connection, String> {
    static BUS: OnceLock<Connection> = OnceLock::new();
    if let Some(bus) = BUS.get() {
        return Ok(bus);
    }
    let bus = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.method_timeout(METHOD_TIMEOUT).build())
        .map_err(|error| format!("GNOME session bus: {error}"))?;
    Ok(BUS.get_or_init(|| bus))
}

pub(crate) fn proxy() -> Result<Proxy<'static>, String> {
    Proxy::new(connection()?, SERVICE, PATH, INTERFACE)
        .map_err(|error| format!("GNOME bridge: {error}"))
}

pub(crate) fn snapshot(titles: bool) -> Result<Snapshot, String> {
    let json: String = proxy()?.call("Snapshot", &(titles,)).map_err(|error| {
        format!(
            "Enable the {} GNOME extension: {error}",
            crate::storage::APP_NAME
        )
    })?;
    parse_snapshot(&json)
}

fn parse_snapshot(json: &str) -> Result<Snapshot, String> {
    let snapshot: Snapshot = serde_json::from_str(json)
        .map_err(|error| format!("Invalid GNOME bridge snapshot: {error}"))?;
    if snapshot.version != 1 {
        return Err("Unsupported GNOME bridge version".into());
    }
    Ok(snapshot)
}

pub(crate) fn windows() -> Result<Vec<Value>, String> {
    let json: String = proxy()?
        .call("Windows", &())
        .map_err(|error| format!("GNOME window list: {error}"))?;
    serde_json::from_str(&json).map_err(|error| format!("Invalid GNOME windows: {error}"))
}

pub(crate) fn action(name: &str) -> Result<(), String> {
    proxy()?
        .call("Action", &(name,))
        .map_err(|error| format!("GNOME action {name}: {error}"))
}

/// Request focus for one of this process's own GNOME windows, identified by title.
/// The Shell checks the authenticated caller PID; mapping retries stay off the UI thread.
pub fn request_window_activation(title: String) -> Result<(), String> {
    std::thread::Builder::new()
        .name("gnome-activation".into())
        .spawn(move || {
            let proxy = match proxy() {
                Ok(proxy) => proxy,
                Err(error) => {
                    log::warn!("[gnome] activation: {error}");
                    return;
                }
            };
            let not_mapped = format!("{INTERFACE}.NotMapped");
            for attempt in 1..=ACTIVATION_ATTEMPTS {
                match proxy.call::<_, _, ()>("Activate", &(title.as_str(),)) {
                    Ok(()) => return,
                    Err(zbus::Error::MethodError(name, _, _))
                        if name.as_str() == not_mapped && attempt < ACTIVATION_ATTEMPTS =>
                    {
                        std::thread::sleep(ACTIVATION_RETRY);
                    }
                    Err(error) => {
                        log::warn!("[gnome] activation: {error}");
                        return;
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_focus_and_source_are_explicit() {
        let snapshot =
            parse_snapshot(r#"{"version":1,"window":null,"layouts":[],"current":null}"#).unwrap();
        assert!(snapshot.window.is_none());
        assert!(snapshot.current.is_none());
        assert!(
            parse_snapshot(r#"{"version":2,"window":null,"layouts":[],"current":null}"#).is_err()
        );
        assert!(parse_snapshot(r#"{"version":1}"#).is_err());
    }
}
