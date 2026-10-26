//! Keycode typing and clipboard paste through `NotifyKeyboardKeycode`.

use super::*;

pub(super) fn type_text_keycode(
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

pub(super) enum ClipboardPaste {
    Wayland(crate::mapper::clipboard::Paste),
    Portal(crate::mapper::portal_clipboard::Paste),
}

pub(super) fn prepare_clipboard(
    portal: Option<&crate::mapper::portal_clipboard::Clipboard>,
    text: &str,
    request: &TextRequest,
) -> Result<ClipboardPaste, TextInputFailure> {
    match portal {
        Some(portal) => portal.prepare(text, request).map(ClipboardPaste::Portal),
        None => crate::mapper::clipboard::Paste::prepare(text).map(ClipboardPaste::Wayland),
    }
    .map_err(TextInputFailure::Clipboard)
}

impl ClipboardPaste {
    pub(super) fn arm(&self) -> Result<(), String> {
        match self {
            Self::Wayland(paste) => paste.arm(),
            Self::Portal(paste) => paste.arm(),
        }
    }
    pub(super) fn finish(self, injected: bool) -> Result<(), String> {
        match self {
            Self::Wayland(paste) => paste.finish(injected),
            Self::Portal(paste) => paste.finish(injected),
        }
    }
}

pub(super) fn paste_text(
    portal: &Proxy,
    session: &OwnedObjectPath,
    portal_clipboard: Option<&crate::mapper::portal_clipboard::Clipboard>,
    text: &str,
    shortcut: PasteShortcut,
    request: &TextRequest,
) -> Result<(), TextInputFailure> {
    let clipboard = prepare_clipboard(portal_clipboard, text, request)?;
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
pub(super) fn notify_key(
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
pub(super) fn inject_keycode_combo(
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

pub(in crate::mapper) fn close_session(conn: &Connection, session: &OwnedObjectPath) {
    match Proxy::new(conn, PORTAL_DEST, &**session, IFACE_SESSION) {
        Ok(sp) => {
            if let Err(e) = sp.call_method("Close", &()) {
                log::debug!("[portal] Session.Close failed: {e}");
            }
        }
        Err(e) => log::debug!("[portal] cannot build Session proxy for close: {e}"),
    }
}
