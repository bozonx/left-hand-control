//! Single-instance control channel.
//!
//! Unix: sockets in a private directory (`0700`, owned by the user) under
//! `XDG_RUNTIME_DIR`, or under the temp dir when that is unset, so other
//! users can neither connect nor squat the name. Windows: a loopback TCP
//! port (`SLINT_SHELL_PORT`, default 43176); every request carries a random
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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Socket file name of the settings process.
pub const DEFAULT_SOCKET: &str = "lhc-slint-shell.sock";

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
        Some(runtime) => PathBuf::from(runtime).join("lhc-slint-shell"),
        None => std::env::temp_dir().join(format!("lhc-slint-shell-{uid}")),
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
        return Err(std::io::Error::other(format!("invalid socket name {name:?}")));
    }
    Ok(socket_dir()?.join(name))
}

#[cfg(unix)]
fn own_socket() -> std::io::Result<PathBuf> {
    socket_path(&std::env::var("SLINT_SHELL_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET.into()))
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
fn token_path() -> std::io::Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| std::io::Error::other("local app data directory unknown"))?
        .join("lhc-slint-shell");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("ipc-token"))
}

/// 128 random bits from the standard library's OS-seeded hasher keys.
#[cfg(windows)]
fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |time| time.as_nanos()),
        );
        hasher.write_u32(std::process::id());
        hasher.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

pub struct Server {
    #[cfg(unix)]
    listener: UnixListener,
    #[cfg(unix)]
    path: PathBuf,
    #[cfg(windows)]
    listener: TcpListener,
    #[cfg(windows)]
    token: String,
}

impl Server {
    #[cfg(unix)]
    pub fn bind() -> std::io::Result<Self> {
        let path = own_socket()?;
        if path.exists() {
            match UnixStream::connect(&path) {
                Ok(_) => return Err(std::io::Error::other("slint-shell is already running")),
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(&path)?
                }
                Err(e) => return Err(e),
            }
        }
        // The directory is private, so the socket is reachable only by us.
        let listener = UnixListener::bind(&path)?;
        Ok(Self { listener, path })
    }

    #[cfg(windows)]
    pub fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(tcp_address())?;
        let token = random_token();
        std::fs::write(token_path()?, &token)?;
        Ok(Self { listener, token })
    }

    /// Serve requests on a background thread, handing commands to `dispatch`.
    pub fn start(&self, dispatch: Dispatch) -> std::io::Result<()> {
        let listener = self.listener.try_clone()?;
        #[cfg(unix)]
        let auth: Option<String> = None;
        #[cfg(windows)]
        let auth = Some(self.token.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                let reply = reply(receive(&stream, &dispatch, auth.as_deref()));
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
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
    BufReader::new(stream.take(8192)).read_line(&mut line)?;
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
#[cfg(unix)]
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
    auth: Option<String>,
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
        auth,
    };
    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
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
        let dispatch: Dispatch = Arc::new(move |command, _, _, _| sink.lock().unwrap().push(command));
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
    fn socket_names_stay_inside_the_private_dir() {
        assert!(socket_path("../escape.sock").is_err());
        assert!(socket_path("").is_err());
    }
}
