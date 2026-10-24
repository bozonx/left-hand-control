//! Temporary Wayland clipboard ownership with bounded transfers and MIME preservation.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_callback::WlCallback, wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
    zwlr_data_control_source_v1::{self, ZwlrDataControlSourceV1},
};

use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};

// Both protocols carry the same transaction; keep ownership policy shared.
enum Manager {
    Ext(ExtDataControlManagerV1),
    Wlr(ZwlrDataControlManagerV1),
}
#[derive(Clone)]
enum Device {
    Ext(ExtDataControlDeviceV1),
    Wlr(ZwlrDataControlDeviceV1),
}
#[derive(Clone, PartialEq, Eq)]
enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}
#[derive(Clone, PartialEq, Eq)]
enum Source {
    Ext(ExtDataControlSourceV1),
    Wlr(ZwlrDataControlSourceV1),
}

impl Manager {
    fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Result<Self, String> {
        globals
            .bind(qh, 1..=1, ())
            .map(Self::Ext)
            .or_else(|_| globals.bind(qh, 1..=2, ()).map(Self::Wlr))
            .map_err(|_| "compositor does not support safe clipboard ownership".into())
    }
    fn get_data_device(&self, seat: &WlSeat, qh: &QueueHandle<State>) -> Device {
        match self {
            Self::Ext(manager) => Device::Ext(manager.get_data_device(seat, qh, ())),
            Self::Wlr(manager) => Device::Wlr(manager.get_data_device(seat, qh, ())),
        }
    }
    fn create_data_source(&self, qh: &QueueHandle<State>) -> Source {
        match self {
            Self::Ext(manager) => Source::Ext(manager.create_data_source(qh, ())),
            Self::Wlr(manager) => Source::Wlr(manager.create_data_source(qh, ())),
        }
    }
}
impl Device {
    fn set_selection(&self, source: Option<&Source>) {
        match (self, source) {
            (Self::Ext(device), Some(Source::Ext(source))) => device.set_selection(Some(source)),
            (Self::Wlr(device), Some(Source::Wlr(source))) => device.set_selection(Some(source)),
            (Self::Ext(device), None) => device.set_selection(None),
            (Self::Wlr(device), None) => device.set_selection(None),
            _ => log::error!("clipboard source and device protocols differ"),
        }
    }
}
impl Offer {
    fn id(&self) -> wayland_client::backend::ObjectId {
        match self {
            Self::Ext(offer) => offer.id(),
            Self::Wlr(offer) => offer.id(),
        }
    }
    fn receive(&self, mime: String, fd: std::os::fd::BorrowedFd<'_>) {
        match self {
            Self::Ext(offer) => offer.receive(mime, fd),
            Self::Wlr(offer) => offer.receive(mime, fd),
        }
    }
    fn destroy(&self) {
        match self {
            Self::Ext(offer) => offer.destroy(),
            Self::Wlr(offer) => offer.destroy(),
        }
    }
}
impl Source {
    fn offer(&self, mime: String) {
        match self {
            Self::Ext(source) => source.offer(mime),
            Self::Wlr(source) => source.offer(mime),
        }
    }
    fn destroy(&self) {
        match self {
            Self::Ext(source) => source.destroy(),
            Self::Wlr(source) => source.destroy(),
        }
    }
}

const TRANSFER_TIMEOUT: Duration = Duration::from_secs(2);
// Data-control cannot identify the reader. A watcher may read before the target
// processes the shortcut, so a completed transfer alone cannot trigger restore.
const PASTE_GRACE: Duration = Duration::from_millis(200);
/// Limits of a saved clipboard, shared with the portal clipboard.
pub(super) const MAX_CLIPBOARD_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_MIME_TYPES: usize = 128;
pub(super) const TEXT_MIME: &str = "text/plain;charset=utf-8";
pub(super) const PLAIN_TEXT_MIME: &str = "text/plain";
/// How long the clipboard owner may take to take over the clipboard and to
/// arm a paste; shared with the portal clipboard.
pub(super) const PREPARE_TIMEOUT: Duration = Duration::from_secs(5);
pub(super) const ARM_TIMEOUT: Duration = Duration::from_secs(1);
/// How long a paste may take until the clipboard is restored.
const PASTE_TIMEOUT: Duration = Duration::from_secs(3);
const SENSITIVE_MIME: &str = "x-kde-passwordManagerHint";

type Contents = HashMap<String, Arc<[u8]>>;

enum Command {
    Arm(Sender<()>),
    Finish(bool),
}

enum Request {
    /// Snapshot the current selection so a later paste need not read it.
    Warm,
    Paste {
        text: String,
        ready: Sender<Result<(), String>>,
        trigger: Receiver<Command>,
        done: Sender<Result<(), String>>,
    },
}

/// Wakes the clipboard thread from its poll when a request, command or
/// transfer result arrives.
struct Waker {
    read: UnixStream,
    write: UnixStream,
}

impl Waker {
    fn new() -> Result<Self, String> {
        let (read, write) = UnixStream::pair().map_err(|e| e.to_string())?;
        read.set_nonblocking(true).map_err(|e| e.to_string())?;
        write.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Self { read, write })
    }

    fn wake(&self) {
        // A full buffer already holds a pending wake-up.
        let _ = (&self.write).write(&[1]);
    }

    fn drain(&self) {
        while (&self.read).read(&mut [0; 64]).is_ok_and(|n| n > 0) {}
    }
}

/// The clipboard thread: one Wayland connection kept for the whole process,
/// so a paste skips connecting and, while the selection is unchanged, reading it.
struct Service {
    requests: Sender<Request>,
    waker: Arc<Waker>,
}

static SERVICE: Mutex<Option<Service>> = Mutex::new(None);

/// Send `request` to the clipboard thread, starting it if it is not running.
fn submit(request: Request) -> Result<Arc<Waker>, String> {
    let mut service = SERVICE.lock().unwrap_or_else(|e| e.into_inner());
    let request = match service.as_ref() {
        Some(running) => match running.requests.send(request) {
            Ok(()) => {
                running.waker.wake();
                return Ok(running.waker.clone());
            }
            Err(mpsc::SendError(request)) => request,
        },
        None => request,
    };
    let waker = Arc::new(Waker::new()?);
    let (requests, received) = mpsc::channel();
    let thread_waker = waker.clone();
    std::thread::Builder::new()
        .name("lhc-clipboard".into())
        .spawn(move || run(received, thread_waker))
        .map_err(|e| format!("start clipboard owner: {e}"))?;
    requests
        .send(request)
        .map_err(|_| "clipboard owner stopped")?;
    waker.wake();
    *service = Some(Service {
        requests,
        waker: waker.clone(),
    });
    Ok(waker)
}

/// Snapshot the clipboard ahead of a likely paste, e.g. when a popup with
/// text entries opens. Does not block; failures only cost the head start.
pub fn warm() {
    if let Err(error) = submit(Request::Warm) {
        log::debug!("[clipboard] warm: {error}");
    }
}

/// A temporary clipboard owner. Dropping it cancels the paste and restores its snapshot.
pub struct Paste {
    finish: Option<Sender<Command>>,
    done: Receiver<Result<(), String>>,
    waker: Arc<Waker>,
}

impl Paste {
    /// Snapshot all offered formats before taking temporary clipboard ownership.
    pub fn prepare(text: &str) -> Result<Self, String> {
        let (ready, prepared) = mpsc::channel();
        let (finish, trigger) = mpsc::channel();
        let (done, completed) = mpsc::channel();
        let waker = submit(Request::Paste {
            text: text.to_owned(),
            ready,
            trigger,
            done,
        })?;
        prepared
            .recv_timeout(PREPARE_TIMEOUT)
            .map_err(|_| "clipboard preparation timed out")??;
        Ok(Self {
            finish: Some(finish),
            done: completed,
            waker,
        })
    }

    /// Ignore reads before injection; call immediately before sending the paste shortcut.
    pub fn arm(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.finish
            .as_ref()
            .ok_or("clipboard owner stopped")?
            .send(Command::Arm(tx))
            .map_err(|_| "clipboard owner stopped")?;
        self.waker.wake();
        rx.recv_timeout(ARM_TIMEOUT)
            .map_err(|_| "clipboard owner did not arm".into())
    }

    /// Wait for a bounded data transfer, then restore unless another owner took over.
    pub fn finish(mut self, injected: bool) -> Result<(), String> {
        if let Some(finish) = self.finish.take() {
            finish
                .send(Command::Finish(injected))
                .map_err(|_| "clipboard owner stopped")?;
            self.waker.wake();
        }
        self.done
            .recv_timeout(PASTE_TIMEOUT)
            .map_err(|_| "clipboard transfer timed out")?
    }
}

impl Drop for Paste {
    fn drop(&mut self) {
        if let Some(finish) = self.finish.take() {
            let _ = finish.send(Command::Finish(false));
            self.waker.wake();
        }
    }
}

#[derive(Default)]
struct State {
    offers: HashMap<wayland_client::backend::ObjectId, Vec<String>>,
    offer_proxies: HashMap<wayland_client::backend::ObjectId, Offer>,
    selection: Option<Offer>,
    selected: bool,
    /// Formats of `selection` read ahead of a paste; cleared when it changes.
    snapshot: Option<Contents>,
    source: Option<Source>,
    contents: Contents,
    /// `contents` is the restored selection, not a paste in progress.
    restored: bool,
    cancelled: bool,
    transferred: bool,
    transfers: Vec<Receiver<(bool, Result<(), String>)>>,
    waker: Option<Arc<Waker>>,
}

impl State {
    /// Whether this connection still owns the selection it set last.
    fn owns(&self) -> bool {
        self.source.is_some() && !self.cancelled
    }

    fn wake(&self) -> impl Fn() + Send + 'static {
        let waker = self.waker.clone();
        move || {
            if let Some(waker) = &waker {
                waker.wake();
            }
        }
    }
}

struct Session {
    connection: Connection,
    queue: EventQueue<State>,
    manager: Manager,
    device: Device,
    state: State,
}

fn run(requests: Receiver<Request>, waker: Arc<Waker>) {
    let mut session = match Session::connect(waker) {
        Ok(session) => session,
        Err(error) => {
            log::debug!("[clipboard] {error}");
            for request in requests.try_iter() {
                if let Request::Paste { ready, done, .. } = request {
                    let _ = ready.send(Err(error.clone()));
                    let _ = done.send(Err(error.clone()));
                }
            }
            return;
        }
    };
    loop {
        loop {
            match requests.try_recv() {
                Ok(Request::Warm) => {
                    if let Err(error) = session.warm() {
                        log::debug!("[clipboard] warm: {error}");
                    }
                }
                Ok(Request::Paste {
                    text,
                    ready,
                    trigger,
                    done,
                }) => {
                    let result = session.paste(text, &ready, trigger, &done);
                    let _ = ready.send(Err(result
                        .as_ref()
                        .err()
                        .cloned()
                        .unwrap_or_else(|| "clipboard owner stopped".into())));
                    let _ = done.send(result);
                    // Never keep serving the pasted text as the clipboard:
                    // closing the connection drops the ownership.
                    if session.state.owns() && !session.state.restored {
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        if let Err(error) = session.idle() {
            if session.state.owns() {
                super::portal::report(crate::events::TextInputFailure::Clipboard(format!(
                    "restored clipboard connection failed: {error}"
                )));
            }
            #[cfg(test)]
            eprintln!("clipboard connection failed: {error}");
            return;
        }
    }
}

impl Session {
    fn connect(waker: Arc<Waker>) -> Result<Self, String> {
        let connection =
            Connection::connect_to_env().map_err(|e| format!("connect Wayland clipboard: {e}"))?;
        let (globals, mut queue) =
            registry_queue_init::<State>(&connection).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let manager = Manager::bind(&globals, &qh)?;
        let seat: WlSeat = globals
            .bind(&qh, 1..=1, ())
            .map_err(|e| format!("clipboard seat: {e}"))?;
        let device = manager.get_data_device(&seat, &qh);
        let mut state = State {
            waker: Some(waker),
            ..State::default()
        };
        let deadline = Instant::now() + TRANSFER_TIMEOUT;
        while !state.selected {
            if Instant::now() >= deadline {
                return Err("clipboard selection timed out".into());
            }
            pump(&connection, &mut queue, &mut state, deadline)?;
        }
        Ok(Self {
            connection,
            queue,
            manager,
            device,
            state,
        })
    }

    /// Serve the restored selection and wait for the next request.
    fn idle(&mut self) -> Result<(), String> {
        wait(
            &self.connection,
            &mut self.queue,
            &mut self.state,
            PollTimeout::NONE,
        )
    }

    fn barrier(&mut self) -> Result<(), String> {
        barrier(&self.connection, &mut self.queue, &mut self.state)
    }

    fn own(&mut self, contents: Contents) {
        own(
            &self.manager,
            &self.device,
            &self.queue.handle(),
            &mut self.state,
            contents,
        );
    }

    fn warm(&mut self) -> Result<(), String> {
        self.barrier()?;
        if !self.state.owns() && self.state.snapshot.is_none() {
            self.read_selection(Instant::now() + TRANSFER_TIMEOUT)?;
        }
        Ok(())
    }

    /// Formats to restore after a paste: our own restored copy, the snapshot
    /// read ahead, or a fresh read of the selection.
    fn saved(&mut self, deadline: Instant) -> Result<(Contents, &'static str), String> {
        // Apply a newer selection before trusting what we hold.
        self.barrier()?;
        if self.state.owns() && self.state.restored {
            return Ok((self.state.contents.clone(), "restored copy"));
        }
        if let Some(snapshot) = &self.state.snapshot {
            return Ok((snapshot.clone(), "read ahead"));
        }
        Ok((self.read_selection(deadline)?, "read now"))
    }

    fn read_selection(&mut self, deadline: Instant) -> Result<Contents, String> {
        let selection = self.state.selection.clone();
        let saved = snapshot(&self.connection, &mut self.queue, &mut self.state, deadline)?;
        // Receiving each MIME can run external clients; never replace a newer selection.
        self.barrier()?;
        if self.state.selection != selection {
            return Err("clipboard changed while saving it".into());
        }
        self.state.snapshot = Some(saved.clone());
        Ok(saved)
    }

    fn paste(
        &mut self,
        text: String,
        ready: &Sender<Result<(), String>>,
        trigger: Receiver<Command>,
        completed: &Sender<Result<(), String>>,
    ) -> Result<(), String> {
        let start = Instant::now();
        let (saved, origin) = self.saved(start + TRANSFER_TIMEOUT)?;
        if matches!(trigger.try_recv(), Err(mpsc::TryRecvError::Disconnected)) {
            return Err("clipboard preparation cancelled".into());
        }
        self.state.restored = false;
        self.own(HashMap::from([
            (TEXT_MIME.into(), Arc::from(text.as_bytes())),
            (PLAIN_TEXT_MIME.into(), Arc::from(text.into_bytes())),
            (SENSITIVE_MIME.into(), Arc::from(b"secret".as_slice())),
        ]));
        self.barrier()?;
        if self.state.cancelled {
            return Err("clipboard ownership changed before paste".into());
        }
        log::info!(
            "[clipboard] paste ready in {} ms, saved clipboard: {origin}",
            start.elapsed().as_millis()
        );
        let _ = ready.send(Ok(()));
        let deadline = Instant::now() + Duration::from_secs(3);
        let injected = loop {
            match trigger.try_recv() {
                Ok(Command::Finish(injected)) => break injected,
                Ok(Command::Arm(armed)) => {
                    self.state.transferred = false;
                    self.state.transfers.clear();
                    let _ = armed.send(());
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => break false,
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if Instant::now() >= deadline {
                break false;
            }
            if pump(&self.connection, &mut self.queue, &mut self.state, deadline).is_err() {
                break false;
            }
        };
        let mut result = if injected {
            Ok(())
        } else {
            Err("paste cancelled before injection".into())
        };
        if injected {
            let injected_at = Instant::now();
            let deadline = injected_at + TRANSFER_TIMEOUT;
            while !self.state.cancelled
                && (!self.state.transferred
                    || injected_at.elapsed() < PASTE_GRACE
                    || !self.state.transfers.is_empty())
            {
                if let Err(error) =
                    pump(&self.connection, &mut self.queue, &mut self.state, deadline)
                {
                    result = Err(error);
                    break;
                }
                if Instant::now() >= deadline {
                    result = Err("clipboard text transfer did not complete in time".into());
                    break;
                }
            }
        }
        // Apply queued cancellation before restoring; a user copy always takes precedence.
        self.barrier()?;
        if self.state.cancelled {
            return Err("clipboard ownership changed during paste".into());
        }
        if saved.is_empty() {
            self.device.set_selection(None);
            if let Some(source) = self.state.source.take() {
                source.destroy();
            }
            self.state.contents.clear();
            self.connection.flush().map_err(|e| e.to_string())?;
            return result;
        }
        self.own(saved);
        self.state.restored = true;
        self.barrier()?;
        let _ = completed.send(result);
        Ok(())
    }
}

fn own(
    manager: &Manager,
    device: &Device,
    qh: &QueueHandle<State>,
    state: &mut State,
    contents: Contents,
) {
    let source = manager.create_data_source(qh);
    for mime in contents.keys() {
        source.offer(mime.clone());
    }
    let previous = state.source.replace(source.clone());
    state.contents = contents;
    state.cancelled = false;
    state.transferred = false;
    state.transfers.clear();
    device.set_selection(Some(&source));
    if let Some(previous) = previous {
        previous.destroy();
    }
}

fn snapshot(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    deadline: Instant,
) -> Result<Contents, String> {
    let Some(offer) = state.selection.clone() else {
        return Ok(Contents::new());
    };
    let types = state
        .offers
        .get(&offer.id())
        .cloned()
        .ok_or("clipboard offer has no MIME list")?;
    if types.len() > MAX_MIME_TYPES {
        return Err("too many clipboard formats to preserve".into());
    }
    let mut result = Contents::new();
    let mut size = 0;
    for mime in types {
        let (read, write) = UnixStream::pair().map_err(|e| e.to_string())?;
        offer.receive(mime.clone(), write.as_fd());
        drop(write);
        connection.flush().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        let limit = MAX_CLIPBOARD_BYTES - size;
        let wake = state.wake();
        std::thread::Builder::new()
            .name("lhc-clipboard-read".into())
            .spawn(move || {
                let _ = tx.send(read_transfer(read, limit, deadline));
                wake();
            })
            .map_err(|e| format!("start clipboard reader: {e}"))?;
        let bytes = loop {
            if let Ok(result) = rx.try_recv() {
                break result?;
            }
            if Instant::now() >= deadline {
                return Err("saving clipboard timed out; clipboard left unchanged".into());
            }
            pump(connection, queue, state, deadline)?;
        };
        size += bytes.len();
        if size > MAX_CLIPBOARD_BYTES {
            return Err("clipboard is too large to preserve".into());
        }
        result.insert(mime, bytes.into());
    }
    Ok(result)
}

fn barrier(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let done = Arc::new(AtomicBool::new(false));
    connection.display().sync(&queue.handle(), done.clone());
    let deadline = Instant::now() + TRANSFER_TIMEOUT;
    while !done.load(Ordering::SeqCst) {
        if Instant::now() >= deadline {
            return Err("clipboard compositor acknowledgement timed out".into());
        }
        pump(connection, queue, state, deadline)?;
    }
    Ok(())
}

impl Dispatch<WlCallback, Arc<std::sync::atomic::AtomicBool>> for State {
    fn event(
        _: &mut Self,
        _: &WlCallback,
        _: <WlCallback as Proxy>::Event,
        done: &Arc<std::sync::atomic::AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        done.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

fn pump(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    deadline: Instant,
) -> Result<(), String> {
    let timeout = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .min(10) as u16;
    wait(connection, queue, state, PollTimeout::from(timeout))
}

/// Dispatch Wayland events, waiting up to `timeout` for new ones or a wake-up.
fn wait(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    timeout: PollTimeout,
) -> Result<(), String> {
    let dispatched = queue.dispatch_pending(state).map_err(|e| e.to_string())?;
    let transfers_before = state.transfers.len();
    state
        .transfers
        .retain(|transfer| match transfer.try_recv() {
            Ok((text, Ok(()))) => {
                state.transferred |= text;
                false
            }
            Ok((_, Err(_))) | Err(mpsc::TryRecvError::Disconnected) => false,
            Err(mpsc::TryRecvError::Empty) => true,
        });
    use wayland_client::backend::WaylandError;
    let needs_write = match connection.flush() {
        Ok(()) => false,
        Err(WaylandError::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            true
        }
        Err(error) => return Err(error.to_string()),
    };
    // Let callers check acknowledgements and completed transfers before waiting
    // for another event: the event they needed may have just been dispatched.
    if !needs_write && (dispatched > 0 || state.transfers.len() != transfers_before) {
        return Ok(());
    }
    let Some(guard) = queue.prepare_read() else {
        return Ok(());
    };
    let interest = PollFlags::POLLIN
        | if needs_write {
            PollFlags::POLLOUT
        } else {
            PollFlags::empty()
        };
    let waker = state.waker.clone();
    let mut fds = vec![PollFd::new(guard.connection_fd(), interest)];
    if let Some(waker) = &waker {
        fds.push(PollFd::new(waker.read.as_fd(), PollFlags::POLLIN));
    }
    let polled = poll(&mut fds, timeout);
    let wayland_ready = fds[0].revents().is_some_and(|events| !events.is_empty());
    drop(fds);
    if let Some(waker) = &waker {
        waker.drain();
    }
    match polled {
        Ok(_) if !wayland_ready => drop(guard),
        Ok(_) => match guard.read() {
            Ok(_) => {}
            Err(WaylandError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error.to_string()),
        },
        Err(nix::errno::Errno::EINTR) => drop(guard),
        Err(e) => return Err(e.to_string()),
    }
    queue.dispatch_pending(state).map_err(|e| e.to_string())?;
    Ok(())
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
wayland_client::delegate_noop!(State: ignore WlSeat);
wayland_client::delegate_noop!(State: ignore ZwlrDataControlManagerV1);
wayland_client::delegate_noop!(State: ignore ExtDataControlManagerV1);

impl State {
    fn new_offer(&mut self, offer: Offer) {
        self.offers.insert(offer.id(), Vec::new());
        self.offer_proxies.insert(offer.id(), offer);
    }
    fn select(&mut self, offer: Option<Offer>) {
        self.selection = offer;
        self.snapshot = None;
        self.selected = true;
        let current = self.selection.as_ref().map(Offer::id);
        self.offer_proxies.retain(|id, offer| {
            if Some(id) == current.as_ref() {
                true
            } else {
                offer.destroy();
                false
            }
        });
        self.offers.retain(|id, _| Some(id) == current.as_ref());
    }
    fn discard_primary(&mut self, offer: Option<Offer>) {
        if let Some(offer) = offer {
            self.offer_proxies.remove(&offer.id());
            self.offers.remove(&offer.id());
            offer.destroy();
        }
    }
    fn send(&mut self, source: Source, mime: String, fd: OwnedFd) {
        if self.source.as_ref() != Some(&source) || self.transfers.len() >= 16 {
            return;
        }
        let Some(bytes) = self.contents.get(&mime).cloned() else {
            return;
        };
        let text = mime == TEXT_MIME || mime == PLAIN_TEXT_MIME;
        let (tx, rx) = mpsc::channel();
        let wake = self.wake();
        std::thread::spawn(move || {
            let _ = tx.send((text, write_transfer(fd, &bytes)));
            wake();
        });
        self.transfers.push(rx);
    }
}

macro_rules! dispatch_clipboard {
    ($device:ty, $device_mod:ident, $offer:ty, $offer_mod:ident, $source:ty, $source_mod:ident, $variant:ident) => {
        impl Dispatch<$device, ()> for State {
            fn event(state: &mut Self, _: &$device, event: $device_mod::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $device_mod::Event::DataOffer { id } => state.new_offer(Offer::$variant(id)),
                    $device_mod::Event::Selection { id } => state.select(id.map(Offer::$variant)),
                    $device_mod::Event::PrimarySelection { id } => state.discard_primary(id.map(Offer::$variant)),
                    $device_mod::Event::Finished => state.cancelled = true,
                    _ => {}
                }
            }
            event_created_child!(State, $device, [$device_mod::EVT_DATA_OFFER_OPCODE => ($offer, ())]);
        }
        impl Dispatch<$offer, ()> for State {
            fn event(state: &mut Self, offer: &$offer, event: $offer_mod::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                if let $offer_mod::Event::Offer { mime_type } = event { state.offers.entry(offer.id()).or_default().push(mime_type); }
            }
        }
        impl Dispatch<$source, ()> for State {
            fn event(state: &mut Self, source: &$source, event: $source_mod::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $source_mod::Event::Cancelled if state.source.as_ref() == Some(&Source::$variant(source.clone())) => state.cancelled = true,
                    $source_mod::Event::Send { mime_type, fd } => state.send(Source::$variant(source.clone()), mime_type, fd),
                    _ => {}
                }
            }
        }
    };
}
dispatch_clipboard!(
    ExtDataControlDeviceV1,
    ext_data_control_device_v1,
    ExtDataControlOfferV1,
    ext_data_control_offer_v1,
    ExtDataControlSourceV1,
    ext_data_control_source_v1,
    Ext
);
dispatch_clipboard!(
    ZwlrDataControlDeviceV1,
    zwlr_data_control_device_v1,
    ZwlrDataControlOfferV1,
    zwlr_data_control_offer_v1,
    ZwlrDataControlSourceV1,
    zwlr_data_control_source_v1,
    Wlr
);

fn read_transfer(
    mut stream: UnixStream,
    limit: usize,
    deadline: Instant,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("clipboard owner timed out".into());
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(bytes),
            Ok(count) => {
                if count > limit.saturating_sub(bytes.len()) {
                    return Err("clipboard is too large to preserve".into());
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn write_transfer(fd: OwnedFd, bytes: &[u8]) -> Result<(), String> {
    write_transfer_checked(fd, bytes, || Ok(()))
}

pub(super) fn write_transfer_checked(
    fd: OwnedFd,
    bytes: &[u8],
    valid: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    use std::os::fd::AsRawFd;
    let raw = fd.as_raw_fd();
    // SAFETY: the descriptor is owned here; nonblocking writes keep readers bounded.
    unsafe {
        let flags = libc::fcntl(raw, libc::F_GETFL);
        if flags < 0 || libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    let mut file = std::fs::File::from(fd);
    let mut remaining = bytes;
    let deadline = Instant::now() + TRANSFER_TIMEOUT;
    while !remaining.is_empty() {
        valid()?;
        if Instant::now() >= deadline {
            return Err("clipboard reader timed out".into());
        }
        match file.write(remaining) {
            Ok(0) => return Err("clipboard reader closed the transfer".into()),
            Ok(count) => remaining = &remaining[count..],
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("clipboard reader timed out".into());
                }
                let mut fds = [PollFd::new(file.as_fd(), PollFlags::POLLOUT)];
                let _ = poll(&mut fds, PollTimeout::from(10u16));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn connect() -> (Connection, EventQueue<State>, Manager, Device, State) {
        let connection = Connection::connect_to_env().unwrap();
        let (globals, mut queue) = registry_queue_init::<State>(&connection).unwrap();
        let manager = Manager::bind(&globals, &queue.handle()).unwrap();
        let seat: WlSeat = globals.bind(&queue.handle(), 1..=1, ()).unwrap();
        let device = manager.get_data_device(&seat, &queue.handle());
        let mut state = State::default();
        let deadline = Instant::now() + TRANSFER_TIMEOUT;
        while !state.selected {
            assert!(Instant::now() < deadline);
            pump(&connection, &mut queue, &mut state, deadline).unwrap();
        }
        (connection, queue, manager, device, state)
    }

    struct Owner {
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Owner {
        fn new(contents: Contents) -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = stop.clone();
            let (ready, rx) = mpsc::channel();
            let thread = std::thread::spawn(move || {
                let (connection, mut queue, manager, device, mut state) = connect();
                own(&manager, &device, &queue.handle(), &mut state, contents);
                connection.flush().unwrap();
                // A display roundtrip ensures the selection was installed before the test proceeds.
                queue.roundtrip(&mut state).unwrap();
                let _ = ready.send(());
                while !thread_stop.load(Ordering::SeqCst) && !state.cancelled {
                    pump(
                        &connection,
                        &mut queue,
                        &mut state,
                        Instant::now() + Duration::from_millis(20),
                    )
                    .unwrap();
                }
            });
            rx.recv_timeout(Duration::from_secs(3)).unwrap();
            Self {
                stop,
                thread: Some(thread),
            }
        }
    }

    impl Drop for Owner {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn read_all() -> Contents {
        let (connection, mut queue, _, _, mut state) = connect();
        snapshot(
            &connection,
            &mut queue,
            &mut state,
            Instant::now() + TRANSFER_TIMEOUT,
        )
        .unwrap()
    }

    fn formats() -> Contents {
        HashMap::from([
            (
                "text/plain".into(),
                Arc::from("Прежний текст 👋".as_bytes()),
            ),
            ("text/html".into(), Arc::from(b"<b>old</b>".as_slice())),
            (
                "image/png".into(),
                Arc::from([0, 255, 137, 80, 78, 71].as_slice()),
            ),
            (
                "application/octet-stream".into(),
                Arc::from([0, 1, 0, 255].as_slice()),
            ),
        ])
    }

    #[test]
    #[ignore = "requires an isolated compositor with WAYLAND_DISPLAY=lhc-test-*; never run on the user clipboard"]
    fn isolated_clipboard_preserves_formats_cancellation_and_new_copies() {
        assert!(
            std::env::var("WAYLAND_DISPLAY")
                .unwrap()
                .starts_with("lhc-test-")
        );
        let saved = formats();
        let _owner = Owner::new(saved.clone());
        let paste = Paste::prepare("Текст 👋\n\t ").unwrap();
        paste.arm().unwrap();
        let inserted = read_all();
        assert_eq!(&*inserted[TEXT_MIME], "Текст 👋\n\t ".as_bytes());
        paste.finish(true).unwrap();
        assert_eq!(read_all(), saved);

        // A clipboard watcher can read before the target handles the shortcut.
        // Finish must keep serving the text for the delayed target as well.
        let paste = Paste::prepare("delayed target 👋").unwrap();
        paste.arm().unwrap();
        assert_eq!(&*read_all()[TEXT_MIME], "delayed target 👋".as_bytes());
        let finish = std::thread::spawn(move || paste.finish(true));
        std::thread::sleep(Duration::from_millis(80));
        let delayed = read_all();
        let result = finish.join().unwrap();
        assert_eq!(
            delayed.get(TEXT_MIME).map(|bytes| bytes.as_ref()),
            Some("delayed target 👋".as_bytes()),
            "a background reader must not restore the clipboard ahead of the target"
        );
        result.unwrap();
        assert_eq!(read_all(), saved);

        for _ in 0..3 {
            warm();
            let paste = Paste::prepare("cached 👋").unwrap();
            paste.arm().unwrap();
            assert_eq!(&*read_all()[TEXT_MIME], "cached 👋".as_bytes());
            paste.finish(true).unwrap();
            assert_eq!(read_all(), saved);
        }

        let paste = Paste::prepare("must not replace a new user copy").unwrap();
        paste.arm().unwrap();
        let _ = read_all();
        let finish = std::thread::spawn(move || paste.finish(true));
        std::thread::sleep(Duration::from_millis(80));
        let newer = HashMap::from([("image/png".into(), Arc::from(b"new image".as_slice()))]);
        let _new_owner = Owner::new(newer.clone());
        assert!(finish.join().unwrap().is_err());
        assert_eq!(read_all(), newer);

        let paste = Paste::prepare("cancelled").unwrap();
        drop(paste);
        let deadline = Instant::now() + Duration::from_secs(3);
        while read_all() != newer {
            assert!(
                Instant::now() < deadline,
                "cancelled paste did not restore all formats"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        let paste = Paste::prepare("no consumer").unwrap();
        paste.arm().unwrap();
        assert!(paste.finish(true).is_err());
        assert_eq!(read_all(), newer);
    }

    #[test]
    fn a_trickling_owner_cannot_extend_the_snapshot_deadline() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let thread = std::thread::spawn(move || {
            for _ in 0..100 {
                if writer.write_all(b"x").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let start = Instant::now();
        assert!(read_transfer(reader, 1024, start + Duration::from_millis(40)).is_err());
        assert!(start.elapsed() < Duration::from_millis(500));
        thread.join().unwrap();
    }

    #[test]
    fn oversized_snapshot_is_rejected_before_allocating_the_whole_offer() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer.write_all(b"12345").unwrap();
        drop(writer);
        assert!(read_transfer(reader, 4, Instant::now() + Duration::from_secs(1)).is_err());
    }

    #[test]
    fn a_stalled_clipboard_reader_cannot_block_the_worker() {
        let (_reader, writer) = UnixStream::pair().unwrap();
        let start = Instant::now();
        assert!(write_transfer(writer.into(), &vec![0; 2 * 1024 * 1024]).is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn binary_clipboard_data_is_transferred_without_text_conversion() {
        let (mut reader, writer) = UnixStream::pair().unwrap();
        let bytes = vec![0, 255, 0, 128, 42];
        write_transfer(writer.into(), &bytes).unwrap();
        let mut received = Vec::new();
        reader.read_to_end(&mut received).unwrap();
        assert_eq!(received, bytes);
    }
}
