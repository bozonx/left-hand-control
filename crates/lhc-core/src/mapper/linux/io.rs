use super::super::engine::{Engine, Out};
pub(super) use super::super::output::{
    EventSink, LoopDriver, OutputHold, SideEffects, flush_out_with, process_iteration_with,
};
use super::super::portal;
use super::super::system::{DbusArg, DbusCall, SysAction, SysCommand};
use evdev::uinput::VirtualDevice;
use evdev::{Device, EventType, InputEvent, KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::sync::OnceLock;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

nix::ioctl_write_ptr!(set_event_clock, b'E', 0xa0, libc::c_int);

fn event_clock_now() -> Result<SystemTime, String> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is writable and CLOCK_MONOTONIC is a valid clock id.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(format!(
            "clock_gettime: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(SystemTime::UNIX_EPOCH + Duration::new(time.tv_sec as u64, time.tv_nsec as u32))
}

pub(super) struct VirtualEventSink<'a> {
    virt: &'a mut VirtualDevice,
}

impl EventSink for VirtualEventSink<'_> {
    fn emit(&mut self, events: &[(KeyCode, bool)]) -> Result<(), String> {
        let events: Vec<InputEvent> = events
            .iter()
            .map(|&(key, down)| *KeyEvent::new(key, i32::from(down)))
            .collect();
        self.virt
            .emit(&events)
            .map_err(|e| format!("uinput emit: {e}"))
    }
}

pub(super) struct RuntimeSideEffects;

impl SideEffects for RuntimeSideEffects {
    fn held_layer_changed(&mut self, state: super::super::layer_state::HeldLayerState) {
        super::super::layer_state::publish(state);
    }
    fn sleep(&mut self, duration: Duration) {
        if !duration.is_zero() {
            thread::sleep(duration);
        }
    }

    fn type_text(&mut self, text: &str) {
        // portal::type_text can block for seconds on its first call (libei
        // handshake / consent dialog) — never run it on the mapper thread
        // while the keyboard is grabbed.
        match super::super::text_job::TextRequest::enqueue(text) {
            Ok(request) => {
                let job = SideEffectJob::Text(text.to_owned(), request.clone());
                if !enqueue_side_effect(job) {
                    request.complete(true);
                    portal::report(crate::events::TextInputFailure::BackendUnavailable(
                        "text input queue is unavailable or full".into(),
                    ));
                }
            }
            Err(error) => portal::report(error),
        }
    }

    fn text_pending(&self) -> bool {
        super::super::text_job::pending()
    }

    fn text_failed(&mut self) -> bool {
        super::super::text_job::take_failure()
    }

    fn run_system(&mut self, action: &SysAction) {
        run_sys_action(action);
    }

    fn run_command(&mut self, command: &SysCommand) {
        super::super::commands::spawn(command.clone());
    }
}

enum SideEffectJob {
    Text(String, super::super::text_job::TextRequest),
}

fn side_effect_tx() -> &'static mpsc::SyncSender<SideEffectJob> {
    static TX: OnceLock<mpsc::SyncSender<SideEffectJob>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(32);
        match thread::Builder::new()
            .name("lhc-side-effects".into())
            .spawn(move || run_side_effect_worker(rx))
        {
            Ok(_) => {}
            Err(e) => log::debug!("[mapper] could not spawn side-effect worker: {e}"),
        }
        tx
    })
}

fn enqueue_side_effect(job: SideEffectJob) -> bool {
    if let Err(e) = side_effect_tx().try_send(job) {
        log::warn!("[mapper] side-effect queue unavailable: {e}");
        return false;
    }
    true
}

fn run_side_effect_worker(rx: mpsc::Receiver<SideEffectJob>) {
    for job in rx {
        match job {
            SideEffectJob::Text(text, request) => {
                let started = Instant::now();
                let result = portal::type_text(&text, request.clone());
                log::debug!("[text] input took {} ms", started.elapsed().as_millis());
                request.complete(result.is_err());
                if let Err(error) = result {
                    portal::report(error);
                }
            }
        }
    }
}

pub(super) struct MultiDeviceLoopDriver {
    devices: Vec<Device>,
    hotkeys: PopupHotkeys,
    /// `LHC_HOTKEYS=off` disables the popup hotkeys; read once per start
    /// rather than on every key event.
    hotkeys_enabled: bool,
    keys: DeviceKeys,
    last_event: Instant,
    /// Read end of the control wake pipe. The `Handle` writes a byte after
    /// queuing a control message so the poll below returns immediately
    /// instead of sleeping out its full timeout.
    wake_rx: Option<std::os::fd::OwnedFd>,
}

impl MultiDeviceLoopDriver {
    pub(super) fn new(
        devices: Vec<Device>,
        wake_rx: Option<std::os::fd::OwnedFd>,
    ) -> Result<Self, String> {
        for device in &devices {
            let fd = device.as_raw_fd();
            // All sources must use one clock; wall-clock adjustments must not
            // change tap/hold duration or reorder keyboard and mouse events.
            // SAFETY: the device owns this fd; the ioctl reads a valid clock id.
            unsafe { set_event_clock(fd, &libc::CLOCK_MONOTONIC) }
                .map_err(|e| format!("set input event clock: {e}"))?;
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
        Ok(Self {
            devices,
            hotkeys: PopupHotkeys::default(),
            hotkeys_enabled: std::env::var("LHC_HOTKEYS").as_deref() != Ok("off"),
            wake_rx,
            keys: DeviceKeys::default(),
            last_event: Instant::now(),
        })
    }

    fn drain_wake_pipe(&self) {
        let Some(fd) = self.wake_rx.as_ref() else {
            return;
        };
        let mut buf = [0u8; 64];
        loop {
            let n = unsafe {
                libc::read(
                    fd.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n <= 0 {
                break;
            }
        }
    }
}

impl LoopDriver for MultiDeviceLoopDriver {
    fn app_hotkey(&mut self, key: KeyCode, down: bool) -> Option<Option<&'static str>> {
        self.hotkeys.event(
            key,
            down,
            self.hotkeys_enabled && !crate::runtime_state::game_mode_active(),
        )
    }

    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn wait(&mut self, timeout: Duration) -> Result<bool, String> {
        use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

        // Round *up* to whole milliseconds. `poll` takes millisecond
        // granularity; truncating a sub-millisecond remainder to 0 makes
        // `poll` return instantly while the deadline it was meant to wait
        // for has not elapsed, so `tick` does nothing and the loop spins.
        // A genuine zero timeout (deadline already passed) stays 0.
        let timeout_ms: u16 = timeout.as_nanos().div_ceil(1_000_000).min(u16::MAX as u128) as u16;
        let mut pfds: Vec<PollFd> = self
            .devices
            .iter()
            .map(|d| {
                let borrowed = unsafe { BorrowedFd::borrow_raw(d.as_raw_fd()) };
                PollFd::new(borrowed, PollFlags::POLLIN)
            })
            .collect();
        if let Some(wake) = self.wake_rx.as_ref() {
            let borrowed = unsafe { BorrowedFd::borrow_raw(wake.as_raw_fd()) };
            pfds.push(PollFd::new(borrowed, PollFlags::POLLIN));
        }
        match poll(&mut pfds, PollTimeout::from(timeout_ms)) {
            Ok(_) => {
                // POLLERR / POLLHUP fire when the device disappears
                // (keyboard unplugged). They must count as "ready" so the
                // subsequent read surfaces the error and the loop exits —
                // checking POLLIN alone makes poll() return instantly
                // forever without ever reading, i.e. a silent busy-loop.
                let ready_flags = PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP;
                let device_ready = pfds[..self.devices.len()].iter().any(|p| {
                    p.revents()
                        .map(|r| r.intersects(ready_flags))
                        .unwrap_or(false)
                });
                // A wake-pipe byte just means "return to the loop so the
                // control channel gets drained" — it is not device input.
                self.drain_wake_pipe();
                Ok(device_ready)
            }
            Err(nix::errno::Errno::EINTR) => Ok(false),
            Err(e) => Err(format!("poll: {e}")),
        }
    }

    fn fetch_key_events(&mut self) -> Result<Vec<(KeyCode, bool)>, String> {
        self.fetch_timed_key_events().map(|events| {
            events
                .into_iter()
                .map(|(key, down, _)| (key, down))
                .collect()
        })
    }

    fn fetch_timed_key_events(&mut self) -> Result<Vec<(KeyCode, bool, Instant)>, String> {
        let mut events = Vec::new();
        for (source, device) in self.devices.iter_mut().enumerate() {
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
                        events.push((ev.timestamp(), source, KeyCode::new(ev.code()), value == 1));
                    }
                }
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {}
                Err(e) if e.raw_os_error() == Some(libc::ENODEV) => {
                    return Err(super::DEVICE_DISCONNECTED.into());
                }
                Err(e) => return Err(format!("fetch_events: {e}")),
            }
        }
        Ok(self.keys.merge(
            &mut events,
            event_clock_now()?,
            Instant::now(),
            &mut self.last_event,
        ))
    }
}

/// Consume both edges of a popup function key, including when game mode changes
/// while it is held. Modifiers still pass through with balanced releases.
#[derive(Default)]
struct PopupHotkeys {
    held: HashSet<KeyCode>,
    consumed: HashSet<KeyCode>,
}

impl PopupHotkeys {
    fn event(&mut self, key: KeyCode, down: bool, enabled: bool) -> Option<Option<&'static str>> {
        if !down {
            self.held.remove(&key);
            return self.consumed.remove(&key).then_some(None);
        }
        let fresh = self.held.insert(key);
        if self.consumed.contains(&key) {
            return Some(None);
        }
        let ctrl = self.held.contains(&KeyCode::KEY_LEFTCTRL)
            || self.held.contains(&KeyCode::KEY_RIGHTCTRL);
        let alt =
            self.held.contains(&KeyCode::KEY_LEFTALT) || self.held.contains(&KeyCode::KEY_RIGHTALT);
        if !enabled || !fresh || !ctrl || !alt {
            return None;
        }
        let action = match key {
            KeyCode::KEY_F11 => "app:showEmojiMenuLast",
            KeyCode::KEY_F12 => "app:showQuickMenuLast",
            _ => return None,
        };
        self.consumed.insert(key);
        Some(Some(action))
    }
}

/// Merge equal key codes from independent devices without releasing another
/// device's held key. Ordering is established before ownership is updated.
#[derive(Default)]
struct DeviceKeys {
    held: HashSet<(usize, KeyCode)>,
    owners: HashMap<KeyCode, usize>,
}

impl DeviceKeys {
    fn merge(
        &mut self,
        events: &mut [(SystemTime, usize, KeyCode, bool)],
        wall: SystemTime,
        now: Instant,
        last: &mut Instant,
    ) -> Vec<(KeyCode, bool, Instant)> {
        events.sort_by_key(|(at, ..)| *at);
        let mut out = Vec::new();
        for &(at, source, key, down) in events.iter() {
            let age = wall.duration_since(at).unwrap_or_default();
            let at = now.checked_sub(age).unwrap_or(*last).max(*last).min(now);
            *last = at;
            if self.transition(source, key, down) {
                out.push((key, down, at));
            }
        }
        out
    }

    fn transition(&mut self, source: usize, key: KeyCode, down: bool) -> bool {
        if down {
            if !self.held.insert((source, key)) {
                return false;
            }
            let owners = self.owners.entry(key).or_default();
            *owners += 1;
            *owners == 1
        } else {
            if self.held.remove(&(source, key)) {
                let owners = self.owners.entry(key).or_default();
                *owners = owners.saturating_sub(1);
            }
            self.owners.get(&key).copied().unwrap_or(0) == 0
        }
    }
}

pub(super) fn process_iteration<D: LoopDriver>(
    driver: &mut D,
    engine: &mut Engine,
    hold: &mut OutputHold,
    virt: &mut VirtualDevice,
    out_buf: &mut Vec<Out>,
) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects;
    process_iteration_with(driver, engine, hold, &mut sink, &mut effects, out_buf)
}

pub(super) fn flush_out(virt: &mut VirtualDevice, buf: &mut Vec<Out>) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects;
    flush_out_with(&mut sink, &mut effects, buf)
}

/// Dispatch a resolved system action on the current OS/DE.
fn run_sys_action(action: &SysAction) {
    match action {
        SysAction::Spawn(cmd) => spawn_system(cmd),
        SysAction::Dbus(call) => call_dbus(call),
        SysAction::AppEvent(event) => {
            super::super::emit_app_event(event);
        }
    }
}

fn spawn_system(cmd: &SysCommand) {
    let mut c = std::process::Command::new(&cmd.program);
    c.args(&cmd.args);
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match c.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            log::debug!("[mapper] spawned side-effect pid={pid}");
            // Detach the wait so a hung child cannot stall the side-effect worker.
            std::thread::spawn(move || match child.wait() {
                Ok(status) if status.success() => {
                    log::debug!("[mapper] side-effect pid={pid} exited: {status}");
                }
                Ok(status) => {
                    log::debug!("[mapper] side-effect pid={pid} failed: {status}");
                }
                Err(e) => {
                    log::debug!("[mapper] side-effect pid={pid} wait failed: {e}");
                }
            });
        }
        Err(e) => {
            log::debug!("[mapper] spawn system {:?} failed: {}", cmd.program, e);
            report_action_failure(format!("Cannot start {}: {e}", cmd.program));
        }
    }
}

/// Enqueue a DBus call on the dedicated `lhc-dbus` worker. Calls stay
/// ordered among themselves (macros rely on that) but never block the
/// side-effect worker, so a hung DBus service cannot stall text
/// injection or shell commands queued after it.
fn call_dbus(call: &DbusCall) {
    if dbus_tx().try_send(call.clone()).is_err() {
        report_action_failure(format!(
            "System action queue unavailable or full: {}.{}",
            call.destination, call.method
        ));
        log::debug!(
            "[mapper] dbus {}.{} dropped (worker unavailable)",
            call.destination,
            call.method
        );
    }
}

fn dbus_tx() -> &'static mpsc::SyncSender<DbusCall> {
    static TX: OnceLock<mpsc::SyncSender<DbusCall>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(32);
        match thread::Builder::new()
            .name("lhc-dbus".into())
            .spawn(move || run_dbus_worker(rx))
        {
            Ok(_) => {}
            Err(e) => log::debug!("[mapper] could not spawn dbus worker: {e}"),
        }
        tx
    })
}

fn run_dbus_worker(rx: mpsc::Receiver<DbusCall>) {
    // One long-lived session connection instead of a connection per call.
    let mut conn: Option<zbus::blocking::Connection> = None;
    for call in rx {
        let c = match &conn {
            Some(c) => c.clone(),
            None => match zbus::blocking::Connection::session() {
                Ok(c) => {
                    conn = Some(c.clone());
                    c
                }
                Err(e) => {
                    log::debug!("[mapper] dbus connect session bus failed: {e}");
                    report_action_failure(format!("Cannot connect to session bus: {e}"));
                    continue;
                }
            },
        };
        let (tx, done) = mpsc::channel();
        let call_clone = call.clone();
        thread::spawn(move || {
            let _ = tx.send(dispatch_dbus_call(&c, &call_clone));
        });

        match done.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {
                if call.destination == "org.kde.keyboard" && call.method == "setLayout" {
                    super::super::refresh_layout();
                }
                log::debug!(
                    "[mapper] dbus {} {} {}.{}",
                    call.destination,
                    call.path,
                    call.interface.as_deref().unwrap_or(""),
                    call.method,
                );
            }
            Ok(Err(e)) => {
                report_action_failure(format!("{}.{} failed: {e}", call.destination, call.method));
                log::debug!(
                    "[mapper] dbus {}.{} failed: {}",
                    call.destination,
                    call.method,
                    e
                );
                // Method-level errors keep the connection; anything else
                // (transport, serialization) forces a reconnect next call.
                if !matches!(e, zbus::Error::MethodError(..)) {
                    conn = None;
                }
            }
            Err(_) => {
                report_action_failure(format!("{}.{} timed out", call.destination, call.method));
                log::debug!(
                    "[mapper] dbus {}.{} timed out after 5s",
                    call.destination,
                    call.method
                );
                conn = None;
            }
        }
    }
}

fn report_action_failure(error: String) {
    crate::events::emit(crate::events::CoreEvent::MapperActionFailed(error));
}

fn dispatch_dbus_call(conn: &zbus::blocking::Connection, call: &DbusCall) -> zbus::Result<()> {
    use zbus::zvariant::StructureBuilder;

    if call.args.is_empty() {
        conn.call_method(
            Some(call.destination.as_str()),
            call.path.as_str(),
            call.interface.as_deref(),
            call.method.as_str(),
            &(),
        )?;
        return Ok(());
    }
    let mut b = StructureBuilder::new();
    for a in &call.args {
        b = match a {
            DbusArg::U32(v) => b.add_field(*v),
            DbusArg::I32(v) => b.add_field(*v),
            DbusArg::Bool(v) => b.add_field(*v),
            DbusArg::Str(s) => b.add_field(s.clone()),
        };
    }
    let body = b.build()?;
    conn.call_method(
        Some(call.destination.as_str()),
        call.path.as_str(),
        call.interface.as_deref(),
        call.method.as_str(),
        &body,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires /dev/uinput and access to synthetic /dev/input devices"]
    fn synthetic_devices_use_monotonic_timestamps_and_preserve_cross_device_order() {
        fn synthetic() -> (VirtualDevice, Device) {
            let mut keys = evdev::AttributeSet::new();
            keys.insert(KeyCode::KEY_A);
            keys.insert(KeyCode::KEY_B);
            let mut virt = VirtualDevice::builder()
                .unwrap()
                .name("LHC isolated mapper test")
                .with_keys(&keys)
                .unwrap()
                .build()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut device = loop {
                let candidate = virt
                    .enumerate_dev_nodes_blocking()
                    .unwrap()
                    .filter_map(Result::ok)
                    .find_map(|path| Device::open(path).ok());
                if let Some(device) = candidate {
                    break device;
                }
                assert!(
                    Instant::now() < deadline,
                    "synthetic device did not become readable"
                );
                thread::sleep(Duration::from_millis(10));
            };
            // Grab our synthetic device before emitting anything: no test
            // keystroke may reach the user's compositor or focused window.
            device.grab().unwrap();
            (virt, device)
        }
        let (mut first, first_input) = synthetic();
        let (mut second, second_input) = synthetic();
        let mut driver = MultiDeviceLoopDriver::new(vec![first_input, second_input], None).unwrap();
        second.emit(&[*KeyEvent::new(KeyCode::KEY_B, 1)]).unwrap();
        thread::sleep(Duration::from_millis(2));
        first.emit(&[*KeyEvent::new(KeyCode::KEY_A, 1)]).unwrap();
        thread::sleep(Duration::from_millis(2));
        second.emit(&[*KeyEvent::new(KeyCode::KEY_B, 0)]).unwrap();
        thread::sleep(Duration::from_millis(2));
        first.emit(&[*KeyEvent::new(KeyCode::KEY_A, 0)]).unwrap();
        assert!(driver.wait(Duration::from_secs(1)).unwrap());
        let events = driver.fetch_timed_key_events().unwrap();
        assert_eq!(
            events
                .iter()
                .map(|(key, down, _)| (*key, *down))
                .collect::<Vec<_>>(),
            vec![
                (KeyCode::KEY_B, true),
                (KeyCode::KEY_A, true),
                (KeyCode::KEY_B, false),
                (KeyCode::KEY_A, false)
            ]
        );
        for pair in events.windows(2) {
            assert!(pair[1].2 > pair[0].2);
        }
    }

    #[test]
    fn events_are_merged_by_capture_time_and_keep_their_original_spacing() {
        let now = Instant::now();
        let wall = SystemTime::now();
        let mut events = vec![
            (wall - Duration::from_millis(5), 0, KeyCode::KEY_A, false),
            (wall - Duration::from_millis(25), 1, KeyCode::BTN_SIDE, true),
            (wall - Duration::from_millis(40), 0, KeyCode::KEY_A, true),
        ];
        let mut last = now - Duration::from_secs(1);
        let events = DeviceKeys::default().merge(&mut events, wall, now, &mut last);
        assert_eq!(
            events,
            vec![
                (KeyCode::KEY_A, true, now - Duration::from_millis(40)),
                (KeyCode::BTN_SIDE, true, now - Duration::from_millis(25)),
                (KeyCode::KEY_A, false, now - Duration::from_millis(5)),
            ]
        );
    }

    #[test]
    fn independent_devices_share_ownership_without_duplicate_transitions() {
        let mut keys = DeviceKeys::default();
        assert!(keys.transition(0, KeyCode::KEY_A, true));
        assert!(!keys.transition(0, KeyCode::KEY_A, true));
        assert!(!keys.transition(1, KeyCode::KEY_A, true));
        assert!(!keys.transition(0, KeyCode::KEY_A, false));
        assert!(!keys.transition(0, KeyCode::KEY_A, false));
        assert!(keys.transition(1, KeyCode::KEY_A, false));
    }

    #[test]
    fn app_actions_bypass_the_text_worker() {
        let received = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = received.clone();
        crate::events::bus().subscribe(move |event| {
            if matches!(event, crate::events::CoreEvent::AppAction(name) if name == "audit-popup-test") {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
        RuntimeSideEffects.run_system(&SysAction::AppEvent("audit-popup-test".into()));
        assert!(received.load(std::sync::atomic::Ordering::SeqCst));
    }
}

#[cfg(test)]
mod popup_hotkey_tests {
    use super::*;

    #[test]
    fn popup_chord_never_emits_function_key_even_if_disabled_before_release() {
        for (key, action) in [
            (KeyCode::KEY_F11, "app:showEmojiMenuLast"),
            (KeyCode::KEY_F12, "app:showQuickMenuLast"),
        ] {
            let mut hotkeys = PopupHotkeys::default();
            assert_eq!(hotkeys.event(KeyCode::KEY_LEFTCTRL, true, true), None);
            assert_eq!(hotkeys.event(KeyCode::KEY_RIGHTALT, true, true), None);
            assert_eq!(hotkeys.event(key, true, true), Some(Some(action)));
            assert_eq!(hotkeys.event(key, true, true), Some(None));
            assert_eq!(hotkeys.event(KeyCode::KEY_LEFTCTRL, false, false), None);
            assert_eq!(hotkeys.event(key, false, false), Some(None));
            assert_eq!(hotkeys.event(key, true, true), None);
        }
    }

    #[test]
    fn disabled_hotkeys_and_plain_function_keys_pass_through() {
        let mut hotkeys = PopupHotkeys::default();
        assert_eq!(hotkeys.event(KeyCode::KEY_F11, true, true), None);
        hotkeys.event(KeyCode::KEY_F11, false, true);
        hotkeys.event(KeyCode::KEY_RIGHTCTRL, true, true);
        hotkeys.event(KeyCode::KEY_LEFTALT, true, true);
        assert_eq!(hotkeys.event(KeyCode::KEY_F11, true, false), None);
        assert_eq!(hotkeys.event(KeyCode::KEY_F11, false, true), None);
    }
}
