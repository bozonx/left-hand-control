//! The libei backend: typing through an EIS connection from the portal.

use super::*;

pub(super) struct LibeiBackend {
    pub(super) clipboard: Option<crate::mapper::portal_clipboard::Clipboard>,
    pub(super) conn: Connection,
    pub(super) session_handle: OwnedObjectPath,
    pub(super) context: ei::Context,
    pub(super) converter: EiEventConverter,
    pub(super) connection: reis::event::Connection,
    pub(super) keyboard_device: Option<(ei::Device, ei::Keyboard)>,
    pub(super) keyboard_emulating: bool,
    pub(super) sequence: u32,
    /// XKB keymap text of the keyboard device, as the compositor sent it.
    pub(super) keymap: Option<Vec<u8>>,
    /// Active layout group the compositor reported.
    pub(super) group: Option<u32>,
    /// Key table built from `keymap` for a group.
    pub(super) table: Option<((u32, u32), Arc<KeyTable>)>,
    pub(super) locked: u32,
    pub(super) depressed: u32,
    pub(super) latched: u32,
}

impl LibeiBackend {
    pub(super) fn new(request: &TextRequest) -> Result<Self, String> {
        let (conn, session_handle, clipboard) = init_portal(request)?;
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
        let handshake = match ei_handshake_with_timeout(&context, EI_SETUP_TIMEOUT) {
            Ok(handshake) => handshake,
            Err(e) => {
                close_session(&conn, &session_handle);
                return Err(e);
            }
        };
        let converter = EiEventConverter::new(&context, handshake);
        let connection = converter.connection().clone();
        let mut backend = Self {
            clipboard,
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
        if let Err(e) = backend.wait_until(EI_SETUP_TIMEOUT, Self::has_keyboard) {
            close_session(&backend.conn, &backend.session_handle);
            return Err(e);
        }
        Ok(backend)
    }

    /// Type `text`, pasting it with `clipboard_fallback` when the layout
    /// lacks a character. `Err` means the EI connection is unusable; other
    /// failures are reported and leave the session open.
    pub(super) fn type_text(
        &mut self,
        text: &str,
        clipboard_fallback: Option<PasteShortcut>,
        request: &TextRequest,
    ) -> Result<(), TextInputFailure> {
        request.check()?;
        self.pump_events()
            .map_err(TextInputFailure::BackendUnavailable)?;
        if !self.has_keyboard() {
            self.wait_until(EI_KEYBOARD_TIMEOUT, Self::has_keyboard)
                .map_err(TextInputFailure::BackendUnavailable)?;
        }
        // Events sent while the compositor paused the device are a protocol
        // error, e.g. on the lock screen.
        if !self.keyboard_emulating
            && self
                .wait_until(EI_EMULATION_TIMEOUT, |backend| backend.keyboard_emulating)
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
    pub(super) fn key_table(&mut self) -> Result<Arc<KeyTable>, TextInputFailure> {
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

    pub(super) fn paste_text(
        &mut self,
        text: &str,
        shortcut: PasteShortcut,
        request: &TextRequest,
    ) -> Result<(), TextInputFailure> {
        let clipboard = prepare_clipboard(self.clipboard.as_ref(), text, request)?;
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

    pub(super) fn inject_keycode_combo(
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

    pub(super) fn synchronize(&mut self, request: &TextRequest) -> Result<(), String> {
        let done = Arc::new(AtomicBool::new(false));
        let completed = done.clone();
        let callback = self.connection.connection().sync(1);
        self.converter
            .add_callback_handler(callback, move |_| completed.store(true, Ordering::SeqCst));
        self.context
            .flush()
            .map_err(|e| format!("EI sync flush: {e}"))?;
        let deadline = Instant::now() + EI_ACK_TIMEOUT;
        while !done.load(Ordering::SeqCst) {
            request.check().map_err(|e| format!("{e:?}"))?;
            self.pump_events()?;
            if Instant::now() >= deadline {
                return Err("EI input acknowledgement timed out".into());
            }
            if !done.load(Ordering::SeqCst) {
                thread::sleep(EI_ACK_POLL_INTERVAL);
            }
        }
        Ok(())
    }

    pub(super) fn inject_key(&mut self, key: u32, state: KeyState) -> Result<(), String> {
        let Some((device, keyboard)) = self.keyboard_device.as_ref() else {
            return Err("no EI keyboard device".into());
        };
        keyboard.key(key, state);
        device.frame(self.connection.serial(), monotonic_micros());
        self.context.flush().map_err(|e| format!("EI flush: {e}"))
    }

    pub(super) fn wait_until(
        &mut self,
        timeout: Duration,
        ready: fn(&Self) -> bool,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            self.pump_events()?;
            if ready(self) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("EI keyboard device is not available".into());
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    pub(super) fn pump_events(&mut self) -> Result<(), String> {
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

    pub(super) fn is_keyboard(&self, device: &ei::Device) -> bool {
        self.keyboard_device
            .as_ref()
            .is_some_and(|(keyboard, _)| keyboard == device)
    }

    pub(super) fn handle_event(&mut self, event: EiEvent) -> Result<(), String> {
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

    pub(super) fn has_keyboard(&self) -> bool {
        self.keyboard_device.is_some()
    }

    pub(super) fn start_keyboard_emulation(&mut self, device: &ei::Device) {
        if self.keyboard_emulating {
            return;
        }
        device.start_emulating(self.connection.serial(), self.sequence);
        self.sequence += 1;
        self.keyboard_emulating = true;
    }

    pub(super) fn close(self) {
        if self.clipboard.is_none() {
            close_session(&self.conn, &self.session_handle);
        }
    }
}

/// Read the XKB keymap an EI keyboard device carries.
pub(super) fn read_keymap(keymap: &reis::event::Keymap) -> Option<Vec<u8>> {
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

pub(super) fn monotonic_micros() -> u64 {
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

pub(super) fn ei_handshake_with_timeout(
    context: &ei::Context,
    timeout: Duration,
) -> Result<reis::handshake::HandshakeResp, String> {
    let mut handshaker =
        reis::handshake::EiHandshaker::new(crate::storage::APP_NAME, ContextType::Sender);
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
        thread::sleep(POLL_INTERVAL);
    }
    Err("EI handshake timed out".into())
}
