//! Headless end-to-end mapper tests: a synthetic uinput keyboard feeds the real
//! evdev → engine → uinput path and the mapper's virtual keyboard is read back.
//!
//! They need `/dev/uinput` and readable `/dev/input/event*`, so they are ignored
//! by default: `cargo test -p lhc-core --test mapper_e2e -- --ignored`.
//! Both devices are grabbed, so no test keystroke reaches the desktop session.
#![cfg(target_os = "linux")]

use evdev::{
    AttributeSet, Device, EventType, InputEvent, KeyCode, KeyEvent, uinput::VirtualDevice,
};
use lhc_core::mapper::runtime;
use serde_json::json;
use std::{
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime},
};

const MAPPER_DEVICE_NAME: &str = "LeftHandControl Virtual Keyboard";

/// The mapper runtime is process-global: scenarios run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());
static RUNTIME_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();

/// Serialize a scenario. The first call moves the cross-process mapper lock
/// to a private runtime directory, so a mapper of a running desktop instance
/// does not block the test (its grabbed keyboard is not ours anyway).
fn serial() -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    RUNTIME_DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY: every scenario holds SERIAL before it touches the mapper,
        // so no other test thread reads the environment concurrently.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", dir.path()) };
        dir
    });
    guard
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    code: KeyCode,
    down: bool,
    time: SystemTime,
}

fn down(code: KeyCode) -> (KeyCode, bool) {
    (code, true)
}

fn up(code: KeyCode) -> (KeyCode, bool) {
    (code, false)
}

fn wait_until<T>(what: &str, timeout: Duration, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

/// A synthetic physical keyboard the mapper grabs.
struct Keyboard {
    device: VirtualDevice,
    path: String,
}

impl Keyboard {
    fn new() -> Self {
        Self::named("LHC e2e physical keyboard")
    }

    fn named(name: &str) -> Self {
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in 1..=127 {
            keys.insert(KeyCode::new(code));
        }
        let mut device = VirtualDevice::builder()
            .expect("open /dev/uinput")
            .name(name)
            .with_keys(&keys)
            .unwrap()
            .build()
            .unwrap();
        let path = wait_until("synthetic keyboard node", Duration::from_secs(2), || {
            device
                .enumerate_dev_nodes_blocking()
                .ok()?
                .filter_map(Result::ok)
                .find(|path| Device::open(path).is_ok())
        });
        Self {
            device,
            path: path.to_string_lossy().into_owned(),
        }
    }

    fn send(&mut self, code: KeyCode, down: bool) {
        self.device
            .emit(&[*KeyEvent::new(code, i32::from(down))])
            .unwrap();
        // Separate reports give the engine distinct timestamps.
        thread::sleep(Duration::from_millis(5));
    }

    fn tap(&mut self, code: KeyCode) {
        self.send(code, true);
        self.send(code, false);
    }
}

/// The running mapper and a grabbed reader of its virtual keyboard.
struct Mapper {
    output: Option<Device>,
}

impl Mapper {
    fn start(keyboard: &Keyboard, config: serde_json::Value) -> Self {
        // A running desktop instance has a virtual keyboard with the same
        // name; only the node that appears now belongs to this mapper.
        let existing: Vec<_> = mapper_devices().map(|(path, _)| path).collect();
        runtime::start(
            &keyboard.path,
            None,
            serde_json::from_value(config).unwrap(),
        )
        .expect("start mapper");
        // Created before any wait: a failed wait still stops the mapper, so
        // the next scenario does not fail on a mapper left running.
        let mut mapper = Self { output: None };
        mapper.attach(&existing);
        mapper
    }

    /// Grab the mapper virtual keyboard that is not in `existing`.
    fn attach(&mut self, existing: &[std::path::PathBuf]) {
        let mut output = wait_until("mapper virtual keyboard", Duration::from_secs(5), || {
            mapper_devices()
                .find(|(path, _)| !existing.contains(path))
                .map(|(_, device)| device)
        });
        output.grab().expect("grab mapper virtual keyboard");
        output.set_nonblocking(true).unwrap();
        self.output = Some(output);
    }

    /// Key events until the output is quiet for `quiet` or `limit` elapses.
    fn read(&mut self, quiet: Duration, limit: Duration) -> Vec<Key> {
        let deadline = Instant::now() + limit;
        let mut last = Instant::now();
        let mut keys = Vec::new();
        let output = self.output.as_mut().expect("output reader was taken");
        while Instant::now() < deadline && last.elapsed() < quiet {
            match output.fetch_events() {
                Ok(events) => {
                    for event in events.filter(is_key_change) {
                        keys.push(Key {
                            code: KeyCode::new(event.code()),
                            down: event.value() == 1,
                            time: event.timestamp(),
                        });
                        last = Instant::now();
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                // The mapper destroyed its device: nothing more can arrive.
                Err(_) => break,
            }
            thread::sleep(Duration::from_millis(2));
        }
        keys
    }

    /// Wait for `count` key events however long the output holds them back,
    /// then make sure nothing else follows.
    fn wait_keys(&mut self, count: usize) -> Vec<(KeyCode, bool)> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut keys = Vec::new();
        while keys.len() < count && Instant::now() < deadline {
            keys.extend(self.keys());
        }
        keys.extend(self.keys());
        keys
    }

    fn keys(&mut self) -> Vec<(KeyCode, bool)> {
        self.read(Duration::from_millis(150), Duration::from_secs(3))
            .into_iter()
            .map(|key| (key.code, key.down))
            .collect()
    }
}

impl Drop for Mapper {
    fn drop(&mut self) {
        let _ = runtime::stop();
    }
}

fn mapper_devices() -> impl Iterator<Item = (std::path::PathBuf, Device)> {
    evdev::enumerate().filter(|(_, device)| device.name() == Some(MAPPER_DEVICE_NAME))
}

fn is_key_change(event: &InputEvent) -> bool {
    event.event_type() == EventType::KEY && event.value() != 2
}

/// Start the reader before triggering teardown and bound its lifetime even if
/// a regression leaves the virtual device connected.
fn read_until_disconnected(mut output: Device) -> thread::JoinHandle<Vec<(KeyCode, bool)>> {
    use std::os::fd::AsRawFd;
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
    output.set_nonblocking(true).unwrap();
    let reader = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut keys = Vec::new();
        ready_tx.send(()).unwrap();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for output device disconnect"
            );
            let mut fd = libc::pollfd {
                fd: output.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: fd points to one valid pollfd and output owns its descriptor.
            let result = unsafe { libc::poll(&mut fd, 1, remaining.as_millis() as i32) };
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                panic!("poll output device: {error}");
            }
            match output.fetch_events() {
                Ok(events) => keys.extend(
                    events
                        .filter(is_key_change)
                        .map(|event| (KeyCode::new(event.code()), event.value() == 1)),
                ),
                Err(error) if error.raw_os_error() == Some(libc::ENODEV) => return keys,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("read output device: {error}"),
            }
        }
    });
    ready_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("output reader ready");
    reader
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_passthrough_keeps_unmapped_keys() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(&keyboard, json!({}));
    keyboard.tap(KeyCode::KEY_A);
    keyboard.tap(KeyCode::KEY_S);
    assert_eq!(
        mapper.keys(),
        [
            down(KeyCode::KEY_A),
            up(KeyCode::KEY_A),
            down(KeyCode::KEY_S),
            up(KeyCode::KEY_S)
        ]
    );
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_emits_a_chord() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(
        &keyboard,
        json!({"rules": [{"key": "KeyA", "tapAction": "Ctrl+KeyC"}]}),
    );
    keyboard.tap(KeyCode::KEY_A);
    assert_eq!(
        mapper.keys(),
        [
            down(KeyCode::KEY_LEFTCTRL),
            down(KeyCode::KEY_C),
            up(KeyCode::KEY_C),
            up(KeyCode::KEY_LEFTCTRL)
        ]
    );
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_macro_pause_delays_the_next_step() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(
        &keyboard,
        json!({
            "rules": [{"key": "KeyM", "tapAction": "macro:hi"}],
            "macros": [{"id": "hi", "steps": [
                {"action": "KeyH"}, {"action": "pause:200"}, {"action": "KeyI"}
            ]}],
        }),
    );
    keyboard.tap(KeyCode::KEY_M);
    let keys = mapper.read(Duration::from_millis(400), Duration::from_secs(3));
    let codes: Vec<_> = keys.iter().map(|key| (key.code, key.down)).collect();
    assert_eq!(
        codes,
        [
            down(KeyCode::KEY_H),
            up(KeyCode::KEY_H),
            down(KeyCode::KEY_I),
            up(KeyCode::KEY_I)
        ]
    );
    let pause = keys[2].time.duration_since(keys[1].time).unwrap();
    assert!(pause >= Duration::from_millis(190), "pause was {pause:?}");
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_stop_releases_held_keys() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(&keyboard, json!({}));
    keyboard.send(KeyCode::KEY_LEFTSHIFT, true);
    keyboard.send(KeyCode::KEY_A, true);
    assert_eq!(
        mapper.keys(),
        [down(KeyCode::KEY_LEFTSHIFT), down(KeyCode::KEY_A)]
    );
    let reader = read_until_disconnected(mapper.output.take().unwrap());
    runtime::stop().unwrap();
    let mut released = reader.join().unwrap();
    released.sort_by_key(|(code, _)| code.code());
    assert_eq!(released, [up(KeyCode::KEY_A), up(KeyCode::KEY_LEFTSHIFT)]);
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_unplugged_keyboard_waits_until_explicit_stop() {
    let _serial = serial();
    let keyboard = Keyboard::new();
    let _mapper = Mapper::start(&keyboard, json!({}));
    assert!(runtime::status().running);
    assert!(!runtime::status().waiting_for_device);
    drop(keyboard);
    wait_until(
        "waiting for keyboard status",
        Duration::from_secs(2),
        || runtime::status().waiting_for_device.then_some(()),
    );
    assert!(runtime::status().running);
    assert!(runtime::execute_action("KeyA".into()).is_err());
    let before = Instant::now();
    runtime::stop().unwrap();
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "stop interrupts the reconnect wait"
    );
    assert!(!runtime::status().running);
    assert!(!runtime::status().waiting_for_device);
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_unplug_releases_held_keys() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(&keyboard, json!({}));
    keyboard.send(KeyCode::KEY_LEFTCTRL, true);
    assert_eq!(mapper.keys(), [down(KeyCode::KEY_LEFTCTRL)]);
    let reader = read_until_disconnected(mapper.output.take().unwrap());
    drop(keyboard);
    assert_eq!(reader.join().unwrap(), [up(KeyCode::KEY_LEFTCTRL)]);
    assert!(
        runtime::status().running,
        "reconnect keeps the mapper enabled"
    );
}

/// A symlinked keyboard path that is unplugged and replugged.
struct LinkedKeyboard {
    _dir: tempfile::TempDir,
    link: std::path::PathBuf,
}

impl LinkedKeyboard {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("keyboard");
        Self { _dir: dir, link }
    }

    fn plug(&self, name: &str) -> Keyboard {
        let _ = std::fs::remove_file(&self.link);
        let mut keyboard = Keyboard::named(name);
        std::os::unix::fs::symlink(&keyboard.path, &self.link).unwrap();
        keyboard.path = self.link.to_string_lossy().into_owned();
        keyboard
    }
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_replug_ignores_unrelated_device_and_uses_latest_config() {
    let _serial = serial();
    let existing: Vec<_> = mapper_devices().map(|(path, _)| path).collect();
    let linked = LinkedKeyboard::new();
    let keyboard = linked.plug("LHC e2e physical keyboard");
    let mut mapper = Mapper::start(&keyboard, json!({}));
    mapper.output = None;
    drop(keyboard);
    wait_until("disconnected mapper", Duration::from_secs(2), || {
        runtime::status().waiting_for_device.then_some(())
    });
    assert!(runtime::execute_action("KeyD".into()).is_err());
    for target in ["KeyB", "KeyD", "KeyC"] {
        runtime::update_config(
            serde_json::from_value(json!({"rules": [{"key": "KeyA", "tapAction": target}]}))
                .unwrap(),
        )
        .unwrap();
    }

    let other = linked.plug("LHC e2e unrelated keyboard");
    thread::sleep(Duration::from_millis(300));
    assert!(
        mapper_devices().all(|(path, _)| existing.contains(&path)),
        "unrelated device must not be grabbed"
    );
    drop(other);

    let mut replacement = linked.plug("LHC e2e physical keyboard");
    mapper.attach(&existing);
    replacement.tap(KeyCode::KEY_A);
    assert_eq!(mapper.keys(), [down(KeyCode::KEY_C), up(KeyCode::KEY_C)]);
    assert!(!runtime::status().waiting_for_device);
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_popup_hotkey_is_consumed_before_compositor() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(&keyboard, json!({}));
    keyboard.send(KeyCode::KEY_LEFTCTRL, true);
    keyboard.send(KeyCode::KEY_LEFTALT, true);
    keyboard.tap(KeyCode::KEY_F11);
    keyboard.send(KeyCode::KEY_LEFTALT, false);
    keyboard.send(KeyCode::KEY_LEFTCTRL, false);
    // No shell acknowledges popup focus here: the modifiers come out only
    // when the output's popup hold expires, so wait for them by count.
    assert_eq!(
        mapper.wait_keys(4),
        [
            down(KeyCode::KEY_LEFTCTRL),
            down(KeyCode::KEY_LEFTALT),
            up(KeyCode::KEY_LEFTALT),
            up(KeyCode::KEY_LEFTCTRL),
        ]
    );
}
