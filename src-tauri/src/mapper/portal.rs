// XDG Desktop Portal RemoteDesktop backend.
//
// Why this instead of `zwp_virtual_keyboard_v1` on Wayland: in recent
// versions of KDE Plasma (6.x) and GNOME the virtual-keyboard Wayland
// protocol is not advertised to ordinary clients, so any attempt to bind
// it fails with "global not found in registry". The RemoteDesktop portal
// is the officially blessed, cross-compositor way to inject input:
//
//   1. `CreateSession()`        – async, via a `Request` object + `Response` signal.
//   2. `SelectDevices(keyboard)`– declares what we plan to inject; this is
//                                 also where we set `persist_mode = 2` and
//                                 pass back any previously-saved
//                                 `restore_token` so the user does not see
//                                 the consent dialog again.
//   3. `Start()`                – shows the permission dialog (only once
//                                 per machine if persist mode + token work).
//   4. `NotifyKeyboardKeysym()` – fires X keysyms; theoretically
//      layout-independent but in practice compositor-dependent (see below).
//
// The session and its DBus connection are owned by a process-wide
// singleton: it is created lazily on the first injection request and
// lives until the process exits. This means the consent dialog is shown
// once per *process*, not once per `start_mapper`/`stop_mapper` cycle —
// which used to retrigger it on every config change.
//
// On top of that we read `restore_token` from the `Start` response and
// persist it next to the user data, so subsequent app launches can
// restore the permission silently.
//
// Text injection strategy (to work around a KDE Plasma bug where
// `NotifyKeyboardKeysym` looks up the physical key for the keysym in the
// current layout but emits it without the required modifier, producing the
// wrong character when a non-Latin layout is active):
//
//   1. Load the current XKB keymap and build a keysym→(keycode, level) table.
//   2. For each character, look it up in the table and inject via
//      `NotifyKeyboardKeycode` with the appropriate modifier keys — this is
//      truly layout-driven and always correct for characters in the layout.
//   3. If the character is absent from the current layout (e.g. typing `?`
//      while Russian is active), fall back to `wl-copy` + Ctrl+V paste.
//   4. If `wl-copy` is unavailable, fall back to `NotifyKeyboardKeysym`
//      (original behaviour, works on compositors with correct implementations).

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::ffi::{c_char, CString};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reis::ei::{self, handshake::ContextType, keyboard::KeyState};
use reis::enumflags2::BitFlags;
use reis::event::{DeviceCapability, EiEvent, EiEventConverter};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value};

enum XkbContext {}
enum XkbKeymap {}

#[repr(C)]
struct XkbRuleNames {
    rules: *const c_char,
    model: *const c_char,
    layout: *const c_char,
    variant: *const c_char,
    options: *const c_char,
}

#[link(name = "xkbcommon")]
unsafe extern "C" {
    fn xkb_utf32_to_keysym(ucs: u32) -> u32;
    fn xkb_context_new(flags: u32) -> *mut XkbContext;
    fn xkb_context_unref(ctx: *mut XkbContext);
    fn xkb_keymap_new_from_names(
        ctx: *mut XkbContext,
        names: *const XkbRuleNames,
        flags: u32,
    ) -> *mut XkbKeymap;
    fn xkb_keymap_unref(keymap: *mut XkbKeymap);
    fn xkb_keymap_min_keycode(keymap: *mut XkbKeymap) -> u32;
    fn xkb_keymap_max_keycode(keymap: *mut XkbKeymap) -> u32;
    fn xkb_keymap_num_layouts_for_key(keymap: *mut XkbKeymap, key: u32) -> u32;
    fn xkb_keymap_num_levels_for_key(keymap: *mut XkbKeymap, key: u32, layout: u32) -> u32;
    fn xkb_keymap_key_get_syms_by_level(
        keymap: *mut XkbKeymap,
        key: u32,
        layout: u32,
        level: u32,
        syms_out: *mut *const u32,
    ) -> i32;
}

// --- Portal DBus identifiers --------------------------------------------

const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE_REMOTE: &str = "org.freedesktop.portal.RemoteDesktop";
const IFACE_REQUEST: &str = "org.freedesktop.portal.Request";
const IFACE_SESSION: &str = "org.freedesktop.portal.Session";

/// Keyboard capability bit in the RemoteDesktop "types" field.
const DEVICE_TYPE_KEYBOARD: u32 = 1;

/// `persist_mode` for SelectDevices: 2 = persist until explicitly revoked.
const PERSIST_MODE_PERMANENT: u32 = 2;

const STATE_RELEASED: u32 = 0;
const STATE_PRESSED: u32 = 1;

const RESTORE_TOKEN_FILE: &str = "portal-remote-desktop.token";

// --- XKB keymap lookup for layout-aware text injection ------------------

// evdev keycodes for modifier keys used in text injection.
const KEY_LEFTSHIFT_EVDEV: u32 = 42;
const KEY_RIGHTALT_EVDEV: u32 = 100; // AltGr
const KEY_LEFTCTRL_EVDEV: u32 = 29;
const KEY_V_EVDEV: u32 = 47;

// XKB keysym for Latin lowercase 'v'. Used for the Ctrl+V paste shortcut
// instead of the evdev keycode so that the compositor sees the correct keysym
// regardless of the currently active keyboard layout (e.g. Russian 'м' ≠ 'v').
const XKB_KEY_V_LOWER: u32 = 0x0076;

// Map XKB level index to the evdev modifier keys that must be held.
// Covers the vast majority of European layouts:
//   level 0 = base (no mods), 1 = Shift, 2 = AltGr, 3 = Shift+AltGr.
fn level_to_mods(level: u32) -> &'static [u32] {
    match level {
        0 => &[],
        1 => &[KEY_LEFTSHIFT_EVDEV],
        2 => &[KEY_RIGHTALT_EVDEV],
        3 => &[KEY_LEFTSHIFT_EVDEV, KEY_RIGHTALT_EVDEV],
        _ => &[],
    }
}

struct KeycodeEntry {
    evdev: u32,
    level: u32,
}

type KeymapCache = Option<(String, Arc<HashMap<u32, KeycodeEntry>>)>;
static KEYMAP_CACHE: Mutex<KeymapCache> = Mutex::new(None);

fn keymap_table() -> Arc<HashMap<u32, KeycodeEntry>> {
    let layout = crate::layout::cached_layout_short()
        .or_else(|| std::env::var("XKB_DEFAULT_LAYOUT").ok())
        .unwrap_or_else(|| "us".to_string());

    if let Ok(mut guard) = KEYMAP_CACHE.lock() {
        if let Some((ref k, ref t)) = *guard {
            if *k == layout {
                return Arc::clone(t);
            }
        }
        let t = Arc::new(build_keymap_table(&layout));
        *guard = Some((layout, Arc::clone(&t)));
        return t;
    }
    Arc::new(HashMap::new())
}

fn build_keymap_table(layout: &str) -> HashMap<u32, KeycodeEntry> {
    let mut map: HashMap<u32, KeycodeEntry> = HashMap::new();

    let Ok(layout_c) = CString::new(layout) else {
        return map;
    };
    let variant_c = std::env::var("XKB_DEFAULT_VARIANT")
        .ok()
        .and_then(|v| CString::new(v).ok());
    let variant_ptr = variant_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

    unsafe {
        let ctx = xkb_context_new(0);
        if ctx.is_null() {
            return map;
        }
        let names = XkbRuleNames {
            rules: std::ptr::null(),
            model: std::ptr::null(),
            layout: layout_c.as_ptr(),
            variant: variant_ptr,
            options: std::ptr::null(),
        };
        let keymap = xkb_keymap_new_from_names(ctx, &names, 0);
        xkb_context_unref(ctx);
        if keymap.is_null() {
            log::debug!("[portal] xkb_keymap_new_from_names({layout:?}) failed");
            return map;
        }

        let min_kc = xkb_keymap_min_keycode(keymap);
        let max_kc = xkb_keymap_max_keycode(keymap);

        for kc in min_kc..=max_kc {
            if xkb_keymap_num_layouts_for_key(keymap, kc) == 0 {
                continue;
            }
            let nlevels = xkb_keymap_num_levels_for_key(keymap, kc, 0).min(4);
            for level in 0..nlevels {
                let mut syms: *const u32 = std::ptr::null();
                let nsyms = xkb_keymap_key_get_syms_by_level(keymap, kc, 0, level, &mut syms);
                if nsyms <= 0 || syms.is_null() {
                    continue;
                }
                for i in 0..nsyms as usize {
                    let sym = *syms.add(i);
                    if sym == 0 {
                        continue;
                    }
                    let evdev = kc.saturating_sub(8);
                    if evdev == 0 {
                        continue;
                    }
                    // Keep lowest-level entry (fewest modifiers needed).
                    map.entry(sym).or_insert(KeycodeEntry { evdev, level });
                }
            }
        }

        xkb_keymap_unref(keymap);
        log::debug!(
            "[portal] XKB keymap {:?}: {} keysyms indexed",
            layout,
            map.len()
        );
        map
    }
}

// --- Public API ---------------------------------------------------------

enum Cmd {
    Type(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextBackend {
    Libei,
    Keycode,
    Clipboard,
    Ydotool,
}

#[derive(Clone, Debug)]
struct TextBackendConfig {
    backend: TextBackend,
    ydotool_path: Option<String>,
}

static TEXT_BACKEND: Mutex<TextBackendConfig> = Mutex::new(TextBackendConfig {
    backend: TextBackend::Libei,
    ydotool_path: None,
});
static LIBEI_FALLBACK_LOGGED: AtomicBool = AtomicBool::new(false);

/// Switch between text injection strategies.
///   "libei" — default Wayland-native backend. Currently falls back to the
///              RemoteDesktop keycode path while the EI transport is wired.
///   "clipboard" — use wl-copy + Ctrl+V for the whole string at once.
///   "ydotool" — run a ydotool-compatible executable as `type <text>`.
///   anything else — XKB keycode injection.
pub fn set_text_backend(mode: &str, ydotool_path: Option<&str>) {
    let backend = match mode {
        "libei" => TextBackend::Libei,
        "clipboard" => TextBackend::Clipboard,
        "ydotool" => TextBackend::Ydotool,
        _ => TextBackend::Keycode,
    };
    let ydotool_path = ydotool_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned);
    if let Ok(mut cfg) = TEXT_BACKEND.lock() {
        *cfg = TextBackendConfig {
            backend,
            ydotool_path,
        };
    }
    log::debug!("[portal] text backend: {:?}", backend);
}

/// Sender to the current portal worker thread. Cleared when the worker is
/// gone so a later literal injection can retry the portal handshake.
static PORTAL_TX: Mutex<Option<Sender<Cmd>>> = Mutex::new(None);
static LIBEI_TX: Mutex<Option<Sender<Cmd>>> = Mutex::new(None);

/// Path of the directory where we persist the restore token.
static TOKEN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Register where the worker should read/write the restore token.
/// Must be called once at app startup (before the first injection).
/// Calling it again is a no-op.
pub fn set_token_dir(dir: PathBuf) {
    let _ = TOKEN_DIR.set(dir);
}

/// Send a literal string to the active session. Lazily starts the
/// singleton worker on first call. Non-blocking: returns immediately,
/// the actual portal handshake (and its consent dialog) runs on a
/// dedicated thread.
pub fn type_text(text: &str) {
    let cfg = TEXT_BACKEND
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or(TextBackendConfig {
            backend: TextBackend::Keycode,
            ydotool_path: None,
        });
    match cfg.backend {
        TextBackend::Ydotool => {
            type_text_ydotool(text, cfg.ydotool_path);
            return;
        }
        TextBackend::Libei => {
            if !LIBEI_FALLBACK_LOGGED.swap(true, Ordering::Relaxed) {
                log::debug!(
                    "[portal] libei text backend is selected; using RemoteDesktop.ConnectToEIS when available"
                );
            }
            if type_text_libei(text) {
                return;
            }
            log::debug!("[portal] libei backend unavailable; falling back to RemoteDesktop keycode injection");
        }
        TextBackend::Clipboard | TextBackend::Keycode => {}
    }
    type_text_portal(text);
}

fn type_text_ydotool(text: &str, ydotool_path: Option<String>) {
    let text = text.to_string();
    let program = ydotool_path.unwrap_or_else(|| "ydotool".to_string());
    match thread::Builder::new()
        .name("lhc-ydotool".into())
        .spawn(move || {
            let status = std::process::Command::new(&program)
                .arg("type")
                .arg(&text)
                .status();
            match status {
                Ok(status) if status.success() => {}
                Ok(status) => log::debug!("[ydotool] {program:?} exited with {status}"),
                Err(e) => log::debug!("[ydotool] failed to run {program:?}: {e}"),
            }
        }) {
        Ok(_) => {}
        Err(e) => log::debug!("[ydotool] could not spawn worker: {e}"),
    }
}

fn type_text_portal(text: &str) {
    let Ok(mut slot) = PORTAL_TX.lock() else {
        log::debug!("[portal] literal {:?} dropped (worker lock poisoned)", text);
        return;
    };

    if let Some(tx) = slot.as_ref() {
        if tx.send(Cmd::Type(text.to_string())).is_ok() {
            return;
        }
        log::debug!("[portal] worker gone; retrying backend init");
        *slot = None;
    }

    let Some(tx) = start_portal_singleton() else {
        log::debug!("[portal] literal {:?} dropped (worker spawn failed)", text);
        return;
    };
    if tx.send(Cmd::Type(text.to_string())).is_err() {
        log::debug!(
            "[portal] literal {:?} dropped (worker exited during init)",
            text
        );
        return;
    }
    *slot = Some(tx);
}

fn type_text_libei(text: &str) -> bool {
    let Ok(mut slot) = LIBEI_TX.lock() else {
        log::debug!("[libei] literal {:?} dropped (worker lock poisoned)", text);
        return false;
    };

    if let Some(tx) = slot.as_ref() {
        if tx.send(Cmd::Type(text.to_string())).is_ok() {
            return true;
        }
        log::debug!("[libei] worker gone; retrying backend init");
        *slot = None;
    }

    let Some(tx) = start_libei_singleton() else {
        return false;
    };
    if tx.send(Cmd::Type(text.to_string())).is_err() {
        log::debug!(
            "[libei] literal {:?} dropped (worker exited during init)",
            text
        );
        return false;
    }
    *slot = Some(tx);
    true
}

fn start_portal_singleton() -> Option<Sender<Cmd>> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    let spawn_result = thread::Builder::new()
        .name("lhc-portal".into())
        .spawn(move || portal_worker(rx));
    match spawn_result {
        Ok(_join) => Some(tx),
        Err(e) => {
            log::debug!("[portal] could not spawn worker thread: {e}");
            None
        }
    }
}

fn start_libei_singleton() -> Option<Sender<Cmd>> {
    let (tx, rx) = mpsc::channel::<Cmd>();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    let spawn_result = thread::Builder::new()
        .name("lhc-libei".into())
        .spawn(move || libei_worker(rx, ready_tx));
    match spawn_result {
        Ok(_join) => match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Some(tx),
            Ok(Err(e)) => {
                log::debug!("[libei] backend unavailable: {e}");
                None
            }
            Err(e) => {
                log::debug!("[libei] backend init timed out: {e}");
                None
            }
        },
        Err(e) => {
            log::debug!("[libei] could not spawn worker thread: {e}");
            None
        }
    }
}

fn portal_worker(rx: Receiver<Cmd>) {
    let (conn, session_handle) = match init_portal() {
        Ok(v) => {
            log::debug!("[portal] backend online");
            v
        }
        Err(e) => {
            log::debug!("[portal] backend unavailable: {e}");
            drop(rx);
            return;
        }
    };

    let portal = match Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE) {
        Ok(p) => p,
        Err(e) => {
            log::debug!("[portal] could not re-create RemoteDesktop proxy: {e}");
            close_session(&conn, &session_handle);
            return;
        }
    };

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Type(text) => inject_text(&portal, &session_handle, &text),
        }
    }

    close_session(&conn, &session_handle);
}

fn libei_worker(rx: Receiver<Cmd>, ready_tx: mpsc::SyncSender<Result<(), String>>) {
    let mut backend = match LibeiBackend::new() {
        Ok(backend) => {
            let _ = ready_tx.send(Ok(()));
            backend
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e));
            drop(rx);
            return;
        }
    };

    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Type(text) => {
                if let Err(e) = backend.type_text(&text) {
                    log::debug!("[libei] text injection failed: {e}");
                    break;
                }
            }
        }
    }

    backend.close();
}

struct LibeiBackend {
    conn: Connection,
    session_handle: OwnedObjectPath,
    context: ei::Context,
    converter: EiEventConverter,
    connection: reis::event::Connection,
    keyboard_device: Option<(ei::Device, ei::Keyboard)>,
    keyboard_emulating: bool,
    sequence: u32,
}

impl LibeiBackend {
    fn new() -> Result<Self, String> {
        let (conn, session_handle) = init_portal()?;
        let portal = match Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE) {
            Ok(portal) => portal,
            Err(e) => {
                close_session(&conn, &session_handle);
                return Err(format!("create RemoteDesktop proxy for EIS: {e}"));
            }
        };
        let empty: HashMap<String, Value> = HashMap::new();
        let fd: OwnedFd = match portal.call("ConnectToEIS", &(&session_handle, empty)) {
            Ok(fd) => fd,
            Err(e) => {
                close_session(&conn, &session_handle);
                return Err(format!("ConnectToEIS: {e}"));
            }
        };
        let socket = UnixStream::from(std::os::fd::OwnedFd::from(fd));
        let context = match ei::Context::new(socket) {
            Ok(context) => context,
            Err(e) => {
                close_session(&conn, &session_handle);
                return Err(format!("create EI context: {e}"));
            }
        };
        let handshake = match ei_handshake_with_timeout(&context, Duration::from_secs(5)) {
            Ok(handshake) => handshake,
            Err(e) => {
                close_session(&conn, &session_handle);
                return Err(e);
            }
        };
        let converter = EiEventConverter::new(&context, handshake);
        let connection = converter.connection().clone();
        let mut backend = Self {
            conn,
            session_handle,
            context,
            converter,
            connection,
            keyboard_device: None,
            keyboard_emulating: false,
            sequence: 1,
        };
        if let Err(e) = backend.wait_for_keyboard(Duration::from_secs(5)) {
            close_session(&backend.conn, &backend.session_handle);
            return Err(e);
        }
        log::debug!("[libei] backend online");
        Ok(backend)
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.pump_events()?;
        if !self.has_keyboard() {
            self.wait_for_keyboard(Duration::from_secs(2))?;
        }
        let table = keymap_table();
        if text
            .chars()
            .any(|ch| !table.contains_key(&unsafe { xkb_utf32_to_keysym(ch as u32) }))
        {
            self.type_text_via_clipboard(text)
        } else {
            for ch in text.chars() {
                let keysym = unsafe { xkb_utf32_to_keysym(ch as u32) };
                let entry = table
                    .get(&keysym)
                    .ok_or_else(|| format!("no keycode for {ch:?}"))?;
                self.inject_keycode_combo(level_to_mods(entry.level), entry.evdev)?;
            }
            self.context.flush().map_err(|e| format!("EI flush: {e}"))?;
            Ok(())
        }
    }

    fn type_text_via_clipboard(&mut self, text: &str) -> Result<(), String> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("wl-copy unavailable for libei clipboard fallback: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("write wl-copy stdin: {e}"))?;
        }
        thread::spawn(move || {
            let _ = child.wait();
        });
        thread::sleep(Duration::from_millis(50));
        self.inject_keycode_combo(&[KEY_LEFTCTRL_EVDEV], KEY_V_EVDEV)
    }

    fn inject_keycode_combo(&mut self, mods: &[u32], key: u32) -> Result<(), String> {
        for &m in mods {
            self.inject_key(m, KeyState::Press)?;
        }
        self.inject_key(key, KeyState::Press)?;
        self.inject_key(key, KeyState::Released)?;
        for &m in mods.iter().rev() {
            self.inject_key(m, KeyState::Released)?;
        }
        Ok(())
    }

    fn inject_key(&mut self, key: u32, state: KeyState) -> Result<(), String> {
        let Some((device, keyboard)) = self.keyboard_device.as_ref() else {
            return Err("no EI keyboard device".into());
        };
        keyboard.key(key, state);
        device.frame(self.connection.serial(), monotonic_micros());
        self.context.flush().map_err(|e| format!("EI flush: {e}"))
    }

    fn wait_for_keyboard(&mut self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.pump_events()?;
            if self.has_keyboard() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err("EI keyboard device was not advertised".into())
    }

    fn pump_events(&mut self) -> Result<(), String> {
        self.context.read().map_err(|e| format!("EI read: {e}"))?;
        while let Some(result) = self.context.pending_event() {
            let event = match result {
                reis::PendingRequestResult::Request(event) => event,
                reis::PendingRequestResult::ParseError(e) => {
                    return Err(format!("EI parse error: {e}"));
                }
                reis::PendingRequestResult::InvalidObject(id) => {
                    return Err(format!("EI invalid object: {id}"));
                }
            };
            self.converter
                .handle_event(event)
                .map_err(|e| format!("EI event error: {e}"))?;
        }
        while let Some(event) = self.converter.next_event() {
            self.handle_event(event)?;
        }
        self.context.flush().map_err(|e| format!("EI flush: {e}"))?;
        Ok(())
    }

    fn handle_event(&mut self, event: EiEvent) -> Result<(), String> {
        match event {
            EiEvent::Disconnected(e) => {
                return Err(format!("EI disconnected: {:?}", e.explanation));
            }
            EiEvent::SeatAdded(e) => {
                e.seat
                    .bind_capabilities(BitFlags::from(DeviceCapability::Keyboard));
            }
            EiEvent::DeviceAdded(e) => {
                if let Some(keyboard) = e.device.interface::<ei::Keyboard>() {
                    let device = e.device.device().clone();
                    self.start_keyboard_emulation(&device);
                    self.keyboard_device = Some((device, keyboard));
                }
            }
            EiEvent::DevicePaused(e) => {
                if self
                    .keyboard_device
                    .as_ref()
                    .is_some_and(|(device, _)| device == e.device.device())
                {
                    self.keyboard_emulating = false;
                }
            }
            EiEvent::DeviceResumed(e) => {
                if self
                    .keyboard_device
                    .as_ref()
                    .is_some_and(|(device, _)| device == e.device.device())
                {
                    self.start_keyboard_emulation(e.device.device());
                }
            }
            EiEvent::DeviceRemoved(e) => {
                if self
                    .keyboard_device
                    .as_ref()
                    .is_some_and(|(device, _)| device == e.device.device())
                {
                    self.keyboard_device = None;
                    self.keyboard_emulating = false;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn has_keyboard(&self) -> bool {
        self.keyboard_device.is_some()
    }

    fn start_keyboard_emulation(&mut self, device: &ei::Device) {
        if self.keyboard_emulating {
            return;
        }
        device.start_emulating(self.connection.serial(), self.sequence);
        self.sequence += 1;
        self.keyboard_emulating = true;
    }

    fn close(self) {
        close_session(&self.conn, &self.session_handle);
    }
}

fn monotonic_micros() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let ok = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) } == 0;
    if ok {
        (ts.tv_sec as u64 * 1_000_000) + (ts.tv_nsec as u64 / 1_000)
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }
}

fn ei_handshake_with_timeout(
    context: &ei::Context,
    timeout: Duration,
) -> Result<reis::handshake::HandshakeResp, String> {
    let mut handshaker =
        reis::handshake::EiHandshaker::new("Left Hand Control", ContextType::Sender);
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        context
            .read()
            .map_err(|e| format!("EI handshake read: {e}"))?;
        while let Some(result) = context.pending_event() {
            let event = match result {
                reis::PendingRequestResult::Request(event) => event,
                reis::PendingRequestResult::ParseError(e) => {
                    return Err(format!("EI handshake parse error: {e}"));
                }
                reis::PendingRequestResult::InvalidObject(id) => {
                    return Err(format!("EI handshake invalid object: {id}"));
                }
            };
            if let Some(handshake) = handshaker
                .handle_event(event)
                .map_err(|e| format!("EI handshake: {e}"))?
            {
                return Ok(handshake);
            }
        }
        context
            .flush()
            .map_err(|e| format!("EI handshake flush: {e}"))?;
        thread::sleep(Duration::from_millis(10));
    }
    Err("EI handshake timed out".into())
}

fn inject_text(portal: &Proxy, session: &OwnedObjectPath, text: &str) {
    let backend = TEXT_BACKEND
        .lock()
        .map(|guard| guard.backend)
        .unwrap_or(TextBackend::Keycode);
    match backend {
        TextBackend::Clipboard => inject_full_text_via_clipboard(portal, session, text),
        TextBackend::Libei | TextBackend::Keycode | TextBackend::Ydotool => {
            inject_text_keycode(portal, session, text)
        }
    }
}

fn inject_text_keycode(portal: &Proxy, session: &OwnedObjectPath, text: &str) {
    let table = keymap_table();
    let empty: HashMap<String, Value> = HashMap::new();
    for ch in text.chars() {
        let keysym: u32 = unsafe { xkb_utf32_to_keysym(ch as u32) };
        if let Some(entry) = table.get(&keysym) {
            let mods = level_to_mods(entry.level);
            if inject_keycode_combo(portal, session, &empty, mods, entry.evdev) {
                continue;
            }
        }
        inject_keysym(portal, session, &empty, keysym, ch);
    }
}

fn inject_full_text_via_clipboard(portal: &Proxy, session: &OwnedObjectPath, text: &str) {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let empty: HashMap<String, Value> = HashMap::new();

    let mut child = match Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            log::debug!(
                "[portal] wl-copy unavailable in clipboard mode ({e}), falling back to keycode"
            );
            inject_text_keycode(portal, session, text);
            return;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    thread::spawn(move || {
        let _ = child.wait();
    });

    thread::sleep(std::time::Duration::from_millis(50));
    if !inject_paste(portal, session, &empty) {
        log::debug!("[portal] clipboard paste: Ctrl+V injection failed");
    }
}

/// Inject Ctrl+V to paste clipboard content, layout-independently.
///
/// Ctrl is sent via keycode (modifier keys are not layout-sensitive).
/// V is sent via keysym (XKB_KEY_v = 0x76) so the compositor sees the
/// Latin 'v' keysym regardless of the active layout — pressing evdev
/// keycode 47 in a Russian layout would produce 'м', not 'v'.
fn inject_paste(portal: &Proxy, session: &OwnedObjectPath, empty: &HashMap<String, Value>) -> bool {
    if let Err(e) = portal.call_method(
        "NotifyKeyboardKeycode",
        &(session, empty, KEY_LEFTCTRL_EVDEV, STATE_PRESSED),
    ) {
        log::debug!("[portal] paste: Ctrl press failed: {e}");
        return false;
    }
    let v_ok = portal
        .call_method(
            "NotifyKeyboardKeysym",
            &(session, empty, XKB_KEY_V_LOWER, STATE_PRESSED),
        )
        .is_ok()
        && portal
            .call_method(
                "NotifyKeyboardKeysym",
                &(session, empty, XKB_KEY_V_LOWER, STATE_RELEASED),
            )
            .is_ok();
    if !v_ok {
        log::debug!("[portal] paste: V keysym injection failed");
    }
    let _ = portal.call_method(
        "NotifyKeyboardKeycode",
        &(session, empty, KEY_LEFTCTRL_EVDEV, STATE_RELEASED),
    );
    v_ok
}

fn inject_keycode_combo(
    portal: &Proxy,
    session: &OwnedObjectPath,
    empty: &HashMap<String, Value>,
    mods: &[u32],
    key: u32,
) -> bool {
    for &m in mods {
        if let Err(e) =
            portal.call_method("NotifyKeyboardKeycode", &(session, empty, m, STATE_PRESSED))
        {
            log::debug!("[portal] mod keycode {m} press failed: {e}");
            for &m2 in mods {
                let _ = portal.call_method(
                    "NotifyKeyboardKeycode",
                    &(session, empty, m2, STATE_RELEASED),
                );
            }
            return false;
        }
    }
    let ok = portal
        .call_method(
            "NotifyKeyboardKeycode",
            &(session, empty, key, STATE_PRESSED),
        )
        .is_ok()
        && portal
            .call_method(
                "NotifyKeyboardKeycode",
                &(session, empty, key, STATE_RELEASED),
            )
            .is_ok();
    for &m in mods.iter().rev() {
        let _ = portal.call_method(
            "NotifyKeyboardKeycode",
            &(session, empty, m, STATE_RELEASED),
        );
    }
    ok
}

fn inject_keysym(
    portal: &Proxy,
    session: &OwnedObjectPath,
    empty: &HashMap<String, Value>,
    keysym: u32,
    ch: char,
) {
    for state in [STATE_PRESSED, STATE_RELEASED] {
        if let Err(e) = portal.call_method("NotifyKeyboardKeysym", &(session, empty, keysym, state))
        {
            log::debug!("[portal] NotifyKeyboardKeysym({ch:?}, state={state}) failed: {e}");
            break;
        }
    }
}

fn close_session(conn: &Connection, session: &OwnedObjectPath) {
    match Proxy::new(conn, PORTAL_DEST, &**session, IFACE_SESSION) {
        Ok(sp) => {
            if let Err(e) = sp.call_method("Close", &()) {
                log::debug!("[portal] Session.Close failed: {e}");
            }
        }
        Err(e) => log::debug!("[portal] cannot build Session proxy for close: {e}"),
    }
}

// --- Portal handshake ---------------------------------------------------

fn init_portal() -> Result<(Connection, OwnedObjectPath), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    let unique_name = conn
        .unique_name()
        .ok_or_else(|| "session bus did not give us a unique name".to_string())?
        .to_string();
    let sender_escaped = unique_name.trim_start_matches(':').replace('.', "_");

    let portal = Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE)
        .map_err(|e| format!("create RemoteDesktop proxy: {e}"))?;

    log::debug!("[portal] handshake: CreateSession");
    let session_token = gen_token("s");
    let (_, results) = {
        let handle_token = gen_token("c");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token.clone()));
        opts.insert(
            "session_handle_token".into(),
            Value::from(session_token.clone()),
        );
        call_with_response(&conn, &portal, "CreateSession", &(opts,), &request_path)?
    };
    let session_handle = extract_session_handle(&results)?;
    log::debug!("[portal] session: {}", session_handle.as_str());

    let saved_token = load_restore_token();
    if saved_token.is_some() {
        log::debug!("[portal] handshake: SelectDevices (with saved restore_token)");
    } else {
        log::debug!("[portal] handshake: SelectDevices (keyboard, persist_mode=2)");
    }
    {
        let handle_token = gen_token("d");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token));
        opts.insert("types".into(), Value::from(DEVICE_TYPE_KEYBOARD));
        opts.insert("persist_mode".into(), Value::from(PERSIST_MODE_PERMANENT));
        if let Some(tok) = &saved_token {
            opts.insert("restore_token".into(), Value::from(tok.clone()));
        }
        call_with_response(
            &conn,
            &portal,
            "SelectDevices",
            &(&session_handle, opts),
            &request_path,
        )?;
    }

    log::debug!("[portal] handshake: Start (consent dialog if no valid restore_token)");
    let start_results = {
        let handle_token = gen_token("t");
        let request_path = make_request_path(&sender_escaped, &handle_token);
        let mut opts: HashMap<String, Value> = HashMap::new();
        opts.insert("handle_token".into(), Value::from(handle_token));
        let (_, results) = call_with_response(
            &conn,
            &portal,
            "Start",
            &(&session_handle, "", opts),
            &request_path,
        )?;
        results
    };

    if let Some(token) = extract_restore_token(&start_results) {
        if saved_token.as_deref() != Some(token.as_str()) {
            persist_restore_token(&token);
        }
    } else if saved_token.is_some() {
        // The portal dropped our token (revoked / invalidated).
        log::debug!("[portal] no restore_token returned; clearing saved token");
        clear_restore_token();
    }

    log::debug!("[portal] handshake: complete, ready to inject keysyms");
    Ok((conn, session_handle))
}

/// Generic portal pattern: subscribe to the Response signal on the derived
/// Request object path first, then call the method. Return the Response
/// payload `(status, results)`; an error is returned for any non-zero
/// status so the caller can abort the handshake.
fn call_with_response<T>(
    conn: &Connection,
    portal: &Proxy<'_>,
    method: &'static str,
    args: &T,
    request_path: &str,
) -> Result<(u32, HashMap<String, OwnedValue>), String>
where
    T: serde::Serialize + zbus::zvariant::DynamicType,
{
    // Subscribe BEFORE calling to avoid racing the signal.
    let request = Proxy::new(conn, PORTAL_DEST, request_path, IFACE_REQUEST)
        .map_err(|e| format!("{method}: create Request proxy: {e}"))?;
    let mut signals = request
        .receive_signal("Response")
        .map_err(|e| format!("{method}: subscribe Response: {e}"))?;

    portal
        .call_method(method, args)
        .map_err(|e| format!("{method}: call: {e}"))?;

    let (sig_tx, sig_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = sig_tx.send(signals.next());
    });
    let msg = match sig_rx.recv_timeout(Duration::from_secs(30)) {
        Ok(Some(msg)) => msg,
        Ok(None) => return Err(format!("{method}: Response stream closed")),
        Err(_) => return Err(format!("{method}: portal response timed out")),
    };
    let body = msg.body();
    let (status, results): (u32, HashMap<String, OwnedValue>) = body
        .deserialize()
        .map_err(|e| format!("{method}: decode Response: {e}"))?;
    if status != 0 {
        let reason = match status {
            1 => "user cancelled",
            2 => "other error",
            _ => "unknown",
        };
        return Err(format!(
            "{method}: portal returned status {status} ({reason})"
        ));
    }
    Ok((status, results))
}

fn extract_session_handle(
    results: &HashMap<String, OwnedValue>,
) -> Result<OwnedObjectPath, String> {
    let v = results
        .get("session_handle")
        .ok_or_else(|| "CreateSession: response missing `session_handle`".to_string())?;
    // The portal returns the handle as a plain string even though it is
    // semantically an object path.
    let s: String = v
        .try_clone()
        .map_err(|e| format!("session_handle clone: {e}"))?
        .try_into()
        .map_err(|e| format!("session_handle not a string: {e}"))?;
    OwnedObjectPath::try_from(s).map_err(|e| format!("session_handle invalid path: {e}"))
}

fn extract_restore_token(results: &HashMap<String, OwnedValue>) -> Option<String> {
    let v = results.get("restore_token")?;
    let cloned = v.try_clone().ok()?;
    cloned.try_into().ok()
}

// --- Restore-token persistence -----------------------------------------

fn restore_token_path() -> Option<PathBuf> {
    TOKEN_DIR.get().map(|d| d.join(RESTORE_TOKEN_FILE))
}

/// In-memory cache so we don't hit the filesystem on every handshake
/// (handshake is rare, but reads are cheap and this also gives us a sane
/// fallback when the disk path was not registered yet).
static TOKEN_CACHE: Mutex<Option<String>> = Mutex::new(None);

fn load_restore_token() -> Option<String> {
    if let Ok(guard) = TOKEN_CACHE.lock() {
        if let Some(cached) = guard.clone() {
            return Some(cached);
        }
    }
    let path = restore_token_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let token = trimmed.to_string();
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = Some(token.clone());
    }
    Some(token)
}

fn persist_restore_token(token: &str) {
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = Some(token.to_string());
    }
    let Some(path) = restore_token_path() else {
        log::debug!("[portal] cannot persist restore_token (TOKEN_DIR not set)");
        return;
    };
    if let Err(e) = write_token_atomic(&path, token) {
        log::debug!("[portal] failed to persist restore_token: {e}");
    } else {
        log::debug!("[portal] restore_token saved to {}", path.display());
    }
}

fn clear_restore_token() {
    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = None;
    }
    if let Some(path) = restore_token_path() {
        let _ = std::fs::remove_file(path);
    }
}

fn write_token_atomic(path: &Path, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("token.tmp");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

// --- Helpers ------------------------------------------------------------

/// Unique-ish token for portal request/session correlation. The token is
/// scoped per-process so a plain counter (plus prefix) is sufficient.
fn gen_token(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("lhc_{prefix}_{ts}_{n}")
}

fn make_request_path(sender_escaped: &str, handle_token: &str) -> String {
    format!("/org/freedesktop/portal/desktop/request/{sender_escaped}/{handle_token}")
}
