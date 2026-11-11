//! Linux mapper loop I/O: reading grabbed devices, emitting through uinput and
//! running side effects off the input thread.

use super::super::engine::{Engine, EngineOutput};

/// How long a system D-Bus call may take before it is abandoned.
const DBUS_CALL_TIMEOUT: Duration = Duration::from_secs(5);
/// Jobs queued for a worker thread before senders drop new ones.
const WORKER_QUEUE: usize = 32;
pub(super) use super::super::output::{
    DriverInput, EventSink, LoopDriver, OutputHold, SideEffects, flush_out_with,
    process_iteration_with, resolve_literals,
};
use super::super::system::{DbusArg, DbusCall, ProcessInvocation, SystemAction};
use crate::text_input::portal;
use evdev::uinput::VirtualDevice;
use evdev::{Device, EventType, InputEvent, KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

/// Pause before trying to open the selected mouse again.
const MOUSE_RETRY: Duration = Duration::from_secs(1);

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

pub(super) type MouseOutput = Arc<Mutex<Option<VirtualDevice>>>;

pub(super) struct OutputDevices {
    pub(super) keyboard: VirtualDevice,
    pub(super) mouse: MouseOutput,
}

pub(super) struct VirtualEventSink<'a> {
    virt: &'a mut OutputDevices,
}

impl EventSink for VirtualEventSink<'_> {
    fn emit(&mut self, events: &[(KeyCode, bool)]) -> Result<(), String> {
        // Split only at device boundaries to retain the order of modifier
        // presses and mouse buttons even when both occur in one engine batch.
        for group in events.chunk_by(|a, b| {
            super::NON_KEYBOARD_BUTTON_CODES.contains(&a.0.code())
                == super::NON_KEYBOARD_BUTTON_CODES.contains(&b.0.code())
        }) {
            let batch: Vec<_> = group
                .iter()
                .map(|&(key, down)| *KeyEvent::new(key, i32::from(down)))
                .collect();
            if super::NON_KEYBOARD_BUTTON_CODES.contains(&group[0].0.code()) {
                let mut mouse = self.virt.mouse.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(mouse) = mouse.as_mut() {
                    mouse.emit(&batch).map_err(|e| format!("mouse emit: {e}"))?;
                    continue;
                }
            }
            self.virt
                .keyboard
                .emit(&batch)
                .map_err(|e| format!("uinput emit: {e}"))?;
        }
        Ok(())
    }

    fn pointer(&mut self, event: &[InputEvent]) -> Result<(), String> {
        if let Some(mouse) = self
            .virt
            .mouse
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            mouse
                .emit(event)
                .map_err(|e| format!("pointer emit: {e}"))?;
        }
        Ok(())
    }
}

/// Build the replacement before grabbing: any setup failure leaves the
/// physical pointer usable. A fresh device also handles reconnect capability changes.
fn grab_mouse(mouse: &mut Device) -> Result<VirtualDevice, String> {
    if mouse
        .get_key_state()
        .map_err(|e| format!("mouse key state: {e}"))?
        .iter()
        .next()
        .is_some()
    {
        // Its existing press belongs to the physical device in the compositor.
        // Wait for release before moving subsequent clicks to a new device.
        return Err("mouse buttons are still held".into());
    }
    let build = || -> std::io::Result<VirtualDevice> {
        let mut builder = VirtualDevice::builder()?
            .name(super::VIRTUAL_POINTER_NAME)
            .with_properties(mouse.properties())?;
        let mut keys = mouse
            .supported_keys()
            .map(|keys| keys.iter().collect::<evdev::AttributeSet<KeyCode>>())
            .unwrap_or_default();
        for code in 0x110..=0x117 {
            keys.insert(KeyCode::new(code));
        }
        builder = builder.with_keys(&keys)?;
        if let Some(axes) = mouse.supported_relative_axes() {
            builder = builder.with_relative_axes(axes)?;
        }
        if let Some(axes) = mouse.supported_absolute_axes() {
            let state = mouse.get_abs_state()?;
            for axis in axes.iter() {
                builder = builder.with_absolute_axis(&evdev::UinputAbsSetup::new(
                    axis,
                    evdev::AbsInfo::from(state[axis.0 as usize]),
                ))?;
            }
        }
        builder.build()
    };
    let virt = build().map_err(|e| format!("create virtual pointer: {e}"))?;
    mouse.grab().map_err(|e| format!("grab mouse: {e}"))?;
    Ok(virt)
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
        submit_text(text, crate::text_input::job::TextRequest::enqueue(text));
    }

    fn literal_stroke(&self, text: &str) -> Option<super::super::action::Keystroke> {
        let mut chars = text.chars();
        let (ch, None) = (chars.next()?, chars.next()) else {
            return None;
        };
        let (key, mods) = portal::key_for_char(ch)?;
        Some(super::super::action::Keystroke {
            mods: mods.into_iter().map(KeyCode::new).collect(),
            key: KeyCode::new(key),
        })
    }

    fn type_macro_text(&mut self, text: &str, job: &crate::text_input::MacroText) {
        submit_text(
            text,
            crate::text_input::job::TextRequest::enqueue_macro(text, job.clone()),
        );
    }

    fn text_pending(&self) -> bool {
        crate::text_input::job::pending()
    }

    fn run_system(&mut self, action: &SystemAction) {
        run_sys_action(action);
    }

    fn run_command(&mut self, command: &ProcessInvocation) {
        super::super::commands::spawn(command.clone());
    }
}

// Submission stays on the mapper thread; injection runs on the text worker.
fn submit_text(
    text: &str,
    request: Result<crate::text_input::job::TextRequest, crate::events::TextInputFailure>,
) {
    match request {
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

enum SideEffectJob {
    Text(String, crate::text_input::job::TextRequest),
}

fn side_effect_tx() -> &'static mpsc::SyncSender<SideEffectJob> {
    static TX: OnceLock<mpsc::SyncSender<SideEffectJob>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(WORKER_QUEUE);
        match thread::Builder::new()
            .name("tecloid-side-effects".into())
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
    devices: Vec<Option<Device>>,
    pub(super) mouse_output: MouseOutput,
    mouse_path: Option<String>,
    mouse_identity: Option<super::DeviceIdentity>,
    next_mouse_retry: Instant,
    mouse_status: Option<super::super::mouse::Monitor>,
    hotkeys: PopupHotkeys,
    /// `TECLOID_HOTKEYS=off` disables the popup hotkeys; read once per start
    /// rather than on every key event.
    hotkeys_enabled: bool,
    keys: DeviceKeys,
    last_event: Instant,
    /// Read end of the control wake pipe. The `Handle` writes a byte after
    /// queuing a control message so the poll below returns immediately
    /// instead of sleeping out its full timeout.
    wake_rx: Option<std::os::fd::OwnedFd>,
}

fn device_rdev(device: &Device) -> Option<u64> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the device owns a valid fd and stat points to writable storage.
    if unsafe { libc::fstat(device.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    Some(unsafe { stat.assume_init() }.st_rdev)
}

impl MultiDeviceLoopDriver {
    pub(super) fn new(
        devices: Vec<Device>,
        mouse_path: Option<&str>,
        wake_rx: Option<std::os::fd::OwnedFd>,
    ) -> Result<Self, String> {
        let mut devices = devices;
        Self::prepare_device(&devices[0])?;
        let mouse_output = if let Some(mouse) = devices.get_mut(1) {
            match Self::prepare_device(mouse).and_then(|()| grab_mouse(mouse)) {
                Ok(output) => Some(output),
                Err(error) => {
                    log::warn!("[mapper] prepare mouse: {error}");
                    devices.truncate(1);
                    None
                }
            }
        } else {
            None
        };
        Ok(Self {
            mouse_output: Arc::new(Mutex::new(mouse_output)),
            mouse_identity: devices.get(1).map(super::DeviceIdentity::of),
            devices: {
                let mut slots: Vec<_> = devices.into_iter().map(Some).collect();
                slots.resize_with(2, || None);
                slots
            },
            mouse_path: mouse_path.map(str::to_owned),
            next_mouse_retry: Instant::now() + MOUSE_RETRY,
            mouse_status: None,
            hotkeys: PopupHotkeys::default(),
            hotkeys_enabled: crate::env::popup_hotkeys_enabled(),
            wake_rx,
            keys: DeviceKeys::default(),
            last_event: Instant::now(),
        })
    }

    fn prepare_device(device: &Device) -> Result<(), String> {
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
        Ok(())
    }

    pub(super) fn monitor_mouse(&mut self, monitor: super::super::mouse::Monitor) {
        if self.devices[1].is_some() {
            monitor.set(super::super::mouse::MouseStatus::Connected);
        } else if self.mouse_path.is_some()
            && monitor.get() == super::super::mouse::MouseStatus::NotSelected
        {
            monitor.set(super::super::mouse::MouseStatus::Unavailable);
        }
        self.mouse_status = Some(monitor);
    }

    fn set_mouse_status(&self, status: super::super::mouse::MouseStatus) {
        if let Some(monitor) = &self.mouse_status {
            monitor.set(status);
        }
    }

    fn reconnect_mouse(&mut self) {
        use super::super::mouse::MouseStatus;
        if self.devices[1].is_some() || Instant::now() < self.next_mouse_retry {
            return;
        }
        self.next_mouse_retry = Instant::now() + MOUSE_RETRY;
        let Some(path) = self.mouse_path.as_deref() else {
            return;
        };
        let mut mouse = match Device::open(path) {
            Ok(mouse) => mouse,
            Err(error) => {
                self.set_mouse_status(MouseStatus::from_error(&error));
                return;
            }
        };
        let identity = super::DeviceIdentity::of(&mouse);
        if self
            .mouse_identity
            .as_ref()
            .is_some_and(|expected| !expected.matches(&identity, path))
        {
            self.set_mouse_status(MouseStatus::DeviceChanged);
            return;
        }
        let Some(rdev) = device_rdev(&mouse) else {
            self.set_mouse_status(MouseStatus::Unavailable);
            return;
        };
        if super::is_grabbed_keyboard(rdev) {
            self.set_mouse_status(MouseStatus::InUse);
            return;
        }
        if let Err(error) = Self::prepare_device(&mouse) {
            log::debug!("[mapper] prepare mouse on reconnect: {error}");
            self.set_mouse_status(MouseStatus::Unavailable);
            return;
        };
        let output = match grab_mouse(&mut mouse) {
            Ok(output) => output,
            Err(error) => {
                log::warn!("[mapper] reconnect mouse: {error}");
                self.set_mouse_status(MouseStatus::Unavailable);
                return;
            }
        };
        *self.mouse_output.lock().unwrap_or_else(|e| e.into_inner()) = Some(output);
        self.set_mouse_status(MouseStatus::Connected);
        self.devices[1] = Some(mouse);
        self.mouse_identity = Some(identity);
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
        self.reconnect_mouse();

        // Round *up* to whole milliseconds. `poll` takes millisecond
        // granularity; truncating a sub-millisecond remainder to 0 makes
        // `poll` return instantly while the deadline it was meant to wait
        // for has not elapsed, so `tick` does nothing and the loop spins.
        // A genuine zero timeout (deadline already passed) stays 0.
        let timeout_ms: u16 = timeout.as_nanos().div_ceil(1_000_000).min(u16::MAX as u128) as u16;
        let mut pfds: Vec<PollFd> = self
            .devices
            .iter()
            .flatten()
            .map(|d| {
                let borrowed = unsafe { BorrowedFd::borrow_raw(d.as_raw_fd()) };
                PollFd::new(borrowed, PollFlags::POLLIN)
            })
            .collect();
        let device_count = pfds.len();
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
                let device_ready = pfds[..device_count].iter().any(|p| {
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
        Ok(self
            .fetch_inputs()?
            .into_iter()
            .filter_map(|input| match input {
                DriverInput::Key { key, down, at } => Some((key, down, at)),
                _ => None,
            })
            .collect())
    }

    fn fetch_inputs(&mut self) -> Result<Vec<DriverInput>, String> {
        let mut events = Vec::new();
        let mut pointer = Vec::new();
        for (source, slot) in self.devices.iter_mut().enumerate() {
            let Some(device) = slot.as_mut() else {
                continue;
            };
            let mut disconnected = None;
            match device.fetch_events() {
                Ok(iter) => {
                    let mut frame = Vec::new();
                    for ev in iter {
                        if ev.event_type() != EventType::KEY {
                            if source == 1
                                && matches!(
                                    ev.event_type(),
                                    EventType::RELATIVE | EventType::ABSOLUTE
                                )
                            {
                                frame.push(ev);
                            }
                            if ev.event_type() == EventType::SYNCHRONIZATION && !frame.is_empty() {
                                pointer.push(std::mem::take(&mut frame));
                            }
                            continue;
                        }
                        if !frame.is_empty() {
                            pointer.push(std::mem::take(&mut frame));
                        }
                        let value = ev.value();
                        if value == 2 {
                            continue;
                        }
                        events.push((ev.timestamp(), source, KeyCode::new(ev.code()), value == 1));
                    }
                    if !frame.is_empty() {
                        pointer.push(frame);
                    }
                }
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {}
                Err(e) if e.raw_os_error() == Some(libc::ENODEV) => {
                    if source == 0 {
                        return Err(super::DEVICE_DISCONNECTED.into());
                    }
                    disconnected = Some(super::super::mouse::MouseStatus::Missing);
                    self.keys
                        .release_source(source, &mut events, event_clock_now()?);
                    self.next_mouse_retry = Instant::now() + MOUSE_RETRY;
                }
                Err(e) if source == 1 => {
                    log::warn!("[mapper] read mouse: {e}");
                    disconnected = Some(super::super::mouse::MouseStatus::from_error(&e));
                    self.keys
                        .release_source(source, &mut events, event_clock_now()?);
                    self.next_mouse_retry = Instant::now() + MOUSE_RETRY;
                }
                Err(e) => return Err(format!("fetch_events: {e}")),
            };
            if let Some(status) = disconnected {
                *slot = None;
                if let Some(monitor) = &self.mouse_status {
                    monitor.set(status);
                }
            }
        }
        let wall = event_clock_now()?;
        let now = Instant::now();
        let previous = self.last_event;
        let mut inputs: Vec<_> = self
            .keys
            .merge(&mut events, wall, now, &mut self.last_event)
            .into_iter()
            .map(|(key, down, at)| DriverInput::Key { key, down, at })
            .collect();
        for event in pointer {
            let at = now
                .checked_sub(
                    wall.duration_since(event[0].timestamp())
                        .unwrap_or_default(),
                )
                .unwrap_or(previous)
                .max(previous);
            self.last_event = self.last_event.max(at);
            inputs.push(DriverInput::Pointer { event, at });
        }
        inputs.sort_by_key(|input| match input {
            DriverInput::Key { at, .. } | DriverInput::Pointer { at, .. } => *at,
            DriverInput::Repeat(_) => now,
        });
        Ok(inputs)
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
            KeyCode::KEY_F11 => crate::config::actions::SHOW_EMOJI_MENU_RECENT,
            KeyCode::KEY_F12 => crate::config::actions::SHOW_QUICK_MENU_RECENT,
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
    fn release_source(
        &self,
        source: usize,
        events: &mut Vec<(SystemTime, usize, KeyCode, bool)>,
        at: SystemTime,
    ) {
        let mut keys: HashSet<_> = self
            .held
            .iter()
            .filter_map(|&(owner, key)| (owner == source).then_some(key))
            .collect();
        keys.extend(
            events
                .iter()
                .filter_map(|&(_, owner, key, down)| (owner == source && down).then_some(key)),
        );
        events.extend(keys.into_iter().map(|key| (at, source, key, false)));
    }

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
    virt: &mut OutputDevices,
    out_buf: &mut Vec<EngineOutput>,
) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects;
    process_iteration_with(driver, engine, hold, &mut sink, &mut effects, out_buf)
}

pub(super) fn flush_out(
    virt: &mut OutputDevices,
    buf: &mut Vec<EngineOutput>,
) -> Result<(), String> {
    let mut sink = VirtualEventSink { virt };
    let mut effects = RuntimeSideEffects;
    flush_out_with(&mut sink, &mut effects, buf)
}

/// Dispatch a resolved system action on the current OS/DE.
fn run_sys_action(action: &SystemAction) {
    match action {
        SystemAction::Dbus(call) => call_dbus(call),
        SystemAction::Process(process) => super::super::commands::spawn(process.clone()),
        SystemAction::AppEvent(event) => {
            super::super::emit_app_event(event);
        }
    }
}

/// Mapper generation of D-Bus jobs. Stopping the mapper bumps it, so calls
/// queued by that mapper never run after remapping is off.
static DBUS_GENERATION: AtomicU64 = AtomicU64::new(0);
/// A queued call older than this is dropped: it waited behind a slow call
/// and would fire long after the key press that requested it.
const DBUS_JOB_MAX_AGE: Duration = Duration::from_secs(2);

/// Drop every D-Bus call that has been queued but not started yet.
pub(super) fn invalidate_pending_dbus_calls() {
    DBUS_GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// One queued D-Bus call with the generation and time it was queued at.
struct DbusJob {
    call: DbusCall,
    generation: u64,
    queued: Instant,
}

/// Enqueue a DBus call on the dedicated `tecloid-dbus` worker. Calls stay
/// ordered among themselves (macros rely on that) but never block the
/// side-effect worker, so a hung DBus service cannot stall text
/// injection or shell commands queued after it.
fn call_dbus(call: &DbusCall) {
    let job = DbusJob {
        call: call.clone(),
        generation: DBUS_GENERATION.load(Ordering::SeqCst),
        queued: Instant::now(),
    };
    if dbus_tx().try_send(job).is_err() {
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

fn dbus_tx() -> &'static mpsc::SyncSender<DbusJob> {
    static TX: OnceLock<mpsc::SyncSender<DbusJob>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(WORKER_QUEUE);
        match thread::Builder::new()
            .name("tecloid-dbus".into())
            .spawn(move || {
                run_dbus_worker(rx, &mut SessionBus::default(), &DBUS_GENERATION, |error| {
                    report_action_failure(error)
                })
            }) {
            Ok(_) => {}
            Err(e) => log::debug!("[mapper] could not spawn dbus worker: {e}"),
        }
        tx
    })
}

/// Failure of one D-Bus call.
#[derive(Debug)]
struct DbusFailure {
    message: String,
    /// The connection is unusable; the next call reconnects.
    reconnect: bool,
}

/// Where the worker sends calls; a test substitutes a delayed transport.
trait DbusTransport {
    fn call(&mut self, call: &DbusCall) -> Result<(), DbusFailure>;
}

/// The session bus. The connection's method timeout ends a hung call in
/// the worker itself, so no abandoned call thread keeps running.
#[derive(Default)]
struct SessionBus(Option<zbus::blocking::Connection>);

impl DbusTransport for SessionBus {
    fn call(&mut self, call: &DbusCall) -> Result<(), DbusFailure> {
        // One long-lived session connection instead of a connection per call.
        let conn = match &self.0 {
            Some(conn) => conn.clone(),
            None => {
                let conn = zbus::blocking::connection::Builder::session()
                    .map(|builder| builder.method_timeout(DBUS_CALL_TIMEOUT))
                    .and_then(|builder| builder.build())
                    .map_err(|e| DbusFailure {
                        message: format!("Cannot connect to session bus: {e}"),
                        reconnect: true,
                    })?;
                self.0 = Some(conn.clone());
                conn
            }
        };
        dispatch_dbus_call(&conn, call).map_err(|e| {
            // Method-level errors keep the connection; anything else
            // (transport, serialization, timeout) forces a reconnect.
            let reconnect = !matches!(e, zbus::Error::MethodError(..));
            if reconnect {
                self.0 = None;
            }
            DbusFailure {
                message: format!("{}.{} failed: {e}", call.destination, call.method),
                reconnect,
            }
        })
    }
}

fn run_dbus_worker(
    rx: mpsc::Receiver<DbusJob>,
    transport: &mut dyn DbusTransport,
    generation: &AtomicU64,
    report: impl Fn(String),
) {
    for job in rx {
        let call = &job.call;
        if job.generation != generation.load(Ordering::SeqCst) {
            log::debug!(
                "[mapper] dbus {}.{} skipped: mapper stopped",
                call.destination,
                call.method
            );
            continue;
        }
        if job.queued.elapsed() > DBUS_JOB_MAX_AGE {
            report(format!(
                "{}.{} skipped: queued too long behind a slow system action",
                call.destination, call.method
            ));
            continue;
        }
        match transport.call(call) {
            Ok(()) => {
                if call.switches_layout() {
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
            Err(failure) => {
                log::debug!(
                    "[mapper] dbus {} (reconnect: {})",
                    failure.message,
                    failure.reconnect
                );
                report(failure.message);
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
            DbusArg::Str(s) => b.add_field(s.clone()),
            DbusArg::EmptyVariants => b.add_field(Vec::<zbus::zvariant::OwnedValue>::new()),
            DbusArg::EmptyVariantDict => b.add_field(std::collections::HashMap::<
                String,
                zbus::zvariant::OwnedValue,
            >::new()),
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

    /// Records calls; the first one blocks until released, like a hung
    /// D-Bus service.
    struct DelayedTransport {
        started: mpsc::Sender<String>,
        release: mpsc::Receiver<()>,
        first: bool,
    }

    impl DbusTransport for DelayedTransport {
        fn call(&mut self, call: &DbusCall) -> Result<(), DbusFailure> {
            let _ = self.started.send(call.method.clone());
            if std::mem::take(&mut self.first) {
                let _ = self.release.recv();
            }
            Ok(())
        }
    }

    fn dbus_job(method: &str, generation: &AtomicU64) -> DbusJob {
        DbusJob {
            call: DbusCall {
                destination: "org.example".into(),
                path: "/".into(),
                interface: None,
                method: method.into(),
                args: Vec::new(),
            },
            generation: generation.load(Ordering::SeqCst),
            queued: Instant::now(),
        }
    }

    /// A D-Bus worker on a [`DelayedTransport`] and its observation ends.
    struct DelayedWorker {
        jobs: mpsc::SyncSender<DbusJob>,
        started: mpsc::Receiver<String>,
        release: mpsc::Sender<()>,
        reports: mpsc::Receiver<String>,
        generation: &'static AtomicU64,
        worker: thread::JoinHandle<()>,
    }

    fn delayed_worker() -> DelayedWorker {
        let generation: &'static AtomicU64 = Box::leak(Box::new(AtomicU64::new(0)));
        let (jobs, rx) = mpsc::sync_channel(WORKER_QUEUE);
        let (started_tx, started) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let (report_tx, reports) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut transport = DelayedTransport {
                started: started_tx,
                release: release_rx,
                first: true,
            };
            run_dbus_worker(rx, &mut transport, generation, |error| {
                let _ = report_tx.send(error);
            });
        });
        DelayedWorker {
            jobs,
            started,
            release,
            reports,
            generation,
            worker,
        }
    }

    #[test]
    fn stopping_the_mapper_drops_dbus_calls_queued_behind_a_delayed_one() {
        let DelayedWorker {
            jobs,
            started,
            release,
            reports,
            generation,
            worker,
        } = delayed_worker();
        jobs.send(dbus_job("Slow", generation)).unwrap();
        assert_eq!(
            started.recv_timeout(Duration::from_secs(1)).unwrap(),
            "Slow"
        );
        jobs.send(dbus_job("Queued", generation)).unwrap();
        // The mapper stops while the first call is still in the transport.
        generation.fetch_add(1, Ordering::SeqCst);
        jobs.send(dbus_job("NextMapper", generation)).unwrap();
        release.send(()).unwrap();
        drop(jobs);
        worker.join().unwrap();
        assert_eq!(started.try_iter().collect::<Vec<_>>(), ["NextMapper"]);
        assert!(reports.try_recv().is_err());
    }

    #[test]
    fn stale_dbus_calls_are_reported_instead_of_run_late() {
        let DelayedWorker {
            jobs,
            started,
            release,
            reports,
            generation,
            worker,
        } = delayed_worker();
        jobs.send(dbus_job("Slow", generation)).unwrap();
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        let mut stale = dbus_job("Stale", generation);
        stale.queued -= DBUS_JOB_MAX_AGE + Duration::from_millis(1);
        jobs.send(stale).unwrap();
        release.send(()).unwrap();
        drop(jobs);
        worker.join().unwrap();
        assert!(started.try_recv().is_err());
        assert!(reports.try_recv().unwrap().contains("Stale"));
    }

    #[test]
    fn disconnect_releases_only_mouse_ownership_and_pending_presses() {
        let mut keys = DeviceKeys::default();
        assert!(keys.transition(0, KeyCode::KEY_A, true));
        assert!(!keys.transition(1, KeyCode::KEY_A, true));
        assert!(keys.transition(1, KeyCode::BTN_LEFT, true));
        let now = Instant::now();
        let wall = SystemTime::now();
        let mut last = now;
        let mut events = vec![(wall, 1, KeyCode::BTN_RIGHT, true)];
        keys.release_source(1, &mut events, wall);
        let out = keys.merge(&mut events, wall, now, &mut last);
        assert!(
            out.iter()
                .any(|&(key, down, _)| key == KeyCode::BTN_LEFT && !down)
        );
        assert!(
            !out.iter()
                .any(|&(key, down, _)| key == KeyCode::KEY_A && !down)
        );
        assert!(!keys.held.iter().any(|&(owner, _)| owner == 1));
        assert!(keys.held.contains(&(0, KeyCode::KEY_A)));
        assert!(keys.transition(0, KeyCode::KEY_A, false));
    }

    #[test]
    #[ignore = "requires /dev/uinput and access to synthetic /dev/input devices"]
    fn synthetic_devices_use_monotonic_timestamps_and_preserve_cross_device_order() {
        fn synthetic() -> (VirtualDevice, Device) {
            let mut keys = evdev::AttributeSet::new();
            keys.insert(KeyCode::KEY_A);
            keys.insert(KeyCode::KEY_B);
            let mut virt = VirtualDevice::builder()
                .unwrap()
                .name("Tecloid isolated mapper test")
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
        let mut driver =
            MultiDeviceLoopDriver::new(vec![first_input, second_input], None, None).unwrap();
        let mouse_status = super::super::super::mouse::Monitor::default();
        driver.monitor_mouse(mouse_status.clone());
        assert_eq!(
            mouse_status.get(),
            super::super::super::mouse::MouseStatus::Connected
        );
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
        second.emit(&[*KeyEvent::new(KeyCode::KEY_B, 1)]).unwrap();
        assert!(driver.wait(Duration::from_secs(1)).unwrap());
        driver.fetch_timed_key_events().unwrap();
        drop(second);
        assert!(driver.wait(Duration::from_secs(1)).unwrap());
        let released = driver.fetch_timed_key_events().unwrap();
        assert!(
            released
                .iter()
                .any(|&(key, down, _)| key == KeyCode::KEY_B && !down)
        );
        assert!(driver.devices[1].is_none());
        assert_eq!(
            mouse_status.get(),
            super::super::super::mouse::MouseStatus::Missing
        );
        let (mut replacement, replacement_input) = synthetic();
        let replacement_path = replacement
            .enumerate_dev_nodes_blocking()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        drop(replacement_input);
        let temp = tempfile::tempdir().unwrap();
        let mouse_path = temp.path().join("mouse");
        std::os::unix::fs::symlink(replacement_path, &mouse_path).unwrap();
        driver.mouse_path = Some(mouse_path.to_str().unwrap().into());
        driver.next_mouse_retry = Instant::now();
        driver.wait(Duration::ZERO).unwrap();
        assert_eq!(
            mouse_status.get(),
            super::super::super::mouse::MouseStatus::Connected
        );
        driver.devices[1]
            .as_mut()
            .expect("mouse reconnected without keyboard input")
            .grab()
            .unwrap();
        replacement
            .emit(&[*KeyEvent::new(KeyCode::KEY_B, 1)])
            .unwrap();
        assert!(driver.wait(Duration::from_secs(1)).unwrap());
        assert!(
            driver
                .fetch_timed_key_events()
                .unwrap()
                .iter()
                .any(|&(key, down, _)| key == KeyCode::KEY_B && down)
        );

        first.emit(&[*KeyEvent::new(KeyCode::KEY_A, 1)]).unwrap();
        assert!(driver.wait(Duration::from_secs(1)).unwrap());
        assert!(
            driver
                .fetch_timed_key_events()
                .unwrap()
                .iter()
                .any(|&(key, down, _)| key == KeyCode::KEY_A && down)
        );
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
        let (sent, received) = std::sync::mpsc::channel();
        let _events = crate::events::bus().subscribe(move |event| {
            if matches!(event, crate::events::CoreEvent::AppAction(name) if *name == crate::config::app_action::AppAction::parse("activateProfile5").unwrap()) {
                sent.send(()).unwrap();
            }
        }).unwrap();
        RuntimeSideEffects.run_system(&SystemAction::AppEvent(
            crate::config::app_action::AppAction::parse("activateProfile5").unwrap(),
        ));
        received.recv_timeout(Duration::from_secs(1)).unwrap();
    }
}

#[cfg(test)]
mod popup_hotkey_tests {
    use super::*;

    #[test]
    fn popup_chord_never_emits_function_key_even_if_disabled_before_release() {
        for (key, action) in [
            (KeyCode::KEY_F11, "tecloid:showEmojiMenuRecent"),
            (KeyCode::KEY_F12, "tecloid:showQuickMenuRecent"),
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
