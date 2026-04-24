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

trait EventSink {
    fn emit(&mut self, events: &[InputEvent]) -> Result<(), String>;
}

trait SideEffects {
    fn sleep(&mut self, duration: Duration);
    fn type_text(&mut self, text: &str);
    fn run_system(&mut self, action: &SysAction);
}

trait LoopDriver {
    fn now(&mut self) -> Instant;
    fn wait(&mut self, timeout: Duration) -> Result<bool, String>;
    fn fetch_key_events(&mut self) -> Result<Vec<(Key, bool)>, String>;
}

struct VirtualEventSink<'a> {
    virt: &'a mut VirtualDevice,
}

impl EventSink for VirtualEventSink<'_> {
    fn emit(&mut self, events: &[InputEvent]) -> Result<(), String> {
        self.virt
            .emit(events)
            .map_err(|e| format!("uinput emit: {e}"))
    }
}

struct RuntimeSideEffects<'a> {
    portal: &'a Arc<Mutex<Option<Portal>>>,
}

impl SideEffects for RuntimeSideEffects<'_> {
    fn sleep(&mut self, duration: Duration) {
        if !duration.is_zero() {
            thread::sleep(duration);
        }
    }

    fn type_text(&mut self, text: &str) {
        match self.portal.lock() {
            Ok(slot) => match slot.as_ref() {
                Some(p) => p.type_text(text),
                None => eprintln!("[mapper] literal {:?} dropped (portal unavailable)", text),
            },
            Err(_) => eprintln!(
                "[mapper] literal {:?} dropped (portal state poisoned)",
                text
            ),
        }
    }

    fn run_system(&mut self, action: &SysAction) {
        run_sys_action(action);
    }
}

struct DeviceLoopDriver {
    device: Device,
}

impl DeviceLoopDriver {
    fn new(device: Device) -> Result<Self, String> {
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
        Ok(Self { device })
    }
}

impl LoopDriver for DeviceLoopDriver {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn wait(&mut self, timeout: Duration) -> Result<bool, String> {
        use nix::poll::{poll, PollFd, PollFlags, PollTimeout};

        let timeout_ms: u16 = timeout
            .as_millis()
            .min(u16::MAX as u128)
            .try_into()
            .unwrap_or(u16::MAX);
        let borrowed = unsafe { BorrowedFd::borrow_raw(self.device.as_raw_fd()) };
        let mut pfds = [PollFd::new(borrowed, PollFlags::POLLIN)];
        match poll(&mut pfds, PollTimeout::from(timeout_ms)) {
            Ok(_) => Ok(pfds[0]
                .revents()
                .map(|r| r.contains(PollFlags::POLLIN))
                .unwrap_or(false)),
            Err(nix::errno::Errno::EINTR) => Ok(false),
            Err(e) => Err(format!("poll: {e}")),
        }
    }

    fn fetch_key_events(&mut self) -> Result<Vec<(Key, bool)>, String> {
        match self.device.fetch_events() {
            Ok(iter) => {
                let mut out = Vec::new();
                for ev in iter {
                    if ev.event_type() != EventType::KEY {
                        continue;
                    }
                    let value = ev.value();
                    if value == 2 {
                        continue;
                    }
                    out.push((Key::new(ev.code()), value == 1));
                }
                Ok(out)
            }
            Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => Ok(Vec::new()),
            Err(e) => Err(format!("fetch_events: {e}")),
        }
    }
}

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
    let driver = DeviceLoopDriver::new(device)?;
    run_loop(driver, virt, cfg, stop)
}

fn run_loop<D: LoopDriver>(
    mut driver: D,
    mut virt: VirtualDevice,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
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

    while !stop.load(Ordering::SeqCst) {
        process_iteration(&mut driver, &mut engine, &mut virt, &portal, &mut out_buf)?;
    }

    engine.shutdown(&mut out_buf);
    flush_out(&mut virt, &portal, &mut out_buf)?;
    Ok(())
}

fn process_iteration<D: LoopDriver>(
    driver: &mut D,
    engine: &mut Engine,
    virt: &mut VirtualDevice,
    portal: &Arc<Mutex<Option<Portal>>>,
    out_buf: &mut Vec<Out>,
) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects { portal };
    process_iteration_with(driver, engine, &mut sink, &mut effects, out_buf)
}

fn process_iteration_with<D: LoopDriver, S: EventSink, E: SideEffects>(
    driver: &mut D,
    engine: &mut Engine,
    sink: &mut S,
    effects: &mut E,
    out_buf: &mut Vec<Out>,
) -> Result<(), String> {
    const MAX_WAIT: Duration = Duration::from_millis(100);

    let now = driver.now();
    let wait = engine
        .next_deadline(now)
        .map(|d| d.min(MAX_WAIT))
        .unwrap_or(MAX_WAIT);

    if driver.wait(wait)? {
        for (key, down) in driver.fetch_key_events()? {
            engine.handle(key, down, driver.now(), out_buf);
        }
    }

    engine.tick(driver.now(), out_buf);
    flush_out_with(sink, effects, out_buf)?;
    Ok(())
}

fn emit_stroke_tap<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    ks: &super::action::Keystroke,
    mod_delay: Duration,
) -> Result<(), String> {
    if !ks.mods.is_empty() {
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in &ks.mods {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 1));
        }
        sink.emit(&ev)?;
        effects.sleep(mod_delay);
    }

    let down_up = [
        InputEvent::new(EventType::KEY, ks.key.code(), 1),
        InputEvent::new(EventType::KEY, ks.key.code(), 0),
    ];
    sink.emit(&down_up)?;

    if !ks.mods.is_empty() {
        effects.sleep(mod_delay);
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in ks.mods.iter().rev() {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 0));
        }
        sink.emit(&ev)?;
    }

    Ok(())
}

fn emit_chord_press<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    ks: &super::action::Keystroke,
    mod_delay: Duration,
) -> Result<(), String> {
    if !ks.mods.is_empty() {
        let mut ev = Vec::with_capacity(ks.mods.len());
        for m in &ks.mods {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 1));
        }
        sink.emit(&ev)?;
        effects.sleep(mod_delay);
    }
    let down = [InputEvent::new(EventType::KEY, ks.key.code(), 1)];
    sink.emit(&down)?;
    Ok(())
}

fn emit_chord_release<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    key: Key,
    mods: &[Key],
    mod_delay: Duration,
) -> Result<(), String> {
    let up = [InputEvent::new(EventType::KEY, key.code(), 0)];
    sink.emit(&up)?;

    if !mods.is_empty() {
        effects.sleep(mod_delay);
        let mut ev = Vec::with_capacity(mods.len());
        for m in mods.iter().rev() {
            ev.push(InputEvent::new(EventType::KEY, m.code(), 0));
        }
        sink.emit(&ev)?;
    }
    Ok(())
}

fn flush_out(
    virt: &mut VirtualDevice,
    portal: &Arc<Mutex<Option<Portal>>>,
    buf: &mut Vec<Out>,
) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects { portal };
    flush_out_with(&mut sink, &mut effects, buf)
}

fn flush_out_with<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    buf: &mut Vec<Out>,
) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }
    // We emit in chunks: most Out variants go into a single atomic batch, but
    // RunMacro needs to sleep between steps so we flush what we've got, run
    // the macro with blocking sleeps, and continue with a fresh batch.
    let mut events: Vec<InputEvent> = Vec::with_capacity(buf.len() * 3);

    fn flush_events<S: EventSink>(
        sink: &mut S,
        events: &mut Vec<InputEvent>,
    ) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        sink.emit(events)?;
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
                flush_events(sink, &mut events)?;
                emit_stroke_tap(sink, effects, &ks, mod_delay)?;
            }
            Out::ChordPress { ks, mod_delay } => {
                flush_events(sink, &mut events)?;
                emit_chord_press(sink, effects, &ks, mod_delay)?;
            }
            Out::ChordRelease {
                key,
                mods,
                mod_delay,
            } => {
                flush_events(sink, &mut events)?;
                emit_chord_release(sink, effects, key, &mods, mod_delay)?;
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
                flush_events(sink, &mut events)?;
                run_macro(sink, effects, &steps, step_pause, mod_delay)?;
            }
            Out::RunSystem(action) => {
                flush_events(sink, &mut events)?;
                effects.run_system(&action);
            }
            Out::Literal(text) => {
                flush_events(sink, &mut events)?;
                effects.type_text(&text);
            }
        }
    }
    flush_events(sink, &mut events)?;
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

fn run_macro<S: EventSink, E: SideEffects>(
    sink: &mut S,
    effects: &mut E,
    steps: &[MacroStepItem],
    step_pause: Duration,
    mod_delay: Duration,
) -> Result<(), String> {
    for (i, step) in steps.iter().enumerate() {
        if i > 0 && !step_pause.is_zero() {
            effects.sleep(step_pause);
        }

        let ks = match step {
            MacroStepItem::Stroke(ks) => ks,
            MacroStepItem::System(action) => {
                effects.run_system(action);
                continue;
            }
            MacroStepItem::Literal(text) => {
                effects.type_text(text);
                continue;
            }
        };

        emit_stroke_tap(sink, effects, ks, mod_delay)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{flush_out_with, process_iteration_with, EventSink, LoopDriver, SideEffects};
    use crate::mapper::action::{Keystroke, MacroStepItem};
    use crate::mapper::config::{ActionSpec, AppConfig, LayerKeymap, Rule, Settings};
    use crate::mapper::engine::{Engine, Out};
    use crate::mapper::system::{DbusArg, DbusCall, SysAction, SysCommand};
    use evdev::{EventType, InputEvent, Key};
    use std::collections::{HashMap, VecDeque};
    use std::time::Duration;
    use std::time::Instant;

    #[derive(Default)]
    struct FakeSink {
        batches: Vec<Vec<(u16, i32)>>,
    }

    impl EventSink for FakeSink {
        fn emit(&mut self, events: &[InputEvent]) -> Result<(), String> {
            self.batches.push(
                events
                    .iter()
                    .map(|ev| {
                        assert_eq!(ev.event_type(), EventType::KEY);
                        (ev.code(), ev.value())
                    })
                    .collect(),
            );
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeEffects {
        sleeps: Vec<Duration>,
        texts: Vec<String>,
        systems: Vec<String>,
    }

    impl SideEffects for FakeEffects {
        fn sleep(&mut self, duration: Duration) {
            if !duration.is_zero() {
                self.sleeps.push(duration);
            }
        }

        fn type_text(&mut self, text: &str) {
            self.texts.push(text.to_string());
        }

        fn run_system(&mut self, action: &SysAction) {
            let label = match action {
                SysAction::Spawn(SysCommand { program, args }) => {
                    format!("spawn:{program}:{args:?}")
                }
                SysAction::Dbus(DbusCall {
                    destination,
                    method,
                    args,
                    ..
                }) => format!("dbus:{destination}:{method}:{}", args.len()),
            };
            self.systems.push(label);
        }
    }

    fn key_events(batch: &[(u16, i32)]) -> Vec<(u16, i32)> {
        batch.to_vec()
    }

    struct FakeDriver {
        now_values: VecDeque<Instant>,
        waits: VecDeque<Result<bool, String>>,
        fetches: VecDeque<Result<Vec<(Key, bool)>, String>>,
        seen_waits: Vec<Duration>,
    }

    impl FakeDriver {
        fn new(
            now_values: Vec<Instant>,
            waits: Vec<Result<bool, String>>,
            fetches: Vec<Result<Vec<(Key, bool)>, String>>,
        ) -> Self {
            Self {
                now_values: now_values.into(),
                waits: waits.into(),
                fetches: fetches.into(),
                seen_waits: Vec::new(),
            }
        }
    }

    impl LoopDriver for FakeDriver {
        fn now(&mut self) -> Instant {
            self.now_values.pop_front().expect("now value")
        }

        fn wait(&mut self, timeout: Duration) -> Result<bool, String> {
            self.seen_waits.push(timeout);
            self.waits.pop_front().expect("wait result")
        }

        fn fetch_key_events(&mut self) -> Result<Vec<(Key, bool)>, String> {
            self.fetches.pop_front().expect("fetch result")
        }
    }

    fn empty_cfg() -> AppConfig {
        AppConfig {
            rules: Vec::new(),
            layer_keymaps: HashMap::new(),
            macros: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn flush_out_routes_key_batches_macros_literals_and_systems() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut buf = vec![
            Out::KeyRaw {
                key: Key::KEY_A,
                down: true,
            },
            Out::ReleaseMods(vec![Key::KEY_LEFTSHIFT]),
            Out::RunMacro {
                steps: vec![
                    MacroStepItem::Stroke(Keystroke {
                        mods: vec![Key::KEY_LEFTCTRL],
                        key: Key::KEY_C,
                    }),
                    MacroStepItem::Literal("x".into()),
                    MacroStepItem::System(SysAction::Spawn(SysCommand {
                        program: "spectacle".into(),
                        args: vec!["-r".into()],
                    })),
                ],
                step_pause: Duration::from_millis(5),
                mod_delay: Duration::from_millis(2),
            },
            Out::Literal("tail".into()),
            Out::RunSystem(SysAction::Dbus(DbusCall {
                destination: "org.test".into(),
                path: "/org/test".into(),
                interface: Some("org.test.Interface".into()),
                method: "Fire".into(),
                args: vec![DbusArg::Bool(true)],
            })),
        ];

        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");

        assert!(buf.is_empty());
        assert_eq!(sink.batches.len(), 4);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(Key::KEY_A.code(), 1), (Key::KEY_LEFTSHIFT.code(), 0)]
        );
        assert_eq!(
            key_events(&sink.batches[1]),
            vec![(Key::KEY_LEFTCTRL.code(), 1)]
        );
        assert_eq!(
            key_events(&sink.batches[2]),
            vec![(Key::KEY_C.code(), 1), (Key::KEY_C.code(), 0)]
        );
        assert_eq!(
            key_events(&sink.batches[3]),
            vec![(Key::KEY_LEFTCTRL.code(), 0)]
        );
        assert_eq!(
            effects.sleeps,
            vec![
                Duration::from_millis(2),
                Duration::from_millis(2),
                Duration::from_millis(5),
                Duration::from_millis(5),
            ]
        );
        assert_eq!(effects.texts, vec!["x".to_string(), "tail".to_string()]);
        assert_eq!(
            effects.systems,
            vec![
                "spawn:spectacle:[\"-r\"]".to_string(),
                "dbus:org.test:Fire:1".to_string()
            ]
        );
    }

    #[test]
    fn flush_out_preserves_chord_press_and_release_order() {
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut buf = vec![
            Out::ChordPress {
                ks: Keystroke {
                    mods: vec![Key::KEY_LEFTALT],
                    key: Key::KEY_TAB,
                },
                mod_delay: Duration::from_millis(3),
            },
            Out::ChordRelease {
                key: Key::KEY_TAB,
                mods: vec![Key::KEY_LEFTALT],
                mod_delay: Duration::from_millis(3),
            },
        ];

        flush_out_with(&mut sink, &mut effects, &mut buf).expect("flush");

        assert_eq!(sink.batches.len(), 4);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(Key::KEY_LEFTALT.code(), 1)]
        );
        assert_eq!(key_events(&sink.batches[1]), vec![(Key::KEY_TAB.code(), 1)]);
        assert_eq!(key_events(&sink.batches[2]), vec![(Key::KEY_TAB.code(), 0)]);
        assert_eq!(
            key_events(&sink.batches[3]),
            vec![(Key::KEY_LEFTALT.code(), 0)]
        );
        assert_eq!(
            effects.sleeps,
            vec![Duration::from_millis(3), Duration::from_millis(3)]
        );
    }

    #[test]
    fn process_iteration_filters_repeats_at_driver_boundary_and_flushes_press_release() {
        let start = Instant::now();
        let mut driver = FakeDriver::new(
            vec![start, start, start, start],
            vec![Ok(true)],
            vec![Ok(vec![(Key::KEY_A, true), (Key::KEY_A, false)])],
        );
        let mut engine = Engine::new(&empty_cfg());
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();
        let mut out_buf = Vec::new();

        process_iteration_with(
            &mut driver,
            &mut engine,
            &mut sink,
            &mut effects,
            &mut out_buf,
        )
        .expect("iteration");

        assert_eq!(driver.seen_waits, vec![Duration::from_millis(100)]);
        assert_eq!(sink.batches.len(), 1);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(Key::KEY_A.code(), 1), (Key::KEY_A.code(), 0)]
        );
        assert!(effects.texts.is_empty());
        assert!(out_buf.is_empty());
    }

    #[test]
    fn process_iteration_uses_tick_to_commit_hold_without_input_ready() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            id: "r_alt".into(),
            key: "AltLeft".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("Escape".into()),
            hold_action: ActionSpec::Action("ControlLeft".into()),
            hold_timeout_ms: Some(10),
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let mut engine = Engine::new(&cfg);
        let mut pending = Vec::new();
        let start = Instant::now();
        engine.handle(Key::KEY_LEFTALT, true, start, &mut pending);

        let mut driver = FakeDriver::new(
            vec![start, start + Duration::from_millis(20)],
            vec![Ok(false)],
            Vec::new(),
        );
        let mut sink = FakeSink::default();
        let mut effects = FakeEffects::default();

        process_iteration_with(
            &mut driver,
            &mut engine,
            &mut sink,
            &mut effects,
            &mut pending,
        )
        .expect("iteration");

        assert_eq!(driver.seen_waits, vec![Duration::from_millis(10)]);
        assert_eq!(sink.batches.len(), 1);
        assert_eq!(
            key_events(&sink.batches[0]),
            vec![(Key::KEY_LEFTCTRL.code(), 1)]
        );
    }
}
