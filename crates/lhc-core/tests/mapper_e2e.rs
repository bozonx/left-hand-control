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
        runtime::start(&keyboard.path, None, &config.to_string()).expect("start mapper");
        let mut output = wait_until("mapper virtual keyboard", Duration::from_secs(2), || {
            mapper_devices()
                .find(|(path, _)| !existing.contains(path))
                .map(|(_, device)| device)
        });
        output.grab().expect("grab mapper virtual keyboard");
        output.set_nonblocking(true).unwrap();
        Self {
            output: Some(output),
        }
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
fn mapper_e2e_remaps_a_key() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(
        &keyboard,
        json!({"rules": [{"key": "KeyA", "tapAction": "KeyB"}]}),
    );
    keyboard.tap(KeyCode::KEY_A);
    assert_eq!(mapper.keys(), [down(KeyCode::KEY_B), up(KeyCode::KEY_B)]);
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
fn mapper_e2e_tap_and_hold_choose_different_actions() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(
        &keyboard,
        json!({
            "rules": [{"key": "CapsLock", "tapAction": "Escape", "holdAction": "ControlLeft",
                       "holdTimeoutMs": 150}],
        }),
    );
    keyboard.tap(KeyCode::KEY_CAPSLOCK);
    assert_eq!(
        mapper.keys(),
        [down(KeyCode::KEY_ESC), up(KeyCode::KEY_ESC)]
    );

    keyboard.send(KeyCode::KEY_CAPSLOCK, true);
    thread::sleep(Duration::from_millis(300));
    keyboard.tap(KeyCode::KEY_X);
    keyboard.send(KeyCode::KEY_CAPSLOCK, false);
    assert_eq!(
        mapper.keys(),
        [
            down(KeyCode::KEY_LEFTCTRL),
            down(KeyCode::KEY_X),
            up(KeyCode::KEY_X),
            up(KeyCode::KEY_LEFTCTRL)
        ]
    );
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_layer_maps_keys_while_held() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(
        &keyboard,
        json!({
            "rules": [{"key": "Tab", "layerId": "nav"}],
            "layerKeymaps": {"nav": {"keys": {"KeyJ": "ArrowDown"}}},
            "settings": {"tapDecision": "holdOnOtherKeyPress"},
        }),
    );
    keyboard.send(KeyCode::KEY_TAB, true);
    keyboard.tap(KeyCode::KEY_J);
    keyboard.send(KeyCode::KEY_TAB, false);
    keyboard.tap(KeyCode::KEY_J);
    assert_eq!(
        mapper.keys(),
        [
            down(KeyCode::KEY_DOWN),
            up(KeyCode::KEY_DOWN),
            down(KeyCode::KEY_J),
            up(KeyCode::KEY_J)
        ]
    );
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_double_tap_runs_its_own_action() {
    let _serial = serial();
    let mut keyboard = Keyboard::new();
    let mut mapper = Mapper::start(
        &keyboard,
        json!({
            "rules": [{"key": "KeyD", "doubleTapAction": "KeyX", "doubleTapTimeoutMs": 200}],
        }),
    );
    keyboard.tap(KeyCode::KEY_D);
    keyboard.tap(KeyCode::KEY_D);
    assert_eq!(mapper.keys(), [down(KeyCode::KEY_X), up(KeyCode::KEY_X)]);

    keyboard.tap(KeyCode::KEY_D);
    assert_eq!(
        mapper
            .read(Duration::from_millis(400), Duration::from_secs(3))
            .into_iter()
            .map(|key| (key.code, key.down))
            .collect::<Vec<_>>(),
        [down(KeyCode::KEY_D), up(KeyCode::KEY_D)]
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
    // The mapper destroys its device right after emitting the releases, and
    // unread events go with it: a blocking reader must already be waiting.
    let mut output = mapper.output.take().unwrap();
    output.set_nonblocking(false).unwrap();
    let reader = thread::spawn(move || {
        let mut keys = Vec::new();
        while let Ok(events) = output.fetch_events() {
            keys.extend(
                events
                    .filter(is_key_change)
                    .map(|event| (KeyCode::new(event.code()), event.value() == 1)),
            );
        }
        keys
    });
    thread::sleep(Duration::from_millis(50));
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
    drop(keyboard);
    thread::sleep(Duration::from_millis(300));
    assert!(runtime::status().running);
    runtime::stop().unwrap();
    assert!(!runtime::status().running);
}

#[test]
#[ignore = "requires /dev/uinput and readable /dev/input"]
fn mapper_e2e_replug_recovers_latest_config_and_releases_held_keys() {
    let _serial = serial();
    let existing: Vec<_> = mapper_devices().map(|(path, _)| path).collect();
    let temp = tempfile::tempdir().unwrap();
    let link = temp.path().join("keyboard");
    let mut keyboard = Keyboard::new();
    std::os::unix::fs::symlink(&keyboard.path, &link).unwrap();
    keyboard.path = link.to_string_lossy().into_owned();
    let mut mapper = Mapper::start(&keyboard, json!({}));
    runtime::update_config(&json!({"rules": [{"key": "KeyA", "tapAction": "KeyB"}]}).to_string())
        .unwrap();
    keyboard.send(KeyCode::KEY_LEFTCTRL, true);
    assert_eq!(mapper.keys(), [down(KeyCode::KEY_LEFTCTRL)]);
    // Read releases before the disconnected virtual device is destroyed.
    let mut output = mapper.output.take().unwrap();
    output.set_nonblocking(false).unwrap();
    let reader = thread::spawn(move || {
        let mut released = Vec::new();
        while let Ok(events) = output.fetch_events() {
            released.extend(
                events
                    .filter(is_key_change)
                    .map(|event| (KeyCode::new(event.code()), event.value() == 1)),
            );
        }
        released
    });
    thread::sleep(Duration::from_millis(50));
    drop(keyboard);
    assert_eq!(reader.join().unwrap(), [up(KeyCode::KEY_LEFTCTRL)]);
    assert!(
        runtime::status().running,
        "reconnect keeps the mapper enabled"
    );
    std::fs::remove_file(&link).unwrap();
    let other = Keyboard::named("LHC e2e unrelated keyboard");
    std::os::unix::fs::symlink(&other.path, &link).unwrap();
    thread::sleep(Duration::from_millis(300));
    assert!(
        mapper_devices().all(|(path, _)| existing.contains(&path)),
        "unrelated device must not be grabbed"
    );
    drop(other);
    std::fs::remove_file(&link).unwrap();
    let mut replacement = Keyboard::new();
    std::os::unix::fs::symlink(&replacement.path, &link).unwrap();
    let mut output = wait_until("reconnected mapper output", Duration::from_secs(5), || {
        mapper_devices()
            .find(|(path, _)| !existing.contains(path))
            .map(|(_, device)| device)
    });
    output.grab().unwrap();
    output.set_nonblocking(true).unwrap();
    mapper.output = Some(output);
    replacement.tap(KeyCode::KEY_A);
    assert_eq!(mapper.keys(), [down(KeyCode::KEY_B), up(KeyCode::KEY_B)]);
    drop(replacement);
    thread::sleep(Duration::from_millis(200));
    let before = Instant::now();
    runtime::stop().unwrap();
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "stop interrupts reconnect"
    );
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
    // No shell acknowledges popup focus in this test, so wait for the hold gate.
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(
        mapper.keys(),
        [
            down(KeyCode::KEY_LEFTCTRL),
            down(KeyCode::KEY_LEFTALT),
            up(KeyCode::KEY_LEFTALT),
            up(KeyCode::KEY_LEFTCTRL),
        ]
    );
}
