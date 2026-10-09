//! Built-in popup hotkeys read from evdev: Ctrl+Alt+F11 (emoji) and
//! Ctrl+Alt+F12 (quick actions). `SLINT_SHELL_HOTKEYS=off` disables them.
//!
//! Devices are rescanned periodically, so keyboards plugged in later and
//! the mapper's virtual keyboard (which carries the input while the mapper
//! grabs the physical one) are picked up.

use crate::command::{Command, Dispatch, Source, Window};
use evdev::{Device, KeyCode};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const RESCAN: Duration = Duration::from_secs(2);

fn is_keyboard(device: &Device) -> bool {
    device.supported_keys().is_some_and(|keys| {
        keys.contains(KeyCode::KEY_LEFTCTRL)
            && keys.contains(KeyCode::KEY_LEFTALT)
            && (keys.contains(KeyCode::KEY_F11) || keys.contains(KeyCode::KEY_F12))
    })
}

/// Modifier keys held right now; left and right count separately.
#[derive(Default)]
struct Modifiers(HashSet<KeyCode>);

impl Modifiers {
    /// Track `key`; returns `true` when it is a modifier.
    fn update(&mut self, key: KeyCode, value: i32) -> bool {
        if !matches!(
            key,
            KeyCode::KEY_LEFTCTRL
                | KeyCode::KEY_RIGHTCTRL
                | KeyCode::KEY_LEFTALT
                | KeyCode::KEY_RIGHTALT
        ) {
            return false;
        }
        if value == 0 {
            self.0.remove(&key);
        } else {
            self.0.insert(key);
        }
        true
    }

    fn ctrl_alt(&self) -> bool {
        let any = |a, b| self.0.contains(&a) || self.0.contains(&b);
        any(KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL)
            && any(KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT)
    }
}

/// Command for a key event, given the modifiers held before it.
fn command(modifiers: &mut Modifiers, key: KeyCode, value: i32) -> Option<Command> {
    if modifiers.update(key, value) || value != 1 || !modifiers.ctrl_alt() {
        return None;
    }
    match key {
        KeyCode::KEY_F11 => Some(Command::Show(Window::EMOJI)),
        KeyCode::KEY_F12 => Some(Command::Show(Window::QUICK)),
        _ => None,
    }
}

fn listen(
    path: PathBuf,
    mut device: Device,
    dispatch: Dispatch,
    watched: Arc<Mutex<HashSet<PathBuf>>>,
) {
    std::thread::spawn(move || {
        log::info!("evdev listening: {}", path.display());
        let mut modifiers = Modifiers::default();
        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if event.event_type() != evdev::EventType::KEY {
                            continue;
                        }
                        if let Some(command) =
                            command(&mut modifiers, KeyCode::new(event.code()), event.value())
                        {
                            dispatch(command, Source::Evdev, Instant::now(), None);
                        }
                    }
                }
                Err(error) => {
                    log::info!("evdev {} closed: {error}", path.display());
                    break;
                }
            }
        }
        // Allow the device to be opened again if it comes back.
        watched
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&path);
    });
}

pub fn start(dispatch: Dispatch) {
    if std::env::var("SLINT_SHELL_HOTKEYS").as_deref() == Ok("off") {
        return;
    }
    let watched: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
    if let Some(path) = std::env::var_os("SLINT_SHELL_INPUT") {
        let path = PathBuf::from(path);
        match Device::open(&path) {
            Ok(device) => listen(path, device, dispatch, watched),
            Err(error) => log::error!("evdev {}: {error}", path.display()),
        }
        return;
    }
    std::thread::spawn(move || {
        let mut warned = false;
        // Devices that are not keyboards or cannot be opened; forgotten now
        // and then because event numbers are reused after unplugging.
        let mut ignored = HashSet::new();
        let mut scan = 0u64;
        loop {
            if scan % 30 == 0 {
                ignored.clear();
            }
            scan += 1;
            let mut found = !watched.lock().unwrap_or_else(|p| p.into_inner()).is_empty();
            let paths = std::fs::read_dir("/dev/input")
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("event"))
                });
            for path in paths {
                if ignored.contains(&path)
                    || watched
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .contains(&path)
                {
                    continue;
                }
                let Some(device) = Device::open(&path).ok().filter(is_keyboard) else {
                    ignored.insert(path);
                    continue;
                };
                found = true;
                watched
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(path.clone());
                listen(path, device, dispatch.clone(), watched.clone());
            }
            if !found && !warned {
                log::warn!(
                    "No readable Ctrl+Alt+F11/F12 evdev devices; check input permissions or SLINT_SHELL_INPUT"
                );
                warned = true;
            }
            std::thread::sleep(RESCAN);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_and_right_modifiers_are_tracked_separately() {
        let mut m = Modifiers::default();
        assert_eq!(command(&mut m, KeyCode::KEY_LEFTCTRL, 1), None);
        assert_eq!(command(&mut m, KeyCode::KEY_RIGHTCTRL, 1), None);
        assert_eq!(command(&mut m, KeyCode::KEY_RIGHTCTRL, 0), None);
        assert_eq!(command(&mut m, KeyCode::KEY_LEFTALT, 1), None);
        assert_eq!(
            command(&mut m, KeyCode::KEY_F11, 1),
            Some(Command::Show(Window::EMOJI))
        );
        assert_eq!(
            command(&mut m, KeyCode::KEY_F11, 2),
            None,
            "repeats do not reopen"
        );
        assert_eq!(command(&mut m, KeyCode::KEY_LEFTALT, 0), None);
        assert_eq!(command(&mut m, KeyCode::KEY_F12, 1), None);
    }
}
