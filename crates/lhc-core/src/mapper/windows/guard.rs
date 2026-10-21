//! A stdin pipe outlives its owner: EOF lets a separate process release held input.

use super::super::input_guard::{Key, finish, read_snapshots, write_snapshot};
use super::{MapperLock, input};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, SendInput,
};

const ROLE: &str = "LHC_INTERNAL_WINDOWS_INPUT_GUARD";
static ENABLED: AtomicBool = AtomicBool::new(false);
static GUARD: Mutex<Option<Guard>> = Mutex::new(None);

/// Call before config, IPC or UI initialization in executables embedding the mapper.
/// Returns true in the internal guard child after cleanup; the caller should exit.
/// Calling it in the owner enables crash recovery for subsequent mapper sessions.
pub fn dispatch() -> Result<bool, String> {
    if std::env::var_os(ROLE).is_none() {
        ENABLED.store(true, Ordering::SeqCst);
        return Ok(false);
    }
    let _lock = MapperLock::join_existing()?;
    std::io::stdout()
        .write_all(b"LHC!")
        .map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut held = BTreeSet::new();
    let journal = read_snapshots(&mut std::io::stdin().lock(), &mut held);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !release(&held) {
        if Instant::now() >= deadline {
            return Err("input guard could not release native keys".into());
        }
        std::thread::sleep(Duration::from_millis(100));
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

struct Guard {
    child: Child,
    writer: Option<ChildStdin>,
    held: BTreeSet<Key>,
    failed: bool,
}

impl Guard {
    fn spawn() -> Result<Self, String> {
        let child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .env(ROLE, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| format!("start input guard: {e}"))?;
        let mut guard = Self {
            child,
            writer: None,
            held: BTreeSet::new(),
            failed: false,
        };
        guard.writer = guard.child.stdin.take();
        let mut stdout = guard
            .child
            .stdout
            .take()
            .ok_or("input guard stdout unavailable")?;
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("lhc-guard-ready".into())
            .spawn(move || {
                let mut ready = [0; 4];
                let result = stdout
                    .read_exact(&mut ready)
                    .map_err(|e| e.to_string())
                    .and_then(|()| {
                        if ready == *b"LHC!" {
                            Ok(())
                        } else {
                            Err("invalid input guard handshake".into())
                        }
                    });
                let _ = tx.send(result);
            })
            .map_err(|e| format!("start input guard handshake: {e}"))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|e| format!("input guard readiness: {e}"))??;
        Ok(guard)
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
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) if status.success() && !self.failed => return,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                _ => break,
            }
        }
        // Also recover if the guard itself failed while the owner is still alive.
        if !release(&self.held) {
            log::error!("input guard fallback could not release keys");
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
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
