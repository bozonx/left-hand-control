//! Popup backend on Linux: winit windows, or the Spell worker process with
//! layer-shell surfaces (`SLINT_SHELL_POPUPS=auto|spell`).

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
            std::thread::sleep(Duration::from_millis(10));
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

/// Whether popups should run in the Spell worker. Quick: only checks the
/// environment and asks the compositor for layer-shell.
pub fn spell_requested() -> Result<bool, Box<dyn std::error::Error>> {
    let mode = std::env::var("SLINT_SHELL_POPUPS").unwrap_or_else(|_| "winit".into());
    if mode == "winit" {
        return Ok(false);
    }
    if mode != "auto" && mode != "spell" {
        return Err("SLINT_SHELL_POPUPS must be winit, auto or spell".into());
    }
    let available = std::env::var_os("WAYLAND_DISPLAY").is_some() && layer_shell_available()?;
    if !available {
        if mode == "spell" {
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

/// Start the worker and wait until it answers. Blocks; call it off the UI
/// thread.
pub fn spawn() -> Result<Worker, Box<dyn std::error::Error>> {
    let socket = format!("lhc-slint-spell-{}.sock", std::process::id());
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("--spell-worker")
        .stdin(std::process::Stdio::piped())
        .env(
            "SLINT_SHELL_PARENT_SOCKET",
            std::env::var("SLINT_SHELL_SOCKET").unwrap_or_else(|_| ipc::DEFAULT_SOCKET.into()),
        )
        .env("SLINT_SHELL_SOCKET", &socket)
        .env_remove("XDG_ACTIVATION_TOKEN");
    match std::env::var_os("SLINT_SHELL_METRICS") {
        Some(metrics) => {
            let mut path = metrics;
            path.push(".spell.csv");
            command.env("SLINT_SHELL_METRICS", path)
        }
        None => command.env_remove("SLINT_SHELL_METRICS"),
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
        std::thread::sleep(Duration::from_millis(20));
    }
    log::info!(
        "popup backend=spell/skia-software; worker pid={}",
        worker.child.as_ref().map_or(0, Child::id)
    );
    Ok(worker)
}
