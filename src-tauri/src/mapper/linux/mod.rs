mod devices;
mod io;

pub use devices::{list_keyboards, list_mice};

use super::config::AppConfig;
use super::engine::{Engine, Out};
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, BusType, Device, InputId, Key};
use io::{flush_out, process_iteration, LoopDriver, MultiDeviceLoopDriver};
#[cfg(test)]
use io::{flush_out_with, process_iteration_with, EventSink, SideEffects};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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

pub fn spawn(
    device_path: String,
    mouse_path: Option<String>,
    cfg: AppConfig,
) -> Result<Handle, String> {
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "[mapper] started: device={} mouse={:?} rules={} layers={}",
            device_path,
            mouse_path,
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
            if let Err(e) = run(device_path, mouse_path, cfg, stop_thread, ready_tx) {
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
    mouse_path: Option<String>,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let mut device = Device::open(&device_path).map_err(|e| format!("open {device_path}: {e}"))?;
    device
        .grab()
        .map_err(|e| format!("grab {device_path} (need perms? add user to input group): {e}"))?;

    let mut devices = vec![device];
    if let Some(ref mp) = mouse_path {
        let mouse = Device::open(mp).map_err(|e| format!("open mouse {mp}: {e}"))?;
        // Mouse is opened without grab: only KEY events are read to interact
        // with modifier tap-hold decisions. REL/ABS (movement) stays
        // untouched so the cursor continues to work.
        devices.push(mouse);
    }

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

    let driver = MultiDeviceLoopDriver::new(devices)?;
    let _ = ready_tx.send(Ok(()));
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

    while !stop.load(Ordering::SeqCst) {
        process_iteration(&mut driver, &mut engine, &mut virt, &mut out_buf)?;
    }

    engine.shutdown(&mut out_buf);
    flush_out(&mut virt, &mut out_buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{flush_out_with, process_iteration_with, EventSink, LoopDriver, SideEffects};
    use crate::mapper::action::{Keystroke, MacroStepItem};
    use crate::mapper::config::{ActionSpec, AppConfig, Rule, Settings};
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
        commands: Vec<String>,
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

        fn run_command(&mut self, command: &SysCommand) {
            self.commands
                .push(format!("cmd:{}:{:?}", command.program, command.args));
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
            commands: Vec::new(),
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
                    MacroStepItem::Command(SysCommand {
                        program: "sh".into(),
                        args: vec!["-lc".into(), "playerctl play-pause".into()],
                    }),
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
            Out::RunCommand(SysCommand {
                program: "sh".into(),
                args: vec!["-lc".into(), "notify-send done".into()],
            }),
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
        assert_eq!(
            effects.commands,
            vec![
                "cmd:sh:[\"-lc\", \"playerctl play-pause\"]".to_string(),
                "cmd:sh:[\"-lc\", \"notify-send done\"]".to_string(),
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
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
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
