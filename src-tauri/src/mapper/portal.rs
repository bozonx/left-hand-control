// XDG Desktop Portal RemoteDesktop backend.
//
// Why this instead of `zwp_virtual_keyboard_v1` on Wayland: in recent
// versions of KDE Plasma (6.x) and GNOME the virtual-keyboard Wayland
// protocol is not advertised to ordinary clients, so any attempt to bind
// it fails with "global not found in registry". The RemoteDesktop portal
// is the officially blessed, cross-compositor way to inject input:
//
//   1. `CreateSession()`        – async, via a `Request` object + `Response` signal.
//   2. `SelectDevices(keyboard)`– declares what we plan to inject; this is
//                                 also where we set `persist_mode = 2` and
//                                 pass back any previously-saved
//                                 `restore_token` so the user does not see
//                                 the consent dialog again.
//   3. `Start()`                – shows the permission dialog (only once
//                                 per machine if persist mode + token work).
//   4. `NotifyKeyboardKeysym()` – fires X keysyms, layout-independent.
//
// The session and its DBus connection are owned by a process-wide
// singleton: it is created lazily on the first injection request and
// lives until the process exits. This means the consent dialog is shown
// once per *process*, not once per `start_mapper`/`stop_mapper` cycle —
// which used to retrigger it on every config change.
//
// On top of that we read `restore_token` from the `Start` response and
// persist it next to the user data, so subsequent app launches can
// restore the permission silently.
//
// For ASCII characters the keysym equals the Unicode code point; for
// anything outside Latin-1 we fall back to the XKB "direct unicode"
// encoding (`0x01000000 | codepoint`). The compositor then translates the
// keysym to the right character regardless of the user's active layout,
// which is exactly what this module exists for.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

#[link(name = "xkbcommon")]
unsafe extern "C" {
    fn xkb_utf32_to_keysym(ucs: u32) -> u32;
}

// --- Portal DBus identifiers --------------------------------------------

const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE_REMOTE: &str = "org.freedesktop.portal.RemoteDesktop";
const IFACE_REQUEST: &str = "org.freedesktop.portal.Request";
const IFACE_SESSION: &str = "org.freedesktop.portal.Session";

/// Keyboard capability bit in the RemoteDesktop "types" field.
const DEVICE_TYPE_KEYBOARD: u32 = 1;

/// `persist_mode` for SelectDevices: 2 = persist until explicitly revoked.
const PERSIST_MODE_PERMANENT: u32 = 2;

const STATE_RELEASED: u32 = 0;
const STATE_PRESSED: u32 = 1;

const RESTORE_TOKEN_FILE: &str = "portal-remote-desktop.token";

// --- Public API ---------------------------------------------------------

enum Cmd {
    Type(String),
}

/// Sender to the current portal worker thread. Cleared when the worker is
/// gone so a later literal injection can retry the portal handshake.
static PORTAL_TX: Mutex<Option<Sender<Cmd>>> = Mutex::new(None);

/// Path of the directory where we persist the restore token.
static TOKEN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Register where the worker should read/write the restore token.
/// Must be called once at app startup (before the first injection).
/// Calling it again is a no-op.
pub fn set_token_dir(dir: PathBuf) {
    let _ = TOKEN_DIR.set(dir);
}

/// Send a literal string to the active session. Lazily starts the
/// singleton worker on first call. Non-blocking: returns immediately,
/// the actual portal handshake (and its consent dialog) runs on a
/// dedicated thread.
pub fn type_text(text: &str) {
    let Ok(mut slot) = PORTAL_TX.lock() else {
        eprintln!("[portal] literal {:?} dropped (worker lock poisoned)", text);
        return;
    };

    if let Some(tx) = slot.as_ref() {
        if tx.send(Cmd::Type(text.to_string())).is_ok() {
            return;
        }
        eprintln!("[portal] worker gone; retrying backend init");
        *slot = None;
    }

    let Some(tx) = start_singleton() else {
        eprintln!("[portal] literal {:?} dropped (worker spawn failed)", text);
        return;
    };
    if tx.send(Cmd::Type(text.to_string())).is_err() {
        eprintln!(
            "[portal] literal {:?} dropped (worker exited during init)",
            text
        );
        return;
    }
    *slot = Some(tx);
}

fn start_singleton() -> Option<Sender<Cmd>> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    let spawn_result = thread::Builder::new()
        .name("lhc-portal".into())
        .spawn(move || worker(rx));
    match spawn_result {
        Ok(_join) => Some(tx),
        Err(e) => {
            eprintln!("[portal] could not spawn worker thread: {e}");
            None
        }
    }
}

fn worker(rx: Receiver<Cmd>) {
    let (conn, session_handle) = match init_portal() {
        Ok(v) => {
            eprintln!("[portal] backend online");
            v
        }
        Err(e) => {
            eprintln!("[portal] backend unavailable: {e}");
            drop(rx);
            return;
        }
    };

    let portal = match Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[portal] could not re-create RemoteDesktop proxy: {e}");
            close_session(&conn, &session_handle);
            return;
        }
    };

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Type(text) => inject_text(&portal, &session_handle, &text),
        }
    }

    close_session(&conn, &session_handle);
}

fn inject_text(portal: &Proxy, session: &OwnedObjectPath, text: &str) {
    for ch in text.chars() {
        let ks = keysym_for(ch);
        let empty: HashMap<String, Value> = HashMap::new();
        for state in [STATE_PRESSED, STATE_RELEASED] {
            if let Err(e) =
                portal.call_method("NotifyKeyboardKeysym", &(session, &empty, ks, state))
            {
                eprintln!("[portal] NotifyKeyboardKeysym({ch:?}, state={state}) failed: {e}");
                return;
            }
        }
    }
}

fn close_session(conn: &Connection, session: &OwnedObjectPath) {
    match Proxy::new(conn, PORTAL_DEST, &**session, IFACE_SESSION) {
        Ok(sp) => {
            if let Err(e) = sp.call_method("Close", &()) {
                eprintln!("[portal] Session.Close failed: {e}");
            }
        }
        Err(e) => eprintln!("[portal] cannot build Session proxy for close: {e}"),
    }
}

// --- Portal handshake ---------------------------------------------------

fn init_portal() -> Result<(Connection, OwnedObjectPath), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| "session bus did not give us a unique name".to_string())?
        .to_string();
    let sender_escaped = unique_name.trim_start_matches(':').replace('.', "_");

    let portal = Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE)
        .map_err(|e| format!("create RemoteDesktop proxy: {e}"))?;

    eprintln!("[portal] handshake: CreateSession");
    let session_token = gen_token("s");
    let (_, results) = {
        let handle_token = gen_token("c");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token.clone()));
        opts.insert(
            "session_handle_token".into(),
            Value::from(session_token.clone()),
        );
        call_with_response(&conn, &portal, "CreateSession", &(opts,), &request_path)?
    };
    let session_handle = extract_session_handle(&results)?;
    eprintln!("[portal] session: {}", session_handle.as_str());

    let saved_token = load_restore_token();
    if saved_token.is_some() {
        eprintln!("[portal] handshake: SelectDevices (with saved restore_token)");
    } else {
        eprintln!("[portal] handshake: SelectDevices (keyboard, persist_mode=2)");
    }
    {
        let handle_token = gen_token("d");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token));
        opts.insert("types".into(), Value::from(DEVICE_TYPE_KEYBOARD));
        opts.insert("persist_mode".into(), Value::from(PERSIST_MODE_PERMANENT));
        if let Some(tok) = &saved_token {
            opts.insert("restore_token".into(), Value::from(tok.clone()));
        }
        call_with_response(
            &conn,
            &portal,
            "SelectDevices",
            &(&session_handle, opts),
            &request_path,
        )?;
    }

    eprintln!("[portal] handshake: Start (consent dialog if no valid restore_token)");
    let start_results = {
        let handle_token = gen_token("t");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token));
        let (_, results) = call_with_response(
            &conn,
            &portal,
            "Start",
            &(&session_handle, "", opts),
            &request_path,
        )?;
        results
    };

    if let Some(token) = extract_restore_token(&start_results) {
        if saved_token.as_deref() != Some(token.as_str()) {
            persist_restore_token(&token);
        }
    } else if saved_token.is_some() {
        // The portal dropped our token (revoked / invalidated).
        eprintln!("[portal] no restore_token returned; clearing saved token");
        clear_restore_token();
    }

    eprintln!("[portal] handshake: complete, ready to inject keysyms");
    Ok((conn, session_handle))
}

/// Generic portal pattern: subscribe to the Response signal on the derived
/// Request object path first, then call the method. Return the Response
/// payload `(status, results)`; an error is returned for any non-zero
/// status so the caller can abort the handshake.
fn call_with_response<T>(
    conn: &Connection,
    portal: &Proxy<'_>,
    method: &'static str,
    args: &T,
    request_path: &str,
) -> Result<(u32, HashMap<String, OwnedValue>), String>
where
    T: serde::Serialize + zbus::zvariant::DynamicType,
{
    // Subscribe BEFORE calling to avoid racing the signal.
    let request = Proxy::new(conn, PORTAL_DEST, request_path, IFACE_REQUEST)
        .map_err(|e| format!("{method}: create Request proxy: {e}"))?;
    let mut signals = request
        .receive_signal("Response")
        .map_err(|e| format!("{method}: subscribe Response: {e}"))?;

    portal
        .call_method(method, args)
        .map_err(|e| format!("{method}: call: {e}"))?;

    let msg = signals
        .next()
        .ok_or_else(|| format!("{method}: Response stream closed"))?;
    let body = msg.body();
    let (status, results): (u32, HashMap<String, OwnedValue>) = body
        .deserialize()
        .map_err(|e| format!("{method}: decode Response: {e}"))?;
    if status != 0 {
        let reason = match status {
            1 => "user cancelled",
            2 => "other error",
            _ => "unknown",
        };
        return Err(format!(
            "{method}: portal returned status {status} ({reason})"
        ));
    }
    Ok((status, results))
}

fn extract_session_handle(
    results: &HashMap<String, OwnedValue>,
) -> Result<OwnedObjectPath, String> {
    let v = results
        .get("session_handle")
        .ok_or_else(|| "CreateSession: response missing `session_handle`".to_string())?;
    // The portal returns the handle as a plain string even though it is
    // semantically an object path.
    let s: String = v
        .try_clone()
        .map_err(|e| format!("session_handle clone: {e}"))?
        .try_into()
        .map_err(|e| format!("session_handle not a string: {e}"))?;
    OwnedObjectPath::try_from(s).map_err(|e| format!("session_handle invalid path: {e}"))
}

fn extract_restore_token(results: &HashMap<String, OwnedValue>) -> Option<String> {
    let v = results.get("restore_token")?;
    let cloned = v.try_clone().ok()?;
    cloned.try_into().ok()
}

// --- Restore-token persistence -----------------------------------------

fn restore_token_path() -> Option<PathBuf> {
    TOKEN_DIR.get().map(|d| d.join(RESTORE_TOKEN_FILE))
}

/// In-memory cache so we don't hit the filesystem on every handshake
/// (handshake is rare, but reads are cheap and this also gives us a sane
/// fallback when the disk path was not registered yet).
static TOKEN_CACHE: Mutex<Option<String>> = Mutex::new(None);

fn load_restore_token() -> Option<String> {
    if let Ok(guard) = TOKEN_CACHE.lock() {
        if let Some(cached) = guard.clone() {
            return Some(cached);
        }
    }
    let path = restore_token_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let token = trimmed.to_string();
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = Some(token.clone());
    }
    Some(token)
}

fn persist_restore_token(token: &str) {
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = Some(token.to_string());
    }
    let Some(path) = restore_token_path() else {
        eprintln!("[portal] cannot persist restore_token (TOKEN_DIR not set)");
        return;
    };
    if let Err(e) = write_token_atomic(&path, token) {
        eprintln!("[portal] failed to persist restore_token: {e}");
    } else {
        eprintln!("[portal] restore_token saved to {}", path.display());
    }
}

fn clear_restore_token() {
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = None;
    }
    if let Some(path) = restore_token_path() {
        let _ = std::fs::remove_file(path);
    }
}

fn write_token_atomic(path: &Path, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("token.tmp");
    std::fs::write(&tmp, token.as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// --- Helpers ------------------------------------------------------------

/// Unique-ish token for portal request/session correlation. The token is
/// scoped per-process so a plain counter (plus prefix) is sufficient.
fn gen_token(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("lhc_{prefix}_{ts}_{n}")
}

fn make_request_path(sender_escaped: &str, handle_token: &str) -> String {
    format!("/org/freedesktop/portal/desktop/request/{sender_escaped}/{handle_token}")
}

/// Map a character to its XKB keysym value. ASCII + Latin-1 are identity,
/// everything else uses the XKB direct-unicode encoding.
fn keysym_for(ch: char) -> i32 {
    unsafe { xkb_utf32_to_keysym(ch as u32) as i32 }
}
