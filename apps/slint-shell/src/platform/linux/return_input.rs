use evdev::{AttributeSet, KeyCode, KeyEvent};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
struct Active(Arc<Mutex<String>>);

#[zbus::interface(name = "org.leftHandControl.SlintProbe")]
impl Active {
    fn update(&self, id: String) {
        log::info!("probe active window: {id}");
        *self.0.lock().unwrap() = id;
    }
}

pub struct ReturnInput {
    connection: zbus::blocking::Connection,
    script_name: String,
    _script: tempfile::NamedTempFile,
    active: Active,
    target: Option<String>,
    device: crate::test_keyboard::Keyboard,
    pending: Option<(Instant, String, String)>,
    restore_requested: bool,
    test_mode: bool,
}

impl ReturnInput {
    pub fn new() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let test_mode = std::env::var("SLINT_SHELL_TEST_INJECT").as_deref() == Ok("1");
        if !test_mode && std::env::var("SLINT_SHELL_INSERT").as_deref() != Ok("1") {
            return Ok(None);
        }
        let active = Active::default();
        let bus_name = format!("org.leftHandControl.SlintProbe.p{}", std::process::id());
        let connection = zbus::blocking::connection::Builder::session()?
            .name(bus_name.clone())?
            .serve_at("/Probe", active.clone())?
            .build()?;
        let script_name = format!("lhc-slint-probe-{}", std::process::id());
        let mut script = tempfile::NamedTempFile::new()?;
        write!(
            script,
            "function report() {{ let w = workspace.activeWindow; callDBus('{bus_name}', '/Probe', 'org.leftHandControl.SlintProbe', 'Update', w ? String(w.internalId) : ''); }} workspace.windowActivated.connect(report); report();"
        )?;
        let proxy = zbus::blocking::Proxy::new(
            &connection,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )?;
        let id: i32 = proxy.call(
            "loadScript",
            &(script.path().to_string_lossy().as_ref(), &script_name),
        )?;
        if id < 0 {
            return Err("KWin focus probe script failed to load".into());
        }
        let path = format!("/Scripting/Script{id}");
        let run =
            zbus::blocking::Proxy::new(&connection, "org.kde.KWin", path, "org.kde.kwin.Script")?;
        if let Err(error) = run.call::<_, _, ()>("run", &()) {
            let _: Result<bool, _> = proxy.call("unloadScript", &script_name);
            return Err(error.into());
        }
        let keys: AttributeSet<KeyCode> =
            [KeyCode::KEY_A, KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_INSERT]
                .into_iter()
                .collect();
        let device = match crate::test_keyboard::Keyboard::new("Slint return-input probe", &keys) {
            Ok(device) => device,
            Err(error) => {
                let _: Result<bool, _> = proxy.call("unloadScript", &script_name);
                return Err(error);
            }
        };
        Ok(Some(Self {
            connection,
            script_name,
            _script: script,
            active,
            target: None,
            device,
            pending: None,
            restore_requested: false,
            test_mode,
        }))
    }

    pub fn capture(&mut self) {
        self.pending = None;
        let id = self.active.0.lock().unwrap().clone();
        log::info!("probe captured target: {id}");
        self.target = (!id.is_empty()).then_some(id);
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub fn selected(&mut self, text: String) -> bool {
        self.restore_requested = false;
        self.pending = self.target.take().map(|id| (Instant::now(), id, text));
        self.pending.is_some()
    }

    pub fn poll(&mut self, keyboard_released: bool) -> Option<&'static str> {
        let (start, id, text) = self.pending.as_ref()?;
        if start.elapsed() > Duration::from_secs(2) {
            self.pending = None;
            return Some("return_input_timeout");
        }
        if !keyboard_released {
            return None;
        }
        if !self.restore_requested {
            self.restore_requested = true;
            if let Err(error) = activate_window(&self.connection, id) {
                log::error!("restore focus: {error}");
                self.pending = None;
                return Some("return_focus_failed");
            }
            return None;
        }
        if *self.active.0.lock().unwrap() != *id
            || (!crate::test_keyboard::isolated() && !modifiers_released())
        {
            return None;
        }
        let result = if self.test_mode {
            self.device.emit(&[
                *KeyEvent::new(KeyCode::KEY_A, 1),
                *KeyEvent::new(KeyCode::KEY_A, 0),
            ])
        } else {
            paste(&mut self.device, text)
        };
        self.pending = None;
        Some(match (self.test_mode, result) {
            (true, Ok(())) => "test_input_sent",
            (true, Err(_)) => "test_input_failed",
            (false, Ok(())) => "selected_input_sent",
            (false, Err(error)) => {
                log::error!("selected input: {error}");
                "selected_input_failed"
            }
        })
    }
}

fn paste(device: &mut crate::test_keyboard::Keyboard, text: &str) -> std::io::Result<()> {
    let previous = Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout);
    set_clipboard(text.as_bytes())?;
    std::thread::sleep(Duration::from_millis(50));
    device.emit(&[*KeyEvent::new(KeyCode::KEY_LEFTSHIFT, 1)])?;
    let insert = device.emit(&[
        *KeyEvent::new(KeyCode::KEY_INSERT, 1),
        *KeyEvent::new(KeyCode::KEY_INSERT, 0),
    ]);
    let release = device.emit(&[*KeyEvent::new(KeyCode::KEY_LEFTSHIFT, 0)]);
    insert?;
    release?;
    if let Some(previous) = previous {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            if let Err(error) = set_clipboard(&previous) {
                log::warn!("restore clipboard: {error}");
            }
        });
    }
    Ok(())
}

fn set_clipboard(text: &[u8]) -> std::io::Result<()> {
    let mut child = Command::new("wl-copy").stdin(Stdio::piped()).spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("wl-copy stdin unavailable"))?
        .write_all(text)?;
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "wl-copy exited with {status}"
        )))
    }
}

impl Drop for ReturnInput {
    fn drop(&mut self) {
        if let Ok(proxy) = zbus::blocking::Proxy::new(
            &self.connection,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        ) {
            let _: Result<bool, _> = proxy.call("unloadScript", &self.script_name);
        }
    }
}

pub fn activate_window(
    connection: &zbus::blocking::Connection,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut script = tempfile::NamedTempFile::new()?;
    write!(
        script,
        "let target = {}; for (let w of workspace.windowList()) {{ if (String(w.internalId) === target) {{ workspace.activeWindow = w; break; }} }}",
        serde_json::to_string(id)?
    )?;
    run_script(connection, &script)
}

fn run_script(
    connection: &zbus::blocking::Connection,
    script: &tempfile::NamedTempFile,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = format!("lhc-slint-activate-{}", std::process::id());
    let proxy = zbus::blocking::Proxy::new(
        connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )?;
    let id: i32 = proxy.call(
        "loadScript",
        &(script.path().to_string_lossy().as_ref(), &name),
    )?;
    if id < 0 {
        return Err("activation script failed to load".into());
    }
    let run = zbus::blocking::Proxy::new(
        connection,
        "org.kde.KWin",
        format!("/Scripting/Script{id}"),
        "org.kde.kwin.Script",
    )?;
    let result = run.call::<_, _, ()>("run", &());
    let _: bool = proxy.call("unloadScript", &name)?;
    result?;
    Ok(())
}

fn modifiers_released() -> bool {
    let modifiers = [
        KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_RIGHTSHIFT,
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_RIGHTCTRL,
        KeyCode::KEY_LEFTALT,
        KeyCode::KEY_RIGHTALT,
        KeyCode::KEY_LEFTMETA,
        KeyCode::KEY_RIGHTMETA,
    ];
    let mut checked = false;
    for (_, device) in evdev::enumerate() {
        if !device
            .supported_keys()
            .is_some_and(|keys| modifiers.iter().any(|key| keys.contains(*key)))
        {
            continue;
        }
        checked = true;
        match device.get_key_state() {
            Ok(keys) if modifiers.iter().all(|key| !keys.contains(*key)) => {}
            other => {
                log::debug!(
                    "modifier state blocked injection: {:?}: {other:?}",
                    device.name()
                );
                return false;
            }
        }
    }
    checked
}
