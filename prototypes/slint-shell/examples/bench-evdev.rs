use evdev::{AttributeSet, KeyCode, KeyEvent, uinput::VirtualDevice};
use std::{
    path::PathBuf,
    process::{Child, Command},
    thread::sleep,
    time::Duration,
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let binary = PathBuf::from(
        args.next()
            .ok_or("expected binary path and output CSV path")?,
    );
    let csv = PathBuf::from(args.next().ok_or("expected output CSV path")?);
    let runtime =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR missing")?);
    if runtime.join("lhc-slint-shell.sock").exists() {
        return Err("Stop slint-shell first (remove its socket only if stale)".into());
    }
    let keys: AttributeSet<KeyCode> = [
        KeyCode::KEY_F13,
        KeyCode::KEY_SCROLLLOCK,
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
            .env("SLINT_SHELL_METRICS", &csv)
            .env_remove("XDG_ACTIVATION_TOKEN")
            .spawn()?,
    );
    sleep(Duration::from_secs(2));
    if server.0.try_wait()?.is_some() {
        return Err("server failed to start".into());
    }
    for (name, key) in [
        ("emoji", KeyCode::KEY_F13),
        ("quick", KeyCode::KEY_SCROLLLOCK),
    ] {
        for _ in 0..20 {
            press(&mut device, key)?;
            sleep(Duration::from_millis(400));
            let data = std::fs::read_to_string(&csv)?;
            let latest = data
                .lines()
                .rev()
                .find(|line| line.contains(",t0_trigger,"))
                .ok_or("evdev trigger was not received")?;
            let id = latest.split(',').next().unwrap();
            let prefix = format!("{id},{name},evdev,");
            if !latest.starts_with(&prefix) {
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
                sleep(Duration::from_millis(50));
                let data = std::fs::read_to_string(&csv)?;
                let handled = data
                    .lines()
                    .any(|line| line.starts_with(&format!("{prefix}navigation_handled,")));
                println!("{name} trial {id}: focused, first arrow handled={handled}");
            } else {
                println!(
                    "{name} trial {id}: no current focus (never focused or already hidden); arrow skipped"
                );
            }
            if !Command::new(&binary).arg("hide").status()?.success() {
                return Err("hide failed".into());
            }
            sleep(Duration::from_millis(200));
        }
    }
    if !Command::new(&binary).arg("quit").status()?.success() {
        return Err("quit failed".into());
    }
    server.0.wait()?;
    Ok(())
}
