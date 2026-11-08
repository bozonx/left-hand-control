//! Desktop accessibility preferences (high contrast, reduced motion) that
//! the `System` choice of the matching settings follows.
//!
//! On Linux a watcher thread reads the XDG Settings portal
//! (`org.freedesktop.appearance`: `contrast`, `reduced-motion`; on KDE also
//! `AnimationDurationFactor` of `kdeglobals`, which Plasma's animation speed
//! slider sets), caches the result and emits
//! `CoreEvent::SystemAccessibilityChanged` when it changes. Other platforms
//! report neither preference yet.

use std::sync::Mutex;

/// What the desktop asks applications for; `false` when unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystemAccessibility {
    pub high_contrast: bool,
    pub reduce_motion: bool,
}

static CURRENT: Mutex<SystemAccessibility> = Mutex::new(SystemAccessibility {
    high_contrast: false,
    reduce_motion: false,
});

/// The latest preferences the watcher read.
pub fn current() -> SystemAccessibility {
    *CURRENT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Cache `value`; emit an event when it differs from the cached one.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn publish(value: SystemAccessibility) {
    let changed = {
        let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::replace(&mut *current, value) != value
    };
    if changed {
        log::info!("[accessibility] system preferences: {value:?}");
        crate::events::emit(crate::CoreEvent::SystemAccessibilityChanged(value));
    }
}

/// Start following the desktop preferences. Idempotent; the watcher lives
/// as long as the process.
pub fn start_watcher() {
    #[cfg(target_os = "linux")]
    linux::start();
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{SystemAccessibility, publish};
    use crate::platform::dbus::{PORTAL_PATH, PORTAL_SERVICE};
    use crate::platform::linux::Desktop;
    use std::time::Duration;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedValue, Value};

    const SETTINGS_INTERFACE: &str = "org.freedesktop.portal.Settings";
    const APPEARANCE: &str = "org.freedesktop.appearance";
    const KDE_GLOBALS: &str = "org.kde.kdeglobals.KDE";
    const RETRY_DELAY: Duration = Duration::from_secs(30);

    pub(super) fn start() {
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| {
            let kde = crate::platform::linux::detect().desktop == Desktop::Kde;
            let spawned = std::thread::Builder::new()
                .name("lhc-accessibility".into())
                .spawn(move || {
                    loop {
                        if let Err(error) = watch(kde) {
                            log::debug!("[accessibility] portal settings: {error}");
                        }
                        std::thread::sleep(RETRY_DELAY);
                    }
                });
            if let Err(error) = spawned {
                log::warn!("[accessibility] start watcher: {error}");
            }
        });
    }

    /// Read the preferences, then re-read on every relevant change until
    /// the connection ends.
    fn watch(kde: bool) -> Result<(), String> {
        let connection = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
        let proxy = Proxy::new(&connection, PORTAL_SERVICE, PORTAL_PATH, SETTINGS_INTERFACE)
            .map_err(|e| format!("create proxy: {e}"))?;
        // Subscribe first, so a change between the read and the
        // subscription is not lost.
        let changes = proxy
            .receive_signal("SettingChanged")
            .map_err(|e| format!("subscribe: {e}"))?;
        publish(read(&proxy, kde));
        for message in changes {
            let Ok((namespace, _key, _value)) =
                message.body().deserialize::<(String, String, OwnedValue)>()
            else {
                continue;
            };
            if namespace == APPEARANCE || (kde && namespace == KDE_GLOBALS) {
                publish(read(&proxy, kde));
            }
        }
        Err("signal stream ended".into())
    }

    fn read(proxy: &Proxy<'_>, kde: bool) -> SystemAccessibility {
        let high_contrast = setting(proxy, APPEARANCE, "contrast")
            .and_then(|value| number(&value))
            .is_some_and(|contrast| contrast == 1.0);
        let reduce_motion = setting(proxy, APPEARANCE, "reduced-motion")
            .and_then(|value| number(&value))
            .is_some_and(|motion| motion == 1.0)
            || (kde
                && setting(proxy, KDE_GLOBALS, "AnimationDurationFactor")
                    .and_then(|value| number(&value))
                    .is_some_and(|factor| factor <= 0.0));
        SystemAccessibility {
            high_contrast,
            reduce_motion,
        }
    }

    /// One portal setting; `None` when the portal or the key is missing.
    fn setting(proxy: &Proxy<'_>, namespace: &str, key: &str) -> Option<OwnedValue> {
        proxy
            .call::<_, _, OwnedValue>("ReadOne", &(namespace, key))
            // Portals older than version 2 only have `Read`, which wraps
            // the value in one more variant.
            .or_else(|_| proxy.call::<_, _, OwnedValue>("Read", &(namespace, key)))
            .ok()
    }

    /// A numeric setting: portal enums are `u32`, kdeglobals values may come
    /// as doubles or as their config-file text.
    fn number(value: &Value<'_>) -> Option<f64> {
        match value {
            Value::Value(inner) => number(inner),
            Value::U32(n) => Some(f64::from(*n)),
            Value::I32(n) => Some(f64::from(*n)),
            Value::U8(n) => Some(f64::from(*n)),
            Value::F64(n) => Some(*n),
            Value::Str(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn numbers_are_read_through_variants_and_text() {
            assert_eq!(number(&Value::U32(1)), Some(1.0));
            assert_eq!(number(&Value::Value(Box::new(Value::U32(1)))), Some(1.0));
            assert_eq!(number(&Value::Str("0".into())), Some(0.0));
            assert_eq!(number(&Value::Str("0.5".into())), Some(0.5));
            assert_eq!(number(&Value::Bool(true)), None);
        }
    }
}
