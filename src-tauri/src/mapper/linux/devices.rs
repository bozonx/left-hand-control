use super::VIRTUAL_DEVICE_NAME;
use crate::mapper::KeyboardDevice;
use evdev::{Device, Key};
use std::path::PathBuf;

pub fn list_keyboards() -> Result<Vec<KeyboardDevice>, String> {
    let mut out = Vec::new();
    let dir = std::fs::read_dir("/dev/input").map_err(|e| format!("read /dev/input: {e}"))?;
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
    eprintln!(
        "[mapper] list_keyboards: found {} event* paths",
        paths.len()
    );

    for path in paths {
        let Ok(dev) = Device::open(&path) else {
            eprintln!("[mapper]   {}: open failed", path.display());
            continue;
        };
        let name = dev.name().unwrap_or("(unknown)").to_string();
        let is_kb = is_keyboard(&dev);
        eprintln!(
            "[mapper]   {}: name='{}' is_keyboard={}",
            path.display(),
            name,
            is_kb
        );
        if !is_kb {
            continue;
        }
        if name == VIRTUAL_DEVICE_NAME {
            continue;
        }
        out.push(KeyboardDevice {
            path: path.to_string_lossy().to_string(),
            name,
        });
    }
    eprintln!("[mapper] list_keyboards -> {} keyboards", out.len());
    Ok(out)
}

pub fn list_mice() -> Result<Vec<KeyboardDevice>, String> {
    let mut out = Vec::new();
    let dir = std::fs::read_dir("/dev/input").map_err(|e| format!("read /dev/input: {e}"))?;
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
    eprintln!("[mapper] list_mice: found {} event* paths", paths.len());

    for path in paths {
        let Ok(dev) = Device::open(&path) else {
            eprintln!("[mapper]   {}: open failed", path.display());
            continue;
        };
        let name = dev.name().unwrap_or("(unknown)").to_string();
        let is_m = is_mouse(&dev);
        eprintln!(
            "[mapper]   {}: name='{}' is_mouse={}",
            path.display(),
            name,
            is_m
        );
        if !is_m {
            continue;
        }
        if name == VIRTUAL_DEVICE_NAME {
            continue;
        }
        out.push(KeyboardDevice {
            path: path.to_string_lossy().to_string(),
            name,
        });
    }
    eprintln!("[mapper] list_mice -> {} mice", out.len());
    Ok(out)
}

fn is_keyboard(dev: &Device) -> bool {
    let Some(keys) = dev.supported_keys() else {
        return false;
    };
    // Heuristic: considered a keyboard if it reports several letter keys
    // plus space and enter. Excludes mice, power buttons, sleep keys, etc.
    let required = [
        Key::KEY_A,
        Key::KEY_S,
        Key::KEY_D,
        Key::KEY_F,
        Key::KEY_SPACE,
        Key::KEY_ENTER,
    ];
    required.iter().all(|k| keys.contains(*k))
}

fn is_mouse(dev: &Device) -> bool {
    let Some(keys) = dev.supported_keys() else {
        return false;
    };
    // Heuristic: mouse has left/right buttons but NOT letter keys.
    // We also skip devices that already pass the keyboard heuristic
    // (combined keyboard+mouse combos should be grabbed as a keyboard).
    keys.contains(Key::BTN_LEFT) && keys.contains(Key::BTN_RIGHT) && !is_keyboard(dev)
}
