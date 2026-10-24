//! Popup backend on Linux: winit windows, or the Spell worker process with
//! layer-shell surfaces (`LHC_POPUPS=auto|spell`).

use crate::{
    command::{Command, Source},
    ipc,
};
use std::{
    process::Child,
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
}

impl Worker {
    pub fn is_alive(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }

    pub fn send(
        &self,
        command: &Command,
        source: Source,
        start: Instant,
        token: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        ipc::send(&self.socket, command, source, start, token)
    }

    pub fn shutdown(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let socket = std::mem::take(&mut self.socket);
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
    let socket = format!("lhc-slint-spell-{}.sock", std::process::id());
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
        if worker
            .send(&Command::Ping, Source::Ipc, Instant::now(), None)
            .is_ok()
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err("Spell worker startup timed out".into());
        }
        std::thread::sleep(STARTUP_POLL);
    }
    log::info!(
        "popup backend=spell/skia-software; worker pid={}",
        worker.child.as_ref().map_or(0, Child::id)
    );
    Ok(worker)
}
