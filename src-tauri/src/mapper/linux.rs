// Linux evdev + uinput glue.

#![cfg(target_os = "linux")]

use super::config::AppConfig;
use super::engine::{Engine, Out};
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

    let join = thread::Builder::new()
        .name("lhc-mapper".into())
        .spawn(move || {
            if let Err(e) = run_loop(device, virt, cfg, stop_thread) {
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
        flush_out(&mut virt, &mut out_buf)?;
    }

    // Graceful shutdown: release anything still held.
    engine.shutdown(&mut out_buf);
    flush_out(&mut virt, &mut out_buf)?;
    // Device is dropped here, which releases the grab automatically.
    Ok(())
}

fn flush_out(virt: &mut VirtualDevice, buf: &mut Vec<Out>) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }
    let mut events: Vec<InputEvent> = Vec::with_capacity(buf.len() * 3);
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
        }
    }
    virt.emit(&events).map_err(|e| format!("uinput emit: {e}"))?;
    Ok(())
}
