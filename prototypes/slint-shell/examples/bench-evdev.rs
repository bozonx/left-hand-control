use evdev::{AttributeSet, KeyCode, KeyEvent, uinput::VirtualDevice};
use std::{
    path::PathBuf,
    process::{Child, Command},
    thread::sleep,
    time::{Duration, Instant},
};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn press(device: &mut VirtualDevice, key: KeyCode) -> std::io::Result<()> {
    device.emit(&[*KeyEvent::new(key, 1)])?;
    device.emit(&[*KeyEvent::new(key, 0)])
}

fn press_hotkey(device: &mut VirtualDevice, key: KeyCode) -> std::io::Result<()> {
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
    let mut args = std::env::args().skip(1);
    let binary = PathBuf::from(
        args.next()
            .ok_or("expected binary path and output CSV path")?,
    );
    let csv = PathBuf::from(args.next().ok_or("expected output CSV path")?);
    let count = std::env::var("SLINT_SHELL_BENCH_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(20);
    let socket = format!("lhc-evdev-bench-{}.sock", std::process::id());
    let keys: AttributeSet<KeyCode> = [
        KeyCode::KEY_F11,
        KeyCode::KEY_F12,
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_LEFTALT,
        KeyCode::KEY_DOWN,
        KeyCode::KEY_ESC,
        KeyCode::KEY_A,
        KeyCode::KEY_Z,
        KeyCode::KEY_SPACE,
        KeyCode::KEY_ENTER,
        KeyCode::KEY_LEFTSHIFT,
    ]
    .into_iter()
    .collect();
    let mut device = VirtualDevice::builder()?
        .name("Slint Shell stage 3 probe")
        .with_keys(&keys)?
        .build()?;
    let input = device
        .enumerate_dev_nodes_blocking()?
        .next()
        .ok_or("no virtual input node")??;
    let mut server = Server(
        Command::new(&binary)
            .env("SLINT_SHELL_INPUT", input)
            .env("SLINT_SHELL_SOCKET", &socket)
            .env("SLINT_SHELL_METRICS", &csv)
            .env_remove("XDG_ACTIVATION_TOKEN")
            .spawn()?,
    );
    sleep(Duration::from_secs(2));
    if server.0.try_wait()?.is_some() {
        return Err("server failed to start".into());
    }
    let spell_csv = PathBuf::from(format!("{}.spell.csv", csv.display()));
    let csv = if std::env::var("SLINT_SHELL_POPUPS").is_ok_and(|mode| mode != "winit")
        && spell_csv.exists()
    {
        spell_csv
    } else {
        csv
    };
    let mut failures = 0;
    for (name, key) in [("emoji", KeyCode::KEY_F11), ("quick", KeyCode::KEY_F12)] {
        for _ in 0..count {
            let before = std::fs::read_to_string(&csv)?;
            let previous = before
                .lines()
                .rev()
                .find(|line| line.contains(",t0_trigger,"))
                .and_then(|line| line.split(',').next())
                .unwrap_or("0");
            press_hotkey(&mut device, key)?;
            let deadline = Instant::now() + Duration::from_secs(2);
            let data = loop {
                let data = std::fs::read_to_string(&csv)?;
                if let Some(latest) = data
                    .lines()
                    .rev()
                    .find(|line| line.contains(",t0_trigger,"))
                {
                    let id = latest.split(',').next().unwrap();
                    let prefix = format!("{id},{name},evdev,");
                    if id != previous
                        && latest.starts_with(&prefix)
                        && data
                            .lines()
                            .any(|line| line.starts_with(&format!("{prefix}t4_focused,")))
                    {
                        break data;
                    }
                }
                if Instant::now() >= deadline {
                    break data;
                }
                sleep(Duration::from_millis(5));
            };
            let latest = data
                .lines()
                .rev()
                .find(|line| line.contains(",t0_trigger,"))
                .ok_or("evdev trigger was not received")?;
            let id = latest.split(',').next().unwrap();
            let prefix = format!("{id},{name},evdev,");
            if id == previous || !latest.starts_with(&prefix) {
                return Err("wrong trigger source or target".into());
            }
            let focused = data
                .lines()
                .any(|line| line.starts_with(&format!("{prefix}t4_focused,")));
            let hidden = data
                .lines()
                .any(|line| line.starts_with(&format!("{prefix}hidden,")));
            if focused && !hidden {
                press(&mut device, KeyCode::KEY_DOWN)?;
                let deadline = Instant::now() + Duration::from_millis(500);
                let handled = loop {
                    let data = std::fs::read_to_string(&csv)?;
                    if data
                        .lines()
                        .any(|line| line.starts_with(&format!("{prefix}navigation_handled,")))
                    {
                        break true;
                    }
                    if Instant::now() >= deadline {
                        break false;
                    }
                    sleep(Duration::from_millis(5));
                };
                if !handled {
                    failures += 1;
                }
                println!("{name} trial {id}: focused, first arrow handled={handled}");
            } else {
                failures += 1;
                println!(
                    "{name} trial {id}: no current focus (never focused or already hidden); arrow skipped"
                );
            }
            if !Command::new(&binary)
                .env("SLINT_SHELL_SOCKET", &socket)
                .arg("hide")
                .status()?
                .success()
            {
                return Err("hide failed".into());
            }
            sleep(Duration::from_millis(200));
        }
    }
    if !Command::new(&binary)
        .env("SLINT_SHELL_SOCKET", &socket)
        .arg("quit")
        .status()?
        .success()
    {
        return Err("quit failed".into());
    }
    server.0.wait()?;
    if failures > 0 {
        return Err(format!("{failures}/{} trials failed focus/navigation", count * 2).into());
    }
    Ok(())
}
