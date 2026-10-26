//! RemoteDesktop portal handshake.

use super::*;

pub(super) type PortalSession = (
    Connection,
    OwnedObjectPath,
    Option<crate::text_input::portal_clipboard::Clipboard>,
);

pub(super) fn init_portal(text_request: &TextRequest) -> Result<PortalSession, String> {
    let conn = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.method_timeout(DBUS_METHOD_TIMEOUT).build())
        .map_err(|e| format!("connect session bus: {e}"))?;
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| "session bus did not give us a unique name".to_string())?
        .to_string();
    let sender_escaped = unique_name.trim_start_matches(':').replace('.', "_");

    let portal = Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE)
        .map_err(|e| format!("create RemoteDesktop proxy: {e}"))?;

    log::debug!("[portal] handshake: CreateSession");
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
        call_with_response(
            &conn,
            &portal,
            "CreateSession",
            &(opts,),
            &request_path,
            PORTAL_RESPONSE_TIMEOUT,
            text_request,
        )?
    };
    let session_handle = extract_session_handle(&results)?;
    let mut pending = PendingSession {
        conn: &conn,
        handle: &session_handle,
        active: true,
    };
    log::debug!("[portal] session: {}", session_handle.as_str());

    let wants_clipboard = matches!(
        TEXT_BACKEND.lock().unwrap_or_else(|e| e.into_inner()).mode,
        TextMode::Libei | TextMode::Clipboard
    );
    let clipboard_requested = wants_clipboard
        && matches!(
            crate::platform::linux::detect(),
            crate::platform::linux::Session {
                desktop: crate::platform::linux::Desktop::Gnome,
                session_type: crate::platform::linux::SessionType::Wayland,
                ..
            }
        );
    let token_scope = if clipboard_requested {
        TokenScope::Clipboard
    } else {
        TokenScope::Keyboard
    };
    let saved_token = load_restore_token(token_scope);
    if saved_token.is_some() {
        log::debug!("[portal] handshake: SelectDevices (with saved restore_token)");
    } else {
        log::debug!("[portal] handshake: SelectDevices (keyboard, persist_mode=2)");
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
            PORTAL_RESPONSE_TIMEOUT,
            text_request,
        )?;
    }

    let clipboard = if clipboard_requested {
        Some(crate::text_input::portal_clipboard::Clipboard::request(
            &conn,
            &session_handle,
        )?)
    } else {
        None
    };

    log::debug!("[portal] handshake: Start (consent dialog if no valid restore_token)");
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
            CONSENT_TIMEOUT,
            text_request,
        )?;
        results
    };

    if let Some(clipboard) = &clipboard {
        let enabled = start_results
            .get("clipboard_enabled")
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false);
        clipboard.enabled(enabled);
    }
    if let Some(token) = extract_restore_token(&start_results) {
        if saved_token.as_deref() != Some(token.as_str()) {
            persist_restore_token(token_scope, &token);
        }
    } else if saved_token.is_some() {
        // The portal dropped our token (revoked / invalidated).
        log::debug!("[portal] no restore_token returned; clearing saved token");
        clear_restore_token(token_scope);
    }

    log::debug!("[portal] handshake: complete");
    pending.active = false;
    drop(pending);
    Ok((conn, session_handle, clipboard))
}

/// Generic portal pattern: subscribe to the Response signal on the derived
/// Request object path first, then call the method. Return the Response
/// payload `(status, results)`; an error is returned for any non-zero
/// status so the caller can abort the handshake.
pub(super) fn call_with_response<T>(
    conn: &Connection,
    portal: &Proxy<'_>,
    method: &'static str,
    args: &T,
    request_path: &str,
    timeout: Duration,
    text_request: &TextRequest,
) -> Result<(u32, HashMap<String, OwnedValue>), String>
where
    T: serde::Serialize + zbus::zvariant::DynamicType,
{
    // Subscribe BEFORE calling to avoid racing the signal.
    let request = Proxy::new(conn, PORTAL_DEST, request_path, IFACE_REQUEST)
        .map_err(|e| format!("{method}: create Request proxy: {e}"))?;
    use zbus::export::futures_core::Stream;
    let deadline = Instant::now() + timeout;
    let valid = || text_request.check_lifetime().map_err(|e| format!("{e:?}"));
    // Cargo feature unification can select zbus's Tokio executor (via ksni).
    // Keep polling and SignalStream destruction inside the matching runtime.
    let response = zbus::block_on(async {
        let mut signals = poll_until(request.inner().receive_signal("Response"), deadline, valid)?
            .map_err(|e| format!("{method}: subscribe Response: {e}"))?;
        poll_until(portal.inner().call_method(method, args), deadline, valid)?
            .map_err(|e| format!("{method}: call: {e}"))?;
        poll_until(
            std::future::poll_fn(|cx| std::pin::Pin::new(&mut signals).poll_next(cx)),
            deadline,
            valid,
        )?
        .ok_or_else(|| format!("{method}: Response stream closed"))
    });
    let msg = match response {
        Ok(msg) => msg,
        Err(error) => {
            let _ = request.call_noreply("Close", &());
            return Err(error);
        }
    };
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

pub(super) struct PendingSession<'a> {
    pub(super) conn: &'a Connection,
    pub(super) handle: &'a OwnedObjectPath,
    pub(super) active: bool,
}

impl Drop for PendingSession<'_> {
    fn drop(&mut self) {
        if self.active {
            close_session(self.conn, self.handle);
        }
    }
}

/// Poll the bus's asynchronous operations without retaining a blocked signal-reader thread.
pub(super) fn poll_until<T>(
    future: impl std::future::Future<Output = T>,
    deadline: Instant,
    valid: impl Fn() -> Result<(), String>,
) -> Result<T, String> {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        valid()?;
        if Instant::now() >= deadline {
            return Err("portal operation timed out".into());
        }
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return Ok(value);
        }
        thread::sleep(FUTURE_POLL_INTERVAL);
    }
}

pub(super) fn extract_session_handle(
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

pub(super) fn extract_restore_token(results: &HashMap<String, OwnedValue>) -> Option<String> {
    let v = results.get("restore_token")?;
    let cloned = v.try_clone().ok()?;
    cloned.try_into().ok()
}

// --- Helpers ------------------------------------------------------------

/// Unique-ish token for portal request/session correlation. The token is
/// scoped per-process so a plain counter (plus prefix) is sufficient.
pub(super) fn gen_token(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("lhc_{prefix}_{ts}_{n}")
}

pub(super) fn make_request_path(sender_escaped: &str, handle_token: &str) -> String {
    crate::platform::dbus::portal_request_path(sender_escaped, handle_token)
}
