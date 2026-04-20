// Linux evdev + uinput glue.

#![cfg(target_os = "linux")]

use super::action::MacroStepItem;
use super::config::AppConfig;
use super::engine::{Engine, Out};
use super::system::SysCommand;
use super::vkbd::Vkbd;
use super::KeyboardDevice;
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, BusType, Device, EventType, InputEvent, InputId, Key};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const VIRTUAL_DEVICE_NAME: &str = "LeftHandControl Virtual Keyboard";

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
        self.join.as_ref().map(|j| !j.is_finished()).unwrap_or(false)
    }

    pub fn last_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|g| g.clone())
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
        let Ok(dev) = Device::open(&path) else { continue };
        if !is_keyboard(&dev) {
            continue;
        }
        let name = dev
            .name()
            .unwrap_or("(unknown)")
            .to_string();
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
        Key::KEY_A, Key::KEY_S, Key::KEY_D, Key::KEY_F,
        Key::KEY_SPACE, Key::KEY_ENTER,
    ];
    required.iter().all(|k| keys.contains(*k))
}

pub fn spawn(device_path: String, cfg: AppConfig) -> Result<Handle, String> {
    // Pre-open the physical device to validate before spawning the thread.
    let mut device =
        Device::open(&device_path).map_err(|e| format!("open {device_path}: {e}"))?;
    device
        .grab()
        .map_err(|e| format!("grab {device_path} (need perms? add user to input group): {e}"))?;

    // Virtual device: advertise every key we might emit.
    let mut all_keys = AttributeSet::<Key>::new();
    // Add the typical keyboard range — KEY_ESC (1) .. KEY_MICMUTE (248) covers
    // everything we map. Going wider is cheap; uinput doesn't care.
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

    eprintln!(
        "[mapper] started: device={} rules={} layers={}",
        device_path,
        cfg.rules.len(),
        cfg.layer_keymaps.len()
    );
    for r in &cfg.rules {
        eprintln!(
            "[mapper]   rule key={} layer={:?} tap={:?} holdMs={:?}",
            r.key, r.layer_id, r.tap_action, r.hold_timeout_ms
        );
    }
    for (lid, km) in &cfg.layer_keymaps {
        eprintln!("[mapper]   keymap[{lid}] keys={}", km.keys.len());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let stop_thread = stop.clone();
    let err_thread = error.clone();

    // Start the Wayland virtual-keyboard backend used for typing
    // character literals independently of the system layout. Failure here
    // is non-fatal: the mapper still handles key remaps, layers, macros
    // and system actions — it just logs a warning and refuses to emit
    // literals (instead of emitting layout-dependent garbage).
    let vkbd = match Vkbd::try_start() {
        Ok(v) => {
            eprintln!("[mapper] wayland vkbd backend online");
            Some(v)
        }
        Err(e) => {
            eprintln!("[mapper] wayland vkbd backend unavailable: {e}");
            None
        }
    };

    let join = thread::Builder::new()
        .name("lhc-mapper".into())
        .spawn(move || {
            if let Err(e) = run_loop(device, virt, cfg, stop_thread, vkbd) {
                eprintln!("[mapper] run_loop error: {e}");
                if let Ok(mut slot) = err_thread.lock() {
                    *slot = Some(e);
                }
            }
        })
        .map_err(|e| format!("spawn thread: {e}"))?;

    Ok(Handle {
        stop,
        join: Some(join),
        error,
    })
}

fn run_loop(
    mut device: Device,
    mut virt: VirtualDevice,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
    vkbd: Option<Vkbd>,
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
        flush_out(&mut virt, vkbd.as_ref(), &mut out_buf)?;
    }

    // Graceful shutdown: release anything still held.
    engine.shutdown(&mut out_buf);
    flush_out(&mut virt, vkbd.as_ref(), &mut out_buf)?;
    // Device is dropped here, which releases the grab automatically.
    // `vkbd` dropping joins the Wayland thread and destroys the virtual
    // keyboard cleanly.
    drop(vkbd);
    Ok(())
}

fn flush_out(
    virt: &mut VirtualDevice,
    vkbd: Option<&Vkbd>,
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
                events.push(InputEvent::new(EventType::KEY, key.code(), if down { 1 } else { 0 }));
            }
            Out::Stroke(ks) => {
                for m in &ks.mods {
                    events.push(InputEvent::new(EventType::KEY, m.code(), 1));
                }
                events.push(InputEvent::new(EventType::KEY, ks.key.code(), 1));
                events.push(InputEvent::new(EventType::KEY, ks.key.code(), 0));
                for m in ks.mods.iter().rev() {
                    events.push(InputEvent::new(EventType::KEY, m.code(), 0));
                }
            }
            Out::PressMods(mods) => {
                for m in mods {
                    events.push(InputEvent::new(EventType::KEY, m.code(), 1));
                }
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
                run_macro(virt, vkbd, &steps, step_pause, mod_delay)?;
            }
            Out::RunSystem(cmd) => {
                flush_events(virt, &mut events)?;
                spawn_system(&cmd);
            }
            Out::Literal(ch) => {
                // Flush anything queued through uinput before handing the
                // character off to the Wayland vkbd backend, so the order
                // of emitted events matches the order engine produced them.
                flush_events(virt, &mut events)?;
                match vkbd {
                    Some(v) => {
                        if !v.type_char(ch) {
                            eprintln!(
                                "[mapper] literal {ch:?} not in US symbol table, ignored"
                            );
                        }
                    }
                    None => eprintln!(
                        "[mapper] literal {ch:?} dropped (vkbd backend unavailable)"
                    ),
                }
            }
        }
    }
    flush_events(virt, &mut events)?;
    Ok(())
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
            eprintln!(
                "[mapper] spawn system {:?} failed: {}",
                cmd.program, e
            );
        }
    }
}

fn run_macro(
    virt: &mut VirtualDevice,
    vkbd: Option<&Vkbd>,
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
            MacroStepItem::System(cmd) => {
                spawn_system(cmd);
                continue;
            }
            MacroStepItem::Literal(ch) => {
                // Literals go through the Wayland virtual keyboard so they
                // produce the right character regardless of the active
                // system layout. No modifiers, no mod_delay; we just fire
                // the character and move on to the next macro step.
                if let Some(v) = vkbd {
                    if !v.type_char(*ch) {
                        eprintln!(
                            "[mapper] macro step literal {ch:?} not in US symbol table"
                        );
                    }
                } else {
                    eprintln!(
                        "[mapper] macro step literal {ch:?} dropped (vkbd unavailable)"
                    );
                }
                continue;
            }
        };

        // 1) press modifiers
        if !ks.mods.is_empty() {
            let mut ev = Vec::with_capacity(ks.mods.len());
            for m in &ks.mods {
                ev.push(InputEvent::new(EventType::KEY, m.code(), 1));
            }
            virt.emit(&ev).map_err(|e| format!("uinput emit (macro mods-down): {e}"))?;
            if !mod_delay.is_zero() {
                thread::sleep(mod_delay);
            }
        }

        // 2) press + release main key
        let down_up = [
            InputEvent::new(EventType::KEY, ks.key.code(), 1),
            InputEvent::new(EventType::KEY, ks.key.code(), 0),
        ];
        virt.emit(&down_up)
            .map_err(|e| format!("uinput emit (macro key): {e}"))?;

        // 3) release modifiers
        if !ks.mods.is_empty() {
            if !mod_delay.is_zero() {
                thread::sleep(mod_delay);
            }
            let mut ev = Vec::with_capacity(ks.mods.len());
            for m in ks.mods.iter().rev() {
                ev.push(InputEvent::new(EventType::KEY, m.code(), 0));
            }
            virt.emit(&ev)
                .map_err(|e| format!("uinput emit (macro mods-up): {e}"))?;
        }
    }
    Ok(())
}
