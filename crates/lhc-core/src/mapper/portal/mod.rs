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

mod hardware;
mod inject;
mod keymap;
mod libei;
mod session;
mod token;
mod worker;

use self::hardware::*;
pub(super) use self::inject::close_session;
use self::inject::*;
use self::keymap::*;
use self::libei::*;
use self::session::*;
use self::token::*;
use self::worker::*;

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

// --- Portal DBus identifiers --------------------------------------------

pub(super) use crate::platform::dbus::{PORTAL_PATH, PORTAL_SERVICE as PORTAL_DEST};
const IFACE_REMOTE: &str = "org.freedesktop.portal.RemoteDesktop";
use crate::platform::dbus::PORTAL_REQUEST_INTERFACE as IFACE_REQUEST;
const IFACE_SESSION: &str = "org.freedesktop.portal.Session";

/// Keyboard capability bit in the RemoteDesktop "types" field.
const DEVICE_TYPE_KEYBOARD: u32 = 1;

/// `persist_mode` for SelectDevices: 2 = persist until explicitly revoked.
const PERSIST_MODE_PERMANENT: u32 = 2;

/// Wait for portal responses that need no user decision.
const PORTAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// `Start` may show the consent dialog: leave the user time to read it.
const CONSENT_TIMEOUT: Duration = Duration::from_secs(180);
/// How long typing waits for the user to release physical modifiers.
const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_secs(2);
/// Pause between checks of a condition that is polled.
const POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Pause between checks of a worker reply, so a cancelled request is noticed.
const REPLY_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Pause between polls of a portal future.
const FUTURE_POLL_INTERVAL: Duration = Duration::from_millis(5);
/// Timeout of a single D-Bus method call to the portal.
const DBUS_METHOD_TIMEOUT: Duration = Duration::from_secs(2);
/// EI handshake and the first keyboard device of a new session.
const EI_SETUP_TIMEOUT: Duration = Duration::from_secs(5);
/// Keyboard device of a running session that has to be (re)announced.
const EI_KEYBOARD_TIMEOUT: Duration = Duration::from_secs(2);
/// Start of keyboard emulation after the device is known.
const EI_EMULATION_TIMEOUT: Duration = Duration::from_secs(1);
/// Acknowledgement of sent input by the compositor.
const EI_ACK_TIMEOUT: Duration = Duration::from_secs(2);
const EI_ACK_POLL_INTERVAL: Duration = Duration::from_millis(1);
/// Deadline of a typing tool: a base plus a share per character.
const TEXT_TOOL_BASE_TIMEOUT_MS: u64 = 5_000;
const TEXT_TOOL_PER_CHAR_MS: u64 = 50;

const STATE_RELEASED: u32 = 0;
const STATE_PRESSED: u32 = 1;

const RESTORE_TOKEN_FILE: &str = "portal-remote-desktop.token";
const CLIPBOARD_RESTORE_TOKEN_FILE: &str = "portal-remote-desktop-clipboard.token";

pub(super) fn report(failure: TextInputFailure) {
    log::warn!("[text] text not typed: {failure:?}");
    crate::events::emit(CoreEvent::TextInputFailed(failure));
}

// --- Public API ---------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct TextBackendConfig {
    mode: TextMode,
    paste: PasteShortcut,
    /// Used instead of `paste` when the destination is a terminal.
    terminal_paste: Option<PasteShortcut>,
    ydotool_path: Option<String>,
    xdotool_path: Option<String>,
}

static TEXT_BACKEND: Mutex<TextBackendConfig> = Mutex::new(TextBackendConfig {
    mode: TextMode::Libei,
    paste: PasteShortcut::ShiftInsert,
    terminal_paste: Some(PasteShortcut::CtrlShiftV),
    ydotool_path: None,
    xdotool_path: None,
});

/// Select the text injection mode, paste shortcuts and external tool paths.
///
/// Changing the mode closes the open portal sessions and forgets earlier
/// start failures; the same mode keeps them, so pushing an unrelated config
/// change neither reopens a declined consent dialog nor drops the session.
pub fn set_text_backend(
    mode: TextMode,
    paste: PasteShortcut,
    terminal_paste: Option<PasteShortcut>,
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
        terminal_paste,
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

/// Read the clipboard ahead of a likely text input, e.g. when a popup opens,
/// if the selected mode may paste through it.
pub fn prepare_text_input() {
    let mode = TEXT_BACKEND.lock().unwrap_or_else(|e| e.into_inner()).mode;
    if matches!(mode, TextMode::Libei | TextMode::Clipboard) {
        super::clipboard::warm();
    }
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
    let released = Instant::now() + MODIFIER_RELEASE_TIMEOUT;
    let hardware = HardwareState::capture();
    while let Err(error) = hardware.modifiers_released() {
        request.check()?;
        if Instant::now() >= released {
            return Err(error);
        }
        thread::sleep(POLL_INTERVAL);
    }
    let cfg = TEXT_BACKEND
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let paste = cfg
        .terminal_paste
        .filter(|_| request.is_terminal())
        .unwrap_or(cfg.paste);
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
        TextMode::Libei | TextMode::LibeiPure => LIBEI.type_text(text, cfg.mode, paste, request),
        TextMode::Keycode | TextMode::Clipboard => PORTAL.type_text(text, cfg.mode, paste, request),
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
    let timeout_ms =
        TEXT_TOOL_BASE_TIMEOUT_MS + TEXT_TOOL_PER_CHAR_MS * text.chars().count() as u64;
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

        clear_restore_token(TokenScope::Keyboard);
        assert!(load_restore_token(TokenScope::Keyboard).is_none());

        persist_restore_token(TokenScope::Keyboard, "secret-42");
        assert_eq!(
            load_restore_token(TokenScope::Keyboard).as_deref(),
            Some("secret-42")
        );

        assert!(load_restore_token(TokenScope::Clipboard).is_none());
        persist_restore_token(TokenScope::Clipboard, "clipboard-grant");
        assert_eq!(
            load_restore_token(TokenScope::Clipboard).as_deref(),
            Some("clipboard-grant")
        );
        assert_ne!(
            restore_token_path(TokenScope::Keyboard),
            restore_token_path(TokenScope::Clipboard)
        );

        persist_restore_token(TokenScope::Keyboard, "secret-99");
        assert_eq!(
            load_restore_token(TokenScope::Keyboard).as_deref(),
            Some("secret-99")
        );

        clear_restore_token(TokenScope::Keyboard);
        assert!(load_restore_token(TokenScope::Keyboard).is_none());

        assert_eq!(
            load_restore_token(TokenScope::Clipboard).as_deref(),
            Some("clipboard-grant")
        );
        clear_restore_token(TokenScope::Clipboard);
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
    #[test]
    #[ignore = "requires a disposable GNOME Wayland guest, a focused receiver and portal consent"]
    fn gnome_clipboard_portal_native() {
        assert_eq!(std::env::var("LHC_GNOME_PORTAL_TEST").as_deref(), Ok("1"));
        let data = tempfile::tempdir().unwrap();
        set_token_dir(data.path().to_owned());
        set_text_backend(
            TextMode::Libei,
            PasteShortcut::ShiftInsert,
            None,
            None,
            None,
        );
        crate::layout::refresh_cache().unwrap();
        assert!(crate::active_window::detect_active_window_now().is_some());
        super::super::text_job::start();
        let text = "Привет, ёж 😀";
        let request = TextRequest::enqueue(text).unwrap();
        println!("Native portal probe ready for consent");
        let result = type_text(text, request.clone());
        request.complete(result.is_err());
        assert_eq!(result, Ok(()));
        for (mime, expected) in [
            ("text/plain;charset=utf-8", b"lhc-before-unicode".as_slice()),
            ("application/x-lhc-e2e", b"binary\0\xffpayload".as_slice()),
        ] {
            let output = std::process::Command::new("wl-paste")
                .args(["--no-newline", "--type", mime])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, expected, "restoring {mime}");
        }
        println!("Native portal Unicode and MIME restoration passed");
    }
}
