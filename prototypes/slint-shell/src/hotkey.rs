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
            keys.contains(KeyCode::KEY_F13) || keys.contains(KeyCode::KEY_SCROLLLOCK)
        }) {
            continue;
        }
        count += 1;
        let dispatch = dispatch.clone();
        std::thread::spawn(move || {
            log::info!("evdev listening: {}", path.display());
            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() != evdev::EventType::KEY || event.value() != 1 {
                                continue;
                            }
                            let command = match KeyCode::new(event.code()) {
                                KeyCode::KEY_F13 => Command::Show("emoji"),
                                KeyCode::KEY_SCROLLLOCK => Command::Show("quick"),
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
            "No readable F13/ScrollLock evdev devices; check input permissions or SLINT_SHELL_INPUT"
        );
    }
}
