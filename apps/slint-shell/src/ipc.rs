//! Single-instance control channel.
//!
//! Unix: sockets in a private directory (`0700`, owned by the user) under
//! `XDG_RUNTIME_DIR`, or under the temp dir when that is unset, so other
//! users can neither connect nor squat the name. Windows: a loopback TCP
//! port (`LHC_PORT`, default 43176); every request carries a random
//! token the server writes to the user's local app data, which other users
//! cannot read. Each request is one JSON line; the reply is `queued` or
//! `error: …`. `queued` means the command reached the UI queue, not that a
//! window is ready for input.

use crate::command::{Command, Dispatch, Source};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Name of the per-user runtime directories and of the tray item. Kept
/// from the shell's former name so running instances still find each other.
pub const RUNTIME_NAME: &str = "lhc-slint-shell";

/// Socket file name of the settings process.
#[cfg(unix)]
pub const DEFAULT_SOCKET: &str = "lhc-slint-shell.sock";

/// Upper bound of one request line; a request is a short JSON object.
const MAX_REQUEST_BYTES: u64 = 4 * 1024 * 1024;
/// Upper bound of the server's reply line (`queued` or an error).
const MAX_REPLY_BYTES: u64 = 4096;
/// How often the nonblocking listener checks for a connection or a stop.
const ACCEPT_POLL: Duration = Duration::from_millis(20);
/// Read/write timeout of an accepted request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
/// Read/write timeout of a client waiting for the reply.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(unix)]
type Connection = UnixStream;
#[cfg(windows)]
type Connection = TcpStream;

/// Bound the entire request, including a client that keeps trickling bytes.
struct RequestReader<'a> {
    stream: &'a Connection,
    deadline: Instant,
    stopped: &'a AtomicBool,
}

impl Read for RequestReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.stopped.load(Ordering::Acquire) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "IPC server stopped",
                ));
            }
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "IPC request deadline exceeded",
                ));
            }
            self.stream
                .set_read_timeout(Some(remaining.min(ACCEPT_POLL)))?;
            match self.stream.read(buf) {
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                result => return result,
            }
        }
    }
}

/// Loopback port of the IPC server on Windows unless `LHC_PORT` sets one.
#[cfg(windows)]
const DEFAULT_PORT: u16 = 43176;

#[derive(Serialize, Deserialize)]
struct Request {
    #[serde(default)]
    source: Option<String>,
    /// Trigger time as UNIX nanoseconds, so latency metrics include IPC.
    #[serde(default)]
    trigger_ns: Option<u128>,
    command: String,
    token: Option<String>,
    /// Windows only: proof that the client runs as the same user.
    #[serde(default)]
    auth: Option<String>,
}

/// Private directory holding the sockets; created `0700` when missing and
/// rejected when another user owns it or others can access it.
#[cfg(unix)]
pub fn socket_dir() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let uid = unsafe { libc::getuid() };
    let dir = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime) => PathBuf::from(runtime).join(RUNTIME_NAME),
        None => std::env::temp_dir().join(format!("{RUNTIME_NAME}-{uid}")),
    };
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let meta = std::fs::symlink_metadata(&dir)?;
    if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::other(format!(
            "{} is not a private directory of this user",
            dir.display()
        )));
    }
    Ok(dir)
}

/// Path of socket `name` (a bare file name) in [`socket_dir`].
#[cfg(unix)]
pub fn socket_path(name: &str) -> std::io::Result<PathBuf> {
    if name.is_empty() || name.contains('/') {
        return Err(std::io::Error::other(format!(
            "invalid socket name {name:?}"
        )));
    }
    Ok(socket_dir()?.join(name))
}

#[cfg(unix)]
fn own_socket() -> std::io::Result<PathBuf> {
    socket_path(&std::env::var(crate::env::SOCKET).unwrap_or_else(|_| DEFAULT_SOCKET.into()))
}

#[cfg(windows)]
fn tcp_address() -> SocketAddrV4 {
    let port = std::env::var(crate::env::PORT)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)
}

#[cfg(windows)]
fn token_path() -> std::io::Result<PathBuf> {
    let dir = if lhc_core::storage::isolated() {
        lhc_core::storage::StoragePaths::resolve()
            .map_err(std::io::Error::other)?
            .settings_dir()
    } else {
        dirs::data_local_dir()
            .ok_or_else(|| std::io::Error::other("local app data directory unknown"))?
            .join(RUNTIME_NAME)
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("ipc-token"))
}

/// Generate a token with the Windows system cryptographic RNG.
#[cfg(windows)]
fn random_token() -> std::io::Result<String> {
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut bytes = [0u8; 16];
    // SAFETY: the system RNG needs no algorithm handle and receives a valid buffer.
    let status = unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status.0 < 0 {
        return Err(std::io::Error::other(format!(
            "generate IPC token: NTSTATUS {:#010x}",
            status.0
        )));
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub struct Server {
    #[cfg(unix)]
    listener: UnixListener,
    #[cfg(unix)]
    path: PathBuf,
    /// Held through socket cleanup; the lock file must never be unlinked.
    #[cfg(unix)]
    _lock: std::fs::File,
    #[cfg(windows)]
    listener: TcpListener,
    #[cfg(windows)]
    token: String,
    stopped: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Server {
    #[cfg(unix)]
    pub fn bind() -> std::io::Result<Self> {
        Self::bind_path(own_socket()?)
    }

    #[cfg(unix)]
    fn bind_path(path: PathBuf) -> std::io::Result<Self> {
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "left-hand-control is already running",
            ),
            std::fs::TryLockError::Error(error) => error,
        })?;
        if path.exists() {
            match UnixStream::connect(&path) {
                Ok(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        "left-hand-control is already running",
                    ));
                }
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(&path)?
                }
                Err(e) => return Err(e),
            }
        }
        // The directory is private, so the socket is reachable only by us.
        let listener = UnixListener::bind(&path)?;
        Ok(Self {
            listener,
            path,
            _lock: lock,
            stopped: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        })
    }

    #[cfg(windows)]
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(tcp_address())?;
        let token = random_token()?;
        std::fs::write(token_path()?, &token)?;
        Ok(Self {
            listener,
            token,
            stopped: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        })
    }

    /// Serve requests on a background thread, handing commands to `dispatch`.
    pub fn start(&self, dispatch: Dispatch) -> std::io::Result<()> {
        let listener = self.listener.try_clone()?;
        listener.set_nonblocking(true)?;
        let stopped = self.stopped.clone();
        #[cfg(unix)]
        let auth: Option<String> = None;
        #[cfg(windows)]
        let auth = Some(self.token.clone());
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(ACCEPT_POLL);
                        continue;
                    }
                    Err(error) => {
                        log::warn!("IPC accept: {error}");
                        break;
                    }
                };
                let timeout = Some(REQUEST_TIMEOUT);
                // Windows inherits nonblocking mode from the listener. Each
                // accepted request uses blocking I/O with bounded timeouts.
                if let Err(error) = stream
                    .set_nonblocking(false)
                    .and_then(|()| stream.set_read_timeout(timeout))
                    .and_then(|()| stream.set_write_timeout(timeout))
                {
                    log::warn!("IPC timeout: {error}");
                    continue;
                }
                let reader = RequestReader {
                    stream: &stream,
                    deadline: Instant::now() + REQUEST_TIMEOUT,
                    stopped: &stopped,
                };
                let reply = reply(receive(reader, &dispatch, auth.as_deref()));
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        *self
            .thread
            .lock()
            .map_err(|_| std::io::Error::other("IPC thread lock poisoned"))? = Some(thread);
        Ok(())
    }
}

impl Server {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(mut thread) = self.thread.lock()
            && let Some(thread) = thread.take()
            && thread.join().is_err()
        {
            log::error!("IPC server thread panicked");
        }
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(windows)]
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

fn reply(result: Result<(), Box<dyn std::error::Error>>) -> String {
    match result {
        Ok(()) => "queued\n".into(),
        Err(error) => format!("error: {error}\n"),
    }
}

fn receive(
    stream: impl Read,
    dispatch: &Dispatch,
    auth: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut line = String::new();
    BufReader::new(stream.take(MAX_REQUEST_BYTES)).read_line(&mut line)?;
    let received = Instant::now();
    let request: Request = serde_json::from_str(&line)?;
    if auth.is_some() && request.auth.as_deref() != auth {
        return Err("not authorized".into());
    }
    let command = Command::parse(&request.command)?;
    dispatch(
        command,
        Source::from_wire(request.source.as_deref()),
        forwarded_start(request.trigger_ns, received),
        request.token,
    );
    Ok(())
}

/// CLI entry: forward one command to the running instance.
pub fn client(command: &Command) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        exchange(
            UnixStream::connect(own_socket()?)?,
            command,
            Source::Ipc,
            Instant::now(),
            std::env::var("XDG_ACTIVATION_TOKEN").ok(),
            None,
        )
    }
    #[cfg(windows)]
    {
        let auth = std::fs::read_to_string(token_path()?)?;
        exchange(
            TcpStream::connect(tcp_address())?,
            command,
            Source::Ipc,
            Instant::now(),
            None,
            Some(auth.trim().to_owned()),
        )
    }
}

/// Send to the socket named `name` in [`socket_dir`].
#[cfg(target_os = "linux")]
pub fn send(
    name: &str,
    command: &Command,
    source: Source,
    start: Instant,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    exchange(
        UnixStream::connect(socket_path(name)?)?,
        command,
        source,
        start,
        token,
        None,
    )
}

trait Stream: Read + Write {
    fn set_timeout(&self, timeout: Duration) -> std::io::Result<()>;
}

#[cfg(unix)]
impl Stream for UnixStream {
    fn set_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(timeout))?;
        self.set_write_timeout(Some(timeout))
    }
}

#[cfg(windows)]
impl Stream for TcpStream {
    fn set_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(timeout))?;
        self.set_write_timeout(Some(timeout))
    }
}

fn exchange(
    mut stream: impl Stream,
    command: &Command,
    source: Source,
    start: Instant,
    token: Option<String>,
    auth: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_timeout(CLIENT_TIMEOUT)?;
    let request = Request {
        command: command.to_string(),
        token,
        source: Some(source.as_str().into()),
        trigger_ns: SystemTime::now()
            .checked_sub(start.elapsed())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
        auth,
    };
    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    let mut reply = String::new();
    BufReader::new(stream.take(MAX_REPLY_BYTES)).read_line(&mut reply)?;
    if reply != "queued\n" {
        return Err(reply.trim_end().to_owned().into());
    }
    Ok(())
}

/// Map a forwarded wall-clock trigger onto this process's monotonic clock.
/// Sensitive to clock changes during a measurement series.
fn forwarded_start(trigger_ns: Option<u128>, fallback: Instant) -> Instant {
    trigger_ns
        .and_then(|ns| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_nanos();
            let age = u64::try_from(now.checked_sub(ns)?).ok()?;
            Instant::now().checked_sub(Duration::from_nanos(age))
        })
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn dispatch() -> (Dispatch, Arc<Mutex<Vec<Command>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let dispatch: Dispatch =
            Arc::new(move |command, _, _, _| sink.lock().unwrap().push(command));
        (dispatch, seen)
    }

    fn line(command: &str, auth: Option<&str>) -> Vec<u8> {
        let request = Request {
            source: None,
            trigger_ns: None,
            command: command.into(),
            token: None,
            auth: auth.map(str::to_owned),
        };
        format!("{}\n", serde_json::to_string(&request).unwrap()).into_bytes()
    }

    #[test]
    fn requests_need_the_token_when_one_is_set() {
        let (dispatch, seen) = dispatch();
        assert!(receive(&line("ping", None)[..], &dispatch, Some("secret")).is_err());
        assert!(receive(&line("ping", Some("wrong"))[..], &dispatch, Some("secret")).is_err());
        receive(&line("ping", Some("secret"))[..], &dispatch, Some("secret")).unwrap();
        receive(&line("hide", None)[..], &dispatch, None).unwrap();
        assert_eq!(*seen.lock().unwrap(), [Command::Ping, Command::Hide]);
        assert!(receive(&b"not json\n"[..], &dispatch, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_binds_preserve_the_winning_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc.sock");
        // Leave a stale socket, as after an unclean process exit.
        drop(UnixListener::bind(&path).unwrap());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let result = Server::bind_path(path.clone());
                    // Keep the winner alive until both bind attempts have finished.
                    barrier.wait();
                    match result {
                        Ok(server) => {
                            assert!(UnixStream::connect(&path).is_ok());
                            drop(server);
                            true
                        }
                        Err(error) => {
                            assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
                            false
                        }
                    }
                })
            })
            .collect();
        let won = threads
            .into_iter()
            .map(|thread| usize::from(thread.join().unwrap()))
            .sum::<usize>();
        assert_eq!(won, 1);
        assert!(!path.exists());
        // Cleanup releases the lock, allowing a subsequent instance to start.
        let server = Server::bind_path(path.clone()).unwrap();
        assert!(UnixStream::connect(&path).is_ok());
        drop(server);
    }

    #[cfg(unix)]
    #[test]
    fn trickling_requests_cannot_extend_the_deadline() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let finished = Arc::new(AtomicBool::new(false));
        let done = finished.clone();
        let writing = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !done.load(Ordering::Acquire) && Instant::now() < deadline {
                if writer.write_all(b" ").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let (dispatch, seen) = dispatch();
        let stopped = AtomicBool::new(false);
        let start = Instant::now();
        let result = receive(
            RequestReader {
                stream: &reader,
                deadline: start + Duration::from_millis(100),
                stopped: &stopped,
            },
            &dispatch,
            None,
        );
        let elapsed = start.elapsed();
        finished.store(true, Ordering::Release);
        writing.join().unwrap();
        assert_eq!(
            result
                .unwrap_err()
                .downcast::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::TimedOut
        );
        assert!(elapsed < Duration::from_millis(700), "{elapsed:?}");
        assert!(seen.lock().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn stopping_interrupts_an_incomplete_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ipc.sock");
        let server = Server::bind_path(path.clone()).unwrap();
        let (dispatch, seen) = dispatch();
        server.start(dispatch).unwrap();
        let mut stream = UnixStream::connect(&path).unwrap();
        stream.write_all(b"{").unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        server.stop();
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_millis(700), "{elapsed:?}");
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn accepted_connections_wait_for_fragmented_requests() {
        #[cfg(unix)]
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let path = dir.path().join("ipc.sock");
        #[cfg(unix)]
        let server = Server::bind_path(path.clone()).unwrap();
        #[cfg(windows)]
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        #[cfg(windows)]
        let address = listener.local_addr().unwrap();
        #[cfg(windows)]
        let server = Server {
            listener,
            token: "secret".into(),
            stopped: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        };
        let (dispatch, seen) = dispatch();
        server.start(dispatch).unwrap();
        #[cfg(unix)]
        let mut stream = UnixStream::connect(path).unwrap();
        #[cfg(windows)]
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        // Let the server accept before bytes arrive, then split the JSON line
        // across two writes. Neither accept nor the first fragment is a request.
        std::thread::sleep(Duration::from_millis(100));
        let payload = line("hide", Some("secret"));
        let middle = payload.len() / 2;
        stream.write_all(&payload[..middle]).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(seen.lock().unwrap().is_empty());
        stream.write_all(&payload[middle..]).unwrap();
        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response).unwrap();
        assert_eq!(response, "queued\n");
        assert_eq!(*seen.lock().unwrap(), [Command::Hide]);
    }

    #[cfg(unix)]
    #[test]
    fn socket_names_stay_inside_the_private_dir() {
        assert!(socket_path("../escape.sock").is_err());
        assert!(socket_path("").is_err());
    }
}
