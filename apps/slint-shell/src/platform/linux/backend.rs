//! Popup backend on Linux: winit windows, or the Spell worker process with
//! layer-shell surfaces (`LHC_POPUPS=auto|spell`).

use crate::{
    command::{Command, Source, WorkerCommand},
    ipc,
};
use std::{
    process::Child,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};

/// Poll intervals while the worker quits and while it starts.
const QUIT_POLL: Duration = Duration::from_millis(10);
const STARTUP_POLL: Duration = Duration::from_millis(20);

/// How long a starting worker may take to answer on its socket.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a quitting worker may take before it is killed.
const QUIT_TIMEOUT: Duration = Duration::from_secs(1);

struct Probe;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Probe {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

pub fn layer_shell_available() -> Result<bool, Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;
    let (globals, _) = registry_queue_init::<Probe>(&connection)?;
    Ok(globals
        .contents()
        .with_list(|list| list.iter().any(|g| g.interface == "zwlr_layer_shell_v1")))
}

pub struct Worker {
    child: Option<Child>,
    /// Socket name in [`ipc::socket_dir`].
    socket: String,
    outgoing: Option<mpsc::SyncSender<WorkerRequest>>,
    stopped: Arc<AtomicBool>,
}

struct WorkerRequest {
    command: WorkerCommand,
    source: Source,
    start: Instant,
    token: Option<String>,
    completed: Box<dyn FnOnce(Result<(), String>) + Send>,
}

impl Worker {
    pub fn is_alive(&mut self) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && self
                .child
                .as_mut()
                .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }

    /// Enqueue without blocking the caller. The sender preserves command order
    /// and stops on failure so a show cannot overtake a failed config update.
    pub fn send(
        &self,
        command: &WorkerCommand,
        source: Source,
        start: Instant,
        token: Option<String>,
        completed: impl FnOnce(Result<(), String>) + Send + 'static,
    ) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("Spell sender stopped".into());
        }
        self.outgoing
            .as_ref()
            .ok_or("Spell sender not started")?
            .try_send(WorkerRequest {
                command: command.clone(),
                source,
                start,
                token,
                completed: Box::new(completed),
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Spell command queue full".into(),
                mpsc::TrySendError::Disconnected(_) => "Spell sender stopped".into(),
            })
    }

    fn start_sender(&mut self) -> std::io::Result<()> {
        let (tx, rx) = mpsc::sync_channel::<WorkerRequest>(64);
        let socket = self.socket.clone();
        let stopped = self.stopped.clone();
        std::thread::Builder::new()
            .name("lhc-popup-ipc".into())
            .spawn(move || {
                while let Ok(request) = rx.recv() {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    if let Err(error) = ipc::send(
                        &socket,
                        &request.command.into(),
                        request.source,
                        request.start,
                        request.token,
                    ) {
                        stopped.store(true, Ordering::Release);
                        (request.completed)(Err(error.to_string()));
                        break;
                    }
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    (request.completed)(Ok(()));
                }
            })?;
        self.outgoing = Some(tx);
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.outgoing.take();
        let Some(mut child) = self.child.take() else {
            return;
        };
        let socket = std::mem::take(&mut self.socket);
        // Joining and socket I/O must not run on the UI thread.
        std::thread::spawn(move || {
            let _ = ipc::send(&socket, &Command::Quit, Source::Ipc, Instant::now(), None);
            let deadline = Instant::now() + QUIT_TIMEOUT;
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(QUIT_POLL);
            }
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            if let Err(error) = child.wait() {
                log::error!("wait for Spell worker: {error}");
            }
            if let Ok(path) = ipc::socket_path(&socket) {
                let _ = std::fs::remove_file(path);
            }
        });
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn wayland_session() -> bool {
    lhc_core::platform::linux::detect()
        .wayland_display
        .is_some()
}

/// Whether popups should run in the Spell worker. Quick: only checks the
/// environment and asks the compositor for layer-shell.
pub fn spell_requested() -> Result<bool, Box<dyn std::error::Error>> {
    let mode = super::super::PopupMode::from_env()?;
    if mode == super::super::PopupMode::Winit {
        return Ok(false);
    }
    let available = wayland_session() && layer_shell_available()?;
    if !available {
        if mode == super::super::PopupMode::Spell {
            return Err("compositor does not advertise zwlr_layer_shell_v1".into());
        }
        log::info!(
            "popup backend=winit-fallback; layer-shell unavailable; compositor controls centering and activation"
        );
        return Ok(false);
    }
    if !cfg!(feature = "spell") {
        return Err("build with --features spell to use layer-shell".into());
    }
    Ok(true)
}

/// Whether the worker can show layer-shell surfaces, whatever popup mode was
/// requested: a Spell build on a compositor with layer-shell.
pub fn layer_shell_supported() -> bool {
    cfg!(feature = "spell")
        && wayland_session()
        && layer_shell_available().unwrap_or_else(|error| {
            log::warn!("layer-shell check: {error}");
            false
        })
}

/// Start the worker and wait until it answers. Blocks; call it off the UI
/// thread.
pub fn spawn() -> Result<Worker, Box<dyn std::error::Error>> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let socket = format!(
        "lhc-slint-spell-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("--spell-worker")
        .stdin(std::process::Stdio::piped())
        .env(
            crate::env::PARENT_SOCKET,
            std::env::var(crate::env::SOCKET).unwrap_or_else(|_| ipc::DEFAULT_SOCKET.into()),
        )
        .env(crate::env::SOCKET, &socket)
        .env_remove("XDG_ACTIVATION_TOKEN");
    match std::env::var_os(crate::env::METRICS) {
        Some(metrics) => {
            let mut path = metrics;
            path.push(".spell.csv");
            command.env(crate::env::METRICS, path)
        }
        None => command.env_remove(crate::env::METRICS),
    };
    let mut worker = Worker {
        child: Some(command.spawn()?),
        socket,
        outgoing: None,
        stopped: Arc::new(AtomicBool::new(false)),
    };
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Some(status) = worker
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok().flatten())
        {
            return Err(format!("Spell worker exited: {status}").into());
        }
        if ipc::send(
            &worker.socket,
            &Command::Ping,
            Source::Ipc,
            Instant::now(),
            None,
        )
        .is_ok()
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err("Spell worker startup timed out".into());
        }
        std::thread::sleep(STARTUP_POLL);
    }
    worker.start_sender()?;
    log::info!(
        "popup backend=spell/skia-software; worker pid={}",
        worker.child.as_ref().map_or(0, Child::id)
    );
    Ok(worker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
    };

    fn sender() -> (Worker, UnixListener) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let socket = format!(
            "test-popup-sender-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let listener = UnixListener::bind(ipc::socket_path(&socket).unwrap()).unwrap();
        let mut worker = Worker {
            child: None,
            socket,
            outgoing: None,
            stopped: Arc::new(AtomicBool::new(false)),
        };
        worker.start_sender().unwrap();
        (worker, listener)
    }

    #[test]
    fn worker_sender_keeps_fifo_without_waiting_on_the_caller() {
        let (mut worker, listener) = sender();
        let path = ipc::socket_path(&worker.socket).unwrap();
        let (entered, entry) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let mut commands = Vec::new();
            for n in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let request: serde_json::Value = serde_json::from_str(&line).unwrap();
                commands.push(request["command"].as_str().unwrap().to_owned());
                if n == 0 {
                    entered.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(2)).unwrap();
                }
                stream.write_all(b"queued\n").unwrap();
            }
            commands
        });
        let (completion, completions) = mpsc::channel();
        let first = completion.clone();
        worker
            .send(
                &WorkerCommand::PopupLayout(None),
                Source::Ipc,
                Instant::now(),
                None,
                move |result| {
                    let _ = first.send(result);
                },
            )
            .unwrap();
        entry.recv_timeout(Duration::from_secs(2)).unwrap();
        // The first exchange is still waiting for its reply. Enqueueing the
        // following command must nevertheless return before we release it.
        worker
            .send(
                &WorkerCommand::Hide,
                Source::Ipc,
                Instant::now(),
                None,
                move |result| {
                    let _ = completion.send(result);
                },
            )
            .unwrap();
        assert!(completions.try_recv().is_err());
        release.send(()).unwrap();
        assert_eq!(server.join().unwrap(), ["popup-layout null", "hide"]);
        assert_eq!(
            completions.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(())
        );
        assert_eq!(
            completions.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(())
        );
        worker.shutdown();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_worker_exchange_stops_following_commands() {
        let (mut worker, listener) = sender();
        let path = ipc::socket_path(&worker.socket).unwrap();
        let (entered, entry) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            entered.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
            stream.write_all(b"error: rejected\n").unwrap();
            listener
        });
        let (failure, failures) = mpsc::channel();
        worker
            .send(
                &WorkerCommand::PopupLayout(None),
                Source::Ipc,
                Instant::now(),
                None,
                move |result| {
                    let _ = failure.send(result);
                },
            )
            .unwrap();
        entry.recv_timeout(Duration::from_secs(2)).unwrap();
        let show = WorkerCommand::Show(crate::command::Popup::Quick);
        worker
            .send(&show, Source::Ipc, Instant::now(), None, |_| {})
            .unwrap();
        release.send(()).unwrap();
        assert_eq!(
            failures.recv_timeout(Duration::from_secs(2)).unwrap(),
            Err("error: rejected".into())
        );
        assert!(
            worker
                .send(&show, Source::Ipc, Instant::now(), None, |_| {})
                .is_err()
        );
        let listener = server.join().unwrap();
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        worker.shutdown();
        std::fs::remove_file(path).unwrap();
    }
}
