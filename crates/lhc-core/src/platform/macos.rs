//! macOS: the frontmost application, running applications and whether
//! the frontmost application covers a display.
//!
//! `NSWorkspace` keeps these lists current while the main run loop runs,
//! which the Slint event loop does. Window titles would need the Screen
//! Recording permission, so they are not read.

use std::ffi::c_void;

use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CGRect};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGGetActiveDisplayList,
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption,
    kCGNullWindowID, kCGWindowBounds, kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
};
use objc2_foundation::{NSBundle, NSString};

use crate::runtime_state::ActiveWindow;

pub(crate) fn active_window() -> Option<ActiveWindow> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let app_id = app
        .bundleIdentifier()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let process_name = executable_name(&app);
    if app_id.is_empty() && process_name.is_none() {
        return None;
    }
    Some(ActiveWindow {
        title: String::new(),
        app_id,
        process_name,
        fullscreen: covers_display(app.processIdentifier()),
    })
}

fn executable_name(app: &NSRunningApplication) -> Option<String> {
    app.executableURL()?
        .lastPathComponent()
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
}

/// Executable names of the running applications.
pub(crate) fn running_process_names() -> Option<Vec<String>> {
    let apps = NSWorkspace::sharedWorkspace().runningApplications();
    Some(
        apps.iter()
            .filter(|app| !app.isTerminated())
            .filter_map(|app| executable_name(&app))
            .collect(),
    )
}

/// The frontmost application's name when its `Info.plist` files it under
/// games (`public.app-category.games` or a `*-games` subcategory).
pub(crate) fn frontmost_game() -> Option<String> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let url = app.bundleURL()?;
    let bundle = NSBundle::bundleWithURL(&url)?;
    let category =
        bundle.objectForInfoDictionaryKey(&NSString::from_str("LSApplicationCategoryType"))?;
    let category = category.downcast_ref::<NSString>()?.to_string();
    let game = category == "public.app-category.games"
        || category
            .strip_prefix("public.app-category.")
            .is_some_and(|name| name.ends_with("-games"));
    game.then(|| {
        app.bundleIdentifier()
            .map(|id| id.to_string())
            .or_else(|| executable_name(&app))
            .unwrap_or(category)
    })
}

/// Whether a normal-level window of `pid` covers a whole display.
/// Window bounds need no permission, unlike window titles.
fn covers_display(pid: i32) -> Option<bool> {
    let windows = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    )?;
    let displays = display_bounds();
    for index in 0..windows.count() {
        // SAFETY: the window list holds one dictionary per window.
        let window = unsafe { &*(windows.value_at_index(index) as *const CFDictionary) };
        if number(window, unsafe { kCGWindowOwnerPID }) != Some(i64::from(pid))
            || number(window, unsafe { kCGWindowLayer }) != Some(0)
        {
            continue;
        }
        // SAFETY: `kCGWindowBounds` maps to a dictionary representation of a rect.
        let bounds = unsafe { window.value(key(kCGWindowBounds)) } as *const CFDictionary;
        if bounds.is_null() {
            continue;
        }
        let mut rect = CGRect::default();
        if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&*bounds), &mut rect) } {
            continue;
        }
        if displays.iter().any(|display| covers(&rect, display)) {
            return Some(true);
        }
    }
    Some(false)
}

fn covers(window: &CGRect, display: &CGRect) -> bool {
    window.origin.x <= display.origin.x
        && window.origin.y <= display.origin.y
        && window.origin.x + window.size.width >= display.origin.x + display.size.width
        && window.origin.y + window.size.height >= display.origin.y + display.size.height
}

fn display_bounds() -> Vec<CGRect> {
    let mut ids: [CGDirectDisplayID; 16] = [0; 16];
    let mut count = 0_u32;
    let status = unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
    if status.0 != 0 {
        return Vec::new();
    }
    ids[..(count as usize).min(ids.len())]
        .iter()
        .map(|id| CGDisplayBounds(*id))
        .collect()
}

fn key(name: &CFString) -> *const c_void {
    (name as *const CFString).cast()
}

fn number(dictionary: &CFDictionary, name: &CFString) -> Option<i64> {
    // SAFETY: the window-info keys asked for here map to numbers.
    let value = unsafe { dictionary.value(key(name)) } as *const CFNumber;
    if value.is_null() {
        return None;
    }
    unsafe { &*value }.as_i64()
}

/// Exact foreground destination for returning popup text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTextTarget {
    /// Process owning the captured window.
    pub process_id: i32,
    window_id: u32,
}

/// Capture a normal foreground window without reading its private title.
pub fn capture_text_target() -> Option<NativeTextTarget> {
    let process_id = NSWorkspace::sharedWorkspace()
        .frontmostApplication()?
        .processIdentifier();
    let windows = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    )?;
    for index in 0..windows.count() {
        // SAFETY: CGWindowListCopyWindowInfo returns an array of window dictionaries.
        let window = unsafe { &*(windows.value_at_index(index) as *const CFDictionary) };
        if number(window, unsafe { kCGWindowOwnerPID }) == Some(i64::from(process_id))
            && number(window, unsafe { kCGWindowLayer }) == Some(0)
        {
            let window_id = u32::try_from(number(window, unsafe { kCGWindowNumber })?).ok()?;
            return Some(NativeTextTarget {
                process_id,
                window_id,
            });
        }
    }
    None
}

/// Activate the captured application's existing front window, refusing unrelated focus changes.
#[allow(deprecated)]
pub fn restore_text_target(target: NativeTextTarget, selection_process: u32) -> Result<(), String> {
    let current = capture_text_target().ok_or("cannot determine foreground window")?;
    if current.process_id != target.process_id && current.process_id as u32 != selection_process {
        return Err("foreground changed after selection".into());
    }
    if current.process_id == target.process_id && current != target {
        return Err("another window of the destination application has focus".into());
    }
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(target.process_id)
        .ok_or("destination application exited")?;
    if !app.activateWithOptions(
        objc2_app_kit::NSApplicationActivationOptions::ActivateIgnoringOtherApps,
    ) {
        return Err("application activation was rejected".into());
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while std::time::Instant::now() < deadline {
        if capture_text_target() == Some(target) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err("destination window did not become foreground".into())
}

/// Whether Shift, Control, Option and Command are all released.
pub fn text_modifiers_released() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceFlagsState(state: i32) -> u64;
    }
    let flags = unsafe { CGEventSourceFlagsState(0) };
    flags & ((1 << 17) | (1 << 18) | (1 << 19) | (1 << 20)) == 0
}
