//! Input device descriptions shared by the backends and the device pickers.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct KeyboardDevice {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    pub path: String,
    pub name: String,
    pub is_keyboard: bool,
    pub is_mouse: bool,
    /// Created by software (uinput) rather than attached hardware.
    pub is_virtual: bool,
}

/// Whether two device paths open the same device: equal, or symlinks such
/// as `/dev/input/by-id/…` resolving to the same `/dev/input/eventN`.
pub fn same_device(a: &str, b: &str) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(a), Ok(b)) if a == b
        )
}

const INPUT_PREFIX: &str = "/dev/input/";
/// Longest path [`short_path`] returns in a row; longer ones keep their distinctive end.
pub const MAX_DETAIL_CHARS: usize = 60;

/// Device name, or one read from its `by-id` link when the kernel gives none.
pub fn label(name: &str, path: &str) -> String {
    let name = name.trim();
    if !name.is_empty() && name != "(unknown)" {
        return name.to_owned();
    }
    path.strip_prefix("/dev/input/by-id/")
        .map(|link| {
            let link = strip_event_suffix(link);
            let link = ["usb-", "bluetooth-", "i2c-", "serio-", "platform-"]
                .iter()
                .find_map(|bus| link.strip_prefix(bus))
                .unwrap_or(link);
            let link = link.split(':').next().unwrap_or(link);
            let link = match link.rsplit_once("-if") {
                Some((head, tail))
                    if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) =>
                {
                    head
                }
                _ => link,
            };
            link.replace('_', " ").trim().to_owned()
        })
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| short_path(path))
}

/// Path without the shared `/dev/input/` prefix and udev's `-event-*`
/// suffix, cut from the left when still too long.
pub fn short_path(path: &str) -> String {
    let Some(rest) = path.strip_prefix(INPUT_PREFIX) else {
        return path.to_owned();
    };
    let rest = strip_event_suffix(rest);
    let rest = match rest.split_once('/') {
        Some((dir, link)) => format!("{dir} · {link}"),
        None => rest.to_owned(),
    };
    let count = rest.chars().count();
    if count <= MAX_DETAIL_CHARS {
        return rest;
    }
    let tail: String = rest.chars().skip(count - (MAX_DETAIL_CHARS - 1)).collect();
    format!("…{tail}")
}

fn strip_event_suffix(link: &str) -> &str {
    match link.rfind("-event") {
        Some(at) if !link[at + 6..].contains(['/', '.']) && at > 0 => &link[..at],
        _ => link,
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn device_rows_show_readable_names_and_short_paths() {
        let sofle = "/dev/input/by-id/usb-MTKB_W-SOFLE_vial:f64c2b3c-event-kbd";
        assert_eq!(label("", sofle), "MTKB W-SOFLE vial");
        assert_eq!(label("(unknown)", sofle), "MTKB W-SOFLE vial");
        assert_eq!(label("Logitech", sofle), "Logitech");
        assert_eq!(
            label(
                "",
                "/dev/input/by-id/usb-Logitech_USB_Receiver-if02-event-mouse"
            ),
            "Logitech USB Receiver"
        );
        assert_eq!(short_path(sofle), "by-id · usb-MTKB_W-SOFLE_vial:f64c2b3c");
        assert_eq!(short_path("/dev/input/event18"), "event18");
        assert_eq!(label("", "/dev/input/event18"), "event18");
        assert_eq!(short_path("windows:keyboard"), "windows:keyboard");
        let long = short_path(
            "/dev/input/by-path/pci-0000:00:14.3-platform-PNP0C0D:00-very-long-name-for-a-device-event",
        );
        assert!(long.starts_with('…') && long.ends_with("long-name-for-a-device"));
        assert_eq!(long.chars().count(), MAX_DETAIL_CHARS);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::same_device;
    use std::os::unix::fs::symlink;

    #[test]
    fn symlinks_to_one_event_device_are_the_same_device() {
        let temp = tempfile::tempdir().unwrap();
        let path = |name: &str| temp.path().join(name).to_string_lossy().into_owned();
        std::fs::write(temp.path().join("event3"), "").unwrap();
        std::fs::write(temp.path().join("event4"), "").unwrap();
        symlink("event3", temp.path().join("usb-Kbd-event-kbd")).unwrap();

        assert!(same_device(&path("event3"), &path("usb-Kbd-event-kbd")));
        assert!(!same_device(&path("event4"), &path("usb-Kbd-event-kbd")));
        // Missing paths match only themselves.
        assert!(same_device(&path("gone"), &path("gone")));
        assert!(!same_device(&path("gone"), &path("event3")));
    }
}
