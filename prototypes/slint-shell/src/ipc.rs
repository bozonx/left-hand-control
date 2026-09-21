use crate::{Command, Dispatch};
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
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct Request {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    trigger_ns: Option<u128>,
    command: String,
    token: Option<String>,
}

fn path() -> std::io::Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    Ok(dir.join(std::env::var("SLINT_SHELL_SOCKET").unwrap_or("lhc-slint-shell.sock".into())))
}

#[cfg(unix)]
pub struct Server(UnixListener, PathBuf);

#[cfg(windows)]
pub struct Server(TcpListener);

#[cfg(unix)]
impl Server {
    pub fn bind() -> std::io::Result<Self> {
        let path = path()?;
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
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
                    let mut line = String::new();
                    BufReader::new((&stream).take(8192)).read_line(&mut line)?;
                    let start = Instant::now();
                    let req: Request = serde_json::from_str(&line)?;
                    let command = Command::parse(&req.command)?;
                    let source = match req.source.as_deref() {
                        Some("button") => "button",
                        Some("tray") => "tray",
                        Some("evdev") => "evdev",
                        _ => "ipc",
                    };
                    let start = req
                        .trigger_ns
                        .and_then(|ns| {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .ok()?
                                .as_nanos();
                            let age = u64::try_from(now.checked_sub(ns)?).ok()?;
                            Instant::now().checked_sub(Duration::from_nanos(age))
                        })
                        .unwrap_or(start);
                    dispatch(command, source, start, req.token);
                    Ok(())
                })();
                let reply = match result {
                    Ok(()) => "queued\n".into(),
                    Err(e) => format!("error: {e}\n"),
                };
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Ok(())
    }
}

#[cfg(windows)]
impl Server {
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(tcp_address())?;
        Ok(Self(listener))
    }

    pub fn start(&self, dispatch: Dispatch) -> std::io::Result<()> {
        let listener = self.0.try_clone()?;
        std::thread::spawn(move || serve(listener, dispatch));
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.1);
    }
}

pub fn client(command: String) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        send(
            &path()?,
            command,
            "ipc",
            Instant::now(),
            std::env::var("XDG_ACTIVATION_TOKEN").ok(),
        )
    }
    #[cfg(windows)]
    {
        send_tcp(command, "ipc", Instant::now(), None)
    }
}

#[cfg(unix)]
pub fn send(
    path: &std::path::Path,
    command: String,
    source: &str,
    start: Instant,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    Command::parse(&command)?;
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = Request {
        command,
        token,
        source: Some(source.into()),
        trigger_ns: std::time::SystemTime::now()
            .checked_sub(start.elapsed())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos()),
    };
    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply != "queued\n" {
        return Err(reply.into());
    }

    Ok(())
}

#[cfg(windows)]
fn tcp_address() -> SocketAddrV4 {
    let port = std::env::var("SLINT_SHELL_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(43176);
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)
}

#[cfg(windows)]
fn send_tcp(
    command: String,
    source: &str,
    start: Instant,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect(tcp_address())?;
    send_stream(stream, command, source, start, token)
}

#[cfg(windows)]
fn serve(listener: TcpListener, dispatch: Dispatch) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let result = receive(&mut stream, &dispatch);
        let reply = match result {
            Ok(()) => "queued\n".into(),
            Err(error) => format!("error: {error}\n"),
        };
        let _ = stream.write_all(reply.as_bytes());
    }
}

#[cfg(windows)]
fn receive(stream: &mut TcpStream, dispatch: &Dispatch) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut line = String::new();
    BufReader::new((&*stream).take(8192)).read_line(&mut line)?;
    let start = Instant::now();
    let req: Request = serde_json::from_str(&line)?;
    let command = Command::parse(&req.command)?;
    let source = match req.source.as_deref() {
        Some("button") => "button",
        Some("tray") => "tray",
        Some("evdev") => "evdev",
        _ => "ipc",
    };
    dispatch(
        command,
        source,
        forwarded_start(req.trigger_ns, start),
        req.token,
    );
    Ok(())
}

#[cfg(windows)]
fn send_stream(
    mut stream: TcpStream,
    command: String,
    source: &str,
    start: Instant,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    Command::parse(&command)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = Request {
        command,
        token,
        source: Some(source.into()),
        trigger_ns: system_trigger(start),
    };
    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply != "queued\n" {
        return Err(reply.into());
    }
    Ok(())
}

#[cfg(windows)]
fn system_trigger(start: Instant) -> Option<u128> {
    std::time::SystemTime::now()
        .checked_sub(start.elapsed())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

#[cfg(windows)]
fn forwarded_start(trigger_ns: Option<u128>, fallback: Instant) -> Instant {
    trigger_ns
        .and_then(|ns| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_nanos();
            let age = u64::try_from(now.checked_sub(ns)?).ok()?;
            Instant::now().checked_sub(Duration::from_nanos(age))
        })
        .unwrap_or(fallback)
}
