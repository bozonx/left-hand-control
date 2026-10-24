//! An orphaned guard survives owner-tree termination; stdin EOF releases held input.

use super::super::input_guard::{Key, finish, read_snapshots, write_snapshot};
use super::{MapperLock, input};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, SendInput,
};

/// How long the guard tries to release keys left held by a crashed mapper.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(30);
const RELEASE_POLL: Duration = Duration::from_millis(100);
/// How long the guard process may take to report that it is ready.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the guard process may take to exit after its input closes.
const EXIT_TIMEOUT: Duration = Duration::from_secs(2);
const EXIT_POLL: Duration = Duration::from_millis(10);

const ROLE: &str = "LHC_INTERNAL_WINDOWS_INPUT_GUARD";
static ENABLED: AtomicBool = AtomicBool::new(false);
static GUARD: Mutex<Option<Guard>> = Mutex::new(None);

/// Call before config, IPC or UI initialization in executables embedding the mapper.
/// Returns true in the internal guard child after cleanup; the caller should exit.
/// Calling it in the owner enables crash recovery for subsequent mapper sessions.
pub fn dispatch() -> Result<bool, String> {
    let role = std::env::var_os(ROLE);
    if role.as_deref() == Some(std::ffi::OsStr::new("launch")) {
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .env(ROLE, "1")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|e| format!("launch independent input guard: {e}"))?;
        return Ok(true);
    }
    if role.is_none() {
        ENABLED.store(true, Ordering::SeqCst);
        return Ok(false);
    }
    let _lock = MapperLock::join_existing()?;
    let mut ready = b"LHC!".to_vec();
    ready.extend(std::process::id().to_le_bytes());
    std::io::stdout()
        .write_all(&ready)
        .map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut held = BTreeSet::new();
    let journal = read_snapshots(&mut std::io::stdin().lock(), &mut held);
    let deadline = Instant::now() + RELEASE_TIMEOUT;
    while !release(&held) {
        if Instant::now() >= deadline {
            return Err("input guard could not release native keys".into());
        }
        std::thread::sleep(RELEASE_POLL);
    }
    journal.map_err(|e| format!("input guard journal: {e}"))?;
    Ok(true)
}

fn release(held: &BTreeSet<Key>) -> bool {
    if held.is_empty() {
        return true;
    }
    // Mask an otherwise bare Win/Alt release, as normal shortcut injection does.
    let mut events = vec![
        input::keyboard(0xe8, 0, KEYBD_EVENT_FLAGS(0)),
        input::keyboard(0xe8, 0, KEYEVENTF_KEYUP),
    ];
    events.extend(held.iter().map(|key| {
        input::keyboard(
            key.vk,
            key.scan,
            KEYBD_EVENT_FLAGS(key.flags) | KEYEVENTF_KEYUP,
        )
    }));
    unsafe { SendInput(&events, std::mem::size_of::<INPUT>() as i32) as usize == events.len() }
}

struct GuardProcess {
    handle: OwnedHandle,
    pid: u32,
}

impl GuardProcess {
    fn handle(&self) -> HANDLE {
        HANDLE(self.handle.as_raw_handle())
    }

    fn try_wait(&self) -> Result<Option<bool>, String> {
        match unsafe { WaitForSingleObject(self.handle(), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                unsafe { GetExitCodeProcess(self.handle(), &mut code) }
                    .map_err(|e| e.to_string())?;
                Ok(Some(code == 0))
            }
            _ => Err(std::io::Error::last_os_error().to_string()),
        }
    }

    fn kill(&self) {
        let _ = unsafe { TerminateProcess(self.handle(), 1) };
        let _ = unsafe { WaitForSingleObject(self.handle(), 2000) };
    }
}

struct Guard {
    child: GuardProcess,
    writer: Option<ChildStdin>,
    held: BTreeSet<Key>,
    failed: bool,
}

impl Guard {
    fn spawn() -> Result<Self, String> {
        let mut launcher = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .env(ROLE, "launch")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| format!("start input guard launcher: {e}"))?;
        let writer = launcher.stdin.take();
        let mut stdout = launcher
            .stdout
            .take()
            .ok_or("input guard stdout unavailable")?;
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("lhc-guard-ready".into())
            .spawn(move || {
                let mut ready = [0; 8];
                let result = stdout
                    .read_exact(&mut ready)
                    .map_err(|e| e.to_string())
                    .and_then(|()| {
                        if ready[..4] == *b"LHC!" {
                            Ok(u32::from_le_bytes([ready[4], ready[5], ready[6], ready[7]]))
                        } else {
                            Err("invalid input guard handshake".into())
                        }
                    });
                let _ = tx.send(result);
            })
            .map_err(|e| format!("start input guard handshake: {e}"))?;
        let pid = rx
            .recv_timeout(HANDSHAKE_TIMEOUT)
            .map_err(|e| format!("input guard readiness: {e}"))??;
        let handle = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                false,
                pid,
            )
        }
        .map_err(|e| format!("open independent input guard: {e}"))?;
        let child = GuardProcess {
            handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
            pid,
        };
        // No live ancestry through the launcher may remain when injection starts.
        if !launcher.wait().map_err(|e| e.to_string())?.success() {
            child.kill();
            return Err("input guard launcher failed".into());
        }
        Ok(Self {
            child,
            writer,
            held: BTreeSet::new(),
            failed: false,
        })
    }

    fn publish(&mut self) -> Result<(), String> {
        let result = self
            .writer
            .as_mut()
            .ok_or_else(|| "input guard pipe closed".to_string())
            .and_then(|writer| {
                write_snapshot(writer, &self.held).map_err(|e| format!("input guard journal: {e}"))
            });
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn before(&mut self, events: &[INPUT]) -> Result<(), String> {
        if self.failed {
            return Err("input guard is unavailable".into());
        }
        if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
            self.failed = true;
            return Err("input guard exited unexpectedly".into());
        }
        let mut changed = false;
        for event in events {
            if let Some((key, true)) = transition(event) {
                changed |= self.held.insert(key);
            }
        }
        if changed {
            self.publish()?;
        }
        Ok(())
    }

    fn after(&mut self, events: &[INPUT]) -> Result<(), String> {
        if finish(&mut self.held, events.iter().filter_map(transition)) {
            self.publish()?;
        }
        Ok(())
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.writer.take();
        let deadline = Instant::now() + EXIT_TIMEOUT;
        loop {
            match self.child.try_wait() {
                Ok(Some(true)) if !self.failed => return,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(EXIT_POLL),
                _ => break,
            }
        }
        // Also recover if the guard itself failed while the owner is still alive.
        if !release(&self.held) {
            log::error!("input guard fallback could not release keys");
        }
        self.child.kill();
    }
}

fn transition(event: &INPUT) -> Option<(Key, bool)> {
    if event.r#type != INPUT_KEYBOARD {
        return None;
    }
    let key = unsafe { event.Anonymous.ki };
    Some((
        Key {
            vk: key.wVk.0,
            scan: key.wScan,
            flags: (key.dwFlags & !KEYEVENTF_KEYUP).0,
        },
        !key.dwFlags.contains(KEYEVENTF_KEYUP),
    ))
}

pub(super) struct Session;

impl Session {
    pub(super) fn start() -> Result<Self, String> {
        if ENABLED.load(Ordering::SeqCst) {
            let guard = Guard::spawn()?;
            *GUARD.lock().unwrap_or_else(|e| e.into_inner()) = Some(guard);
        }
        Ok(Self)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let guard = GUARD.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(guard);
    }
}

pub(super) fn failed() -> bool {
    let mut slot = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    slot.as_mut().is_some_and(|guard| {
        if !matches!(guard.child.try_wait(), Ok(None)) {
            guard.failed = true;
        }
        guard.failed
    })
}

/// Serialize journal publication with SendInput, including text and system shortcuts.
pub(super) fn send(
    events: &[INPUT],
    inject: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut slot = GUARD.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(guard) = slot.as_mut()
        && let Err(error) = guard.before(events)
    {
        guard.failed = true;
        return Err(error);
    }
    inject()?;
    if let Some(guard) = slot.as_mut() {
        guard.after(events)?;
    }
    Ok(())
}

/// PID of the independent input guard, for lifecycle diagnostics.
pub fn pid() -> Option<u32> {
    GUARD
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|guard| guard.child.pid)
}
