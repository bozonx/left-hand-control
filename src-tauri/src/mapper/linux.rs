// Linux evdev + uinput glue.

#![cfg(target_os = "linux")]

use super::action::MacroStepItem;
use super::config::AppConfig;
use super::engine::{Engine, Out};
use super::portal::Portal;
use super::system::{DbusArg, DbusCall, SysAction, SysCommand};
use super::KeyboardDevice;
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, BusType, Device, EventType, InputEvent, InputId, Key};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const VIRTUAL_DEVICE_NAME: &str = "LeftHandControl Virtual Keyboard";
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// Handle to a running mapper thread.
pub struct Handle {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}

impl Handle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }

    pub fn is_alive(&self) -> bool {
        self.join
            .as_ref()
            .map(|j| !j.is_finished())
            .unwrap_or(false)
    }

    pub fn last_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|g| g.clone())
    }

    pub fn reap_if_finished(&mut self) -> bool {
        if self.is_alive() {
            return false;
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        true
    }
}

pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    let mut out = Vec::new();
    let dir = std::fs::read_dir("/dev/input").map_err(|e| format!("read /dev/input: {e}"))?;
    let mut paths: Vec<PathBuf> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("event"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();

    for path in paths {
        let Ok(dev) = Device::open(&path) else {
            continue;
        };
        if !is_keyboard(&dev) {
            continue;
        }
        let name = dev.name().unwrap_or("(unknown)").to_string();
        if name == VIRTUAL_DEVICE_NAME {
            continue;
        }
        out.push(KeyboardDevice {
            path: path.to_string_lossy().to_string(),
            name,
        });
    }
    Ok(out)
}

fn is_keyboard(dev: &Device) -> bool {
    let Some(keys) = dev.supported_keys() else {
        return false;
    };
    // Heuristic: considered a keyboard if it reports several letter keys
    // plus space and enter. Excludes mice, power buttons, sleep keys, etc.
    let required = [
        Key::KEY_A,
        Key::KEY_S,
        Key::KEY_D,
        Key::KEY_F,
        Key::KEY_SPACE,
        Key::KEY_ENTER,
    ];
    required.iter().all(|k| keys.contains(*k))
}

pub fn spawn(device_path: String, cfg: AppConfig) -> Result<Handle, String> {
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "[mapper] started: device={} rules={} layers={}",
            device_path,
            cfg.rules.len(),
            cfg.layer_keymaps.len()
        );
        for r in &cfg.rules {
            eprintln!(
                "[mapper]   rule key={} layer={:?} tap={:?} hold={:?} holdMs={:?}",
                r.key, r.layer_id, r.tap_action, r.hold_action, r.hold_timeout_ms
            );
        }
        for (lid, km) in &cfg.layer_keymaps {
            eprintln!("[mapper]   keymap[{lid}] keys={}", km.keys.len());
        }
    }

    let _ = Device::open(&device_path).map_err(|e| format!("open {device_path}: {e}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let stop_thread = stop.clone();
    let err_thread = error.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

    let join = thread::Builder::new()
        .name("lhc-mapper".into())
        .spawn(move || {
            if let Err(e) = run(device_path, cfg, stop_thread, ready_tx) {
                eprintln!("[mapper] run_loop error: {e}");
                if let Ok(mut slot) = err_thread.lock() {
                    *slot = Some(e);
                }
            }
        })
        .map_err(|e| format!("spawn thread: {e}"))?;

    match ready_rx.recv_timeout(START_TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let _ = join.join();
            return Err(e);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            stop.store(true, Ordering::SeqCst);
            let _ = join.join();
            return Err("mapper start timed out".into());
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = join.join();
            return Err("mapper worker exited before reporting readiness".into());
        }
    }

    Ok(Handle {
        stop,
        join: Some(join),
        error,
    })
}

fn run(
    device_path: String,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let mut device = Device::open(&device_path).map_err(|e| format!("open {device_path}: {e}"))?;
    device
        .grab()
        .map_err(|e| format!("grab {device_path} (need perms? add user to input group): {e}"))?;

    let mut all_keys = AttributeSet::<Key>::new();
    for code in 1u16..=248 {
        all_keys.insert(Key::new(code));
    }
    let virt = VirtualDeviceBuilder::new()
        .map_err(|e| format!("uinput builder (need /dev/uinput access?): {e}"))?
        .name(VIRTUAL_DEVICE_NAME)
        .input_id(InputId::new(BusType::BUS_USB, 0x1d6b, 0x0104, 1))
        .with_keys(&all_keys)
        .map_err(|e| format!("uinput with_keys: {e}"))?
        .build()
        .map_err(|e| format!("uinput build: {e}"))?;

    let _ = ready_tx.send(Ok(()));
    run_loop(device, virt, cfg, stop)
}

fn run_loop(
    mut device: Device,
    mut virt: VirtualDevice,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

    // Set O_NONBLOCK so fetch_events() returns EAGAIN when the kernel
    // buffer is empty instead of blocking.
    let fd = device.as_raw_fd();
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return Err("fcntl F_GETFL failed".into());
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err("fcntl F_SETFL O_NONBLOCK failed".into());
        }
    }

    let mut engine = Engine::new(&cfg);
    let mut out_buf: Vec<Out> = Vec::with_capacity(16);
    let portal: Arc<Mutex<Option<Portal>>> = Arc::new(Mutex::new(None));
    let portal_init_slot = portal.clone();

    let _ = thread::Builder::new()
        .name("lhc-portal-init".into())
        .spawn(move || match Portal::try_start() {
            Ok(p) => {
                eprintln!("[mapper] portal backend online");
                if let Ok(mut slot) = portal_init_slot.lock() {
                    *slot = Some(p);
                }
            }
            Err(e) => eprintln!("[mapper] portal backend unavailable: {e}"),
        });

    // Keep the BorrowedFd alive for the whole loop.
    let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };

    // Max sleep so we can notice `stop` being set promptly even without a deadline.
    const MAX_WAIT: Duration = Duration::from_millis(100);

    while !stop.load(Ordering::SeqCst) {
        let now = Instant::now();
        let wait = engine
            .next_deadline(now)
            .map(|d| d.min(MAX_WAIT))
            .unwrap_or(MAX_WAIT);

        let timeout_ms: u16 = wait
            .as_millis()
            .min(u16::MAX as u128)
            .try_into()
            .unwrap_or(u16::MAX);
        let mut pfds = [PollFd::new(borrowed, PollFlags::POLLIN)];
        match poll(&mut pfds, PollTimeout::from(timeout_ms)) {
            Ok(_) => {}
            Err(nix::errno::Errno::EINTR) => continue,
            Err(e) => return Err(format!("poll: {e}")),
        }

        let ready = pfds[0]
            .revents()
            .map(|r| r.contains(PollFlags::POLLIN))
            .unwrap_or(false);

        if ready {
            match device.fetch_events() {
                Ok(iter) => {
                    for ev in iter {
                        if ev.event_type() != EventType::KEY {
                            continue;
                        }
                        // value: 0 = up, 1 = down, 2 = repeat
                        let value = ev.value();
                        if value == 2 {
                            // Drop auto-repeats from the physical device; uinput
                            // consumers can generate their own repeats from our
                            // down events. Simpler + avoids double-firing.
                            continue;
                        }
                        let key = Key::new(ev.code());
                        engine.handle(key, value == 1, Instant::now(), &mut out_buf);
                    }
                }
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {}
                Err(e) => return Err(format!("fetch_events: {e}")),
            }
        }

        engine.tick(Instant::now(), &mut out_buf);
        flush_out(&mut virt, &portal, &mut out_buf)?;
    }

    engine.shutdown(&mut out_buf);
    flush_out(&mut virt, &portal, &mut out_buf)?;
    Ok(())
}

fn emit_stroke_tap(
    virt: &mut VirtualDevice,
    ks: &super::action::Keystroke,
    mod_delay: Duration,
) -> Result<(), String> {
    if !ks.mods.is_empty() {
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in &ks.mods {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 1));
        }
        virt.emit(&ev)
            .map_err(|e| format!("uinput emit (stroke mods-down): {e}"))?;
        if !mod_delay.is_zero() {
            thread::sleep(mod_delay);
        }
    }

    let down_up = [
        InputEvent::new(EventType::KEY, ks.key.code(), 1),
        InputEvent::new(EventType::KEY, ks.key.code(), 0),
    ];
    virt.emit(&down_up)
        .map_err(|e| format!("uinput emit (stroke key): {e}"))?;

    if !ks.mods.is_empty() {
        if !mod_delay.is_zero() {
            thread::sleep(mod_delay);
        }
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in ks.mods.iter().rev() {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 0));
        }
        virt.emit(&ev)
            .map_err(|e| format!("uinput emit (stroke mods-up): {e}"))?;
    }

    Ok(())
}

fn emit_chord_press(
    virt: &mut VirtualDevice,
    ks: &super::action::Keystroke,
    mod_delay: Duration,
) -> Result<(), String> {
    if !ks.mods.is_empty() {
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in &ks.mods {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 1));
        }
        virt.emit(&ev)
            .map_err(|e| format!("uinput emit (chord mods-down): {e}"))?;
        if !mod_delay.is_zero() {
            thread::sleep(mod_delay);
        }
    }
    let down = [InputEvent::new(EventType::KEY, ks.key.code(), 1)];
    virt.emit(&down)
        .map_err(|e| format!("uinput emit (chord key-down): {e}"))?;
    Ok(())
}

fn emit_chord_release(
    virt: &mut VirtualDevice,
    key: Key,
    mods: &[Key],
    mod_delay: Duration,
) -> Result<(), String> {
    let up = [InputEvent::new(EventType::KEY, key.code(), 0)];
    virt.emit(&up)
        .map_err(|e| format!("uinput emit (chord key-up): {e}"))?;

    if !mods.is_empty() {
        if !mod_delay.is_zero() {
            thread::sleep(mod_delay);
        }
        let mut ev = Vec::with_capacity(mods.len());
        for m in mods.iter().rev() {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 0));
        }
        virt.emit(&ev)
            .map_err(|e| format!("uinput emit (chord mods-up): {e}"))?;
    }
    Ok(())
}

fn flush_out(
    virt: &mut VirtualDevice,
    portal: &Arc<Mutex<Option<Portal>>>,
    buf: &mut Vec<Out>,
) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }
    // We emit in chunks: most Out variants go into a single atomic batch, but
    // RunMacro needs to sleep between steps so we flush what we've got, run
    // the macro with blocking sleeps, and continue with a fresh batch.
    let mut events: Vec<InputEvent> = Vec::with_capacity(buf.len() * 3);

    fn flush_events(virt: &mut VirtualDevice, events: &mut Vec<InputEvent>) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        virt.emit(events).map_err(|e| format!("uinput emit: {e}"))?;
        events.clear();
        Ok(())
    }

    for out in buf.drain(..) {
        match out {
            Out::KeyRaw { key, down } => {
                events.push(InputEvent::new(
                    EventType::KEY,
                    key.code(),
                    if down { 1 } else { 0 },
                ));
            }
            Out::Stroke { ks, mod_delay } => {
                flush_events(virt, &mut events)?;
                emit_stroke_tap(virt, &ks, mod_delay)?;
            }
            Out::ChordPress { ks, mod_delay } => {
                flush_events(virt, &mut events)?;
                emit_chord_press(virt, &ks, mod_delay)?;
            }
            Out::ChordRelease {
                key,
                mods,
                mod_delay,
            } => {
                flush_events(virt, &mut events)?;
                emit_chord_release(virt, key, &mods, mod_delay)?;
            }
            Out::ReleaseMods(mods) => {
                for m in mods {
                    events.push(InputEvent::new(EventType::KEY, m.code(), 0));
                }
            }
            Out::RunMacro {
                steps,
                step_pause,
                mod_delay,
            } => {
                // Emit whatever we've accumulated first so it doesn't get
                // intermixed with the macro's timing.
                flush_events(virt, &mut events)?;
                run_macro(virt, portal, &steps, step_pause, mod_delay)?;
            }
            Out::RunSystem(action) => {
                flush_events(virt, &mut events)?;
                run_sys_action(&action);
            }
            Out::Literal(text) => {
                flush_events(virt, &mut events)?;
                match portal.lock() {
                    Ok(slot) => match slot.as_ref() {
                        Some(p) => p.type_text(&text),
                        None => eprintln!("[mapper] literal {:?} dropped (portal unavailable)", text),
                    },
                    Err(_) => eprintln!("[mapper] literal {:?} dropped (portal state poisoned)", text),
                }
            }
        }
    }
    flush_events(virt, &mut events)?;
    Ok(())
}

/// Dispatch a resolved system action on the current OS/DE.
fn run_sys_action(action: &SysAction) {
    match action {
        SysAction::Spawn(cmd) => spawn_system(cmd),
        SysAction::Dbus(call) => call_dbus(call),
    }
}

fn spawn_system(cmd: &SysCommand) {
    let mut c = std::process::Command::new(&cmd.program);
    c.args(&cmd.args);
    // Detach stdio so the child doesn't inherit our FDs in weird ways.
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match c.spawn() {
        Ok(_child) => {
            eprintln!("[mapper] spawned system: {} {:?}", cmd.program, cmd.args);
        }
        Err(e) => {
            eprintln!("[mapper] spawn system {:?} failed: {}", cmd.program, e);
        }
    }
}

/// Lazily-initialised, process-wide session-bus connection. We keep it
/// alive so each `switchDesktopN` is a single roundtrip instead of a
/// `fork+exec` of a helper binary like `qdbus`.
fn session_bus() -> Option<&'static zbus::blocking::Connection> {
    use std::sync::OnceLock;
    static CONN: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
    CONN.get_or_init(|| match zbus::blocking::Connection::session() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("[mapper] dbus session bus unavailable: {e}");
            None
        }
    })
    .as_ref()
}

fn call_dbus(call: &DbusCall) {
    use zbus::zvariant::StructureBuilder;

    let Some(conn) = session_bus() else {
        eprintln!(
            "[mapper] dbus {}.{} skipped (no session bus)",
            call.destination, call.method
        );
        return;
    };

    let result = if call.args.is_empty() {
        conn.call_method(
            Some(call.destination.as_str()),
            call.path.as_str(),
            call.interface.as_deref(),
            call.method.as_str(),
            &(),
        )
    } else {
        let mut b = StructureBuilder::new();
        for a in &call.args {
            b = match a {
                DbusArg::U32(v) => b.add_field(*v),
                DbusArg::I32(v) => b.add_field(*v),
                DbusArg::Bool(v) => b.add_field(*v),
                DbusArg::Str(s) => b.add_field(s.clone()),
            };
        }
        let body = match b.build() {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "[mapper] dbus {}.{}: failed to build body: {}",
                    call.destination, call.method, e
                );
                return;
            }
        };
        conn.call_method(
            Some(call.destination.as_str()),
            call.path.as_str(),
            call.interface.as_deref(),
            call.method.as_str(),
            &body,
        )
    };

    match result {
        Ok(_) => {
            eprintln!(
                "[mapper] dbus {} {} {}.{}",
                call.destination,
                call.path,
                call.interface.as_deref().unwrap_or(""),
                call.method,
            );
        }
        Err(e) => {
            eprintln!(
                "[mapper] dbus {}.{} failed: {}",
                call.destination, call.method, e
            );
        }
    }
}

fn run_macro(
    virt: &mut VirtualDevice,
    portal: &Arc<Mutex<Option<Portal>>>,
    steps: &[MacroStepItem],
    step_pause: Duration,
    mod_delay: Duration,
) -> Result<(), String> {
    for (i, step) in steps.iter().enumerate() {
        if i > 0 && !step_pause.is_zero() {
            thread::sleep(step_pause);
        }

        let ks = match step {
            MacroStepItem::Stroke(ks) => ks,
            MacroStepItem::System(action) => {
                run_sys_action(action);
                continue;
            }
            MacroStepItem::Literal(text) => {
                match portal.lock() {
                    Ok(slot) => match slot.as_ref() {
                        Some(p) => p.type_text(text),
                        None => eprintln!(
                            "[mapper] macro step literal {:?} dropped (portal unavailable)",
                            text
                        ),
                    },
                    Err(_) => eprintln!(
                        "[mapper] macro step literal {:?} dropped (portal state poisoned)",
                        text
                    ),
                }
                continue;
            }
        };

        emit_stroke_tap(virt, ks, mod_delay)?;
    }
    Ok(())
}
