//! File and folder selection through the XDG desktop portal, so the
//! dialog matches the desktop (KDE, GNOME, …) without a toolkit of our own.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedValue, Value};

/// Blocks until the user picks a file (or a folder when `directory`) and
/// returns its path; `Ok(None)` when the dialog was cancelled.
pub fn pick(title: &str, directory: bool) -> Result<Option<PathBuf>, String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let conn = Connection::session().map_err(|e| format!("session bus: {e}"))?;
    let sender = conn
        .unique_name()
        .ok_or("session bus did not give us a unique name")?
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!(
        "lhc_file_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    // Subscribe before calling so the Response signal cannot be missed.
    let request = Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        request_path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(|e| format!("file chooser: {e}"))?;
    let mut responses = request
        .receive_signal("Response")
        .map_err(|e| format!("file chooser: {e}"))?;
    let chooser = Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )
    .map_err(|e| format!("file chooser: {e}"))?;
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("modal", Value::from(true));
    if directory {
        options.insert("directory", Value::from(true));
    }
    chooser
        .call_method("OpenFile", &("", title, options))
        .map_err(|e| format!("file chooser: {e}"))?;
    let message = responses.next().ok_or("file chooser closed")?;
    let (status, results): (u32, HashMap<String, OwnedValue>) = message
        .body()
        .deserialize()
        .map_err(|e| format!("file chooser: {e}"))?;
    if status != 0 {
        return Ok(None);
    }
    let uris: Vec<String> = results
        .get("uris")
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| value.try_into().ok())
        .unwrap_or_default();
    Ok(uris.first().and_then(|uri| file_path(uri)))
}

/// Local path of a `file://` URI, percent-decoded.
fn file_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = encoded
                .get(i + 1..i + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            decoded.push(byte);
            i += 3;
            continue;
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(decoded)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            file_path("file:///home/me/my%20script.sh"),
            Some(PathBuf::from("/home/me/my script.sh"))
        );
        assert_eq!(
            file_path("file:///tmp/%D1%84.sh"),
            Some(PathBuf::from("/tmp/ф.sh"))
        );
        assert_eq!(file_path("https://example.com"), None);
    }
}
