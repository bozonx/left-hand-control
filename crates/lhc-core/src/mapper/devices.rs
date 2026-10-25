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
