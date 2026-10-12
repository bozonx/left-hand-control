//! Wayland compositors with `zwlr_foreign_toplevel_management_v1`
//! (niri, Wayfire, labwc, river, COSMIC and other wlroots-based ones):
//! a thread listens to toplevel events, so the active window and its
//! fullscreen state arrive without polling or helper commands.
//!
//! The protocol carries no process id, so windows are known by app id.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};

use super::ActiveWindow;

/// How often the event thread checks whether it should stop.
const STOP_POLL_MS: u16 = 500;

/// The active toplevel as last reported; `None` before the first report.
static CURRENT: Mutex<Option<Option<ActiveWindow>>> = Mutex::new(None);
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

struct Session {
    stop: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// Connect and listen in the background; `false` when the compositor
/// does not offer the protocol.
pub(super) fn start() -> bool {
    stop();
    let stop = Arc::new(AtomicBool::new(false));
    let alive = Arc::new(AtomicBool::new(true));
    let (thread_stop, thread_alive) = (stop.clone(), alive.clone());
    let (ready, bound) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("active-window-wlr".into())
        .spawn(move || {
            run(&thread_stop, &ready);
            thread_alive.store(false, Ordering::SeqCst);
        });
    let Ok(thread) = thread else {
        return false;
    };
    if bound.recv().unwrap_or(false) {
        if let Ok(mut session) = SESSION.lock() {
            *session = Some(Session {
                stop,
                alive,
                thread,
            });
        }
        true
    } else {
        let _ = thread.join();
        false
    }
}

/// Whether the event thread runs.
pub(super) fn active() -> bool {
    SESSION.lock().ok().is_some_and(|session| {
        session
            .as_ref()
            .is_some_and(|session| session.alive.load(Ordering::SeqCst))
    })
}

pub(super) fn stop() {
    let session = SESSION.lock().ok().and_then(|mut session| session.take());
    if let Some(session) = session {
        session.stop.store(true, Ordering::SeqCst);
        let _ = session.thread.join();
    }
    if let Ok(mut current) = CURRENT.lock() {
        *current = None;
    }
}

/// The active window, once the compositor reported one.
pub(super) fn current() -> Option<Option<ActiveWindow>> {
    CURRENT.lock().ok()?.clone()
}

#[derive(Default, Clone)]
struct Toplevel {
    title: String,
    app_id: String,
    activated: bool,
    fullscreen: bool,
}

#[derive(Default)]
struct State {
    /// Values being collected until the next `done`.
    pending: HashMap<ObjectId, Toplevel>,
    toplevels: HashMap<ObjectId, Toplevel>,
    /// Some toplevel changed since the active window was last published.
    changed: bool,
}

impl State {
    /// The toplevel `id` with this round's changes so far.
    fn pending(&mut self, id: &ObjectId) -> &mut Toplevel {
        let known = self.toplevels.get(id).cloned().unwrap_or_default();
        self.pending.entry(id.clone()).or_insert(known)
    }

    fn publish(&mut self) {
        if !std::mem::take(&mut self.changed) {
            return;
        }
        let window = self
            .toplevels
            .values()
            .find(|toplevel| toplevel.activated)
            .filter(|toplevel| !toplevel.app_id.is_empty() || !toplevel.title.is_empty())
            .map(|toplevel| ActiveWindow {
                title: toplevel.title.clone(),
                app_id: toplevel.app_id.clone(),
                process_name: None,
                fullscreen: Some(toplevel.fullscreen),
            });
        let Ok(mut current) = CURRENT.lock() else {
            return;
        };
        if current.as_ref() != Some(&window) {
            *current = Some(window);
            drop(current);
            super::wake();
        }
    }
}

fn run(stop: &AtomicBool, ready: &mpsc::Sender<bool>) {
    let Some((connection, mut queue)) = Connection::connect_to_env().ok().and_then(|connection| {
        let (globals, queue) = registry_queue_init::<State>(&connection).ok()?;
        globals
            .bind::<ZwlrForeignToplevelManagerV1, State, ()>(&queue.handle(), 1..=3, ())
            .ok()?;
        Some((connection, queue))
    }) else {
        let _ = ready.send(false);
        return;
    };
    let _ = ready.send(true);
    let mut state = State::default();
    while !stop.load(Ordering::SeqCst) {
        if queue.dispatch_pending(&mut state).is_err() {
            break;
        }
        state.publish();
        if connection.flush().is_err() {
            break;
        }
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let mut fds = [PollFd::new(guard.connection_fd(), PollFlags::POLLIN)];
        match poll(&mut fds, PollTimeout::from(STOP_POLL_MS)) {
            Ok(0) => drop(guard),
            Ok(_) => {
                if guard.read().is_err() {
                    break;
                }
            }
            Err(nix::errno::Errno::EINTR) => drop(guard),
            Err(_) => break,
        }
    }
    log::debug!("[active-window] wlr toplevel events stopped");
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            state.pending.insert(toplevel.id(), Toplevel::default());
        }
    }

    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_foreign_toplevel_handle_v1::Event;
        let id = handle.id();
        match event {
            Event::Title { title } => state.pending(&id).title = title,
            Event::AppId { app_id } => state.pending(&id).app_id = app_id,
            Event::State { state: values } => {
                let values = parse_states(&values);
                let toplevel = state.pending(&id);
                toplevel.activated = values.contains(&STATE_ACTIVATED);
                toplevel.fullscreen = values.contains(&STATE_FULLSCREEN);
            }
            Event::Done => {
                if let Some(toplevel) = state.pending.remove(&id) {
                    state.toplevels.insert(id, toplevel);
                    state.changed = true;
                }
            }
            Event::Closed => {
                state.pending.remove(&id);
                if state.toplevels.remove(&id).is_some() {
                    state.changed = true;
                }
                handle.destroy();
            }
            _ => {}
        }
    }
}

const STATE_ACTIVATED: u32 = 2;
const STATE_FULLSCREEN: u32 = 3;

/// The `state` event's array of native-endian `u32` values.
fn parse_states(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_toplevel_states() {
        let bytes: Vec<u8> = [0_u32, 2, 3]
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect();
        assert_eq!(parse_states(&bytes), [0, 2, 3]);
        assert!(parse_states(&[1, 0]).is_empty());
    }

    #[test]
    fn publishes_the_activated_toplevel() {
        let mut state = State::default();
        let toplevel = |app_id: &str, activated, fullscreen| Toplevel {
            title: String::new(),
            app_id: app_id.into(),
            activated,
            fullscreen,
        };
        state
            .toplevels
            .insert(ObjectId::null(), toplevel("game", true, true));
        state.changed = true;
        state.publish();
        let window = current().flatten().unwrap();
        assert_eq!(window.app_id, "game");
        assert_eq!(window.fullscreen, Some(true));
        state
            .toplevels
            .insert(ObjectId::null(), toplevel("game", false, true));
        state.changed = true;
        state.publish();
        assert_eq!(current(), Some(None));
    }
}
