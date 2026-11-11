//! Well-known D-Bus names of the desktop services the app talks to.

pub const SERVICE_UNKNOWN_ERROR: &str = "org.freedesktop.DBus.Error.ServiceUnknown";
pub const BUS_SERVICE: &str = "org.freedesktop.DBus";

pub const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
pub const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
pub const PORTAL_REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";

/// Object path of a portal request made with `handle_token`;
/// `sender_escaped` is the unique bus name without `:` and with `.` → `_`.
pub fn portal_request_path(sender_escaped: &str, handle_token: &str) -> String {
    format!("{PORTAL_PATH}/request/{sender_escaped}/{handle_token}")
}

pub const NOTIFICATIONS_SERVICE: &str = "org.freedesktop.Notifications";
pub const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
pub const NOTIFICATIONS_INTERFACE: &str = "org.freedesktop.Notifications";

pub const KWIN_SERVICE: &str = "org.kde.KWin";
pub const KWIN_PATH: &str = "/KWin";
pub const KWIN_INTERFACE: &str = "org.kde.KWin";
pub const KWIN_SCRIPTING_PATH: &str = "/Scripting";
pub const KWIN_SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
pub const KWIN_SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";

pub const KGLOBALACCEL_SERVICE: &str = "org.kde.kglobalaccel";
pub const KGLOBALACCEL_PATH: &str = "/kglobalaccel";
pub const KGLOBALACCEL_INTERFACE: &str = "org.kde.KGlobalAccel";
pub const KGLOBALACCEL_COMPONENT_INTERFACE: &str = "org.kde.kglobalaccel.Component";

pub const KEYBOARD_SERVICE: &str = "org.kde.keyboard";
pub const KEYBOARD_LAYOUTS_PATH: &str = "/Layouts";
pub const KEYBOARD_LAYOUTS_INTERFACE: &str = "org.kde.KeyboardLayouts";
pub const KEYBOARD_SET_LAYOUT: &str = "setLayout";

pub const PLASMA_SHELL_SERVICE: &str = "org.kde.plasmashell";
pub const PLASMA_OSD_PATH: &str = "/org/kde/osdService";
pub const PLASMA_OSD_INTERFACE: &str = "org.kde.osdService";
