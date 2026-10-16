// Text injection on Linux for `text:` actions.
//
// Two of the modes go through the XDG Desktop Portal RemoteDesktop
// interface, the cross-compositor way to inject input on Wayland (the
// `zwp_virtual_keyboard_v1` protocol is not offered to ordinary clients on
// KDE Plasma 6 or GNOME):
//
//   1. `CreateSession()`        – async, via a `Request` object + `Response` signal.
//   2. `SelectDevices(keyboard)`– declares what we plan to inject; this is
//                                 also where we set `persist_mode = 2` and
//                                 pass back any previously-saved
//                                 `restore_token` so the user does not see
//                                 the consent dialog again.
//   3. `Start()`                – shows the permission dialog (only once
//                                 per machine if persist mode + token work).
//   4. `ConnectToEIS()` (libei modes) or `NotifyKeyboardKeycode()` (keycode
//      mode and the paste shortcut of the clipboard mode).
//
// Each backend owns one session on its own worker thread. The session is
// created on the first text and kept until the mode changes or the session
// breaks, so the consent dialog is not shown on every mapper restart. The
// `restore_token` from `Start` is persisted next to the user data, so later
// app launches restore the permission silently.
//
// Characters are typed as keycodes of the active layout (`NotifyKeyboardKeysym`
// emits keysyms without the needed modifier on KDE, giving wrong characters
// in non-Latin layouts). libei uses the keymap and layout group the
// compositor sends; the keycode mode rebuilds the keymap from the active
// layout name and variant.
//
// Text modes must never silently switch injection mechanisms: hidden fallbacks
// mask failures and make backend debugging unreliable. The only allowed fallback
// is the explicitly selected libei + clipboard mode, which pastes the whole text
// when the keymap lacks a character. It still requires a working libei session.
// Every failure is reported as `CoreEvent::TextInputFailed`, never retried
// through another input method.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::ffi::{CString, c_char};
use std::os::unix::fs::FileExt;
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

use super::text_job::TextRequest;
use crate::events::{CoreEvent, TextInputFailure};
use crate::profile::model::{PasteShortcut, TextMode};

enum XkbContext {}
enum XkbKeymap {}
enum XkbState {}

#[repr(C)]
struct XkbRuleNames {
    rules: *const c_char,
    model: *const c_char,
    layout: *const c_char,
    variant: *const c_char,
    options: *const c_char,
}

const XKB_KEYMAP_FORMAT_TEXT_V1: u32 = 1;

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
    fn xkb_keymap_new_from_buffer(
        ctx: *mut XkbContext,
        buffer: *const c_char,
        length: usize,
        format: u32,
        flags: u32,
    ) -> *mut XkbKeymap;
    fn xkb_keymap_unref(keymap: *mut XkbKeymap);
    fn xkb_keymap_min_keycode(keymap: *mut XkbKeymap) -> u32;
    fn xkb_keymap_max_keycode(keymap: *mut XkbKeymap) -> u32;
    fn xkb_state_new(keymap: *mut XkbKeymap) -> *mut XkbState;
    fn xkb_state_unref(state: *mut XkbState);
    fn xkb_state_update_mask(
        state: *mut XkbState,
        depressed_mods: u32,
        latched_mods: u32,
        locked_mods: u32,
        depressed_layout: u32,
        latched_layout: u32,
        locked_layout: u32,
    ) -> u32;
    fn xkb_state_update_key(state: *mut XkbState, key: u32, direction: u32) -> u32;
    fn xkb_state_key_get_one_sym(state: *mut XkbState, key: u32) -> u32;
    fn xkb_state_serialize_mods(state: *mut XkbState, components: u32) -> u32;
    fn xkb_state_serialize_layout(state: *mut XkbState, components: u32) -> u32;
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

/// Wait for portal responses that need no user decision.
const PORTAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// `Start` may show the consent dialog: leave the user time to read it.
const CONSENT_TIMEOUT: Duration = Duration::from_secs(180);

const STATE_RELEASED: u32 = 0;
const STATE_PRESSED: u32 = 1;

const RESTORE_TOKEN_FILE: &str = "portal-remote-desktop.token";

// --- XKB keymap lookup for layout-aware text injection ------------------

// evdev keycodes for modifier keys used in text injection.
const KEY_LEFTCTRL_EVDEV: u32 = 29;
const KEY_LEFTSHIFT_EVDEV: u32 = 42;
const KEY_V_EVDEV: u32 = 47;
const KEY_INSERT_EVDEV: u32 = 110;

/// Modifiers and key of a paste shortcut as evdev codes.
fn paste_keys(shortcut: PasteShortcut) -> (&'static [u32], u32) {
    match shortcut {
        PasteShortcut::ShiftInsert => (&[KEY_LEFTSHIFT_EVDEV], KEY_INSERT_EVDEV),
        PasteShortcut::CtrlV => (&[KEY_LEFTCTRL_EVDEV], KEY_V_EVDEV),
        PasteShortcut::CtrlShiftV => (&[KEY_LEFTCTRL_EVDEV, KEY_LEFTSHIFT_EVDEV], KEY_V_EVDEV),
    }
}

#[derive(Clone)]
struct KeycodeEntry {
    evdev: u32,
    mods: Vec<u32>,
}

/// keysym → the key and level that type it.
type KeyTable = HashMap<u32, KeycodeEntry>;

type KeymapCache = Option<(String, Arc<KeyTable>)>;
static KEYMAP_CACHE: Mutex<KeymapCache> = Mutex::new(None);

/// Key table of the active layout, rebuilt from its XKB name and variant.
/// Fails when the layout is unknown (only the KDE backend reports it) or
/// XKB cannot compile it: guessing a layout would type wrong characters.
fn layout_key_table() -> Result<Arc<KeyTable>, TextInputFailure> {
    let layout = crate::runtime_state::layout().ok_or(TextInputFailure::LayoutUnknown)?;
    let variant = layout.variant.trim();
    let caps_lock = caps_lock_state()?;
    let options = crate::layout::xkb_options();
    let cache_key = format!("{}\0{variant}\0{caps_lock}\0{options}", layout.short);
    let mut cache = KEYMAP_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((key, table)) = cache.as_ref()
        && *key == cache_key
    {
        return Ok(Arc::clone(table));
    }
    let table = Arc::new(
        table_from_names_with_options(
            &layout.short,
            (!variant.is_empty()).then_some(variant),
            caps_lock,
            &options,
        )
        .ok_or(TextInputFailure::LayoutUnknown)?,
    );
    *cache = Some((cache_key, Arc::clone(&table)));
    Ok(table)
}

#[cfg(test)]
fn table_from_names(layout: &str, variant: Option<&str>) -> Option<KeyTable> {
    table_from_names_with_caps(layout, variant, false)
}

#[cfg(test)]
fn table_from_names_with_caps(layout: &str, variant: Option<&str>, caps: bool) -> Option<KeyTable> {
    table_from_names_with_options(layout, variant, caps, "")
}

fn table_from_names_with_options(
    layout: &str,
    variant: Option<&str>,
    caps: bool,
    options: &str,
) -> Option<KeyTable> {
    let options = CString::new(options).ok()?;
    let layout_c = CString::new(layout).ok()?;
    let variant_c = match variant {
        Some(variant) => Some(CString::new(variant).ok()?),
        None => None,
    };
    let names = XkbRuleNames {
        rules: std::ptr::null(),
        model: std::ptr::null(),
        layout: layout_c.as_ptr(),
        variant: variant_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
        options: options.as_ptr(),
    };
    // SAFETY: the context and keymap are checked for null and released
    // before returning; `names` points to strings that outlive the call.
    unsafe {
        let ctx = xkb_context_new(0);
        if ctx.is_null() {
            return None;
        }
        let keymap = xkb_keymap_new_from_names(ctx, &names, 0);
        xkb_context_unref(ctx);
        if keymap.is_null() {
            log::warn!("[text] XKB cannot compile layout {layout:?} variant {variant:?}");
            return None;
        }
        let mut locked = 0;
        if caps {
            let state = xkb_state_new(keymap);
            if state.is_null() {
                xkb_keymap_unref(keymap);
                return None;
            }
            // Caps Lock may be configured as Shift Lock; derive its actual action.
            let lock_key = (xkb_keymap_min_keycode(keymap)..=xkb_keymap_max_keycode(keymap))
                .find(|key| matches!(xkb_state_key_get_one_sym(state, *key), 0xffe5 | 0xffe6));
            if let Some(key) = lock_key {
                xkb_state_update_key(state, key, 1);
                xkb_state_update_key(state, key, 0);
                locked = xkb_state_serialize_mods(state, 1 << 2);
            }
            xkb_state_unref(state);
            if locked == 0 {
                xkb_keymap_unref(keymap);
                return None;
            }
        }
        let table = table_from_keymap(keymap, 0, locked);
        xkb_keymap_unref(keymap);
        Some(table)
    }
}

/// Key table of `group` in a keymap the compositor sent as XKB text.
fn table_from_keymap_text(text: &[u8], group: u32, locked: u32) -> Option<KeyTable> {
    let text = text.split(|byte| *byte == 0).next().unwrap_or_default();
    // SAFETY: `text` outlives the call; context and keymap are checked for
    // null and released before returning.
    unsafe {
        let ctx = xkb_context_new(0);
        if ctx.is_null() {
            return None;
        }
        let keymap = xkb_keymap_new_from_buffer(
            ctx,
            text.as_ptr().cast(),
            text.len(),
            XKB_KEYMAP_FORMAT_TEXT_V1,
            0,
        );
        xkb_context_unref(ctx);
        if keymap.is_null() {
            return None;
        }
        let table = table_from_keymap(keymap, group, locked);
        xkb_keymap_unref(keymap);
        Some(table)
    }
}

/// Index the keysyms `group` types, keeping for each the entry that needs
/// the fewest modifiers.
///
/// # Safety
/// `keymap` must be a valid keymap.
unsafe fn table_from_keymap(keymap: *mut XkbKeymap, group: u32, locked: u32) -> KeyTable {
    let mut map = KeyTable::new();
    // SAFETY: the caller owns a live keymap. The state is released before return.
    unsafe {
        let state = xkb_state_new(keymap);
        if state.is_null() {
            return map;
        }
        let keys = xkb_keymap_min_keycode(keymap)..=xkb_keymap_max_keycode(keymap);
        // Discover actual modifier key actions instead of assuming that level N
        // always means Shift/AltGr. This also supports remapped and level-5 keys.
        xkb_state_update_mask(state, 0, 0, 0, 0, 0, group);
        let mut modifiers = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for key in keys.clone() {
            let sym = match xkb_state_key_get_one_sym(state, key) {
                0xffe2 => 0xffe1,
                sym => sym,
            };
            if matches!(sym, 0xffe1 | 0xffe2 | 0xfe03 | 0xfe11 | 0xff7e) && seen.insert(sym) {
                modifiers.push(key);
            }
        }
        // Duplicate left/right Shift adds no characters. Keep enumeration bounded
        // even with hostile or unusual compositor keymaps.
        modifiers.truncate(5);
        let mut combinations: Vec<Vec<u32>> = (0usize..(1 << modifiers.len()))
            .map(|mask| {
                modifiers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, key)| (mask & (1 << i) != 0).then_some(*key))
                    .collect()
            })
            .collect();
        combinations.sort_by_key(Vec::len);
        for mods in combinations {
            for key in keys
                .clone()
                .filter(|key| *key >= 9 && !modifiers.contains(key))
            {
                xkb_state_update_mask(state, 0, 0, locked, 0, 0, group);
                let baseline_locked = xkb_state_serialize_mods(state, 1 << 2);
                let baseline_group = xkb_state_serialize_layout(state, 1 << 7);
                for modifier in &mods {
                    xkb_state_update_key(state, *modifier, 1);
                }
                let sym = xkb_state_key_get_one_sym(state, key);
                for modifier in mods.iter().rev() {
                    xkb_state_update_key(state, *modifier, 0);
                }
                // A typing chord must not toggle a lock or leave another group active.
                if sym != 0
                    && xkb_state_serialize_mods(state, 1 << 2) == baseline_locked
                    && xkb_state_serialize_layout(state, 1 << 7) == baseline_group
                {
                    map.entry(sym).or_insert_with(|| KeycodeEntry {
                        evdev: key - 8,
                        mods: mods.iter().map(|key| key - 8).collect(),
                    });
                }
            }
        }
        xkb_state_unref(state);
    }
    map
}

/// Portal keycode mode has no modifier feed: require readable keyboard LEDs
/// rather than silently assuming Caps Lock is off.
fn caps_lock_state() -> Result<bool, TextInputFailure> {
    HardwareState::capture().caps_lock_state()
}

struct HardwareState(Vec<evdev::Device>);

impl HardwareState {
    fn capture() -> Self {
        Self(evdev::enumerate().map(|(_, device)| device).collect())
    }

    fn caps_lock_state(&self) -> Result<bool, TextInputFailure> {
        let mut caps = None;
        for device in &self.0 {
            if device
                .supported_leds()
                .is_some_and(|leds| leds.contains(evdev::LedCode::LED_CAPSL))
            {
                let leds = device.get_led_state().map_err(|e| {
                    TextInputFailure::BackendUnavailable(format!("read Caps Lock state: {e}"))
                })?;
                let value = leds.contains(evdev::LedCode::LED_CAPSL);
                if caps.is_some_and(|previous| previous != value) {
                    return Err(TextInputFailure::BackendUnavailable(
                        "keyboard Caps Lock states disagree".into(),
                    ));
                }
                caps = Some(value);
            }
        }
        caps.ok_or_else(|| {
            TextInputFailure::BackendUnavailable(
                "Caps Lock state is unavailable; use the libei backend".into(),
            )
        })
    }

    fn modifiers_released(&self) -> Result<(), TextInputFailure> {
        let modifiers = [29, 42, 54, 56, 97, 100, 125, 126];
        for device in &self.0 {
            if device.supported_keys().is_some_and(|keys| {
                modifiers
                    .iter()
                    .any(|key| keys.contains(evdev::KeyCode::new(*key)))
            }) {
                let keys = device.get_key_state().map_err(|e| {
                    TextInputFailure::BackendUnavailable(format!("read held modifiers: {e}"))
                })?;
                if modifiers
                    .iter()
                    .any(|key| keys.contains(evdev::KeyCode::new(*key)))
                {
                    return Err(TextInputFailure::BackendUnavailable(
                        "release keyboard modifiers before text input".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn modifiers_released() -> Result<(), TextInputFailure> {
    HardwareState::capture().modifiers_released()
}

// Text control characters need keyboard keysyms, not Unicode keysyms.
fn text_keysym(ch: char) -> u32 {
    match ch {
        '\n' | '\r' => 0xff0d, // Return
        '\t' => 0xff09,        // Tab
        // SAFETY: a pure lookup without pointers.
        _ => unsafe { xkb_utf32_to_keysym(ch as u32) },
    }
}

fn missing_characters(table: &KeyTable, text: &str) -> bool {
    text.chars().any(|ch| !table.contains_key(&text_keysym(ch)))
}

pub(super) fn report(failure: TextInputFailure) {
    log::warn!("[text] text not typed: {failure:?}");
    crate::events::emit(CoreEvent::TextInputFailed(failure));
}

// --- Public API ---------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct TextBackendConfig {
    mode: TextMode,
    paste: PasteShortcut,
    ydotool_path: Option<String>,
    xdotool_path: Option<String>,
}

static TEXT_BACKEND: Mutex<TextBackendConfig> = Mutex::new(TextBackendConfig {
    mode: TextMode::Libei,
    paste: PasteShortcut::ShiftInsert,
    ydotool_path: None,
    xdotool_path: None,
});

/// Select the text injection mode, paste shortcut and external tool paths.
///
/// Changing the mode closes the open portal sessions and forgets earlier
/// start failures; the same mode keeps them, so pushing an unrelated config
/// change neither reopens a declined consent dialog nor drops the session.
pub fn set_text_backend(
    mode: TextMode,
    paste: PasteShortcut,
    ydotool_path: Option<&str>,
    xdotool_path: Option<&str>,
) {
    let path = |path: Option<&str>| {
        path.map(str::trim)
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned)
    };
    let next = TextBackendConfig {
        mode,
        paste,
        ydotool_path: path(ydotool_path),
        xdotool_path: path(xdotool_path),
    };
    let mode_changed = {
        let mut cfg = TEXT_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
        let changed = cfg.mode != next.mode;
        *cfg = next;
        changed
    };
    if mode_changed {
        LIBEI.stop();
        PORTAL.stop();
        retry_text_backend();
    }
    log::debug!("[text] text mode: {}", mode.as_str());
}

/// Forget earlier backend start failures, so the next text starts the
/// backend (and possibly its consent dialog) again.
pub fn retry_text_backend() {
    LIBEI.clear_failure();
    PORTAL.clear_failure();
}

/// Path of the directory where we persist the restore token.
static TOKEN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Register where the worker should read/write the restore token.
/// Must be called once at app startup (before the first injection).
/// Calling it again is a no-op.
pub fn set_token_dir(dir: PathBuf) {
    let _ = TOKEN_DIR.set(dir);
}

/// Type `text` with the selected mode and return when the backend is done.
/// The first call may block on the portal consent dialog, so run it off the
/// mapper thread. Failures are reported as `CoreEvent::TextInputFailed`.
pub(super) fn type_text(text: &str, request: TextRequest) -> Result<(), TextInputFailure> {
    request.check()?;
    let released = Instant::now() + Duration::from_secs(2);
    let hardware = HardwareState::capture();
    while let Err(error) = hardware.modifiers_released() {
        request.check()?;
        if Instant::now() >= released {
            return Err(error);
        }
        thread::sleep(Duration::from_millis(10));
    }
    let cfg = TEXT_BACKEND
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    match cfg.mode {
        TextMode::Ydotool => run_text_tool(
            cfg.ydotool_path.as_deref().unwrap_or("ydotool"),
            &["type", "--escape=0", "--", text],
            text,
            &request,
        ),
        TextMode::Xdotool => run_text_tool(
            cfg.xdotool_path.as_deref().unwrap_or("xdotool"),
            &["type", "--clearmodifiers", "--", text],
            text,
            &request,
        ),
        TextMode::Libei | TextMode::LibeiPure => {
            LIBEI.type_text(text, cfg.mode, cfg.paste, request)
        }
        TextMode::Keycode | TextMode::Clipboard => {
            PORTAL.type_text(text, cfg.mode, cfg.paste, request)
        }
    }
}

/// Run a typing tool with a deadline that grows with the text, so a hung
/// tool (e.g. ydotool without ydotoold) cannot stall the text queue forever.
fn run_text_tool(
    program: &str,
    args: &[&str],
    text: &str,
    request: &TextRequest,
) -> Result<(), TextInputFailure> {
    request.check()?;
    let timeout_ms = 5_000 + 50 * text.chars().count() as u64;
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    match crate::exec::run_cmd_with_cancel(&mut cmd, timeout_ms, || request.check().is_err()) {
        Some(output) if output.status.success() => request.check(),
        Some(output) => Err(TextInputFailure::Tool(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
        None => Err(TextInputFailure::Tool(format!(
            "{program} failed to start or timed out"
        ))),
    }
}

// --- Backend workers ----------------------------------------------------

struct Cmd {
    text: String,
    mode: TextMode,
    paste: PasteShortcut,
    /// Dropped without a send when the worker hit a fatal error and exits.
    done: Sender<Result<(), TextInputFailure>>,
    request: TextRequest,
}

/// A portal session on its own thread, started on first use.
struct Worker {
    tx: Mutex<Option<Sender<Cmd>>>,
    /// Why the last start failed. Later texts are reported as failed instead
    /// of starting again, which could block and show the dialog every time.
    failure: Mutex<Option<String>>,
    /// Set by `stop` while a text holds `tx`, e.g. during the consent dialog.
    stop_pending: AtomicBool,
    start: fn(TextRequest) -> Result<Sender<Cmd>, String>,
}

static LIBEI: Worker =
    Worker::new(|request| start_worker("lhc-libei", LibeiBackend::new, libei_worker, request));
static PORTAL: Worker =
    Worker::new(|request| start_worker("lhc-portal", PortalBackend::new, portal_worker, request));

impl Worker {
    const fn new(start: fn(TextRequest) -> Result<Sender<Cmd>, String>) -> Self {
        Self {
            tx: Mutex::new(None),
            failure: Mutex::new(None),
            stop_pending: AtomicBool::new(false),
            start,
        }
    }

    fn type_text(
        &self,
        text: &str,
        mode: TextMode,
        paste: PasteShortcut,
        request: TextRequest,
    ) -> Result<(), TextInputFailure> {
        let mut slot = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            if let Some(error) = self
                .failure
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
            {
                return Err(TextInputFailure::BackendUnavailable(error));
            }
            match (self.start)(request.clone()) {
                Ok(tx) => *slot = Some(tx),
                Err(error) => {
                    request.check()?;
                    *self.failure.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.clone());
                    return Err(TextInputFailure::BackendUnavailable(error));
                }
            }
        }
        request.wait_for_target()?;
        let (done, done_rx) = mpsc::channel();
        let cmd = Cmd {
            text: text.to_owned(),
            mode,
            paste,
            done,
            request: request.clone(),
        };
        if !slot.as_ref().is_some_and(|tx| tx.send(cmd).is_ok()) {
            *slot = None;
            return Err(TextInputFailure::BackendUnavailable(
                "the input session has ended".into(),
            ));
        }
        // The worker reports its own fatal error; forget it so the next
        // text opens a new session.
        let result = loop {
            match done_rx.recv_timeout(Duration::from_millis(20)) {
                Ok(result) => break result,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    *slot = None;
                    break Err(TextInputFailure::BackendUnavailable(
                        "the input session has ended".into(),
                    ));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Err(error) = request.check() {
                        *slot = None;
                        break Err(error);
                    }
                }
            }
        };
        if self.stop_pending.swap(false, Ordering::SeqCst)
            || matches!(result, Err(TextInputFailure::BackendUnavailable(_)))
        {
            *slot = None;
        }
        result
    }

    /// Close the session; the worker exits after its current text. Never
    /// blocks: it is called from the mapper thread.
    fn stop(&self) {
        match self.tx.try_lock() {
            Ok(mut slot) => {
                slot.take();
            }
            Err(std::sync::TryLockError::Poisoned(e)) => {
                e.into_inner().take();
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                self.stop_pending.store(true, Ordering::SeqCst);
            }
        }
    }

    fn clear_failure(&self) {
        self.failure
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}

/// Start a worker thread and wait until its session is ready. There is no
/// cancellation delay: waiting for readiness periodically checks the job lifetime.
fn start_worker<B: 'static>(
    name: &'static str,
    init: fn(&TextRequest) -> Result<B, String>,
    run: fn(B, Receiver<Cmd>),
    request: TextRequest,
) -> Result<Sender<Cmd>, String> {
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let init_request = request.clone();
    thread::Builder::new()
        .name(name.into())
        .spawn(move || match init(&init_request) {
            Ok(backend) => {
                let _ = ready_tx.send(Ok(()));
                run(backend, rx);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|e| format!("start {name}: {e}"))?;
    loop {
        match ready_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(result) => {
                result?;
                return Ok(tx);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(format!("{name} stopped while starting"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Consent can temporarily move focus; only lifetime cancellation
                // is checked until Start returns and the original window is restored.
                request.check_lifetime().map_err(|e| format!("{e:?}"))?;
            }
        }
    }
}

struct PortalBackend {
    conn: Connection,
    session_handle: OwnedObjectPath,
    portal: Proxy<'static>,
}

impl PortalBackend {
    fn new(request: &TextRequest) -> Result<Self, String> {
        let (conn, session_handle) = init_portal(request)?;
        match Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, IFACE_REMOTE) {
            Ok(portal) => Ok(Self {
                conn,
                session_handle,
                portal,
            }),
            Err(e) => {
                close_session(&conn, &session_handle);
                Err(format!("create RemoteDesktop proxy: {e}"))
            }
        }
    }
}

fn portal_worker(backend: PortalBackend, rx: Receiver<Cmd>) {
    log::debug!("[portal] backend online");
    for cmd in rx {
        let session = &backend.session_handle;
        let result = cmd.request.check().and_then(|()| match cmd.mode {
            TextMode::Clipboard => {
                paste_text(&backend.portal, session, &cmd.text, cmd.paste, &cmd.request)
            }
            TextMode::Keycode => {
                type_text_keycode(&backend.portal, session, &cmd.text, &cmd.request)
            }
            _ => Err(TextInputFailure::BackendUnavailable(
                "invalid portal mode".into(),
            )),
        });
        let fatal = matches!(result, Err(TextInputFailure::BackendUnavailable(_)));
        let _ = cmd.done.send(result);
        if fatal {
            break;
        }
    }
    close_session(&backend.conn, &backend.session_handle);
}

fn libei_worker(mut backend: LibeiBackend, rx: Receiver<Cmd>) {
    log::debug!("[libei] backend online");
    for cmd in rx {
        let fallback = (cmd.mode == TextMode::Libei).then_some(cmd.paste);
        let result = backend.type_text(&cmd.text, fallback, &cmd.request);
        let fatal = matches!(result, Err(TextInputFailure::BackendUnavailable(_)));
        let _ = cmd.done.send(result);
        if fatal {
            break;
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
    /// XKB keymap text of the keyboard device, as the compositor sent it.
    keymap: Option<Vec<u8>>,
    /// Active layout group the compositor reported.
    group: Option<u32>,
    /// Key table built from `keymap` for a group.
    table: Option<((u32, u32), Arc<KeyTable>)>,
    locked: u32,
    depressed: u32,
    latched: u32,
}

impl LibeiBackend {
    fn new(request: &TextRequest) -> Result<Self, String> {
        let (conn, session_handle) = init_portal(request)?;
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
            keymap: None,
            group: None,
            table: None,
            locked: 0,
            depressed: 0,
            latched: 0,
        };
        if let Err(e) = backend.wait_until(Duration::from_secs(5), Self::has_keyboard) {
            close_session(&backend.conn, &backend.session_handle);
            return Err(e);
        }
        Ok(backend)
    }

    /// Type `text`, pasting it with `clipboard_fallback` when the layout
    /// lacks a character. `Err` means the EI connection is unusable; other
    /// failures are reported and leave the session open.
    fn type_text(
        &mut self,
        text: &str,
        clipboard_fallback: Option<PasteShortcut>,
        request: &TextRequest,
    ) -> Result<(), TextInputFailure> {
        request.check()?;
        self.pump_events()
            .map_err(TextInputFailure::BackendUnavailable)?;
        if !self.has_keyboard() {
            self.wait_until(Duration::from_secs(2), Self::has_keyboard)
                .map_err(TextInputFailure::BackendUnavailable)?;
        }
        // Events sent while the compositor paused the device are a protocol
        // error, e.g. on the lock screen.
        if !self.keyboard_emulating
            && self
                .wait_until(Duration::from_secs(1), |backend| backend.keyboard_emulating)
                .is_err()
        {
            return Err(TextInputFailure::BackendUnavailable(
                "the compositor paused the input device".into(),
            ));
        }
        if self.depressed != 0 || self.latched != 0 {
            return Err(TextInputFailure::BackendUnavailable(
                "release or unlatch keyboard modifiers before text input".into(),
            ));
        }
        self.synchronize(request)
            .map_err(TextInputFailure::BackendUnavailable)?;
        let hardware = HardwareState::capture();
        hardware.modifiers_released()?;
        let table = self.key_table()?;
        request.check()?;
        let text = text.replace("\r\n", "\n");
        if missing_characters(&table, &text) {
            if let Some(shortcut) = clipboard_fallback {
                return self.paste_text(&text, shortcut, request);
            }
            return Err(TextInputFailure::MissingCharacters);
        }
        for ch in text.chars() {
            self.pump_events()
                .map_err(TextInputFailure::BackendUnavailable)?;
            request.check()?;
            if self.group.is_some_and(|group| {
                self.table
                    .as_ref()
                    .is_some_and(|(cached, _)| cached.0 != group || cached.1 != self.locked)
            }) || self.depressed != 0
                || self.latched != 0
            {
                return Err(TextInputFailure::BackendUnavailable(
                    "keyboard state changed during text input".into(),
                ));
            }
            hardware.modifiers_released()?;
            if let Some(entry) = table.get(&text_keysym(ch)) {
                self.inject_keycode_combo(&entry.mods, entry.evdev, request)
                    .map_err(TextInputFailure::BackendUnavailable)?;
            }
        }
        Ok(())
    }

    /// Key table of the layout the compositor uses now. Without a keymap
    /// from the compositor, fail rather than guess its options. Without
    /// a reported group, use the active layout's index.
    fn key_table(&mut self) -> Result<Arc<KeyTable>, TextInputFailure> {
        let Some(keymap) = self.keymap.as_deref() else {
            return Err(TextInputFailure::BackendUnavailable(
                "the compositor did not provide a keyboard keymap".into(),
            ));
        };
        let group = self
            .group
            .or_else(|| crate::runtime_state::layout().map(|layout| layout.index))
            .ok_or(TextInputFailure::LayoutUnknown)?;
        if let Some((cached, table)) = &self.table
            && *cached == (group, self.locked)
        {
            return Ok(Arc::clone(table));
        }
        let table = Arc::new(
            table_from_keymap_text(keymap, group, self.locked).ok_or_else(|| {
                TextInputFailure::BackendUnavailable("cannot read the compositor keymap".into())
            })?,
        );
        self.table = Some(((group, self.locked), Arc::clone(&table)));
        Ok(table)
    }

    fn paste_text(
        &mut self,
        text: &str,
        shortcut: PasteShortcut,
        request: &TextRequest,
    ) -> Result<(), TextInputFailure> {
        let clipboard =
            super::clipboard::Paste::prepare(text).map_err(TextInputFailure::Clipboard)?;
        request.check()?;
        modifiers_released()?;
        clipboard.arm().map_err(TextInputFailure::Clipboard)?;
        let (mods, key) = paste_keys(shortcut);
        let result = self
            .inject_keycode_combo(mods, key, request)
            .map_err(TextInputFailure::BackendUnavailable);
        let transfer = clipboard
            .finish(result.is_ok())
            .map_err(TextInputFailure::Clipboard);
        result.and(transfer)
    }

    fn inject_keycode_combo(
        &mut self,
        mods: &[u32],
        key: u32,
        request: &TextRequest,
    ) -> Result<(), String> {
        let mut pressed = Vec::new();
        let mut result = (|| {
            for &m in mods {
                self.inject_key(m, KeyState::Press)?;
                pressed.push(m);
            }
            self.inject_key(key, KeyState::Press)?;
            self.inject_key(key, KeyState::Released)
        })();
        if result.is_err() {
            let _ = self.inject_key(key, KeyState::Released);
        }
        for m in pressed.into_iter().rev() {
            if let Err(error) = self.inject_key(m, KeyState::Released)
                && result.is_ok()
            {
                result = Err(error);
            }
        }
        result?;
        self.synchronize(request)
    }

    fn synchronize(&mut self, request: &TextRequest) -> Result<(), String> {
        let done = Arc::new(AtomicBool::new(false));
        let completed = done.clone();
        let callback = self.connection.connection().sync(1);
        self.converter
            .add_callback_handler(callback, move |_| completed.store(true, Ordering::SeqCst));
        self.context
            .flush()
            .map_err(|e| format!("EI sync flush: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !done.load(Ordering::SeqCst) {
            request.check().map_err(|e| format!("{e:?}"))?;
            self.pump_events()?;
            if Instant::now() >= deadline {
                return Err("EI input acknowledgement timed out".into());
            }
            if !done.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
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

    fn wait_until(&mut self, timeout: Duration, ready: fn(&Self) -> bool) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            self.pump_events()?;
            if ready(self) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("EI keyboard device is not available".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
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

    fn is_keyboard(&self, device: &ei::Device) -> bool {
        self.keyboard_device
            .as_ref()
            .is_some_and(|(keyboard, _)| keyboard == device)
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
                    self.keymap = e.device.keymap().and_then(read_keymap);
                    self.table = None;
                    self.group = None;
                    self.locked = 0;
                    self.depressed = 0;
                    self.latched = 0;
                    self.keyboard_emulating = false;
                    self.start_keyboard_emulation(&device);
                    self.keyboard_device = Some((device, keyboard));
                }
            }
            EiEvent::KeyboardModifiers(e) if self.is_keyboard(e.device.device()) => {
                self.group = Some(e.group);
                self.locked = e.locked;
                self.depressed = e.depressed;
                self.latched = e.latched;
            }
            EiEvent::DevicePaused(e) if self.is_keyboard(e.device.device()) => {
                self.keyboard_emulating = false;
                self.depressed = 0;
                self.latched = 0;
                self.locked = 0;
                self.group = None;
                self.table = None;
            }
            EiEvent::DeviceResumed(e) if self.is_keyboard(e.device.device()) => {
                self.start_keyboard_emulation(e.device.device());
            }
            EiEvent::DeviceRemoved(e) if self.is_keyboard(e.device.device()) => {
                self.keyboard_device = None;
                self.keyboard_emulating = false;
                self.keymap = None;
                self.table = None;
                self.group = None;
                self.locked = 0;
                self.depressed = 0;
                self.latched = 0;
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

/// Read the XKB keymap an EI keyboard device carries.
fn read_keymap(keymap: &reis::event::Keymap) -> Option<Vec<u8>> {
    if keymap.type_ != ei::keyboard::KeymapType::Xkb {
        return None;
    }
    let file = std::fs::File::from(keymap.fd.try_clone().ok()?);
    let mut text = vec![0; keymap.size as usize];
    match file.read_exact_at(&mut text, 0) {
        Ok(()) => Some(text),
        Err(e) => {
            log::warn!("[libei] cannot read the device keymap: {e}");
            None
        }
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

fn type_text_keycode(
    portal: &Proxy,
    session: &OwnedObjectPath,
    text: &str,
    request: &TextRequest,
) -> Result<(), TextInputFailure> {
    let hardware = HardwareState::capture();
    hardware.modifiers_released()?;
    let table = layout_key_table()?;
    let layout = crate::runtime_state::layout();
    let caps = hardware.caps_lock_state()?;
    let text = text.replace("\r\n", "\n");
    if missing_characters(&table, &text) {
        return Err(TextInputFailure::MissingCharacters);
    }
    for ch in text.chars() {
        request.check()?;
        hardware.modifiers_released()?;
        if layout != crate::runtime_state::layout() || caps != hardware.caps_lock_state()? {
            return Err(TextInputFailure::BackendUnavailable(
                "keyboard state changed during text input".into(),
            ));
        }
        if let Some(entry) = table.get(&text_keysym(ch)) {
            inject_keycode_combo(portal, session, &entry.mods, entry.evdev).map_err(|e| {
                TextInputFailure::BackendUnavailable(format!("NotifyKeyboardKeycode: {e}"))
            })?;
        }
    }
    Ok(())
}

fn paste_text(
    portal: &Proxy,
    session: &OwnedObjectPath,
    text: &str,
    shortcut: PasteShortcut,
    request: &TextRequest,
) -> Result<(), TextInputFailure> {
    let clipboard = super::clipboard::Paste::prepare(text).map_err(TextInputFailure::Clipboard)?;
    request.check()?;
    modifiers_released()?;
    clipboard.arm().map_err(TextInputFailure::Clipboard)?;
    let (mods, key) = paste_keys(shortcut);
    let result = inject_keycode_combo(portal, session, mods, key)
        .map_err(|e| TextInputFailure::BackendUnavailable(format!("NotifyKeyboardKeycode: {e}")));
    let transfer = clipboard
        .finish(result.is_ok())
        .map_err(TextInputFailure::Clipboard);
    result.and(transfer)
}

/// Call `NotifyKeyboardKeycode`. The portal declares the key argument as
/// `i` (signed); sending `u` is rejected with InvalidArgs.
fn notify_key(
    portal: &Proxy,
    session: &OwnedObjectPath,
    key: u32,
    state: u32,
) -> zbus::Result<zbus::message::Message> {
    let empty: HashMap<String, Value> = HashMap::new();
    let key =
        i32::try_from(key).map_err(|_| zbus::Error::Failure(format!("key {key} out of range")))?;
    portal.call_method("NotifyKeyboardKeycode", &(session, empty, key, state))
}

/// Press `mods`, tap `key` and release `mods`. Pressed modifiers are
/// released even when a later call fails.
fn inject_keycode_combo(
    portal: &Proxy,
    session: &OwnedObjectPath,
    mods: &[u32],
    key: u32,
) -> zbus::Result<()> {
    let mut pressed = 0;
    let mut result = Ok(());
    for &m in mods {
        if let Err(e) = notify_key(portal, session, m, STATE_PRESSED) {
            result = Err(e);
            break;
        }
        pressed += 1;
    }
    if result.is_ok() {
        result = notify_key(portal, session, key, STATE_PRESSED)
            .and_then(|_| notify_key(portal, session, key, STATE_RELEASED))
            .map(|_| ());
    }
    if result.is_err() {
        let _ = notify_key(portal, session, key, STATE_RELEASED);
    }
    for &m in mods[..pressed].iter().rev() {
        if let Err(error) = notify_key(portal, session, m, STATE_RELEASED)
            && result.is_ok()
        {
            result = Err(error);
        }
    }
    result
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

fn init_portal(text_request: &TextRequest) -> Result<(Connection, OwnedObjectPath), String> {
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
        call_with_response(
            &conn,
            &portal,
            "CreateSession",
            &(opts,),
            &request_path,
            PORTAL_RESPONSE_TIMEOUT,
            text_request,
        )?
    };
    let session_handle = extract_session_handle(&results)?;
    let mut pending = PendingSession {
        conn: &conn,
        handle: &session_handle,
        active: true,
    };
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
            PORTAL_RESPONSE_TIMEOUT,
            text_request,
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
            CONSENT_TIMEOUT,
            text_request,
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

    log::debug!("[portal] handshake: complete");
    pending.active = false;
    drop(pending);
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
    timeout: Duration,
    text_request: &TextRequest,
) -> Result<(u32, HashMap<String, OwnedValue>), String>
where
    T: serde::Serialize + zbus::zvariant::DynamicType,
{
    // Subscribe BEFORE calling to avoid racing the signal.
    let request = Proxy::new(conn, PORTAL_DEST, request_path, IFACE_REQUEST)
        .map_err(|e| format!("{method}: create Request proxy: {e}"))?;
    use zbus::export::futures_core::Stream;
    let deadline = Instant::now() + timeout;
    let valid = || text_request.check_lifetime().map_err(|e| format!("{e:?}"));
    // Cargo feature unification can select zbus's Tokio executor (via ksni).
    // Keep polling and SignalStream destruction inside the matching runtime.
    let response = zbus::block_on(async {
        let mut signals = poll_until(request.inner().receive_signal("Response"), deadline, valid)?
            .map_err(|e| format!("{method}: subscribe Response: {e}"))?;
        poll_until(portal.inner().call_method(method, args), deadline, valid)?
            .map_err(|e| format!("{method}: call: {e}"))?;
        poll_until(
            std::future::poll_fn(|cx| std::pin::Pin::new(&mut signals).poll_next(cx)),
            deadline,
            valid,
        )?
        .ok_or_else(|| format!("{method}: Response stream closed"))
    });
    let msg = match response {
        Ok(msg) => msg,
        Err(error) => {
            let _ = request.call_noreply("Close", &());
            return Err(error);
        }
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

struct PendingSession<'a> {
    conn: &'a Connection,
    handle: &'a OwnedObjectPath,
    active: bool,
}

impl Drop for PendingSession<'_> {
    fn drop(&mut self) {
        if self.active {
            close_session(self.conn, self.handle);
        }
    }
}

/// Poll the bus's asynchronous operations without retaining a blocked signal-reader thread.
fn poll_until<T>(
    future: impl std::future::Future<Output = T>,
    deadline: Instant,
    valid: impl Fn() -> Result<(), String>,
) -> Result<T, String> {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        valid()?;
        if Instant::now() >= deadline {
            return Err("portal operation timed out".into());
        }
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return Ok(value);
        }
        thread::sleep(Duration::from_millis(5));
    }
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
    if let Ok(guard) = TOKEN_CACHE.lock()
        && let Some(cached) = guard.clone()
    {
        return Some(cached);
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

#[cfg(test)]
mod tests {
    #[test]
    fn text_control_characters_use_keyboard_keysyms() {
        assert_eq!(super::text_keysym('\n'), 0xff0d);
        assert_eq!(super::text_keysym('\r'), 0xff0d);
        assert_eq!(super::text_keysym('\t'), 0xff09);
        assert_eq!(super::text_keysym(' '), 0x20);
        assert_eq!(super::text_keysym('a'), 0x61);
    }

    use super::*;
    use std::collections::HashMap;

    fn has(table: &KeyTable, ch: char) -> bool {
        table.contains_key(&text_keysym(ch))
    }

    #[test]
    fn layout_tables_follow_layout_and_variant() {
        let us = table_from_names("us", None).expect("us layout");
        let ru = table_from_names("ru", None).expect("ru layout");
        let dvorak = table_from_names("us", Some("dvorak")).expect("us(dvorak) layout");
        assert!(has(&us, 'q') && !has(&us, 'й'));
        assert!(has(&ru, 'й') && !has(&ru, 'q'));
        assert!(missing_characters(&us, "hi 👋"));
        assert!(!missing_characters(&us, "Hello,\tworld!\n"));
        // A user label such as "lat" is not an XKB variant.
        assert!(table_from_names("latam", Some("lat")).is_none());
        assert!(has(&dvorak, 'v'));
    }

    #[test]
    fn keymap_tables_use_the_requested_group() {
        let layout = CString::new("us,ru").expect("layout");
        let names = XkbRuleNames {
            rules: std::ptr::null(),
            model: std::ptr::null(),
            layout: layout.as_ptr(),
            variant: std::ptr::null(),
            options: std::ptr::null(),
        };
        // SAFETY: test-only FFI with null checks; everything is released.
        let (latin, cyrillic) = unsafe {
            let ctx = xkb_context_new(0);
            assert!(!ctx.is_null());
            let keymap = xkb_keymap_new_from_names(ctx, &names, 0);
            xkb_context_unref(ctx);
            assert!(!keymap.is_null());
            let tables = (
                table_from_keymap(keymap, 0, 0),
                table_from_keymap(keymap, 1, 0),
            );
            xkb_keymap_unref(keymap);
            tables
        };
        assert!(has(&latin, 'q') && !has(&latin, 'й'));
        assert!(has(&cyrillic, 'й') && !has(&cyrillic, 'q'));
        // Both groups share the same key.
        assert_eq!(
            latin[&text_keysym('q')].evdev,
            cyrillic[&text_keysym('й')].evdev
        );
    }

    #[test]
    fn caps_lock_uses_shift_to_type_lowercase() {
        let table = table_from_names_with_caps("us", None, true).expect("us layout");
        assert_eq!(table[&text_keysym('a')].mods, vec![KEY_LEFTSHIFT_EVDEV]);
        assert!(table[&text_keysym('A')].mods.is_empty());
    }

    #[test]
    fn shift_lock_is_not_mistaken_for_caps_lock() {
        let table = table_from_names_with_options("us", None, true, "caps:shiftlock")
            .expect("Shift Lock layout");
        assert!(table[&text_keysym('A')].mods.is_empty());
        assert!(table[&text_keysym('!')].mods.is_empty());
        assert!(!has(&table, 'a'));
    }

    #[test]
    fn extended_levels_and_remapped_modifiers_are_discovered() {
        let neo = table_from_names("de", Some("neo")).expect("Neo layout");
        assert!(has(&neo, '∀'), "level 7 must not be discarded");
        let remapped =
            table_from_names_with_options("us", Some("altgr-intl"), false, "lv3:caps_switch")
                .expect("remapped AltGr");
        assert!(has(&remapped, 'é'));
        assert!(
            remapped[&text_keysym('é')]
                .mods
                .iter()
                .any(|key| *key == 58 || *key == 100)
        );
    }

    #[test]
    fn portal_wait_is_cancellable_without_a_signal_reader_thread() {
        let start = Instant::now();
        let result = poll_until(
            std::future::pending::<()>(),
            start + Duration::from_secs(5),
            || {
                if start.elapsed() >= Duration::from_millis(20) {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err(), "cancelled");
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(
            poll_until(
                std::future::ready(42),
                Instant::now() + Duration::from_secs(1),
                || Ok(())
            )
            .unwrap(),
            42
        );
    }

    #[test]
    fn extract_restore_token_reads_token_value() {
        let mut map: HashMap<String, OwnedValue> = HashMap::new();
        assert!(extract_restore_token(&map).is_none());

        map.insert(
            "restore_token".to_string(),
            zbus::zvariant::Str::from("my-secret-token").into(),
        );
        assert_eq!(
            extract_restore_token(&map).as_deref(),
            Some("my-secret-token")
        );
    }

    #[test]
    fn make_request_path_formats_correctly() {
        assert_eq!(
            make_request_path("org_example", "tok_1"),
            "/org/freedesktop/portal/desktop/request/org_example/tok_1"
        );
    }

    #[test]
    fn gen_token_produces_unique_values() {
        let a = gen_token("test");
        let b = gen_token("test");
        assert_ne!(a, b);
        assert!(a.starts_with("lhc_test_"));
    }

    #[test]
    fn token_persistence_roundtrip() {
        let temp = std::env::temp_dir().join(format!("lhc-portal-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp).expect("create temp dir");
        set_token_dir(temp.clone());

        clear_restore_token();
        assert!(load_restore_token().is_none());

        persist_restore_token("secret-42");
        assert_eq!(load_restore_token().as_deref(), Some("secret-42"));

        persist_restore_token("secret-99");
        assert_eq!(load_restore_token().as_deref(), Some("secret-99"));

        clear_restore_token();
        assert!(load_restore_token().is_none());

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn write_token_atomic_creates_readable_file() {
        let temp = std::env::temp_dir().join(format!("lhc-atomic-test-{}", std::process::id()));
        write_token_atomic(&temp, "hello").expect("write");
        let contents = std::fs::read_to_string(&temp).expect("read");
        assert_eq!(contents, "hello");
        let _ = std::fs::remove_file(&temp);
    }
}
