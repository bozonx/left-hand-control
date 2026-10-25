use super::VIRTUAL_DEVICE_NAME;
use crate::mapper::{InputDevice, KeyboardDevice};
use crate::platform::INPUT_DIR;
use evdev::{Device, KeyCode};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// udev symlinks to event devices, preferred in this order. Their names
/// survive reboots and replugging, unlike `eventN`.
const STABLE_DIRS: [&str; 2] = [BY_ID_DIR, "/dev/input/by-path"];
/// Links named after the device itself, so they follow it to another port.
pub(super) const BY_ID_DIR: &str = "/dev/input/by-id";

/// Stable symlink for each `/dev/input/eventN` that has one.
fn stable_paths() -> HashMap<PathBuf, PathBuf> {
    stable_paths_in(STABLE_DIRS.map(Path::new))
}

/// Stable symlinks found in `dirs`; an earlier directory wins, and within
/// one directory the first name in sort order.
fn stable_paths_in<'a>(dirs: impl IntoIterator<Item = &'a Path>) -> HashMap<PathBuf, PathBuf> {
    let mut out = HashMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut links: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        links.sort();
        for link in links {
            if let Ok(target) = std::fs::canonicalize(&link) {
                out.entry(target).or_insert(link);
            }
        }
    }
    out
}

/// Path to store for `event`: its stable symlink when udev provides one.
fn stable_path(event: &Path, stable: &HashMap<PathBuf, PathBuf>) -> String {
    std::fs::canonicalize(event)
        .ok()
        .and_then(|target| stable.get(&target))
        .map_or(event, PathBuf::as_path)
        .to_string_lossy()
        .into_owned()
}

pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
    let mut out = Vec::new();
    let dir = std::fs::read_dir(INPUT_DIR).map_err(|e| format!("read {INPUT_DIR}: {e}"))?;
    let mut paths: Vec<PathBuf> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("event"))
                .unwrap_or(false)
        })
        .collect();
    paths.sort();
    log::debug!(
        "[mapper] list_input_devices: found {} event* paths",
        paths.len()
    );

    let stable = stable_paths();
    let mut first_error = None;
    for path in paths {
        let dev = match Device::open(&path) {
            Ok(dev) => dev,
            Err(error) => {
                log::debug!("[mapper]   {}: open failed: {error}", path.display());
                first_error.get_or_insert_with(|| format!("open {}: {error}", path.display()));
                continue;
            }
        };
        let name = dev.name().unwrap_or("(unknown)").to_string();
        let is_keyboard = is_keyboard(&dev);
        let is_mouse = is_mouse(&dev);
        log::debug!(
            "[mapper]   {}: name='{}' is_keyboard={} is_mouse={}",
            path.display(),
            name,
            is_keyboard,
            is_mouse
        );
        if name == VIRTUAL_DEVICE_NAME {
            continue;
        }
        let path_to_store = stable_path(&path, &stable);
        // uinput devices get no udev links and usually no physical location.
        let is_virtual = dev.input_id().bus_type() == evdev::BusType::BUS_VIRTUAL
            || (path_to_store == path.to_string_lossy()
                && dev.physical_path().is_none_or(str::is_empty));
        out.push(InputDevice {
            path: path_to_store,
            name,
            is_keyboard,
            is_mouse,
            is_virtual,
        });
    }
    if out.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }
    log::debug!("[mapper] list_input_devices -> {} devices", out.len());
    Ok(out)
}

pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    Ok(list_input_devices()?
        .into_iter()
        .filter(|device| device.is_keyboard)
        .map(|device| KeyboardDevice {
            path: device.path,
            name: device.name,
        })
        .collect())
}

pub fn list_mice() -> Result<Vec<KeyboardDevice>, String> {
    Ok(list_input_devices()?
        .into_iter()
        .filter(|device| device.is_mouse)
        .map(|device| KeyboardDevice {
            path: device.path,
            name: device.name,
        })
        .collect())
}

fn is_keyboard(dev: &Device) -> bool {
    let Some(keys) = dev.supported_keys() else {
        return false;
    };
    // Heuristic: considered a keyboard if it reports several letter keys
    // plus space and enter. Excludes mice, power buttons, sleep keys, etc.
    let required = [
        KeyCode::KEY_A,
        KeyCode::KEY_S,
        KeyCode::KEY_D,
        KeyCode::KEY_F,
        KeyCode::KEY_SPACE,
        KeyCode::KEY_ENTER,
    ];
    required.iter().all(|k| keys.contains(*k))
}

fn is_mouse(dev: &Device) -> bool {
    let Some(keys) = dev.supported_keys() else {
        return false;
    };
    // Heuristic: mouse has left/right buttons but NOT letter keys.
    // We also skip devices that already pass the keyboard heuristic
    // (mixed interfaces cannot safely be grabbed by the keyboard backend).
    keys.contains(KeyCode::BTN_LEFT) && keys.contains(KeyCode::BTN_RIGHT) && !is_keyboard(dev)
}

#[cfg(test)]
mod tests {
    use super::{stable_path, stable_paths_in};
    use std::os::unix::fs::symlink;

    #[test]
    fn devices_are_stored_by_their_most_stable_name() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let (by_id, by_path) = (root.join("by-id"), root.join("by-path"));
        std::fs::create_dir(&by_id).unwrap();
        std::fs::create_dir(&by_path).unwrap();
        for event in ["event3", "event4", "event5"] {
            std::fs::write(root.join(event), "").unwrap();
        }
        symlink("../event3", by_id.join("usb-Kbd-if01-event-kbd")).unwrap();
        symlink("../event3", by_id.join("usb-Kbd-event-kbd")).unwrap();
        symlink(
            "../event3",
            by_path.join("pci-0000:00:14.0-usb-0:3:1.0-event-kbd"),
        )
        .unwrap();
        symlink(
            "../event4",
            by_path.join("platform-i8042-serio-0-event-kbd"),
        )
        .unwrap();
        let stable = stable_paths_in([by_id.as_path(), by_path.as_path()]);

        // by-id survives moving the keyboard to another port; ties go to
        // the first name, so the choice is the same on every listing.
        assert_eq!(
            stable_path(&root.join("event3"), &stable),
            by_id.join("usb-Kbd-event-kbd").to_string_lossy()
        );
        assert_eq!(
            stable_path(&root.join("event4"), &stable),
            by_path
                .join("platform-i8042-serio-0-event-kbd")
                .to_string_lossy()
        );
        // Without a udev link the event node itself is stored.
        assert_eq!(
            stable_path(&root.join("event5"), &stable),
            root.join("event5").to_string_lossy()
        );
        // Missing directories are skipped.
        assert!(stable_paths_in([root.join("absent").as_path()]).is_empty());
    }
}
