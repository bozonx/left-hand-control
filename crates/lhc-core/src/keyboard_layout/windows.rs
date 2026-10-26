//! Windows keyboard-layout backend.
//!
//! Windows keeps one input language per thread, so the current layout is
//! the one of the foreground window's thread. Layouts are named by their
//! input language: `code` is the language subtag (`en`, `ru`), `display_label`
//! the locale name (`en-US`), `name` its localized display name. A watcher
//! polls the foreground layout, which also follows focus changes.

#![cfg(target_os = "windows")]

use super::KeyboardLayoutInfo;
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Globalization::{
    GetLocaleInfoEx, LCIDToLocaleName, LOCALE_SLOCALIZEDDISPLAYNAME,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyboardLayoutList, HKL};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};
use windows::core::PCWSTR;

const POLL: Duration = Duration::from_millis(200);

fn installed() -> Vec<HKL> {
    let count = unsafe { GetKeyboardLayoutList(None) }.max(0) as usize;
    let mut layouts = vec![HKL::default(); count];
    let count = unsafe { GetKeyboardLayoutList(Some(&mut layouts)) }.max(0) as usize;
    layouts.truncate(count);
    layouts
}

fn foreground_layout() -> Option<HKL> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    let thread = unsafe { GetWindowThreadProcessId(window, None) };
    let layout = unsafe { GetKeyboardLayout(thread) };
    (!layout.is_invalid()).then_some(layout)
}

/// Locale name of the layout's input language, e.g. `en-US`.
fn locale_name(layout: HKL) -> Option<Vec<u16>> {
    let language = (layout.0 as usize & 0xffff) as u32;
    let mut name = [0_u16; 85];
    let length = unsafe { LCIDToLocaleName(language, Some(&mut name), 0) };
    // The length includes the terminating NUL.
    (length > 1).then(|| name[..length as usize].to_vec())
}

fn info(layout: HKL, index: u32) -> Option<KeyboardLayoutInfo> {
    let name = locale_name(layout)?;
    let display = String::from_utf16_lossy(&name[..name.len() - 1]);
    let mut long = [0_u16; 256];
    let length = unsafe {
        GetLocaleInfoEx(
            PCWSTR(name.as_ptr()),
            LOCALE_SLOCALIZEDDISPLAYNAME,
            Some(&mut long),
        )
    };
    let long = String::from_utf16_lossy(&long[..(length.max(1) as usize - 1)]);
    Some(KeyboardLayoutInfo {
        code: short_code(&display),
        display_label: display,
        name: long,
        variant: String::new(),
        index,
        backend: "windows",
    })
}

/// Language subtag of a locale name: `en-US` → `en`.
fn short_code(locale: &str) -> String {
    locale
        .split('-')
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase()
}

pub fn current() -> Result<Option<KeyboardLayoutInfo>, String> {
    let Some(layout) = foreground_layout() else {
        return Ok(None);
    };
    let index = installed()
        .iter()
        .position(|candidate| *candidate == layout)
        .unwrap_or(0) as u32;
    Ok(info(layout, index))
}

pub fn available_layouts() -> Result<Vec<KeyboardLayoutInfo>, String> {
    Ok(installed()
        .into_iter()
        .enumerate()
        .filter_map(|(index, layout)| info(layout, index as u32))
        .collect())
}

/// Ask the foreground window to switch to installed layout `index`
/// (zero-based); the window's thread applies it.
pub fn set_layout(index: u32) -> Result<(), String> {
    let layout = *installed()
        .get(index as usize)
        .ok_or_else(|| format!("Keyboard layout {} is not installed", index + 1))?;
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return Err("No window is focused to switch its keyboard layout".into());
    }
    unsafe {
        PostMessageW(
            Some(window),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(layout.0 as isize),
        )
    }
    .map_err(|error| format!("Keyboard layout switch failed: {error}"))
}

/// Publish the current layout; the posted switch request lands a moment
/// later, so the watcher publishes its result too.
pub fn refresh_cache() -> Result<Option<KeyboardLayoutInfo>, String> {
    let current = current()?;
    if let Some(info) = &current {
        super::publish(info);
    }
    Ok(current)
}

pub fn start_watcher() {
    super::register_watcher(
        std::thread::Builder::new()
            .name("lhc-layout".into())
            .spawn(|| {
                let mut last = None;
                while !super::watcher_stop_requested() {
                    let layout = foreground_layout().map(|layout| layout.0 as usize);
                    if layout != last {
                        last = layout;
                        if let Ok(Some(info)) = current() {
                            super::publish(&info);
                        }
                    }
                    std::thread::sleep(POLL);
                }
            }),
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn short_code_is_the_language_subtag() {
        assert_eq!(super::short_code("en-US"), "en");
        assert_eq!(super::short_code("ru-RU"), "ru");
        assert_eq!(super::short_code("sr-Latn-RS"), "sr");
    }
}
