//! Single-instance control channel.
//!
//! Unix: a `0600` socket in `XDG_RUNTIME_DIR` (or the temp dir). Windows: a
//! loopback-only TCP port (`SLINT_SHELL_PORT`, default 43176). Each request
//! is one JSON line; the reply is `queued` or `error: …`. `queued` means the
//! command reached the UI queue, not that a window is ready for input.

use crate::command::{Command, Dispatch, Source};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::{
    fs::PermissionsExt,
    net::{UnixListener, UnixStream},
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Serialize, Deserialize)]
struct Request {
    #[serde(default)]
    source: Option<String>,
    /// Trigger time as UNIX nanoseconds, so latency metrics include IPC.
    #[serde(default)]
    trigger_ns: Option<u128>,
    command: String,
    token: Option<String>,
}

#[cfg(unix)]
fn path() -> std::path::PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join(std::env::var("SLINT_SHELL_SOCKET").unwrap_or("lhc-slint-shell.sock".into()))
}

#[cfg(windows)]
fn tcp_address() -> SocketAddrV4 {
    let port = std::env::var("SLINT_SHELL_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(43176);
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)
}

#[cfg(unix)]
pub struct Server(UnixListener, std::path::PathBuf);

#[cfg(windows)]
pub struct Server(TcpListener);

#[cfg(unix)]
impl Server {
    pub fn bind() -> std::io::Result<Self> {
        let path = path();
        if path.exists() {
            match UnixStream::connect(&path) {
                Ok(_) => return Err(std::io::Error::other("slint-shell is already running")),
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(&path)?
                }
                Err(e) => return Err(e),
            }
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self(listener, path))
    }

    pub fn start(&self, dispatch: Dispatch) -> std::io::Result<()> {
        let listener = self.0.try_clone()?;
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let reply = reply(receive(&stream, &dispatch));
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.1);
    }
}

#[cfg(windows)]
impl Server {
    pub fn bind() -> std::io::Result<Self> {
        Ok(Self(TcpListener::bind(tcp_address())?))
    }

    pub fn start(&self, dispatch: Dispatch) -> std::io::Result<()> {
        let listener = self.0.try_clone()?;
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let reply = reply(receive(&stream, &dispatch));
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Ok(())
    }
}

fn reply(result: Result<(), Box<dyn std::error::Error>>) -> String {
    match result {
        Ok(()) => "queued\n".into(),
        Err(error) => format!("error: {error}\n"),
    }
}

fn receive(stream: impl Read, dispatch: &Dispatch) -> Result<(), Box<dyn std::error::Error>> {
    let mut line = String::new();
    BufReader::new(stream.take(8192)).read_line(&mut line)?;
    let received = Instant::now();
    let request: Request = serde_json::from_str(&line)?;
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
        send(
            &path(),
            command,
            Source::Ipc,
            Instant::now(),
            std::env::var("XDG_ACTIVATION_TOKEN").ok(),
        )
    }
    #[cfg(windows)]
    {
        exchange(
            TcpStream::connect(tcp_address())?,
            command,
            Source::Ipc,
            Instant::now(),
            None,
        )
    }
}

#[cfg(unix)]
pub fn send(
    path: &std::path::Path,
    command: &Command,
    source: Source,
    start: Instant,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    exchange(UnixStream::connect(path)?, command, source, start, token)
}

trait Stream: Read + Write {
    fn set_timeout(&self, timeout: Duration) -> std::io::Result<()>;
}

#[cfg(unix)]
impl Stream for UnixStream {
    fn set_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(timeout))
    }
}

#[cfg(windows)]
impl Stream for TcpStream {
    fn set_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(timeout))
    }
}

fn exchange(
    mut stream: impl Stream,
    command: &Command,
    source: Source,
    start: Instant,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_timeout(Duration::from_secs(2))?;
    let request = Request {
        command: command.to_string(),
        token,
        source: Some(source.as_str().into()),
        trigger_ns: SystemTime::now()
            .checked_sub(start.elapsed())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
    };
    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply != "queued\n" {
        return Err(reply.into());
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
