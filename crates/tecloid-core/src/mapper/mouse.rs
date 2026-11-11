//! Availability of the optional mouse, independent of keyboard interception.

#[cfg(any(target_os = "linux", test))]
use std::io::ErrorKind;
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};

/// State of the selected mouse. A failed mouse never stops keyboard mapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseStatus {
    #[default]
    NotSelected,
    Ready,
    Connected,
    Missing,
    AccessDenied,
    DeviceChanged,
    InUse,
    Unavailable,
}

impl MouseStatus {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn from_error(error: &std::io::Error) -> Self {
        match error.kind() {
            ErrorKind::NotFound => Self::Missing,
            ErrorKind::PermissionDenied => Self::AccessDenied,
            _ => Self::Unavailable,
        }
    }
}

/// Check access without grabbing the mouse or touching its event stream.
pub fn probe(path: Option<&str>) -> MouseStatus {
    let Some(path) = path.filter(|path| !path.is_empty()) else {
        return MouseStatus::NotSelected;
    };
    #[cfg(target_os = "linux")]
    {
        std::fs::File::open(path)
            .map(|_| MouseStatus::Ready)
            .unwrap_or_else(|error| MouseStatus::from_error(&error))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        MouseStatus::NotSelected
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Default)]
pub(crate) struct Monitor(Arc<Mutex<MouseStatus>>);

#[cfg(target_os = "linux")]
impl Monitor {
    pub fn get(&self) -> MouseStatus {
        *self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub fn set(&self, status: MouseStatus) {
        self.set_with(status, || {
            crate::events::bus().emit(crate::CoreEvent::MapperDeviceChanged)
        });
    }

    fn set_with(&self, status: MouseStatus, changed: impl FnOnce()) {
        let mut current = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if *current == status {
            return;
        }
        *current = status;
        drop(current);
        changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_errors_are_typed_and_missing_paths_are_reported() {
        assert_eq!(
            MouseStatus::from_error(&ErrorKind::PermissionDenied.into()),
            MouseStatus::AccessDenied
        );
        assert_eq!(
            MouseStatus::from_error(&ErrorKind::Other.into()),
            MouseStatus::Unavailable
        );
        assert_eq!(probe(None), MouseStatus::NotSelected);
        #[cfg(target_os = "linux")]
        {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("mouse");
            assert_eq!(probe(path.to_str()), MouseStatus::Missing);
            std::fs::write(&path, "").unwrap();
            assert_eq!(probe(path.to_str()), MouseStatus::Ready);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn repeated_failures_do_not_flood_events_and_recovery_is_announced() {
        let monitor = Monitor::default();
        let shared = monitor.clone();
        let count = std::cell::Cell::new(0);
        for status in [
            MouseStatus::Missing,
            MouseStatus::Missing,
            MouseStatus::AccessDenied,
            MouseStatus::Connected,
            MouseStatus::Connected,
            MouseStatus::Missing,
        ] {
            monitor.set_with(status, || {
                assert_eq!(shared.get(), status);
                count.set(count.get() + 1);
            });
        }
        assert_eq!(count.get(), 4);
        assert_eq!(monitor.get(), MouseStatus::Missing);
    }
}
