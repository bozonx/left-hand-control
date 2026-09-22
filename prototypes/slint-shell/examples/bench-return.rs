#[path = "../src/test_keyboard.rs"]
mod test_keyboard;
use evdev::{AttributeSet, KeyCode, KeyEvent};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::sleep,
    time::{Duration, Instant},
};

slint::slint! {
    export component Receiver inherits Window {
        title: "Slint input return probe";
        width: 620px; height: 220px;
        callback input(string);
        in-out property <string> received;
        VerticalLayout {
            Text { text: "Dedicated input receiver — no shell commands"; }
            Text { text: root.received; }
            keys := FocusScope {
                init => { self.focus(); }
                key-pressed(event) => { root.input(event.text); return accept; }
            }
        }
    }
}

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(mut condition: impl FnMut() -> bool, label: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        if Instant::now() >= deadline {
            return Err(format!("timeout: {label}"));
        }
        sleep(Duration::from_millis(5));
    }
    Ok(())
}
fn press(device: &mut test_keyboard::Keyboard, key: KeyCode) -> std::io::Result<()> {
    device.emit(&[*KeyEvent::new(key, 1), *KeyEvent::new(key, 0)])
}
fn press_hotkey(device: &mut test_keyboard::Keyboard, key: KeyCode) -> std::io::Result<()> {
    if test_keyboard::isolated() {
        let args: Vec<_> = std::env::args().collect();
        let status = Command::new(&args[1])
            .env(
                "SLINT_SHELL_SOCKET",
                format!("lhc-return-bench-{}.sock", std::process::id()),
            )
            .args([
                "show",
                if key == KeyCode::KEY_F11 {
                    "emoji"
                } else {
                    "quick"
                },
            ])
            .status()?;
        if !status.success() {
            return Err(std::io::Error::other("show IPC failed"));
        }
        return Ok(());
    }
    device.emit(&[
        *KeyEvent::new(KeyCode::KEY_LEFTCTRL, 1),
        *KeyEvent::new(KeyCode::KEY_LEFTALT, 1),
        *KeyEvent::new(key, 1),
        *KeyEvent::new(key, 0),
        *KeyEvent::new(KeyCode::KEY_LEFTALT, 0),
        *KeyEvent::new(KeyCode::KEY_LEFTCTRL, 0),
    ])
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let binary = PathBuf::from(args.get(1).ok_or("expected binary and CSV path")?);
    let csv = PathBuf::from(args.get(2).ok_or("expected CSV path")?);
    let receiver = Receiver::new()?;
    let received = Arc::new(Mutex::new(Vec::<String>::new()));
    let input = received.clone();
    let weak = receiver.as_weak();
    receiver.on_input(move |text| {
        input.lock().unwrap().push(text.to_string());
        if let Some(ui) = weak.upgrade() {
            ui.set_received(format!("{:?}", *input.lock().unwrap()).into());
        }
    });
    let focused = Arc::new(AtomicBool::new(false));
    let focus = focused.clone();
    receiver.window().on_winit_window_event(move |_, event| {
        if let slint::winit_030::winit::event::WindowEvent::Focused(value) = event {
            eprintln!("receiver focused={value}");
            focus.store(*value, Ordering::SeqCst);
        }
        slint::winit_030::EventResult::Propagate
    });
    let handle = std::thread::spawn(move || -> Result<(), String> {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            sleep(Duration::from_millis(250));
            activate_receiver()?;
            wait(|| focused.load(Ordering::SeqCst), "receiver initial focus")?;
            let keys: AttributeSet<KeyCode> = [KeyCode::KEY_F11, KeyCode::KEY_F12, KeyCode::KEY_DOWN, KeyCode::KEY_ENTER, KeyCode::KEY_ESC, KeyCode::KEY_A, KeyCode::KEY_LEFTALT, KeyCode::KEY_LEFTCTRL, KeyCode::KEY_BACKSPACE, KeyCode::KEY_L, KeyCode::KEY_T, KeyCode::KEY_6, KeyCode::KEY_1].into_iter().collect();
            let mut device = test_keyboard::Keyboard::new("Slint return benchmark trigger", &keys)?;
            let input = match &mut device {
                test_keyboard::Keyboard::Evdev(device) => device.enumerate_dev_nodes_blocking()?.next().transpose()?,
                test_keyboard::Keyboard::Isolated(_, _) => None,
            };
            let socket = format!("lhc-return-bench-{}.sock", std::process::id());
            let mut command = Command::new(&binary);
            if let Some(input) = input { command.env("SLINT_SHELL_INPUT", input); } else { command.env("SLINT_SHELL_HOTKEYS", "off"); }
            let mut server = Server(command.env("SLINT_SHELL_SOCKET", &socket).env("SLINT_SHELL_METRICS", &csv).env("SLINT_SHELL_TEST_INJECT", "1").env("SLINT_SHELL_POPUPS", "spell").env_remove("XDG_ACTIVATION_TOKEN").spawn()?);
            let worker_csv = PathBuf::from(format!("{}.spell.csv", csv.display()));
            sleep(Duration::from_secs(2));
            if server.0.try_wait()?.is_some() { return Err("server startup failed".into()); }
            activate_receiver()?;
            for (name, hotkey) in [("emoji", KeyCode::KEY_F11), ("quick", KeyCode::KEY_F12)] {
                for trial in 0..20 {
                    wait(|| focused.load(Ordering::SeqCst), "receiver before trigger")?;
                    received.lock().unwrap().clear();
                    let before = std::fs::read_to_string(&worker_csv)?;
                    press_hotkey(&mut device, hotkey)?;
                    wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",t4_focused,")), "popup focus")?;
                    press(&mut device, KeyCode::KEY_DOWN)?;
                    wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",navigation_handled,")), "first arrow")?;
                    press(&mut device, KeyCode::KEY_ENTER)?;
                    wait(|| !received.lock().unwrap().is_empty(), "returned input")?;
                    sleep(Duration::from_millis(100));
                    let actual = received.lock().unwrap().clone();
                    if actual != ["a"] && actual != ["ф"] { return Err(format!("unexpected input/leak: {actual:?}").into()); }
                    let data = std::fs::read_to_string(&worker_csv)?;
                    if !data[before.len()..].contains(",test_input_sent,") { return Err("input was not confirmed by worker".into()); }
                    println!("{name} selection {trial}: focus returned, exactly one test character, no navigation leak");
                    received.lock().unwrap().clear();
                    press_hotkey(&mut device, hotkey)?;
                    wait(|| !focused.load(Ordering::SeqCst), "cancel popup focus")?;
                    press(&mut device, KeyCode::KEY_ESC)?;
                    wait(|| focused.load(Ordering::SeqCst), "cancel focus return")?;
                    sleep(Duration::from_millis(100));
                    if !received.lock().unwrap().is_empty() { return Err("cancel injected or leaked a key".into()); }
                }
            }
            for (name, hotkey) in [("emoji", KeyCode::KEY_F11), ("quick", KeyCode::KEY_F12)] {
                let before = std::fs::read_to_string(&worker_csv)?;
                press_hotkey(&mut device, hotkey)?;
                wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",t4_focused,")), "repeat focus")?;
                device.emit(&[*KeyEvent::new(KeyCode::KEY_DOWN, 1)])?;
                let repeated = wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",navigation_repeated,")), "held arrow repeat");
                device.emit(&[*KeyEvent::new(KeyCode::KEY_DOWN, 0)])?;
                repeated?;
                press_hotkey(&mut device, hotkey)?;
                wait(|| focused.load(Ordering::SeqCst), "repeat hotkey hides popup")?;
                println!("{name}: held arrow repeats, second hotkey dismisses");
            }
            received.lock().unwrap().clear();
            device.emit(&[*KeyEvent::new(KeyCode::KEY_LEFTALT, 1)])?;
            let alt_result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let before = std::fs::read_to_string(&worker_csv)?;
                if test_keyboard::isolated() {
                    press_hotkey(&mut device, KeyCode::KEY_F12)?;
                } else {
                    device.emit(&[
                        *KeyEvent::new(KeyCode::KEY_LEFTCTRL, 1),
                        *KeyEvent::new(KeyCode::KEY_F12, 1),
                        *KeyEvent::new(KeyCode::KEY_F12, 0),
                        *KeyEvent::new(KeyCode::KEY_LEFTCTRL, 0),
                    ])?;
                }
                wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",t4_focused,")), "Alt-held focus")?;
                press(&mut device, KeyCode::KEY_1)?;
                Ok(())
            })();
            device.emit(&[*KeyEvent::new(KeyCode::KEY_LEFTALT, 0)])?;
            alt_result?;
            wait(|| received.lock().unwrap().iter().any(|s| s == "a" || s == "ф"), "Alt+1 choice")?;
            received.lock().unwrap().clear();
            let before = std::fs::read_to_string(&worker_csv)?;
            press_hotkey(&mut device, KeyCode::KEY_F12)?;
            wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",t4_focused,")), "modifier reset focus")?;
            press(&mut device, KeyCode::KEY_A)?;
            wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",filter_changed,")), "filter after modifier reset")?;
            press(&mut device, KeyCode::KEY_ESC)?;
            wait(|| focused.load(Ordering::SeqCst), "filter cancel")?;
            println!("quick: Alt held before opening selects Alt+1, subsequent plain typing filters");
            if test_keyboard::isolated() {
                let connection = zbus::blocking::Connection::session()?;
                let layout = zbus::blocking::Proxy::new(&connection, "org.kde.keyboard", "/Layouts", "org.kde.KeyboardLayouts")?;
                let changed: bool = layout.call("setLayout", &(1_u32,))?;
                if !changed { return Err("Russian layout unavailable in isolated compositor".into()); }
                let before = std::fs::read_to_string(&worker_csv)?;
                press_hotkey(&mut device, KeyCode::KEY_F12)?;
                wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",t4_focused,")), "Cyrillic focus")?;
                device.emit(&[*KeyEvent::new(KeyCode::KEY_LEFTCTRL, 1)])?;
                press(&mut device, KeyCode::KEY_A)?;
                device.emit(&[*KeyEvent::new(KeyCode::KEY_LEFTCTRL, 0)])?;
                press(&mut device, KeyCode::KEY_BACKSPACE)?;
                press(&mut device, KeyCode::KEY_L)?;
                press(&mut device, KeyCode::KEY_T)?;
                wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",filter_cyrillic,")), "Cyrillic text reached search")?;
                press(&mut device, KeyCode::KEY_ESC)?;
                wait(|| focused.load(Ordering::SeqCst), "Cyrillic cancel")?;
                let _: bool = layout.call("setLayout", &(0_u32,))?;
                println!("quick: real Cyrillic keyboard input reached filter");
            }
            let before = std::fs::read_to_string(&worker_csv)?;
            press_hotkey(&mut device, KeyCode::KEY_F11)?;
            wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",t4_focused,")), "stress focus")?;
            press(&mut device, KeyCode::KEY_6)?;
            wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",stress_page,")), "stress page")?;
            press(&mut device, KeyCode::KEY_DOWN)?;
            wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",navigation_handled,")), "stress navigation")?;
            activate_receiver()?;
            wait(|| std::fs::read_to_string(&worker_csv).is_ok_and(|s| s[before.len()..].contains(",hidden,")), "focus loss hides popup")?;
            println!("emoji: stress page navigates, focus loss hides popup");
            let status = Command::new(&binary).env("SLINT_SHELL_SOCKET", socket).arg("quit").status()?;
            if !status.success() { return Err("quit failed".into()); }
            server.0.wait()?;
            Ok(())
        })().map_err(|e| e.to_string());
        let _ = slint::quit_event_loop();
        result
    });
    receiver.run()?;
    handle.join().map_err(|_| "benchmark panicked")??;
    Ok(())
}

fn activate_receiver() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let connection = zbus::blocking::Connection::session()?;
    let mut script = tempfile::NamedTempFile::new()?;
    write!(
        script,
        "for (let w of workspace.windowList()) {{ if (w.pid === {}) {{ workspace.activeWindow = w; break; }} }}",
        std::process::id()
    )?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )?;
    let name = format!("lhc-bench-receiver-{}", std::process::id());
    let id: i32 = proxy.call(
        "loadScript",
        &(script.path().to_string_lossy().as_ref(), &name),
    )?;
    if id < 0 {
        return Err("receiver activation script failed".into());
    }
    let run = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        format!("/Scripting/Script{id}"),
        "org.kde.kwin.Script",
    )?;
    let result = run.call::<_, _, ()>("run", &());
    let _: bool = proxy.call("unloadScript", &name)?;
    result?;
    Ok(())
}
