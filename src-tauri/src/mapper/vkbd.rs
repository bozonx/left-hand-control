// Wayland virtual-keyboard output backend.
//
// We spawn one dedicated thread that owns a `wayland-client` Connection,
// binds `zwp_virtual_keyboard_manager_v1` on the Wayland session of the
// currently logged-in user, and uploads a custom XKB keymap containing
// every character the app supports as a literal (see `us_symbols`).
//
// With that keymap in place, producing a literal character is a matter
// of sending a press+release event for the keycode we assigned to it —
// the compositor applies our keymap to our events, independently of the
// user's currently active system layout. This bypasses every issue that
// the uinput-based approach has with non-US layouts.
//
// The main mapper thread talks to this thread through an `mpsc::Sender<Cmd>`.
//
// Known limitations:
//   * Requires a Wayland compositor that exposes
//     `zwp_virtual_keyboard_manager_v1`. KWin (KDE), Sway, Hyprland, and
//     River support it. GNOME Mutter currently does not — `try_start`
//     returns an error in that case and the caller can fall back to
//     whatever behaviour makes sense (for now: log + no-op on literals).
//   * The virtual keyboard is bound to the user session, so the mapper
//     must run as the same user that owns `$WAYLAND_DISPLAY`. This is
//     already the case for a Tauri app — mentioned here for completeness.

#![cfg(target_os = "linux")]

use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use super::us_symbols::{build_keymap_string, scancode_for};

/// How long to wait for the Wayland thread to finish connecting + uploading
/// the keymap before we consider the backend unavailable. In practice this
/// takes a couple of milliseconds on a warm system; 2 seconds is a huge
/// safety margin.
const INIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll interval when there are no typing commands pending; we still need
/// to periodically drain any Wayland events the compositor sends our way
/// (e.g. a protocol error would arrive as an event on the display queue).
const IDLE_POLL: Duration = Duration::from_millis(50);

/// Public handle. Dropping it asks the background thread to shut down and
/// joins it cleanly.
pub struct Vkbd {
    tx: Sender<Cmd>,
    join: Option<JoinHandle<()>>,
}

enum Cmd {
    Type(char),
    Shutdown,
}

impl Vkbd {
    /// Try to start the background thread and synchronously wait until it
    /// has bound the Wayland globals and uploaded the keymap. Returns an
    /// error with a descriptive message if any step fails — the caller is
    /// expected to degrade gracefully (e.g. log and keep running without
    /// literal-character support).
    pub fn try_start() -> Result<Self, String> {
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();

        let join = thread::Builder::new()
            .name("lhc-vkbd".into())
            .spawn(move || run_thread(init_tx, cmd_rx))
            .map_err(|e| format!("spawn vkbd thread: {e}"))?;

        match init_rx.recv_timeout(INIT_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                tx: cmd_tx,
                join: Some(join),
            }),
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => Err("vkbd: init did not complete within 2s".into()),
        }
    }

    /// Queue a character for the Wayland thread to type. Returns `true`
    /// if the character is in our table; `false` otherwise (caller can log
    /// a helpful message). The send is lossy on shutdown — if the channel
    /// is closed we silently drop the character.
    pub fn type_char(&self, ch: char) -> bool {
        if scancode_for(ch).is_none() {
            return false;
        }
        let _ = self.tx.send(Cmd::Type(ch));
        true
    }
}

impl Drop for Vkbd {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}

// ---------------- background thread ----------------

struct State {
    vkbd: ZwpVirtualKeyboardV1,
}

fn run_thread(init_tx: Sender<Result<(), String>>, cmd_rx: mpsc::Receiver<Cmd>) {
    let (mut state, mut queue) = match init_wayland() {
        Ok(v) => v,
        Err(e) => {
            let _ = init_tx.send(Err(e));
            return;
        }
    };
    let _ = init_tx.send(Ok(()));

    let start = Instant::now();

    loop {
        // Drain any compositor events that have already arrived.
        if let Err(e) = queue.dispatch_pending(&mut state) {
            eprintln!("[vkbd] dispatch_pending error: {e}; shutting down");
            break;
        }
        if let Err(e) = queue.flush() {
            eprintln!("[vkbd] flush error: {e}; shutting down");
            break;
        }

        match cmd_rx.recv_timeout(IDLE_POLL) {
            Ok(Cmd::Type(ch)) => {
                if let Some(sc) = scancode_for(ch) {
                    let t = monotonic_ms(start);
                    state.vkbd.key(t, sc, 1);
                    state.vkbd.key(t, sc, 0);
                    if let Err(e) = queue.flush() {
                        eprintln!("[vkbd] flush after key error: {e}");
                        break;
                    }
                } else {
                    eprintln!("[vkbd] no scancode for {ch:?} (not in US table)");
                }
            }
            Ok(Cmd::Shutdown) => break,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    // Destroy the virtual keyboard explicitly; dropping would do the same
    // but being explicit makes the teardown order easy to read.
    state.vkbd.destroy();
    let _ = queue.flush();
}

fn init_wayland() -> Result<(State, EventQueue<State>), String> {
    let conn = Connection::connect_to_env()
        .map_err(|e| format!("wayland connect: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<State>(&conn).map_err(|e| format!("registry init: {e}"))?;
    let qh = queue.handle();

    // Dump every advertised global once, so that when a bind fails below we
    // can tell from the log whether the protocol is missing entirely or
    // whether we asked for the wrong version range.
    {
        let list = globals.contents().clone_list();
        eprintln!("[vkbd] advertised wayland globals ({}):", list.len());
        for g in &list {
            eprintln!("[vkbd]   v{} {}", g.version, g.interface);
        }
    }

    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=9, ())
        .map_err(|e| format!("bind wl_seat: {e}"))?;
    let manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ()).map_err(|e| {
        format!(
            "bind zwp_virtual_keyboard_manager_v1: {e}. Compositor does not expose the \
             virtual-keyboard protocol to ordinary clients (KDE KWin restricts it to authorised \
             input methods; GNOME Mutter does not implement it at all)."
        )
    })?;

    let vkbd = manager.create_virtual_keyboard(&seat, &qh, ());

    // Upload our custom keymap.
    let keymap = build_keymap_string();
    let (fd, size) = make_keymap_fd(&keymap)
        .map_err(|e| format!("create keymap memfd: {e}"))?;
    // Format 1 = xkb_v1 per the zwp_virtual_keyboard_v1 XML. The
    // generated bindings expose this as a plain u32, not a strongly-typed
    // enum, so we pass the numeric value directly.
    const KEYMAP_FORMAT_XKB_V1: u32 = 1;
    vkbd.keymap(KEYMAP_FORMAT_XKB_V1, fd.as_fd(), size);
    drop(fd); // compositor has mmap'd it already; our handle is no longer needed.

    // Make sure the keymap upload actually reaches the compositor before
    // we return success. Without a roundtrip here we could race the first
    // key() call and have it rejected.
    queue
        .roundtrip(&mut State {
            vkbd: vkbd.clone(),
        })
        .map_err(|e| format!("wayland roundtrip: {e}"))?;

    eprintln!(
        "[vkbd] ready: {} characters on keycodes {}..={}",
        super::us_symbols::US_SYMBOLS.len(),
        super::us_symbols::KEYCODE_BASE,
        super::us_symbols::KEYCODE_BASE + super::us_symbols::US_SYMBOLS.len() as u32 - 1,
    );

    Ok((State { vkbd }, queue))
}

/// Current monotonic timestamp in ms, wrapped to u32 (which the wayland
/// protocol expects for `key.time`).
fn monotonic_ms(start: Instant) -> u32 {
    (start.elapsed().as_millis() & 0xFFFF_FFFF) as u32
}

/// Create an anonymous memfd containing `keymap` + trailing NUL, ready to
/// hand to the compositor via `zwp_virtual_keyboard_v1::keymap`.
fn make_keymap_fd(keymap: &str) -> io::Result<(OwnedFd, u32)> {
    use std::ffi::CString;
    let name = CString::new("lhc-keymap").unwrap();
    let raw = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `memfd_create` returned a valid, owned fd. Wrap it so it's
    // closed automatically on any subsequent error.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };

    let bytes = keymap.as_bytes();
    // Size includes the mandatory trailing NUL terminator: libxkbcommon on
    // the compositor side parses the mmapped region as a C string.
    let size = bytes.len() + 1;

    unsafe {
        if libc::ftruncate(fd.as_raw_fd(), size as i64) < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut written = 0usize;
        while written < bytes.len() {
            let n = libc::write(
                fd.as_raw_fd(),
                bytes.as_ptr().add(written) as *const _,
                bytes.len() - written,
            );
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
            written += n as usize;
        }
    }
    Ok((fd, size as u32))
}

// ---------------- Dispatch impls (mostly no-ops) ----------------
//
// wayland-client 0.31 requires explicit Dispatch impls for every proxy type
// that can fire an event on the queue we own. Our virtual keyboard is a
// write-only client object and the other proxies don't emit anything we
// care about — we still have to provide empty handlers so the compiler is
// happy.

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardManagerV1,
        _: <ZwpVirtualKeyboardManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardV1,
        _: <ZwpVirtualKeyboardV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
