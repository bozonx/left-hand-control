use crate::{Command, Dispatch};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct Request {
    command: String,
    token: Option<String>,
}

fn path() -> std::io::Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| std::io::Error::other("XDG_RUNTIME_DIR is required"))?;
    Ok(PathBuf::from(dir).join("lhc-slint-shell.sock"))
}

pub struct Server(UnixListener, PathBuf);

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
                    dispatch(command, "ipc", start, req.token);
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

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.1);
    }
}

pub fn client(command: String) -> Result<(), Box<dyn std::error::Error>> {
    Command::parse(&command)?;
    let mut stream = UnixStream::connect(path()?)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = Request {
        command,
        token: std::env::var("XDG_ACTIVATION_TOKEN").ok(),
    };
    writeln!(stream, "{}", serde_json::to_string(&request)?)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply != "queued\n" {
        return Err(reply.into());
    }
    print!("{reply}");
    Ok(())
}
