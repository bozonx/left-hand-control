// XDG Desktop Portal RemoteDesktop backend.
//
// Why this instead of `zwp_virtual_keyboard_v1` on Wayland: in recent
// versions of KDE Plasma (6.x) and GNOME the virtual-keyboard Wayland
// protocol is not advertised to ordinary clients, so any attempt to bind
// it fails with "global not found in registry". The RemoteDesktop portal
// is the officially blessed, cross-compositor way to inject input:
//
//   1. `CreateSession()`        – async, via a `Request` object + `Response` signal.
//   2. `SelectDevices(keyboard)`– declares what we plan to inject.
//   3. `Start()`                – shows the permission dialog to the user.
//   4. `NotifyKeyboardKeysym()` – fires X keysyms, layout-independent.
//
// The user approves once per app session; after that we hold the session
// open and keep firing keysyms with negligible latency (one DBus method
// call round-trip each). On shutdown we close the session cleanly.
//
// For ASCII characters the keysym equals the Unicode code point; for
// anything outside Latin-1 we fall back to the XKB "direct unicode"
// encoding (`0x01000000 | codepoint`). The compositor then translates the
// keysym to the right character regardless of the user's active layout,
// which is exactly what this module exists for.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

// --- Portal DBus identifiers --------------------------------------------

const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE_REMOTE: &str = "org.freedesktop.portal.RemoteDesktop";
const IFACE_REQUEST: &str = "org.freedesktop.portal.Request";
const IFACE_SESSION: &str = "org.freedesktop.portal.Session";

/// Keyboard capability bit in the RemoteDesktop "types" field.
const DEVICE_TYPE_KEYBOARD: u32 = 1;

const STATE_RELEASED: u32 = 0;
const STATE_PRESSED: u32 = 1;

/// How long we wait for the portal handshake (dominated by the user
/// interacting with the permission dialog).
const INIT_TIMEOUT: Duration = Duration::from_secs(120);

// --- Public handle ------------------------------------------------------

pub struct Portal {
    tx: Sender<Cmd>,
    join: Option<JoinHandle<()>>,
}

enum Cmd {
    Type(String),
    Shutdown,
}

impl Portal {
    /// Start the background DBus thread and complete the portal
    /// handshake (including the user-visible permission dialog) before
    /// returning. Call this once at mapper startup.
    pub fn try_start() -> Result<Self, String> {
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();

        let join = thread::Builder::new()
            .name("lhc-portal".into())
            .spawn(move || run_thread(init_tx, cmd_rx))
            .map_err(|e| format!("spawn portal thread: {e}"))?;

        match init_rx.recv_timeout(INIT_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                tx: cmd_tx,
                join: Some(join),
            }),
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => Err("portal: init timed out (user did not approve in time?)".into()),
        }
    }

    /// Fire-and-forget: queue a character for the portal thread to inject.
    /// Sending to a dead channel is silently ignored (shutdown path).
    pub fn type_text(&self, text: &str) {
        let _ = self.tx.send(Cmd::Type(text.to_string()));
    }
}

impl Drop for Portal {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}

// --- Worker thread ------------------------------------------------------

fn run_thread(init_tx: Sender<Result<(), String>>, cmd_rx: mpsc::Receiver<Cmd>) {
    let (conn, session_handle) = match init_portal() {
        Ok(v) => v,
        Err(e) => {
            let _ = init_tx.send(Err(e));
            return;
        }
    };
    let _ = init_tx.send(Ok(()));

    let portal = match Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[portal] could not re-create RemoteDesktop proxy: {e}");
            close_session(&conn, &session_handle);
            return;
        }
    };

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            Cmd::Shutdown => break,
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

    eprintln!("[portal] handshake: SelectDevices (keyboard)");
    {
        let handle_token = gen_token("d");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token));
        opts.insert("types".into(), Value::from(DEVICE_TYPE_KEYBOARD));
        call_with_response(
            &conn,
            &portal,
            "SelectDevices",
            &(&session_handle, opts),
            &request_path,
        )?;
    }

    eprintln!("[portal] handshake: Start (user permission dialog)");
    {
        let handle_token = gen_token("t");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token));
        call_with_response(
            &conn,
            &portal,
            "Start",
            &(&session_handle, "", opts),
            &request_path,
        )?;
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
    let code = ch as u32;
    if code < 0x100 {
        code as i32
    } else {
        (0x0100_0000 | code) as i32
    }
}
