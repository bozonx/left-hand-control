//! Clipboard portal ownership on an existing RemoteDesktop session.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, AsRawFd};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use zbus::Message;
use zbus::blocking::Proxy;
use zbus::export::futures_core::Stream;
use zbus::proxy::SignalStream;
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value};

use super::text_job::TextRequest;

const IFACE: &str = "org.freedesktop.portal.Clipboard";
const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_TYPES: usize = 128;
type Contents = HashMap<String, Vec<u8>>;

enum Command {
    Enabled(bool),
    Paste(
        String,
        TextRequest,
        Sender<Result<(), String>>,
        Receiver<Control>,
        Sender<Result<(), String>>,
    ),
}
enum Control {
    Arm(Sender<()>),
    Finish(bool),
}

pub(super) struct Clipboard(Sender<Command>);

pub(super) struct Paste {
    control: Option<Sender<Control>>,
    done: Receiver<Result<(), String>>,
}

impl Clipboard {
    /// Subscribe before RequestClipboard/Start so the initial offer is not lost.
    pub(super) fn request(
        conn: &zbus::blocking::Connection,
        session: &OwnedObjectPath,
    ) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let (ready, initialized) = mpsc::channel();
        let connection = conn.clone();
        let handle = session.clone();
        thread::Builder::new()
            .name("portal-clipboard".into())
            .spawn(move || {
                let result = run(&connection, &handle, rx, ready);
                if let Err(error) = result {
                    log::warn!("[clipboard portal] {error}");
                }
                // Restored MIME data must remain available after the input backend
                // retires. run exits only once another owner replaces that data.
                super::portal::close_session(&connection, &handle);
            })
            .map_err(|e| e.to_string())?;
        initialized
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "clipboard portal subscription timed out")??;
        let portal = zbus::blocking::Proxy::new(
            conn,
            super::portal::PORTAL_DEST,
            super::portal::PORTAL_PATH,
            IFACE,
        )
        .map_err(|e| e.to_string())?;
        let empty: HashMap<String, Value> = HashMap::new();
        portal
            .call::<_, _, ()>("RequestClipboard", &(session, empty))
            .map_err(|e| format!("RequestClipboard: {e}"))?;
        Ok(Self(tx))
    }

    pub(super) fn enabled(&self, enabled: bool) {
        let _ = self.0.send(Command::Enabled(enabled));
    }

    pub(super) fn prepare(&self, text: &str, request: &TextRequest) -> Result<Paste, String> {
        let (ready, prepared) = mpsc::channel();
        let (control, trigger) = mpsc::channel();
        let (done, completed) = mpsc::channel();
        self.0
            .send(Command::Paste(
                text.into(),
                request.clone(),
                ready,
                trigger,
                done,
            ))
            .map_err(|_| "clipboard portal stopped")?;
        prepared
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "clipboard portal preparation timed out")??;
        Ok(Paste {
            control: Some(control),
            done: completed,
        })
    }
}

impl Paste {
    pub(super) fn arm(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.control
            .as_ref()
            .ok_or("clipboard transaction ended")?
            .send(Control::Arm(tx))
            .map_err(|_| "clipboard transaction ended")?;
        rx.recv_timeout(Duration::from_secs(1))
            .map_err(|_| "clipboard portal did not arm".into())
    }
    pub(super) fn finish(mut self, injected: bool) -> Result<(), String> {
        if let Some(tx) = self.control.take() {
            let _ = tx.send(Control::Finish(injected));
        }
        self.done
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "clipboard portal transfer timed out")?
    }
}
impl Drop for Paste {
    fn drop(&mut self) {
        if let Some(tx) = self.control.take() {
            let _ = tx.send(Control::Finish(false));
        }
    }
}

#[derive(Default)]
struct State {
    known: bool,
    enabled: bool,
    own: bool,
    revision: u64,
    types: Vec<String>,
    contents: Contents,
    armed: bool,
    transferred: bool,
    request: Option<TextRequest>,
}

struct Signals(Option<SignalStream<'static>>);

impl Drop for Signals {
    fn drop(&mut self) {
        // zbus's Tokio feature requires signal cleanup inside its runtime.
        // Destination checks and pipe I/O must stay outside that runtime.
        zbus::block_on(async {
            drop(self.0.take());
        });
    }
}

fn run(
    conn: &zbus::blocking::Connection,
    session: &OwnedObjectPath,
    rx: Receiver<Command>,
    ready: Sender<Result<(), String>>,
) -> Result<(), String> {
    let portal = Proxy::new(
        conn,
        super::portal::PORTAL_DEST,
        super::portal::PORTAL_PATH,
        IFACE,
    )
    .map_err(|e| e.to_string())?;
    let signals = match zbus::block_on(portal.inner().receive_all_signals()) {
        Ok(signals) => signals,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return Err(error.to_string());
        }
    };
    let mut signals = Signals(Some(signals));
    let _ = ready.send(Ok(()));
    let mut state = State::default();
    loop {
        drain(&portal, session, &mut signals, &mut state)?;
        match rx.try_recv() {
            Ok(Command::Enabled(enabled)) => {
                state.enabled = enabled;
                // Mutter sends an initial offer only when a clipboard owner
                // exists. With no offer, a granted new session starts empty.
                state.known |= enabled;
            }
            Ok(Command::Paste(text, request, ready, controls, done)) => {
                let result = transaction(
                    &portal,
                    session,
                    &mut signals,
                    &mut state,
                    &text,
                    &request,
                    &ready,
                    &controls,
                );
                let _ = ready.send(result.clone());
                let _ = done.send(result);
            }
            Err(mpsc::TryRecvError::Disconnected) if !state.own => break,
            Err(_) => {}
        }
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn drain(
    portal: &Proxy<'_>,
    session: &OwnedObjectPath,
    signals: &mut Signals,
    state: &mut State,
) -> Result<(), String> {
    loop {
        let stream = signals.0.as_mut().ok_or("clipboard signal stream ended")?;
        let polled = zbus::block_on(async {
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            std::pin::Pin::new(stream).poll_next(&mut cx)
        });
        match polled {
            std::task::Poll::Pending => return Ok(()),
            std::task::Poll::Ready(None) => {
                return Err("clipboard portal signal stream closed".into());
            }
            std::task::Poll::Ready(Some(message)) => event(portal, session, state, message)?,
        }
    }
}

fn event(
    portal: &Proxy<'_>,
    session: &OwnedObjectPath,
    state: &mut State,
    message: Message,
) -> Result<(), String> {
    match message.header().member().map(|m| m.as_str()) {
        Some("SelectionOwnerChanged") => {
            let (handle, options): (OwnedObjectPath, HashMap<String, OwnedValue>) =
                message.body().deserialize().map_err(|e| e.to_string())?;
            if &handle != session {
                return Ok(());
            }
            let own = options
                .get("session_is_owner")
                .and_then(|v| bool::try_from(v).ok())
                .ok_or("clipboard owner signal missing identity")?;
            let types = options
                .get("mime_types")
                .map(|v| v.try_clone().and_then(Vec::<String>::try_from))
                .transpose()
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            state.known = true;
            state.own = own;
            state.types = types;
            state.revision = state.revision.wrapping_add(1);
            if !own {
                state.contents.clear();
            }
        }
        Some("SelectionTransfer") => {
            let (handle, mime, serial): (OwnedObjectPath, String, u32) =
                message.body().deserialize().map_err(|e| e.to_string())?;
            if &handle != session {
                return Ok(());
            }
            let result = if let Some(bytes) = state.contents.get(&mime).filter(|_| {
                state.own
                    && state
                        .request
                        .as_ref()
                        .is_none_or(|request| request.check().is_ok())
            }) {
                let fd: OwnedFd = portal
                    .call("SelectionWrite", &(session, serial))
                    .map_err(|e| e.to_string())?;
                super::clipboard::write_transfer_checked(fd.into(), bytes, || {
                    state.request.as_ref().map_or(Ok(()), |request| {
                        request.check().map_err(|error| format!("{error:?}"))
                    })
                })
            } else {
                Err("clipboard format no longer available".into())
            };
            portal
                .call::<_, _, ()>("SelectionWriteDone", &(session, serial, result.is_ok()))
                .map_err(|e| e.to_string())?;
            if result.is_ok() && state.armed {
                state.transferred = true;
            }
        }
        _ => {}
    }
    Ok(())
}

fn set_selection(
    portal: &Proxy<'_>,
    session: &OwnedObjectPath,
    contents: &Contents,
) -> Result<(), String> {
    let types: Vec<&str> = contents.keys().map(String::as_str).collect();
    let options = HashMap::from([("mime_types", Value::from(types))]);
    portal
        .call::<_, _, ()>("SetSelection", &(session, options))
        .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn transaction(
    portal: &Proxy<'_>,
    session: &OwnedObjectPath,
    signals: &mut Signals,
    state: &mut State,
    text: &str,
    request: &TextRequest,
    ready: &Sender<Result<(), String>>,
    controls: &Receiver<Control>,
) -> Result<(), String> {
    if !state.enabled {
        return Err("clipboard portal permission was not granted".into());
    }
    if !state.known {
        return Err("clipboard portal has not reported the initial owner".into());
    }
    if state.types.len() > MAX_TYPES {
        return Err("too many clipboard formats to preserve".into());
    }
    request.check().map_err(|e| format!("{e:?}"))?;
    let revision = state.revision;
    let saved = if state.own {
        state.contents.clone()
    } else {
        let mut saved = Contents::new();
        let mut total = 0;
        let deadline = Instant::now() + TIMEOUT;
        for mime in &state.types {
            request.check().map_err(|e| format!("{e:?}"))?;
            let fd: OwnedFd = portal
                .call("SelectionRead", &(session, mime))
                .map_err(|e| e.to_string())?;
            let bytes = read_fd(fd, MAX_BYTES - total, deadline)?;
            total += bytes.len();
            saved.insert(mime.clone(), bytes);
        }
        saved
    };
    drain(portal, session, signals, state)?;
    if revision != state.revision {
        return Err("clipboard changed while preserving its formats".into());
    }
    request.check().map_err(|e| format!("{e:?}"))?;
    let temporary = HashMap::from([
        ("text/plain;charset=utf-8".into(), text.as_bytes().to_vec()),
        ("text/plain".into(), text.as_bytes().to_vec()),
    ]);
    set_selection(portal, session, &temporary)?;
    state.contents = temporary;
    state.request = Some(request.clone());
    state.own = true;
    state.armed = false;
    state.transferred = false;
    let result = (|| {
        let deadline = Instant::now() + TIMEOUT;
        drain(portal, session, signals, state)?;
        ready
            .send(Ok(()))
            .map_err(|_| "clipboard preparation cancelled")?;
        let mut injected = None;
        loop {
            drain(portal, session, signals, state)?;
            if !state.own {
                return Err("clipboard replaced during paste".into());
            }
            request.check().map_err(|e| format!("{e:?}"))?;
            match controls.try_recv() {
                Ok(Control::Arm(ack)) => {
                    state.armed = true;
                    state.transferred = false;
                    let _ = ack.send(());
                }
                Ok(Control::Finish(value)) => injected = Some(value),
                Err(mpsc::TryRecvError::Disconnected) if injected.is_none() => {
                    return Err("clipboard paste cancelled".into());
                }
                Err(_) => {}
            }
            if injected == Some(false) {
                return Err("clipboard shortcut failed".into());
            }
            if injected == Some(true) && state.transferred {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("clipboard paste was not consumed".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
    })();
    // A newer owner wins, including during cancellation and failed injection.
    drain(portal, session, signals, state)?;
    state.armed = false;
    state.request = None;
    if state.own {
        set_selection(portal, session, &saved)?;
        state.contents = saved;
    }
    result
}

fn read_fd(fd: OwnedFd, limit: usize, deadline: Instant) -> Result<Vec<u8>, String> {
    let mut file = File::from(std::os::fd::OwnedFd::from(fd));
    // SAFETY: this function owns the descriptor; all reads are bounded below.
    unsafe {
        let flags = libc::fcntl(file.as_raw_fd(), libc::F_GETFL);
        if flags < 0 || libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        if Instant::now() >= deadline {
            return Err("clipboard owner timed out".into());
        }
        match file.read(&mut buffer) {
            Ok(0) => return Ok(bytes),
            Ok(count) if count > limit.saturating_sub(bytes.len()) => {
                return Err("clipboard is too large to preserve".into());
            }
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                let mut fds = [PollFd::new(file.as_fd(), PollFlags::POLLIN)];
                let _ = poll(&mut fds, PollTimeout::from(5u16));
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    #[test]
    fn snapshot_preserves_binary_data_and_enforces_aggregate_limit() {
        let bytes = b"text\0\xffbinary";
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(bytes).unwrap();
        drop(writer);
        let fd: std::os::fd::OwnedFd = reader.into();
        assert_eq!(
            read_fd(fd.into(), bytes.len(), Instant::now() + TIMEOUT).unwrap(),
            bytes
        );

        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(bytes).unwrap();
        drop(writer);
        let fd: std::os::fd::OwnedFd = reader.into();
        assert!(
            read_fd(fd.into(), bytes.len() - 1, Instant::now() + TIMEOUT)
                .unwrap_err()
                .contains("too large")
        );
    }

    #[test]
    fn stalled_clipboard_owner_cannot_block_past_deadline() {
        let (reader, _writer) = UnixStream::pair().unwrap();
        let fd: std::os::fd::OwnedFd = reader.into();
        let started = Instant::now();
        assert!(
            read_fd(fd.into(), MAX_BYTES, started + Duration::from_millis(20))
                .unwrap_err()
                .contains("timed out")
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn cancelled_transfer_stops_before_the_next_write() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (mut reader, writer) = UnixStream::pair().unwrap();
        let fd: std::os::fd::OwnedFd = writer.into();
        let checks = AtomicUsize::new(0);
        let bytes = vec![7; 2 * 1024 * 1024];
        let result = super::super::clipboard::write_transfer_checked(fd, &bytes, || {
            if checks.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err("destination changed".into())
            }
        });
        assert_eq!(result.unwrap_err(), "destination changed");
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert!(!received.is_empty());
        assert!(received.len() < bytes.len());
    }
}
