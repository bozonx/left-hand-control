//! Text injection on Linux for `text:` actions.
//!
//! ```text
//! Two of the modes go through the XDG Desktop Portal RemoteDesktop
//! interface, the cross-compositor way to inject input on Wayland (the
//! `zwp_virtual_keyboard_v1` protocol is not offered to ordinary clients on
//! KDE Plasma 6 or GNOME):
//!
//!   1. `CreateSession()`        – async, via a `Request` object + `Response` signal.
//!   2. `SelectDevices(keyboard)`– declares what we plan to inject; this is
//!                                 also where we set `persist_mode = 2` and
//!                                 pass back any previously-saved
//!                                 `restore_token` so the user does not see
//!                                 the consent dialog again.
//!   3. `Start()`                – shows the permission dialog (only once
//!                                 per machine if persist mode + token work).
//!   4. `ConnectToEIS()` (libei modes) or `NotifyKeyboardKeycode()` (keycode
//!      mode and the paste shortcut of the clipboard mode).
//!
//! Each backend owns one session on its own worker thread. The session is
//! created on the first text and kept until the mode changes or the session
//! breaks, so the consent dialog is not shown on every mapper restart. The
//! `restore_token` from `Start` is persisted next to the user data, so later
//! app launches restore the permission silently.
//!
//! Characters are typed as keycodes of the active layout (`NotifyKeyboardKeysym`
//! emits keysyms without the needed modifier on KDE, giving wrong characters
//! in non-Latin layouts). libei uses the keymap and layout group the
//! compositor sends; the keycode mode rebuilds the keymap from the active
//! layout name and variant.
//!
//! Text modes must never silently switch injection mechanisms: hidden fallbacks
//! mask failures and make backend debugging unreliable. The only allowed fallback
//! is the explicitly selected libei + clipboard mode, which pastes the whole text
//! when the keymap lacks a character. It still requires a working libei session.
//! Every failure is reported as `CoreEvent::TextInputFailed`, never retried
//! through another input method.
//! ```

#![cfg(target_os = "linux")]

mod hardware;
mod inject;
mod keymap;
mod libei;
mod session;
mod token;
mod worker;

use self::hardware::*;
pub(crate) use self::inject::close_session;
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

use crate::config::model::{PasteShortcut, TextMode};
use crate::events::{CoreEvent, TextInputFailure};
use crate::text_input::job::TextRequest;

// --- Portal DBus identifiers --------------------------------------------

pub(crate) use crate::platform::dbus::{PORTAL_PATH, PORTAL_SERVICE as PORTAL_DEST};
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

pub(crate) fn report(failure: TextInputFailure) {
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
    mode: TextMode::LibeiWithClipboardFallback,
    paste: PasteShortcut::CtrlV,
    terminal_paste: Some(PasteShortcut::ShiftInsert),
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

/// Forget portal grants and reopen consent on the next text request.
/// The clipboard owner may outlive the keyboard backend to serve restored data.
pub fn reset_permissions() {
    LIBEI.stop();
    PORTAL.stop();
    clear_restore_token(TokenScope::Keyboard);
    clear_restore_token(TokenScope::Clipboard);
    retry_text_backend();
}

/// Key and modifiers (evdev codes) that type `ch` in the active layout, for
/// sending a one-character text as a plain key press. `None` when the layout
/// is unknown, has no such character, or a modifier is held: the character
/// then goes through the text backend, which handles all three.
pub(crate) fn key_for_char(ch: char) -> Option<(u16, Vec<u16>)> {
    if ch.is_control() {
        return None;
    }
    modifiers_released().ok()?;
    let table = layout_key_table().ok()?;
    let entry = table.get(&text_keysym(ch))?;
    let code = |key: u32| u16::try_from(key).ok();
    Some((
        code(entry.evdev)?,
        entry.mods.iter().map(|m| code(*m)).collect::<Option<_>>()?,
    ))
}

/// Resolve built-in clipboard strokes at playback time. User `KeyC/V/X`
/// bindings remain physical keys; only built-in editing commands are semantic.
pub(crate) fn builtin_clipboard_stroke(
    stroke: &crate::mapper::action::Keystroke,
) -> crate::mapper::action::Keystroke {
    use crate::mapper::key_code::KeyCode as K;
    if stroke.mods != [K::KEY_LEFTCTRL] {
        return stroke.clone();
    }
    let character = match stroke.key {
        K::KEY_C => 'c',
        K::KEY_V => 'v',
        K::KEY_X => 'x',
        _ => return stroke.clone(),
    };
    let terminal = crate::active_window::TextTarget::capture().is_terminal();
    let key = crate::runtime_state::keyboard_layout()
        .and_then(|layout| {
            let layouts = crate::keyboard_layout::shortcut_layouts();
            let group = layout.index as usize;
            if layouts
                .get(group)
                .is_some_and(|(code, variant)| *code == layout.code && *variant == layout.variant)
            {
                let names: Vec<_> = layouts
                    .iter()
                    .map(|(code, variant)| (code.as_str(), variant.as_str()))
                    .collect();
                shortcut_key_from_names(&names, layout.index, "", character)
            } else {
                shortcut_key_from_names(&[(&layout.code, &layout.variant)], 0, "", character)
            }
        })
        .and_then(|key| u16::try_from(key).ok())
        .map(K::new);
    let cfg = TEXT_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    let shortcut = cfg.terminal_paste.filter(|_| terminal).unwrap_or(cfg.paste);
    contextual_clipboard_stroke(stroke, terminal, shortcut, key)
}

fn contextual_clipboard_stroke(
    stroke: &crate::mapper::action::Keystroke,
    terminal: bool,
    paste: PasteShortcut,
    key: Option<crate::mapper::key_code::KeyCode>,
) -> crate::mapper::action::Keystroke {
    use crate::mapper::key_code::KeyCode as K;
    let mut result = stroke.clone();
    if let Some(key) = key {
        result.key = key;
    }
    if stroke.key == K::KEY_V {
        let (mods, key) = paste_keys(paste, Some(u32::from(result.key.code())));
        result.mods = mods.iter().map(|key| K::new(*key as u16)).collect();
        result.key = K::new(key as u16);
    } else if terminal {
        result.mods.push(K::KEY_LEFTSHIFT);
    }
    result
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
pub(crate) fn type_text(text: &str, request: TextRequest) -> Result<(), TextInputFailure> {
    request.prepare()?;
    request.wait_for_target()?;
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
        TextMode::LibeiWithClipboardFallback | TextMode::LibeiOnly => {
            LIBEI.type_text(text, cfg.mode, paste, request)
        }
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
    request.start_tool(Duration::from_millis(timeout_ms))?;
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
    fn builtin_clipboard_keys_follow_layout_and_terminal_paste_settings() {
        use crate::mapper::action::parse_action;
        use crate::mapper::key_code::KeyCode as K;
        let shortcut = |character| shortcut_key_from_names(&[("us", "dvorak")], 0, "", character);
        assert_eq!(shortcut('c'), Some(u32::from(K::KEY_I.code())));
        assert_eq!(shortcut('x'), Some(u32::from(K::KEY_B.code())));
        assert_eq!(shortcut('v'), Some(u32::from(K::KEY_DOT.code())));
        assert_eq!(
            shortcut_key_from_names(&[("ru", ""), ("us", "dvorak")], 0, "", 'c'),
            Some(u32::from(K::KEY_I.code()))
        );
        assert_eq!(
            shortcut_key_from_names(&[("ru", ""), ("us", "dvorak")], 0, "", 'x'),
            Some(u32::from(K::KEY_B.code()))
        );
        let copy = contextual_clipboard_stroke(
            &parse_action("Ctrl+KeyC").unwrap(),
            true,
            PasteShortcut::CtrlShiftV,
            Some(K::KEY_I),
        );
        assert_eq!(copy.mods, [K::KEY_LEFTCTRL, K::KEY_LEFTSHIFT]);
        assert_eq!(copy.key, K::KEY_I);
        let paste = contextual_clipboard_stroke(
            &parse_action("Ctrl+KeyV").unwrap(),
            true,
            PasteShortcut::CtrlShiftV,
            Some(K::KEY_DOT),
        );
        assert_eq!(paste.mods, [K::KEY_LEFTCTRL, K::KEY_LEFTSHIFT]);
        assert_eq!(paste.key, K::KEY_DOT);
        let paste = contextual_clipboard_stroke(
            &parse_action("Ctrl+KeyV").unwrap(),
            false,
            PasteShortcut::ShiftInsert,
            Some(K::KEY_DOT),
        );
        assert_eq!(paste.mods, [K::KEY_LEFTSHIFT]);
        assert_eq!(paste.key, K::KEY_INSERT);
    }

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
    fn main_block_keys_are_preferred_for_typing() {
        // `inet(evdev)` puts `$` and `€` on KEY_DOLLAR/KEY_EURO and the keypad
        // has `*`, `+`, `(`, `)` and `±` without modifiers; Electron and X11
        // clients ignore the special keys, and keypad keys may act as commands.
        let us = table_from_names("us", None).expect("us layout");
        let ru = table_from_names("ru", None).expect("ru layout");
        let shift = vec![KEY_LEFTSHIFT_EVDEV];
        for (ch, key) in [('$', 5), ('*', 9), ('(', 10), (')', 11), ('+', 13)] {
            assert_eq!(us[&text_keysym(ch)].evdev, key, "{ch}");
            assert_eq!(us[&text_keysym(ch)].mods, shift, "{ch}");
        }
        // Russian `/` is on the main block (LSGT), not KEY_KPSLASH.
        assert_eq!(ru[&text_keysym('/')].evdev, 86);
        assert!(!has(&us, '€') && !has(&us, '±'));
        assert!(!has(&ru, '$') && !has(&ru, '€'));
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
    fn paste_v_key_follows_the_layout() {
        // Dvorak types `v` on the physical `.` key; the physical V types `k`.
        const KEY_DOT: u32 = 52;
        let v = |layouts: &[(&str, &str)], group| paste_v_key_from_names(layouts, group, "");
        assert_eq!(v(&[("us", "")], 0), Some(KEY_V_EVDEV));
        assert_eq!(v(&[("us", "dvorak")], 0), Some(KEY_DOT));
        // A Cyrillic layout borrows the shortcut from the Latin group.
        assert_eq!(v(&[("ru", ""), ("us", "dvorak")], 0), Some(KEY_DOT));
        assert_eq!(v(&[("ru", ""), ("us", "")], 0), Some(KEY_V_EVDEV));
        assert_eq!(v(&[("us", "dvorak"), ("ru", "")], 1), Some(KEY_DOT));
        assert_eq!(v(&[("ru", "")], 0), None);
        assert_eq!(v(&[], 0), None);
        assert_eq!(
            paste_keys(PasteShortcut::CtrlV, Some(KEY_DOT)),
            (&[KEY_LEFTCTRL_EVDEV][..], KEY_DOT)
        );
        assert_eq!(paste_keys(PasteShortcut::CtrlShiftV, None).1, KEY_V_EVDEV);
        assert_eq!(
            paste_keys(PasteShortcut::ShiftInsert, Some(KEY_DOT)).1,
            KEY_INSERT_EVDEV
        );
    }

    #[test]
    fn automatic_paste_requires_a_verified_latin_key() {
        assert!(checked_paste_keys(PasteShortcut::CtrlV, None).is_err());
        assert!(checked_paste_keys(PasteShortcut::CtrlShiftV, None).is_err());
        assert_eq!(
            checked_paste_keys(PasteShortcut::ShiftInsert, None)
                .unwrap()
                .1,
            KEY_INSERT_EVDEV
        );
        assert_eq!(
            checked_paste_keys(PasteShortcut::CtrlV, Some(52))
                .unwrap()
                .1,
            52
        );
    }

    #[test]
    fn malformed_compositor_keymap_is_not_missing_characters() {
        assert!(table_from_keymap_text(b"not an XKB keymap", 0, 0).is_none());
        assert!(table_from_keymap_text(b"", 0, 0).is_none());
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
        assert!(a.starts_with("tecloid_test_"));
    }

    #[test]
    fn token_persistence_roundtrip() {
        let temp = std::env::temp_dir().join(format!("tecloid-portal-test-{}", std::process::id()));
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
        let temp = std::env::temp_dir().join(format!("tecloid-atomic-test-{}", std::process::id()));
        write_token_atomic(&temp, "hello").expect("write");
        let contents = std::fs::read_to_string(&temp).expect("read");
        assert_eq!(contents, "hello");
        let _ = std::fs::remove_file(&temp);
    }
    #[test]
    #[ignore = "requires a disposable GNOME Wayland guest, a focused receiver and portal consent"]
    fn gnome_clipboard_portal_native() {
        assert_eq!(
            std::env::var("TECLOID_GNOME_PORTAL_TEST").as_deref(),
            Ok("1")
        );
        let data = tempfile::tempdir().unwrap();
        set_token_dir(data.path().to_owned());
        set_text_backend(
            TextMode::LibeiWithClipboardFallback,
            PasteShortcut::ShiftInsert,
            None,
            None,
            None,
        );
        crate::keyboard_layout::refresh_cache().unwrap();
        assert!(crate::active_window::detect_active_window_now().is_some());
        let _text_session = crate::text_input::job::Session::start();
        let text = "Привет, ёж 😀";
        let request = TextRequest::enqueue(text).unwrap();
        println!("Native portal probe ready for consent");
        let result = type_text(text, request.clone());
        request.complete(result.is_err());
        assert_eq!(result, Ok(()));
        for (mime, expected) in [
            (
                "text/plain;charset=utf-8",
                b"tecloid-before-unicode".as_slice(),
            ),
            (
                "application/x-tecloid-e2e",
                b"binary\0\xffpayload".as_slice(),
            ),
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
