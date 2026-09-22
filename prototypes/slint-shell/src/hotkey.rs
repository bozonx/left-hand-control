use crate::{Command, Dispatch};
use evdev::{Device, KeyCode};
use std::time::Instant;

pub fn start(dispatch: Dispatch) {
    if std::env::var("SLINT_SHELL_HOTKEYS").as_deref() == Ok("off") {
        return;
    }
    let devices: Vec<_> = if let Some(path) = std::env::var_os("SLINT_SHELL_INPUT") {
        match Device::open(&path) {
            Ok(device) => vec![(path.into(), device)],
            Err(error) => {
                log::error!("evdev: {error}");
                vec![]
            }
        }
    } else {
        evdev::enumerate().collect()
    };
    let mut count = 0;
    for (path, mut device) in devices {
        if !device.supported_keys().is_some_and(|keys| {
            keys.contains(KeyCode::KEY_LEFTCTRL)
                && keys.contains(KeyCode::KEY_LEFTALT)
                && (keys.contains(KeyCode::KEY_F11) || keys.contains(KeyCode::KEY_F12))
        }) {
            continue;
        }
        count += 1;
        let dispatch = dispatch.clone();
        std::thread::spawn(move || {
            log::info!("evdev listening: {}", path.display());
            let mut control = false;
            let mut alt = false;
            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() != evdev::EventType::KEY {
                                continue;
                            }
                            let key = KeyCode::new(event.code());
                            match key {
                                KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL => {
                                    control = event.value() != 0;
                                    continue;
                                }
                                KeyCode::KEY_LEFTALT | KeyCode::KEY_RIGHTALT => {
                                    alt = event.value() != 0;
                                    continue;
                                }
                                _ => {}
                            }
                            if event.value() != 1 || !control || !alt {
                                continue;
                            }
                            let command = match key {
                                KeyCode::KEY_F11 => Command::Show("emoji"),
                                KeyCode::KEY_F12 => Command::Show("quick"),
                                _ => continue,
                            };
                            dispatch(command, "evdev", Instant::now(), None);
                        }
                    }
                    Err(error) => {
                        log::error!("evdev {}: {error}", path.display());
                        break;
                    }
                }
            }
        });
    }
    if count == 0 {
        log::warn!(
            "No readable Ctrl+Alt+F11/F12 evdev devices; check input permissions or SLINT_SHELL_INPUT"
        );
    }
}
