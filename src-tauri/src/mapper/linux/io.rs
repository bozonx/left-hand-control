use super::super::action::{Keystroke, MacroStepItem};
use super::super::engine::{Engine, Out};
use super::super::portal;
use super::super::system::{DbusArg, DbusCall, SysAction, SysCommand};
use evdev::uinput::VirtualDevice;
use evdev::{Device, EventType, InputEvent, Key};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::thread;
use std::time::{Duration, Instant};

pub(super) trait EventSink {
    fn emit(&mut self, events: &[InputEvent]) -> Result<(), String>;
}

pub(super) trait SideEffects {
    fn sleep(&mut self, duration: Duration);
    fn type_text(&mut self, text: &str);
    fn run_system(&mut self, action: &SysAction);
    fn run_command(&mut self, command: &SysCommand);
}

pub(super) trait LoopDriver {
    fn now(&mut self) -> Instant;
    fn wait(&mut self, timeout: Duration) -> Result<bool, String>;
    fn fetch_key_events(&mut self) -> Result<Vec<(Key, bool)>, String>;
}

pub(super) struct VirtualEventSink<'a> {
    virt: &'a mut VirtualDevice,
}

impl EventSink for VirtualEventSink<'_> {
    fn emit(&mut self, events: &[InputEvent]) -> Result<(), String> {
        self.virt
            .emit(events)
            .map_err(|e| format!("uinput emit: {e}"))
    }
}

pub(super) struct RuntimeSideEffects;

impl SideEffects for RuntimeSideEffects {
    fn sleep(&mut self, duration: Duration) {
        if !duration.is_zero() {
            thread::sleep(duration);
        }
    }

    fn type_text(&mut self, text: &str) {
        portal::type_text(text);
    }

    fn run_system(&mut self, action: &SysAction) {
        run_sys_action(action);
    }

    fn run_command(&mut self, command: &SysCommand) {
        run_shell_command(command);
    }
}

pub(super) struct MultiDeviceLoopDriver {
    devices: Vec<Device>,
}

impl MultiDeviceLoopDriver {
    pub(super) fn new(devices: Vec<Device>) -> Result<Self, String> {
        for device in &devices {
            let fd = device.as_raw_fd();
            // SAFETY: `fd` is a valid raw file descriptor obtained from an
            // `evdev::Device` that is still alive; `fcntl` only queries and
            // modifies the open-file description flags, never the device itself.
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                if flags < 0 {
                    return Err("fcntl F_GETFL failed".into());
                }
                if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                    return Err("fcntl F_SETFL O_NONBLOCK failed".into());
                }
            }
        }
        Ok(Self { devices })
    }
}

impl LoopDriver for MultiDeviceLoopDriver {
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
        let mut pfds: Vec<PollFd> = self
            .devices
            .iter()
            .map(|d| {
                let borrowed = unsafe { BorrowedFd::borrow_raw(d.as_raw_fd()) };
                PollFd::new(borrowed, PollFlags::POLLIN)
            })
            .collect();
        match poll(&mut pfds, PollTimeout::from(timeout_ms)) {
            Ok(_) => Ok(pfds.iter().any(|p| {
                p.revents()
                    .map(|r| r.contains(PollFlags::POLLIN))
                    .unwrap_or(false)
            })),
            Err(nix::errno::Errno::EINTR) => Ok(false),
            Err(e) => Err(format!("poll: {e}")),
        }
    }

    fn fetch_key_events(&mut self) -> Result<Vec<(Key, bool)>, String> {
        let mut out = Vec::new();
        for device in &mut self.devices {
            match device.fetch_events() {
                Ok(iter) => {
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
                }
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {}
                Err(e) => return Err(format!("fetch_events: {e}")),
            }
        }
        Ok(out)
    }
}

pub(super) fn process_iteration<D: LoopDriver>(
    driver: &mut D,
    engine: &mut Engine,
    virt: &mut VirtualDevice,
    out_buf: &mut Vec<Out>,
) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects;
    process_iteration_with(driver, engine, &mut sink, &mut effects, out_buf)
}

pub(super) fn process_iteration_with<D: LoopDriver, S: EventSink, E: SideEffects>(
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
    ks: &Keystroke,
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
    ks: &Keystroke,
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

pub(super) fn flush_out(virt: &mut VirtualDevice, buf: &mut Vec<Out>) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects;
    flush_out_with(&mut sink, &mut effects, buf)
}

pub(super) fn flush_out_with<S: EventSink, E: SideEffects>(
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
                // Mouse buttons are not grabbed, so they already reach the OS.
                // Re-emitting them would create duplicates.
                let code = key.code();
                if (272..=281).contains(&code) {
                    continue;
                }
                events.push(InputEvent::new(
                    EventType::KEY,
                    code,
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
            Out::RunCommand(command) => {
                flush_events(sink, &mut events)?;
                effects.run_command(&command);
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

fn run_shell_command(command: &SysCommand) {
    spawn_system(command);
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
            if call.destination == "org.kde.keyboard" && call.method == "setLayout" {
                let _ = crate::layout::refresh_cache();
            }
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
            MacroStepItem::Command(command) => {
                effects.run_command(command);
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
