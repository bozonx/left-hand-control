//! KDE Plasma keyboard layout backend (Wayland + X11).
//!
//! Current product support is Linux/KDE, so this backend stays fully native:
//! it talks to DBus directly through `zbus`, which the project already ships
//! for the portal backend. Other desktops remain explicit skeletons for now.
//! Every successful read goes through `super::publish`, which updates the
//! mapper cache and emits `LayoutChanged` only on real changes.
//!
//! The watcher listens to the interface's signals (`layoutChanged`,
//! `layoutListChanged`) and re-reads the layout on each, so nothing is
//! polled. Switching the active layout goes through the same DBus interface
//! (`setLayout(uint)`).

use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};

use super::KeyboardLayoutInfo;

/// Pause before the signal watcher reconnects.
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

type LayoutList = Vec<(String, String, String)>;

use crate::platform::dbus::{
    KEYBOARD_LAYOUTS_INTERFACE as IFACE, KEYBOARD_LAYOUTS_PATH as OBJECT,
    KEYBOARD_SERVICE as SERVICE, SERVICE_UNKNOWN_ERROR,
};

/// Session-bus connection for one-shot calls, opened once: layout
/// actions call in from the key path.
fn session_bus() -> Result<&'static Connection, String> {
    static BUS: OnceLock<Connection> = OnceLock::new();
    if let Some(bus) = BUS.get() {
        return Ok(bus);
    }
    let bus = Connection::session().map_err(|e| format!("Cannot connect to session bus: {e}"))?;
    Ok(BUS.get_or_init(|| bus))
}

fn keyboard_proxy(conn: &Connection) -> Result<Proxy<'static>, String> {
    Proxy::new(conn, SERVICE, OBJECT, IFACE).map_err(|e| format!("create keyboard proxy: {e}"))
}

pub fn current() -> Result<Option<KeyboardLayoutInfo>, String> {
    let result = (|| current_with_proxy(&keyboard_proxy(session_bus()?)?))();
    if !matches!(result, Ok(Some(_))) {
        super::clear();
    }
    result
}

pub fn refresh_cache() -> Result<Option<KeyboardLayoutInfo>, String> {
    current()
}

pub fn available_layouts() -> Result<Vec<KeyboardLayoutInfo>, String> {
    let proxy = keyboard_proxy(session_bus()?)?;
    let list = match call_list(&proxy)? {
        Some(v) => v,
        None => return Ok(vec![]),
    };
    let variants = configured_variants(&list);
    Ok(list
        .into_iter()
        .enumerate()
        .map(|(idx, entry)| KeyboardLayoutInfo {
            code: entry.0,
            display_label: entry.1,
            name: entry.2,
            variant: variants.get(idx).cloned().unwrap_or_default(),
            index: idx as u32,
            backend: "linux-kde",
        })
        .collect())
}

/// Switch the active layout to the given zero-based index.
///
/// Backed by `org.kde.KeyboardLayouts.setLayout(uint) -> bool`. The
/// boolean return is `false` when the index is out of range; we surface
/// that as an error so the shell can fall back to refreshing the
/// list and retrying.
pub fn set_layout(index: u32) -> Result<(), String> {
    let proxy = keyboard_proxy(session_bus()?)?;
    let msg = proxy
        .call_method("setLayout", &(index,))
        .map_err(|e| format!("Failed to switch layout to {index}: {e}"))?;
    let ok: bool = msg
        .body()
        .deserialize()
        .map_err(|e| format!("Failed to decode setLayout response: {e}"))?;
    if !ok {
        return Err(format!("Layout switch to {index} was rejected by KDE"));
    }
    let _ = current_with_proxy(&proxy);
    Ok(())
}

pub fn start_watcher() {
    SIGNAL_WATCHER.reset();
    super::register_watcher(
        thread::Builder::new()
            .name("layout-kde-signal".into())
            .spawn(run_signal_watcher),
    );
}

static SIGNAL_WATCHER: super::signal_watcher::SignalWatcher =
    super::signal_watcher::SignalWatcher::new();

fn run_signal_watcher() {
    SIGNAL_WATCHER.run("kde", RECONNECT_DELAY, || {
        let result = signal_watch_once();
        if result.is_err() {
            super::clear();
        }
        result
    });
}

pub(super) fn interrupt_signal_watcher() {
    SIGNAL_WATCHER.stop();
}

fn signal_watch_once() -> Result<(), String> {
    let conn = Connection::session().map_err(|e| format!("connect session bus: {e}"))?;
    if !SIGNAL_WATCHER.connected(conn.clone()) {
        return Ok(());
    }
    watch_connection(conn, emit_current, super::clear)
}

// OwnerChangedIterator lacks the runtime-aware destruction of SignalIterator.
// zbus can use Tokio when the shell's dependency features are unified.
struct OwnerChanges<'a>(Option<zbus::blocking::proxy::OwnerChangedIterator<'a>>);

impl OwnerChanges<'_> {
    fn changed(&mut self) -> bool {
        self.0
            .as_mut()
            .is_some_and(|owners| owners.next().is_some())
    }
}

impl Drop for OwnerChanges<'_> {
    fn drop(&mut self) {
        let owners = self.0.take();
        zbus::block_on(async move {
            drop(owners);
        });
    }
}

fn watch_connection(
    conn: Connection,
    observe: impl Fn(&Proxy<'_>) -> Result<(), String>,
    unavailable: impl FnOnce() + Send,
) -> Result<(), String> {
    // A stop requested before the connection was published.
    if super::watcher_stop_requested() {
        return Ok(());
    }
    let proxy = keyboard_proxy(&conn)?;
    thread::scope(|scope| {
        let mut owners =
            OwnerChanges(Some(proxy.receive_owner_changed().map_err(|error| {
                format!("subscribe to keyboard service owner: {error}")
            })?));
        let owner_connection = conn.clone();
        thread::Builder::new()
            .name("layout-kde-owner".into())
            .spawn_scoped(scope, move || {
                if owners.changed() {
                    unavailable();
                    // Interrupt the signal iterator so it reconnects to the new service.
                    let _ = owner_connection.close();
                }
            })
            .map_err(|error| format!("watch keyboard service owner: {error}"))?;
        let result = (|| {
            let signals = proxy
                .receive_all_signals()
                .map_err(|error| format!("subscribe to keyboard signals: {error}"))?;
            observe(&proxy)?;
            for _msg in signals {
                if super::watcher_stop_requested() {
                    return Ok(());
                }
                observe(&proxy)?;
            }
            if super::watcher_stop_requested() {
                return Ok(());
            }
            Err("keyboard signal stream ended".to_string())
        })();
        // Also release the owner iterator when a read fails or shutdown interrupts us.
        let _ = conn.close();
        result
    })
}

fn emit_current(proxy: &Proxy<'_>) -> Result<(), String> {
    match current_with_proxy(proxy)? {
        Some(_) => Ok(()),
        None => Err("KDE keyboard layout service is unavailable".into()),
    }
}

fn current_with_proxy(proxy: &Proxy<'_>) -> Result<Option<KeyboardLayoutInfo>, String> {
    let result = read_current(proxy);
    match &result {
        Ok(Some(info)) => super::publish(info),
        _ => super::clear(),
    }
    result
}

fn read_current(proxy: &Proxy<'_>) -> Result<Option<KeyboardLayoutInfo>, String> {
    let list = match call_list(proxy)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let idx = call_index(proxy)?.ok_or("KDE keyboard layout index is unavailable")?;
    let position = checked_position(&list, idx)?;
    let entry = list
        .get(position)
        .cloned()
        .ok_or_else(|| "No keyboard layouts configured".to_string())?;
    let variant = configured_variants(&list)
        .get(position)
        .cloned()
        .unwrap_or_default();
    let info = KeyboardLayoutInfo {
        code: entry.0,
        display_label: entry.1,
        name: entry.2,
        variant,
        index: idx,
        backend: "linux-kde",
    };
    Ok(Some(info))
}

fn checked_position(list: &LayoutList, index: u32) -> Result<usize, String> {
    let position =
        usize::try_from(index).map_err(|_| "KDE keyboard layout index is out of range")?;
    if position >= list.len() {
        return Err("KDE keyboard layout index is out of range".into());
    }
    Ok(position)
}

/// XKB variants of the configured layouts, by index. The DBus API only
/// gives layout codes and user labels, so they come from `kxkbrc`, and only
/// when its layout list matches `list`; otherwise none are known.
fn configured_variants(list: &LayoutList) -> Vec<String> {
    let Some(path) = dirs::config_dir().map(|dir| dir.join("kxkbrc")) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    variants_from_kxkbrc(&text, list)
}

fn variants_from_kxkbrc(text: &str, list: &LayoutList) -> Vec<String> {
    let mut in_layout = false;
    let mut layouts = None;
    let mut variants = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
            continue;
        }
        if !in_layout {
            continue;
        }
        if let Some(value) = line.strip_prefix("LayoutList=") {
            layouts = Some(value);
        } else if let Some(value) = line.strip_prefix("VariantList=") {
            variants = Some(value);
        }
    }
    let split =
        |value: &str| -> Vec<String> { value.split(',').map(|v| v.trim().to_owned()).collect() };
    let (Some(layouts), Some(variants)) = (layouts.map(split), variants.map(split)) else {
        return Vec::new();
    };
    let matches = layouts.len() == list.len()
        && layouts
            .iter()
            .zip(list)
            .all(|(layout, entry)| *layout == entry.0);
    if !matches {
        return Vec::new();
    }
    (0..list.len())
        .map(|idx| variants.get(idx).cloned().unwrap_or_default())
        .collect()
}

fn call_index(proxy: &Proxy<'_>) -> Result<Option<u32>, String> {
    let msg = match proxy.call_method("getLayout", &()) {
        Ok(msg) => msg,
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == SERVICE_UNKNOWN_ERROR => {
            return Ok(None);
        }
        Err(e) => return Err(format!("Failed to get current layout: {e}")),
    };
    let idx: u32 = msg
        .body()
        .deserialize()
        .map_err(|e| format!("Failed to decode getLayout response: {e}"))?;
    Ok(Some(idx))
}

fn call_list(proxy: &Proxy<'_>) -> Result<Option<LayoutList>, String> {
    let msg = match proxy.call_method("getLayoutsList", &()) {
        Ok(msg) => msg,
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == SERVICE_UNKNOWN_ERROR => {
            return Ok(None);
        }
        Err(e) => return Err(format!("Failed to get layout list: {e}")),
    };
    let list: Vec<(String, String, String)> = msg
        .body()
        .deserialize()
        .map_err(|e| format!("Failed to decode getLayoutsList response: {e}"))?;
    Ok(Some(list))
}

pub(super) fn xkb_options() -> String {
    dirs::config_dir()
        .and_then(|dir| std::fs::read_to_string(dir.join("kxkbrc")).ok())
        .map(|text| options_from_kxkbrc(&text))
        .unwrap_or_default()
}

fn options_from_kxkbrc(text: &str) -> String {
    let mut in_layout = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
        } else if in_layout && let Some(options) = line.strip_prefix("Options=") {
            return options.trim().into();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(codes: &[&str]) -> LayoutList {
        codes
            .iter()
            .map(|code| (code.to_string(), String::new(), String::new()))
            .collect()
    }

    #[test]
    fn variants_come_from_matching_kxkbrc() {
        let text =
            "[Layout]\nDisplayNames=,lat\nLayoutList=us,latam\nVariantList=dvorak,\nUse=true\n";
        assert_eq!(
            variants_from_kxkbrc(text, &list(&["us", "latam"])),
            ["dvorak", ""]
        );
    }

    #[test]
    fn variants_are_unknown_when_kxkbrc_differs() {
        let text = "[Layout]\nLayoutList=us,ru\nVariantList=dvorak,\n";
        assert!(variants_from_kxkbrc(text, &list(&["ru", "us"])).is_empty());
        let other_section = "[Other]\nLayoutList=us\nVariantList=dvorak\n";
        assert!(variants_from_kxkbrc(other_section, &list(&["us"])).is_empty());
    }
}

#[cfg(test)]
mod detection_tests {
    use super::*;

    struct Bus(std::process::Child);
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    struct Keyboard;
    #[zbus::interface(name = "org.kde.KeyboardLayouts")]
    impl Keyboard {
        #[zbus(name = "getLayoutsList")]
        fn layouts(&self) -> LayoutList {
            vec![("lhc-test".into(), String::new(), "Test".into())]
        }
        #[zbus(name = "getLayout")]
        fn layout(&self) -> u32 {
            0
        }
    }

    #[test]
    fn private_bus_owner_loss_clears_detection_and_watchers_exit() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        use std::time::Instant;
        let mut bus = Bus(Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is required for private bus tests"));
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim();
        let server = || {
            zbus::blocking::connection::Builder::address(address)
                .unwrap()
                .name(SERVICE)
                .unwrap()
                .serve_at(OBJECT, Keyboard)
                .unwrap()
                .build()
                .unwrap()
        };
        let state = std::sync::Arc::new(crate::runtime_state::RuntimeState::default());
        let publisher = std::sync::Arc::new(std::sync::Mutex::new(
            super::super::LayoutPublisher::default(),
        ));
        for restart in [false, true] {
            let service = server();
            let client = zbus::blocking::connection::Builder::address(address)
                .unwrap()
                .build()
                .unwrap();
            let worker_client = client.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let observed_state = state.clone();
            let lost_state = state.clone();
            let observed_publisher = publisher.clone();
            let lost_publisher = publisher.clone();
            let worker = thread::spawn(move || {
                let _ = tx.send(watch_connection(
                    worker_client,
                    |proxy| {
                        let info = read_current(proxy)?.ok_or("private service unavailable")?;
                        observed_publisher
                            .lock()
                            .unwrap()
                            .publish(&info, &observed_state, |_| {});
                        Ok(())
                    },
                    move || lost_publisher.lock().unwrap().clear(&lost_state, |_| {}),
                ));
            });
            let deadline = Instant::now() + Duration::from_secs(3);
            while state
                .keyboard_layout()
                .is_none_or(|layout| layout.code != "lhc-test")
            {
                if Instant::now() >= deadline {
                    let _ = client.clone().close();
                    panic!("watcher did not publish the private service layout");
                }
                thread::sleep(Duration::from_millis(5));
            }
            if restart {
                // Shutdown with a live owner must also release the owner iterator.
                client.clone().close().unwrap();
            } else {
                service.clone().close().unwrap();
            }
            let result = rx.recv_timeout(Duration::from_secs(3));
            let _ = client.close();
            assert!(result.expect("watcher remained blocked").is_err());
            worker.join().unwrap();
            if !restart {
                assert!(state.keyboard_layout().is_none());
            }
        }
    }

    #[test]
    fn invalid_group_does_not_fall_back_to_first_language() {
        let list = vec![("us".into(), String::new(), String::new())];
        assert_eq!(checked_position(&list, 0).unwrap(), 0);
        assert!(checked_position(&list, 1).is_err());
        assert!(checked_position(&list, u32::MAX).is_err());
        assert!(checked_position(&vec![], 0).is_err());
    }
}
