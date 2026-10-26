//! Backend workers: each portal session lives on its own thread.

use super::*;

pub(super) struct TextInjectionJob {
    pub(super) text: String,
    pub(super) mode: TextMode,
    pub(super) paste: PasteShortcut,
    /// Dropped without a send when the worker hit a fatal error and exits.
    pub(super) done: Sender<Result<(), TextInputFailure>>,
    pub(super) request: TextRequest,
}

/// A portal session on its own thread, started on first use.
pub(super) struct Worker {
    pub(super) tx: Mutex<Option<Sender<TextInjectionJob>>>,
    /// Why the last start failed. Later texts are reported as failed instead
    /// of starting again, which could block and show the dialog every time.
    pub(super) failure: Mutex<Option<String>>,
    /// Set by `stop` while a text holds `tx`, e.g. during the consent dialog.
    pub(super) stop_pending: AtomicBool,
    pub(super) start: fn(TextRequest) -> Result<Sender<TextInjectionJob>, String>,
}

pub(super) static LIBEI: Worker =
    Worker::new(|request| start_worker("lhc-libei", LibeiBackend::new, libei_worker, request));
pub(super) static PORTAL: Worker =
    Worker::new(|request| start_worker("lhc-portal", PortalBackend::new, portal_worker, request));

impl Worker {
    const fn new(start: fn(TextRequest) -> Result<Sender<TextInjectionJob>, String>) -> Self {
        Self {
            tx: Mutex::new(None),
            failure: Mutex::new(None),
            stop_pending: AtomicBool::new(false),
            start,
        }
    }

    pub(super) fn type_text(
        &self,
        text: &str,
        mode: TextMode,
        paste: PasteShortcut,
        request: TextRequest,
    ) -> Result<(), TextInputFailure> {
        let mut slot = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            if let Some(error) = self
                .failure
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
            {
                return Err(TextInputFailure::BackendUnavailable(error));
            }
            match (self.start)(request.clone()) {
                Ok(tx) => *slot = Some(tx),
                Err(error) => {
                    request.check()?;
                    *self.failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.clone());
                    return Err(TextInputFailure::BackendUnavailable(error));
                }
            }
        }
        request.wait_for_target()?;
        let (done, done_rx) = mpsc::channel();
        let cmd = TextInjectionJob {
            text: text.to_owned(),
            mode,
            paste,
            done,
            request: request.clone(),
        };
        if !slot.as_ref().is_some_and(|tx| tx.send(cmd).is_ok()) {
            *slot = None;
            return Err(TextInputFailure::BackendUnavailable(
                "the input session has ended".into(),
            ));
        }
        // The worker reports its own fatal error; forget it so the next
        // text opens a new session.
        let result = loop {
            match done_rx.recv_timeout(REPLY_POLL_INTERVAL) {
                Ok(result) => break result,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    *slot = None;
                    break Err(TextInputFailure::BackendUnavailable(
                        "the input session has ended".into(),
                    ));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Err(error) = request.check() {
                        *slot = None;
                        break Err(error);
                    }
                }
            }
        };
        if self.stop_pending.swap(false, Ordering::SeqCst)
            || matches!(result, Err(TextInputFailure::BackendUnavailable(_)))
        {
            *slot = None;
        }
        result
    }

    /// Close the session; the worker exits after its current text. Never
    /// blocks: it is called from the mapper thread.
    pub(super) fn stop(&self) {
        match self.tx.try_lock() {
            Ok(mut slot) => {
                slot.take();
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                e.into_inner().take();
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                self.stop_pending.store(true, Ordering::SeqCst);
            }
        }
    }

    pub(super) fn clear_failure(&self) {
        self.failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}

/// Start a worker thread and wait until its session is ready. There is no
/// cancellation delay: waiting for readiness periodically checks the job lifetime.
pub(super) fn start_worker<B: 'static>(
    name: &'static str,
    init: fn(&TextRequest) -> Result<B, String>,
    run: fn(B, Receiver<TextInjectionJob>),
    request: TextRequest,
) -> Result<Sender<TextInjectionJob>, String> {
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let init_request = request.clone();
    thread::Builder::new()
        .name(name.into())
        .spawn(move || match init(&init_request) {
            Ok(backend) => {
                let _ = ready_tx.send(Ok(()));
                run(backend, rx);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|e| format!("start {name}: {e}"))?;
    loop {
        match ready_rx.recv_timeout(REPLY_POLL_INTERVAL) {
            Ok(result) => {
                result?;
                return Ok(tx);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(format!("{name} stopped while starting"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Consent can temporarily move focus; only lifetime cancellation
                // is checked until Start returns and the original window is restored.
                request.check_lifetime().map_err(|e| format!("{e:?}"))?;
            }
        }
    }
}

pub(super) struct PortalBackend {
    pub(super) conn: Connection,
    pub(super) session_handle: OwnedObjectPath,
    pub(super) portal: Proxy<'static>,
    pub(super) clipboard: Option<crate::text_input::portal_clipboard::Clipboard>,
}

impl PortalBackend {
    pub(super) fn new(request: &TextRequest) -> Result<Self, String> {
        let (conn, session_handle, clipboard) = init_portal(request)?;
        match Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE) {
            Ok(portal) => Ok(Self {
                conn,
                session_handle,
                portal,
                clipboard,
            }),
            Err(e) => {
                close_session(&conn, &session_handle);
                Err(format!("create RemoteDesktop proxy: {e}"))
            }
        }
    }
}

pub(super) fn portal_worker(backend: PortalBackend, rx: Receiver<TextInjectionJob>) {
    log::debug!("[portal] backend online");
    for cmd in rx {
        let session = &backend.session_handle;
        let result = cmd.request.check().and_then(|()| match cmd.mode {
            TextMode::Clipboard => paste_text(
                &backend.portal,
                session,
                backend.clipboard.as_ref(),
                &cmd.text,
                cmd.paste,
                &cmd.request,
            ),
            TextMode::Keycode => {
                type_text_keycode(&backend.portal, session, &cmd.text, &cmd.request)
            }
            _ => Err(TextInputFailure::BackendUnavailable(
                "invalid portal mode".into(),
            )),
        });
        let fatal = matches!(result, Err(TextInputFailure::BackendUnavailable(_)));
        let _ = cmd.done.send(result);
        if fatal {
            break;
        }
    }
    if backend.clipboard.is_none() {
        close_session(&backend.conn, &backend.session_handle);
    }
}

pub(super) fn libei_worker(mut backend: LibeiBackend, rx: Receiver<TextInjectionJob>) {
    log::debug!("[libei] backend online");
    for cmd in rx {
        let fallback = (cmd.mode == TextMode::Libei).then_some(cmd.paste);
        let result = backend.type_text(&cmd.text, fallback, &cmd.request);
        let fatal = matches!(result, Err(TextInputFailure::BackendUnavailable(_)));
        let _ = cmd.done.send(result);
        if fatal {
            break;
        }
    }
    backend.close();
}
