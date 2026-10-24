mod devices;
mod io;

pub use devices::{list_input_devices, list_keyboards, list_mice};

use super::config::AppConfig;
use super::engine::{Engine, Out};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, Device, InputId, KeyCode};
use io::{LoopDriver, MultiDeviceLoopDriver, OutputHold, flush_out, process_iteration};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const VIRTUAL_DEVICE_NAME: &str = "LeftHandControl Virtual Keyboard";
const DEVICE_DISCONNECTED: &str = "mapper input device disconnected";
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// Device number of the keyboard the mapper holds grabbed. Its physical key
/// state never reaches the compositor: held layer keys must not delay text.
static GRABBED_KEYBOARD: Mutex<Option<u64>> = Mutex::new(None);

/// Whether `rdev` is the keyboard the running mapper grabbed.
pub(super) fn is_grabbed_keyboard(rdev: u64) -> bool {
    *GRABBED_KEYBOARD.lock().unwrap_or_else(|e| e.into_inner()) == Some(rdev)
}

/// Registers the grabbed keyboard until dropped with the mapper loop.
struct GrabbedKeyboard;

impl GrabbedKeyboard {
    fn register(rdev: u64) -> Self {
        *GRABBED_KEYBOARD.lock().unwrap_or_else(|e| e.into_inner()) = Some(rdev);
        Self
    }
}

impl Drop for GrabbedKeyboard {
    fn drop(&mut self) {
        *GRABBED_KEYBOARD.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

pub enum MapperControl {
    Config(Box<AppConfig>),
    Execute(String),
    /// A popup took the keyboard focus; emit the output held for it.
    PopupFocused,
}

/// Handle to a running mapper thread.
pub struct Handle {
    stop: Arc<AtomicBool>,
    control_tx: mpsc::Sender<MapperControl>,
    /// Write end of the wake pipe: one byte per control message so the
    /// mapper's poll loop reacts immediately instead of after its timeout.
    wake_tx: OwnedFd,
    join: Option<thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
    waiting: Arc<AtomicBool>,
}

impl Handle {
    fn wake(&self) {
        let buf = [1u8];
        unsafe {
            libc::write(
                self.wake_tx.as_raw_fd(),
                buf.as_ptr() as *const libc::c_void,
                1,
            );
        }
    }

    pub fn stop(mut self) {
        super::text_job::cancel();
        self.stop.store(true, Ordering::SeqCst);
        self.wake();
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

    pub fn waiting_for_device(&self) -> bool {
        self.waiting.load(Ordering::SeqCst)
    }

    pub fn last_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|g| g.clone())
    }

    pub fn update_config(&self, cfg: AppConfig) -> Result<(), String> {
        self.control_tx
            .send(MapperControl::Config(Box::new(cfg)))
            .map_err(|e| format!("mapper update channel closed: {e}"))?;
        self.wake();
        Ok(())
    }

    pub fn execute_action(&self, action: String) -> Result<(), String> {
        self.control_tx
            .send(MapperControl::Execute(action))
            .map_err(|e| format!("mapper update channel closed: {e}"))?;
        self.wake();
        Ok(())
    }

    pub fn popup_focused(&self) -> Result<(), String> {
        self.control_tx
            .send(MapperControl::PopupFocused)
            .map_err(|e| format!("mapper update channel closed: {e}"))?;
        self.wake();
        Ok(())
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

pub fn spawn(
    device_path: String,
    mouse_path: Option<String>,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
) -> Result<Handle, String> {
    super::text_job::start();
    super::portal::set_text_backend(
        cfg.settings.linux_wayland_text_mode,
        cfg.settings.linux_paste_shortcut,
        cfg.settings.linux_ydotool_path.as_deref(),
        cfg.settings.linux_xdotool_path.as_deref(),
    );
    // Starting the mapper is an explicit user action: give a backend that
    // failed before (e.g. a declined portal dialog) another chance.
    super::portal::retry_text_backend();
    #[cfg(debug_assertions)]
    {
        log::debug!(
            "[mapper] started: device={} mouse={:?} rules={} layers={}",
            device_path,
            mouse_path,
            cfg.rules.len(),
            cfg.layer_keymaps.len()
        );
        for r in &cfg.rules {
            log::debug!(
                "[mapper]   rule key={} layer={:?} tap={:?} hold={:?} holdMs={:?}",
                r.key,
                r.layer_id,
                r.tap_action,
                r.hold_action,
                r.hold_timeout_ms
            );
        }
        for (lid, km) in &cfg.layer_keymaps {
            log::debug!("[mapper]   keymap[{lid}] keys={}", km.keys.len());
        }
    }

    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let waiting = Arc::new(AtomicBool::new(false));
    let waiting_thread = waiting.clone();
    let stop_thread = stop.clone();
    let err_thread = error.clone();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let (control_tx, control_rx) = mpsc::channel::<MapperControl>();
    let (wake_rx, wake_tx) = make_wake_pipe()?;

    let join = thread::Builder::new()
        .name("lhc-mapper".into())
        .spawn(move || {
            use std::panic::catch_unwind;
            let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(
                    device_path,
                    mouse_path,
                    cfg,
                    stop_thread.clone(),
                    ready_tx,
                    control_rx,
                    wake_rx,
                    waiting_thread,
                )
            }));
            if stop_thread.load(Ordering::SeqCst) {
                return;
            }
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    log::debug!("[mapper] run_loop error: {e}");
                    if let Ok(mut slot) = err_thread.lock() {
                        *slot = Some(e.clone());
                    }
                    notify_mapper_stopped(&e);
                }
                Err(panic_info) => {
                    let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "mapper thread panicked".to_string()
                    };
                    log::debug!("[mapper] {msg}");
                    if let Ok(mut slot) = err_thread.lock() {
                        *slot = Some(msg.clone());
                    }
                    notify_mapper_stopped(&msg);
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
            drop(join); // detach — may be stuck in a kernel syscall; stop flag is set, thread cleans up when unblocked
            let err_msg = "mapper start timed out";
            if let Ok(mut slot) = error.lock() {
                *slot = Some(err_msg.into());
            }
            return Err(err_msg.into());
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = join.join();
            let err_msg = error
                .lock()
                .ok()
                .and_then(|slot| slot.clone())
                .unwrap_or_else(|| "mapper worker exited before reporting readiness".into());
            if let Ok(mut slot) = error.lock() {
                *slot = Some(err_msg.clone());
            }
            return Err(err_msg);
        }
    }

    Ok(Handle {
        stop,
        control_tx,
        wake_tx,
        join: Some(join),
        error,
        waiting,
    })
}

/// Tell the shell the mapper thread died unexpectedly (device gone,
/// panic, …) so it can update state without waiting for a status poll.
fn notify_mapper_stopped(error: &str) {
    super::notify_mapper_stopped(error);
}

fn make_wake_pipe() -> Result<(OwnedFd, OwnedFd), String> {
    let mut fds = [0i32; 2];
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if rc != 0 {
        return Err(format!("pipe2: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: pipe2 just returned these fds and nothing else owns them.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

fn acquire_mapper_lock() -> Result<std::fs::File, String> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = base.join(format!("left-hand-control-mapper-{}.lock", unsafe {
        libc::geteuid()
    }));
    lock_mapper_file(&path)
}

fn lock_mapper_file(path: &std::path::Path) -> Result<std::fs::File, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| format!("open mapper lock {}: {error}", path.display()))?;
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        return Err("mapper is already active in another process".into());
    }
    Ok(file)
}

#[allow(clippy::too_many_arguments)]
fn run(
    device_path: String,
    mouse_path: Option<String>,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<Result<(), String>>,
    control_rx: mpsc::Receiver<MapperControl>,
    wake_rx: OwnedFd,
    waiting: Arc<AtomicBool>,
) -> Result<(), String> {
    if stop.load(Ordering::SeqCst) {
        return Err("mapper start cancelled".into());
    }
    let _mapper_lock = acquire_mapper_lock()?;
    let mut cfg = cfg;
    reconnect_loop(
        &stop,
        &device_path,
        || Device::open(&device_path).map_err(|error| error.to_string()),
        |device| {
            let mut identity = DeviceIdentity::of(device);
            if stable_device_path(&device_path) {
                identity.physical = None;
            }
            identity
        },
        |device| {
            let result = run_device(
                device,
                &device_path,
                mouse_path.as_deref(),
                &mut cfg,
                stop.clone(),
                &ready_tx,
                &control_rx,
                &waiting,
                wake_rx
                    .try_clone()
                    .map_err(|e| format!("clone wake pipe: {e}"))?,
            );
            if matches!(&result, Err(error) if error == DEVICE_DISCONNECTED) {
                set_waiting(&waiting, true);
            }
            result
        },
        || thread::sleep(RECONNECT_RETRY),
    )
}

/// How long to wait before looking for a disconnected keyboard again.
const RECONNECT_RETRY: Duration = Duration::from_millis(100);

/// Run the device until it fails. After a disconnect, wait for a device
/// with the same identity at the same path and run it again; any other
/// failure ends the loop.
fn reconnect_loop<D, I: PartialEq>(
    stop: &AtomicBool,
    device_path: &str,
    mut open: impl FnMut() -> Result<D, String>,
    identity: impl Fn(&D) -> I,
    mut run_device: impl FnMut(D) -> Result<(), String>,
    mut wait: impl FnMut(),
) -> Result<(), String> {
    let mut expected = None;
    let mut reconnecting = false;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let device = match open() {
            Ok(device) => device,
            Err(_) if reconnecting => {
                wait();
                continue;
            }
            Err(error) => return Err(format!("open {device_path}: {error}")),
        };
        let current = identity(&device);
        match &expected {
            // Event numbers can be reused by an unrelated keyboard.
            Some(expected) if expected != &current => {
                drop(device);
                wait();
                continue;
            }
            Some(_) => {}
            None => expected = Some(current),
        }
        let result = run_device(device);
        if !matches!(&result, Err(error) if error == DEVICE_DISCONNECTED) {
            return result;
        }
        log::info!("[mapper] input disconnected; waiting for {device_path}");
        reconnecting = true;
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DeviceIdentity {
    id: InputId,
    name: Option<String>,
    physical: Option<String>,
    unique: Option<String>,
}

fn set_waiting(waiting: &AtomicBool, value: bool) {
    if waiting.swap(value, Ordering::SeqCst) != value {
        crate::events::bus().emit(crate::events::CoreEvent::MapperDeviceChanged);
    }
}

fn stable_device_path(path: &str) -> bool {
    std::path::Path::new(path).parent() == Some(std::path::Path::new("/dev/input/by-id"))
}

impl DeviceIdentity {
    fn matches(&self, current: &Self, path: &str) -> bool {
        self.id == current.id
            && self.name == current.name
            && self.unique == current.unique
            && (self.physical == current.physical || stable_device_path(path))
    }

    fn of(device: &Device) -> Self {
        Self {
            id: device.input_id(),
            name: device.name().map(str::to_owned),
            physical: device.physical_path().map(str::to_owned),
            unique: device.unique_name().map(str::to_owned),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_device(
    mut device: Device,
    device_path: &str,
    mouse_path: Option<&str>,
    cfg: &mut AppConfig,
    stop: Arc<AtomicBool>,
    ready_tx: &mpsc::Sender<Result<(), String>>,
    control_rx: &mpsc::Receiver<MapperControl>,
    waiting: &AtomicBool,
    wake_rx: OwnedFd,
) -> Result<(), String> {
    validate_keyboard_capabilities(device.supported_events(), device.supported_keys())?;
    let keyboard = std::fs::metadata(device_path).map_err(|e| format!("stat keyboard: {e}"))?;
    if let Some(mouse_path) = mouse_path
        && let Ok(mouse) = std::fs::metadata(mouse_path)
        && keyboard.rdev() == mouse.rdev()
        && keyboard.ino() == mouse.ino()
    {
        return Err("Keyboard and mouse must use different input devices".into());
    }
    device
        .grab()
        .map_err(|e| format!("grab {device_path} (need perms? add user to input group): {e}"))?;
    // Declared before the loop driver, so it is cleared after the grab ends.
    let _grabbed = GrabbedKeyboard::register(keyboard.rdev());

    let mut devices = vec![device];
    if let Some(mp) = mouse_path
        && let Ok(mouse) = Device::open(mp)
    {
        // Mouse is opened without grab: only KEY events are read to interact
        // with modifier tap-hold decisions. REL/ABS (movement) stays
        // untouched so the cursor continues to work.
        //
        // Known limitation: because the mouse is not grabbed, the physical
        // button event always reaches the OS. Layer remaps of mouse buttons
        // emit the mapped action *in addition to* the physical click, and
        // `Swallow` cannot suppress the physical click. Fixing this would
        // require grabbing the mouse and re-emitting REL/ABS ourselves.
        devices.push(mouse);
    }

    let mut all_keys = AttributeSet::<KeyCode>::new();
    // Keyboard block (1..=248) plus the extended KEY_* block above the
    // BTN_* range: keys.rs maps e.g. KEY_SELECT (353) and KEY_FN (464),
    // which the virtual device must register or their emits are silently
    // dropped by uinput. The BTN_* block (256..=351) is skipped on purpose
    // so the device is not classified as a mouse/joystick.
    for code in (1u16..=248).chain(352..=511) {
        all_keys.insert(KeyCode::new(code));
    }
    // Catalog keys beyond that block, such as KEY_TOUCHPAD_TOGGLE (0x212).
    for key in crate::profile::key_catalog::CATEGORIES
        .iter()
        .flat_map(|keys| keys.iter())
        .filter_map(|name| super::keys::code_to_key(name))
        .filter(|key| key.code() > 511)
    {
        all_keys.insert(key);
    }
    let virt = VirtualDevice::builder()
        .map_err(|e| format!("uinput builder (need /dev/uinput access?): {e}"))?
        .name(VIRTUAL_DEVICE_NAME)
        .input_id(InputId::new(BusType::BUS_USB, 0x1d6b, 0x0104, 1))
        .with_keys(&all_keys)
        .map_err(|e| format!("uinput with_keys: {e}"))?
        .build()
        .map_err(|e| format!("uinput build: {e}"))?;

    let driver = MultiDeviceLoopDriver::new(devices, mouse_path, Some(wake_rx))?;
    super::text_job::start();
    set_waiting(waiting, false);
    let _ = ready_tx.send(Ok(()));
    run_loop(driver, virt, cfg, stop, control_rx)
}

fn validate_keyboard_capabilities(
    events: &evdev::AttributeSetRef<evdev::EventType>,
    keys: Option<&evdev::AttributeSetRef<KeyCode>>,
) -> Result<(), String> {
    if events.contains(evdev::EventType::RELATIVE)
        || events.contains(evdev::EventType::ABSOLUTE)
        || keys.is_some_and(|keys| keys.iter().any(|key| (256..=351).contains(&key.code())))
    {
        return Err("This input device also provides pointer movement or non-keyboard buttons; select its separate keyboard interface to avoid blocking those inputs".into());
    }
    Ok(())
}

fn run_loop<D: LoopDriver>(
    mut driver: D,
    mut virt: VirtualDevice,
    cfg: &mut AppConfig,
    stop: Arc<AtomicBool>,
    control_rx: &mpsc::Receiver<MapperControl>,
) -> Result<(), String> {
    let mut engine = Engine::new(cfg);
    let mut out_buf: Vec<Out> = Vec::with_capacity(16);
    let mut hold = OutputHold::default();
    let result = (|| {
        while !stop.load(Ordering::SeqCst) {
            while let Ok(ctrl) = control_rx.try_recv() {
                match ctrl {
                    MapperControl::Config(next_cfg) => {
                        let next_cfg = *next_cfg;
                        if next_cfg == *cfg {
                            continue;
                        }
                        hold.cancel_actions(&mut out_buf);
                        super::text_job::cancel();
                        log::debug!(
                            "[mapper] live config update: rules={} layers={}",
                            next_cfg.rules.len(),
                            next_cfg.layer_keymaps.len()
                        );
                        super::portal::set_text_backend(
                            next_cfg.settings.linux_wayland_text_mode,
                            next_cfg.settings.linux_paste_shortcut,
                            next_cfg.settings.linux_ydotool_path.as_deref(),
                            next_cfg.settings.linux_xdotool_path.as_deref(),
                        );
                        engine.reconfigure(&next_cfg, &mut out_buf);
                        super::layer_state::publish(engine.held_layer_state());
                        super::text_job::start();
                        *cfg = next_cfg;
                    }
                    MapperControl::Execute(action) => {
                        log::debug!("[mapper] remote execute requested");
                        engine.execute_remote(&action, &mut out_buf);
                    }
                    MapperControl::PopupFocused => hold.release(),
                }
            }
            hold.gate(
                &mut out_buf,
                std::time::Instant::now(),
                super::text_job::pending(),
            );
            flush_out(&mut virt, &mut out_buf)?;
            process_iteration(&mut driver, &mut engine, &mut hold, &mut virt, &mut out_buf)?;
        }
        Ok(())
    })();
    super::text_job::cancel();
    super::layer_state::publish(super::layer_state::HeldLayerState::default());

    hold.discard();
    engine.shutdown(&mut out_buf);
    let _ = flush_out(&mut virt, &mut out_buf);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn stable_identity_allows_port_change_but_not_another_device() {
        let identity = |physical: &str, unique: &str| super::DeviceIdentity {
            id: evdev::InputId::new(evdev::BusType::BUS_USB, 1, 2, 1),
            name: Some("keyboard".into()),
            physical: Some(physical.into()),
            unique: Some(unique.into()),
        };
        let original = identity("port-1", "serial-1");
        let moved = identity("port-2", "serial-1");
        assert!(original.matches(&moved, "/dev/input/by-id/keyboard"));
        assert!(!original.matches(&moved, "/dev/input/event3"));
        assert!(!original.matches(&identity("port-2", "serial-2"), "/dev/input/by-id/keyboard"));
    }

    #[test]
    fn pointer_interfaces_are_rejected_before_grabbing() {
        let mut events = evdev::AttributeSet::new();
        events.insert(evdev::EventType::KEY);
        assert!(super::validate_keyboard_capabilities(&events, None).is_ok());
        let mut keys = evdev::AttributeSet::new();
        keys.insert(evdev::KeyCode::BTN_LEFT);
        assert!(super::validate_keyboard_capabilities(&events, Some(&keys)).is_err());
        for axis in [evdev::EventType::RELATIVE, evdev::EventType::ABSOLUTE] {
            let mut pointer = events.clone();
            pointer.insert(axis);
            assert!(super::validate_keyboard_capabilities(&pointer, None).is_err());
        }
    }

    /// Scripted device opens for [`super::reconnect_loop`]: each entry is
    /// the identity of the device found, or `None` when the path is absent.
    fn reconnect(
        opens: &[Option<u32>],
        runs: &[Result<(), &str>],
    ) -> (Result<(), String>, Vec<u32>, usize) {
        let stop = std::sync::atomic::AtomicBool::new(false);
        let mut opens = opens.iter().copied();
        let mut runs = runs.iter().cloned();
        let mut ran = Vec::new();
        let mut waits = 0;
        let result = super::reconnect_loop(
            &stop,
            "/dev/input/by-id/kbd",
            || {
                opens
                    .next()
                    .flatten()
                    .ok_or_else(|| "No such device".to_string())
            },
            |device: &u32| *device,
            |device| {
                ran.push(device);
                runs.next().expect("scripted run").map_err(str::to_owned)
            },
            || waits += 1,
        );
        (result, ran, waits)
    }

    #[test]
    fn a_missing_keyboard_fails_the_start() {
        let (result, ran, _) = reconnect(&[None], &[]);
        assert_eq!(
            result.unwrap_err(),
            "open /dev/input/by-id/kbd: No such device"
        );
        assert!(ran.is_empty());
    }

    #[test]
    fn a_disconnected_keyboard_is_awaited_and_resumed() {
        let disconnected = Err(super::DEVICE_DISCONNECTED);
        let (result, ran, waits) =
            reconnect(&[Some(7), None, None, Some(7)], &[disconnected, Ok(())]);
        assert_eq!(result, Ok(()));
        assert_eq!(ran, [7, 7]);
        assert_eq!(waits, 2);
    }

    #[test]
    fn another_keyboard_on_the_same_path_is_not_grabbed() {
        let disconnected = Err(super::DEVICE_DISCONNECTED);
        let (result, ran, waits) = reconnect(&[Some(7), Some(9), Some(7)], &[disconnected, Ok(())]);
        assert_eq!(result, Ok(()));
        assert_eq!(ran, [7, 7], "the other keyboard never runs");
        assert_eq!(waits, 1);
    }

    #[test]
    fn errors_other_than_a_disconnect_stop_the_mapper() {
        let (result, ran, waits) = reconnect(&[Some(7)], &[Err("uinput emit: broken")]);
        assert_eq!(result.unwrap_err(), "uinput emit: broken");
        assert_eq!((ran.len(), waits), (1, 0));
    }

    #[test]
    fn stop_ends_the_wait_for_a_keyboard() {
        let stop = std::sync::atomic::AtomicBool::new(false);
        let mut first = true;
        let result = super::reconnect_loop(
            &stop,
            "kbd",
            || {
                if std::mem::take(&mut first) {
                    Ok(1u32)
                } else {
                    Err("gone".to_string())
                }
            },
            |device| *device,
            |_| Err(super::DEVICE_DISCONNECTED.to_string()),
            || stop.store(true, std::sync::atomic::Ordering::SeqCst),
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn mapper_lock_excludes_another_instance() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mapper.lock");
        let first = super::lock_mapper_file(&path).unwrap();
        assert!(super::lock_mapper_file(&path).is_err());
        drop(first);
        assert!(super::lock_mapper_file(&path).is_ok());
    }
}
