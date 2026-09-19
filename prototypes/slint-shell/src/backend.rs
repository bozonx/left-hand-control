use std::{
    path::PathBuf,
    process::Child,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};

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
    child: Child,
    socket: PathBuf,
}

impl Worker {
    pub fn send(
        &self,
        command: String,
        source: &str,
        start: Instant,
        token: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::ipc::send(&self.socket, command, source, start, token)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.send("quit".into(), "ipc", Instant::now(), None);
        for _ in 0..100 {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

pub fn start() -> Result<Option<Worker>, Box<dyn std::error::Error>> {
    let mode = std::env::var("SLINT_SHELL_POPUPS").unwrap_or("winit".into());
    if mode == "winit" {
        return Ok(None);
    }
    if mode != "auto" && mode != "spell" {
        return Err("SLINT_SHELL_POPUPS must be winit, auto or spell".into());
    }
    let available = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        layer_shell_available()?
    } else {
        false
    };
    if !available {
        if mode == "spell" {
            return Err("compositor does not advertise zwlr_layer_shell_v1".into());
        }
        log::info!(
            "popup backend=winit-fallback; layer-shell unavailable; compositor controls centering and activation"
        );
        return Ok(None);
    }
    if !cfg!(feature = "spell") {
        return Err("build with --features spell to use layer-shell".into());
    }
    let socket_name = format!("lhc-slint-spell-{}.sock", std::process::id());
    let socket =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR missing")?)
            .join(&socket_name);
    let metrics = std::env::var("SLINT_SHELL_METRICS").unwrap_or("slint-shell.csv".into());
    let child = std::process::Command::new(std::env::current_exe()?)
        .arg("--spell-worker")
        .stdin(std::process::Stdio::piped())
        .env("SLINT_SHELL_SOCKET", socket_name)
        .env("SLINT_SHELL_METRICS", format!("{metrics}.spell.csv"))
        .env_remove("XDG_ACTIVATION_TOKEN")
        .spawn()?;
    let mut worker = Worker { child, socket };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = worker.child.try_wait()? {
            return Err(format!("Spell worker exited: {status}").into());
        }
        if worker.socket.exists()
            && worker
                .send("ping".into(), "ipc", Instant::now(), None)
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
        worker.child.id()
    );
    Ok(Some(worker))
}
