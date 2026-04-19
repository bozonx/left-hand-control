// Current keyboard layout detection.
//
// Provides a cross-platform abstraction. Each OS has its own submodule
// implementing `current()` (one-shot query) and `watch()` (spawn a background
// watcher that emits `layout-changed` Tauri events when the active layout
// changes).
//
// Frontend contract:
//   * command `get_current_layout()` -> Option<LayoutInfo>
//   * event   `layout-changed`       -> LayoutInfo
//
// Currently supported: Linux + KDE Plasma (Wayland/X11) via DBus
// (`org.kde.keyboard /Layouts`).

use serde::Serialize;

#[cfg(all(target_os = "linux"))]
mod linux_kde;

#[derive(Debug, Clone, Serialize)]
pub struct LayoutInfo {
    /// Short code, e.g. "us", "ru".
    pub short: String,
    /// Optional display/variant name, e.g. "lat".
    pub display: String,
    /// Long human-readable name, e.g. "English (US)".
    pub long: String,
    /// Zero-based index among the configured layouts.
    pub index: u32,
    /// Which backend produced this info.
    pub backend: &'static str,
}

pub fn current() -> Result<Option<LayoutInfo>, String> {
    #[cfg(target_os = "linux")]
    {
        linux_kde::current()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(None)
    }
}

/// Start a background watcher that emits `layout-changed` events.
/// Safe to call once at app startup. No-op if no backend is available.
pub fn start_watcher(app: tauri::AppHandle) {
    #[cfg(target_os = "linux")]
    {
        linux_kde::start_watcher(app);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
    }
}
