//! Windows mapper backend: session-wide low-level hooks feed the shared
//! engine, and its output is injected with `SendInput`.
//!
//! Windows has no per-device interception without a driver, so the mapper
//! handles every physical keyboard of the session; it is offered as the
//! single device [`KEYBOARD_PATH`]. Input to elevated windows and the
//! secure desktop (UAC, Ctrl+Alt+Del, lock screen) bypasses the hooks, and
//! injection into elevated windows is blocked by UIPI.
//! Executables call [`dispatch_input_guard`] at entry to enable a separate
//! process that releases journaled native keys if their mapper owner dies.

mod guard;
mod hook;
mod input;
pub mod system;

pub use guard::{dispatch as dispatch_input_guard, pid as input_guard_pid};
pub use input::type_unicode;

use super::config::AppConfig;
use super::engine::{Engine, Out};
use super::key_code::KeyCode;
use super::output::{
    EventSink, LoopDriver, OutputHold, SideEffects, flush_out_with, process_iteration_with,
};
use super::system::{SysAction, SysCommand};
use super::text_job::TextRequest;
use super::{InputDevice, KeyboardDevice};
use crate::events::{CoreEvent, TextInputFailure};
use hook::{HookEvent, Hooks};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// Device path of "every keyboard of this session".
pub const KEYBOARD_PATH: &str = "windows:keyboard";
/// Device path of "every mouse of this session" (buttons only).
pub const MOUSE_PATH: &str = "windows:mouse";

pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
    Ok(vec![
        InputDevice {
            path: KEYBOARD_PATH.into(),
            name: KEYBOARD_PATH.into(),
            is_keyboard: true,
            is_mouse: false,
            is_virtual: false,
        },
        InputDevice {
            path: MOUSE_PATH.into(),
            name: MOUSE_PATH.into(),
            is_keyboard: false,
            is_mouse: true,
            is_virtual: false,
        },
    ])
}

pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    Ok(vec![KeyboardDevice {
        path: KEYBOARD_PATH.into(),
        name: KEYBOARD_PATH.into(),
    }])
}

pub fn list_mice() -> Result<Vec<KeyboardDevice>, String> {
    Ok(vec![KeyboardDevice {
        path: MOUSE_PATH.into(),
        name: MOUSE_PATH.into(),
    }])
}

enum MapperControl {
    Config(Box<AppConfig>),
    Execute(String),
    /// A popup took the keyboard focus; emit the output held for it.
    PopupFocused,
}

/// Handle to a running mapper thread.
pub struct Handle {
    stop: Arc<AtomicBool>,
    control_tx: mpsc::Sender<MapperControl>,
    wake_tx: mpsc::Sender<HookEvent>,
    join: Option<thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}

impl Handle {
    fn send(&self, control: MapperControl) -> Result<(), String> {
        self.control_tx
            .send(control)
            .map_err(|e| format!("mapper update channel closed: {e}"))?;
        hook::wake(&self.wake_tx);
        Ok(())
    }

    pub fn stop(mut self) {
        super::text_job::cancel();
        self.stop.store(true, Ordering::SeqCst);
        hook::wake(&self.wake_tx);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }

    pub fn last_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|g| g.clone())
    }

    pub fn update_config(&self, cfg: AppConfig) -> Result<(), String> {
        self.send(MapperControl::Config(Box::new(cfg)))
    }

    pub fn execute_action(&self, action: String) -> Result<(), String> {
        self.send(MapperControl::Execute(action))
    }

    pub fn popup_focused(&self) -> Result<(), String> {
        self.send(MapperControl::PopupFocused)
    }

    pub fn reap_if_finished(&mut self) -> bool {
        if self.join.as_ref().is_some_and(|join| !join.is_finished()) {
            return false;
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        true
    }
}

const START_TIMEOUT: Duration = Duration::from_secs(5);

pub fn spawn(
    device_path: String,
    mouse_path: Option<String>,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
) -> Result<Handle, String> {
    if device_path != KEYBOARD_PATH {
        return Err(format!("Unknown keyboard {device_path}"));
    }
    let mouse = match mouse_path.as_deref() {
        None => false,
        Some(MOUSE_PATH) => true,
        Some(other) => return Err(format!("Unknown mouse {other}")),
    };
    super::text_job::start();
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    let (control_tx, control_rx) = mpsc::channel::<MapperControl>();
    let (events_tx, events_rx) = mpsc::channel::<HookEvent>();
    let wake_tx = events_tx.clone();
    let (stop_thread, err_thread) = (stop.clone(), error.clone());
    let join = thread::Builder::new()
        .name("lhc-mapper".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(
                    mouse,
                    cfg,
                    stop_thread.clone(),
                    ready_tx,
                    control_rx,
                    events_tx,
                    events_rx,
                )
            }));
            if stop_thread.load(Ordering::SeqCst) {
                return;
            }
            let message = match result {
                Ok(Ok(())) => return,
                Ok(Err(error)) => error,
                Err(panic) => panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "mapper thread panicked".into()),
            };
            log::debug!("[mapper] {message}");
            if let Ok(mut slot) = err_thread.lock() {
                *slot = Some(message.clone());
            }
            super::notify_mapper_stopped(&message);
        })
        .map_err(|e| format!("spawn thread: {e}"))?;

    match ready_rx.recv_timeout(START_TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = join.join();
            return Err(error);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            stop.store(true, Ordering::SeqCst);
            hook::wake(&wake_tx);
            drop(join);
            return Err("mapper start timed out".into());
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = join.join();
            return Err(error
                .lock()
                .ok()
                .and_then(|slot| slot.clone())
                .unwrap_or_else(|| "mapper worker exited before reporting readiness".into()));
        }
    }
    Ok(Handle {
        stop,
        control_tx,
        wake_tx,
        join: Some(join),
        error,
    })
}

/// Named mutex held while the mapper runs, so only one process of the
/// session installs hooks.
struct MapperLock(HANDLE);

/// Session-local name of the mapper lock mutex.
const MAPPER_LOCK_NAME: windows::core::PCWSTR = w!("Local\\LeftHandControlMapper");

impl MapperLock {
    fn join_existing() -> Result<Self, String> {
        let handle = unsafe { CreateMutexW(None, false, MAPPER_LOCK_NAME) }
            .map_err(|error| format!("open mapper lock for input guard: {error}"))?;
        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            let _ = unsafe { CloseHandle(handle) };
            return Err("input guard owner has no mapper lock".into());
        }
        Ok(Self(handle))
    }

    fn acquire() -> Result<Self, String> {
        let handle = unsafe { CreateMutexW(None, false, MAPPER_LOCK_NAME) }
            .map_err(|error| format!("create mapper lock: {error}"))?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let _ = unsafe { CloseHandle(handle) };
            return Err("mapper is already active in another process".into());
        }
        Ok(Self(handle))
    }
}

impl Drop for MapperLock {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn run(
    mouse: bool,
    cfg: AppConfig,
    stop: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<Result<(), String>>,
    control_rx: mpsc::Receiver<MapperControl>,
    events_tx: mpsc::Sender<HookEvent>,
    events_rx: mpsc::Receiver<HookEvent>,
) -> Result<(), String> {
    if stop.load(Ordering::SeqCst) {
        return Err("mapper start cancelled".into());
    }
    let _lock = MapperLock::acquire()?;
    let _guard = guard::Session::start()?;
    // Declared after the lock so the hooks are removed first on return.
    let hooks = Hooks::install(events_tx, mouse)?;
    let _ = ready_tx.send(Ok(()));
    let result = run_loop(HookDriver::new(events_rx), cfg, &stop, control_rx);
    drop(hooks);
    result
}

fn run_loop(
    mut driver: HookDriver,
    cfg: AppConfig,
    stop: &AtomicBool,
    control_rx: mpsc::Receiver<MapperControl>,
) -> Result<(), String> {
    let mut cfg = cfg;
    let mut engine = Engine::new(&cfg);
    let mut out_buf: Vec<Out> = Vec::with_capacity(16);
    let mut hold = OutputHold::default();
    let mut sink = InjectSink;
    let mut effects = RuntimeSideEffects;
    let result = (|| {
        while !stop.load(Ordering::SeqCst) {
            if guard::failed() {
                return Err("input guard is unavailable; mapper stopped".into());
            }
            while let Ok(control) = control_rx.try_recv() {
                match control {
                    MapperControl::Config(next) => {
                        let next = *next;
                        if next == cfg {
                            continue;
                        }
                        hold.cancel_actions(&mut out_buf);
                        super::text_job::cancel();
                        engine.reconfigure(&next, &mut out_buf);
                        super::layer_state::publish(engine.held_layer_state());
                        super::text_job::start();
                        cfg = next;
                    }
                    MapperControl::Execute(action) => engine.execute_remote(&action, &mut out_buf),
                    MapperControl::PopupFocused => hold.release(),
                }
            }
            hold.gate(&mut out_buf, Instant::now(), super::text_job::pending());
            flush_out_with(&mut sink, &mut effects, &mut out_buf)?;
            process_iteration_with(
                &mut driver,
                &mut engine,
                &mut hold,
                &mut sink,
                &mut effects,
                &mut out_buf,
            )?;
        }
        Ok(())
    })();
    super::text_job::cancel();
    super::layer_state::publish(super::layer_state::HeldLayerState::default());
    hold.discard();
    engine.shutdown(&mut out_buf);
    let _ = flush_out_with(&mut sink, &mut effects, &mut out_buf);
    result
}

/// Hook events as a [`LoopDriver`].
struct HookDriver {
    rx: mpsc::Receiver<HookEvent>,
    keys: VecDeque<(KeyCode, bool, Instant)>,
    repeats: Vec<KeyCode>,
}

impl HookDriver {
    fn new(rx: mpsc::Receiver<HookEvent>) -> Self {
        Self {
            rx,
            keys: VecDeque::new(),
            repeats: Vec::new(),
        }
    }

    fn accept(&mut self, event: HookEvent) {
        match event {
            HookEvent::Key { key, down, at } => self.keys.push_back((key, down, at)),
            HookEvent::Repeat(key) => self.repeats.push(key),
            HookEvent::Wake => {}
        }
    }

    fn has_input(&self) -> bool {
        !self.keys.is_empty() || !self.repeats.is_empty()
    }
}

impl LoopDriver for HookDriver {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn wait(&mut self, timeout: Duration) -> Result<bool, String> {
        if !self.has_input() {
            match self.rx.recv_timeout(timeout) {
                Ok(event) => self.accept(event),
                Err(mpsc::RecvTimeoutError::Timeout) => return Ok(false),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("keyboard hook stopped".into());
                }
            }
        }
        while let Ok(event) = self.rx.try_recv() {
            self.accept(event);
        }
        Ok(self.has_input())
    }

    fn fetch_key_events(&mut self) -> Result<Vec<(KeyCode, bool)>, String> {
        Ok(self
            .keys
            .drain(..)
            .map(|(key, down, _)| (key, down))
            .collect())
    }

    fn fetch_timed_key_events(&mut self) -> Result<Vec<(KeyCode, bool, Instant)>, String> {
        Ok(self.keys.drain(..).collect())
    }

    fn take_repeats(&mut self) -> Vec<KeyCode> {
        std::mem::take(&mut self.repeats)
    }
}

/// Injects key transitions with `SendInput`.
struct InjectSink;

impl EventSink for InjectSink {
    fn emit(&mut self, events: &[(KeyCode, bool)]) -> Result<(), String> {
        let inputs: Vec<_> = events
            .iter()
            .filter_map(|&(key, down)| {
                let event = input::key(key, down);
                if event.is_none() {
                    log::debug!("[mapper] {key:?} has no Windows key; dropped");
                }
                event
            })
            .collect();
        // A blocked injection (elevated foreground window, secure desktop)
        // is not fatal: report it and keep the mapper running.
        if let Err(error) = input::send(&inputs) {
            report_action_failure(error);
        }
        Ok(())
    }
}

struct RuntimeSideEffects;

impl SideEffects for RuntimeSideEffects {
    fn held_layer_changed(&mut self, state: super::layer_state::HeldLayerState) {
        super::layer_state::publish(state);
    }

    fn sleep(&mut self, duration: Duration) {
        if !duration.is_zero() {
            thread::sleep(duration);
        }
    }

    fn type_text(&mut self, text: &str) {
        match TextRequest::enqueue(text) {
            Ok(request) => {
                if text_tx()
                    .try_send((text.to_owned(), request.clone()))
                    .is_err()
                {
                    request.complete(true);
                    report_text(TextInputFailure::BackendUnavailable(
                        "text input queue is unavailable or full".into(),
                    ));
                }
            }
            Err(error) => report_text(error),
        }
    }

    fn text_pending(&self) -> bool {
        super::text_job::pending()
    }

    fn text_failed(&mut self) -> bool {
        super::text_job::take_failure()
    }

    fn run_system(&mut self, action: &SysAction) {
        match action {
            SysAction::Windows(action) => {
                let result = system::run(*action);
                if matches!(action, system::WindowsAction::SwitchLayout(_)) && result.is_ok() {
                    super::refresh_layout();
                }
                if let Err(error) = result {
                    report_action_failure(error);
                }
            }
            SysAction::Spawn(command) => super::commands::spawn(command.clone()),
            SysAction::AppEvent(event) => super::emit_app_event(event),
        }
    }

    fn run_command(&mut self, command: &SysCommand) {
        super::commands::spawn(command.clone());
    }
}

fn report_action_failure(error: String) {
    crate::events::emit(CoreEvent::MapperActionFailed(error));
}

fn report_text(failure: TextInputFailure) {
    crate::events::emit(CoreEvent::TextInputFailed(failure));
}

/// Text is typed on its own thread: the mapper thread keeps reading the
/// hooks while a long text is injected.
/// Texts queued for typing before senders drop new ones.
const TEXT_QUEUE: usize = 32;

fn text_tx() -> &'static mpsc::SyncSender<(String, TextRequest)> {
    static TX: OnceLock<mpsc::SyncSender<(String, TextRequest)>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<(String, TextRequest)>(TEXT_QUEUE);
        if let Err(error) = thread::Builder::new()
            .name("lhc-text".into())
            .spawn(move || {
                for (text, mut request) in rx {
                    let result = request.wait_for_target().and_then(|()| {
                        input::type_unicode(&text, || request.check_progress().is_ok())
                            .map_err(TextInputFailure::BackendUnavailable)
                    });
                    request.complete(result.is_err());
                    if let Err(failure) = result {
                        report_text(failure);
                    }
                }
            })
        {
            log::debug!("[mapper] could not spawn text worker: {error}");
        }
        tx
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_driver_separates_transitions_repeats_and_wakes() {
        let (tx, rx) = mpsc::channel();
        let mut driver = HookDriver::new(rx);
        let at = Instant::now();
        tx.send(HookEvent::Wake).unwrap();
        assert!(!driver.wait(Duration::ZERO).unwrap());
        tx.send(HookEvent::Key {
            key: KeyCode::KEY_A,
            down: true,
            at,
        })
        .unwrap();
        tx.send(HookEvent::Repeat(KeyCode::KEY_A)).unwrap();
        assert!(driver.wait(Duration::from_millis(10)).unwrap());
        assert_eq!(
            driver.fetch_timed_key_events().unwrap(),
            vec![(KeyCode::KEY_A, true, at)]
        );
        assert_eq!(driver.take_repeats(), vec![KeyCode::KEY_A]);
        drop(tx);
        assert!(driver.wait(Duration::ZERO).is_err());
    }

    #[test]
    fn devices_are_the_whole_session() {
        assert_eq!(list_keyboards().unwrap()[0].path, KEYBOARD_PATH);
        assert_eq!(list_mice().unwrap()[0].path, MOUSE_PATH);
        assert!(spawn("x".into(), None, empty(), Arc::default()).is_err());
    }

    fn empty() -> AppConfig {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }
}
