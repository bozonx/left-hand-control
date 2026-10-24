//! KDE Plasma: a KWin script loaded once reports every focus change over
//! D-Bus, so detection needs neither polling nor `kdotool`.
//!
//! The script calls back into a small D-Bus object this process serves
//! on its unique bus name; calls from anyone but KWin are ignored.

use crate::platform::dbus;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, Proxy, fdo::DBusProxy};
use zbus::message::Header;
use zbus::names::BusName;

use super::ActiveWindow;

/// How often an open session is checked, and the pause before reopening a failed one.
const SESSION_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const SESSION_RETRY: Duration = Duration::from_secs(3);

const OBJECT_PATH: &str = "/dev/bozonx/LeftHandControl/KWin";
const INTERFACE: &str = "dev.bozonx.LeftHandControl.KWin";
const ACTIVE_SCRIPT: &str = "left-hand-control-active-window";
const LIST_SCRIPT: &str = "left-hand-control-window-list";
const LIST_TIMEOUT: Duration = Duration::from_millis(1500);

/// Reports the active window as JSON on activation and, while it stays
/// active, on fullscreen, geometry and (with titles) caption changes.
/// Borderless windows covering their output count as fullscreen: many
/// games use them instead of real fullscreen. Plasma 5 names are the
/// fallbacks.
const ACTIVE_SCRIPT_BODY: &str = r#"
var current = null, last = null;
function fullscreen(w) {
  var f = !!w.fullScreen;
  try {
    var g = w.frameGeometry, o = w.output.geometry;
    f = f || (w.normalWindow && w.noBorder && g.x <= o.x && g.y <= o.y &&
      g.x + g.width >= o.x + o.width && g.y + g.height >= o.y + o.height);
  } catch (e) {}
  return f;
}
function report() {
  var w = current, r = {};
  if (w) {
    r.appId = String(w.resourceClass || "");
    r.pid = w.pid;
    r.windowId = String(w.internalId || w.windowId || "");
    r.fullscreen = fullscreen(w);
    if (TITLES) r.title = String(w.caption || "");
  }
  var text = JSON.stringify(r);
  if (text !== last) {
    last = text;
    callDBus(SERVICE, PATH, IFACE, "Active", text);
  }
}
function watch(w, on) {
  if (!w) return;
  var signals = [w.fullScreenChanged, w.frameGeometryChanged];
  if (TITLES) signals.push(w.captionChanged);
  for (var i = 0; i < signals.length; i++) {
    try { if (on) signals[i].connect(report); else signals[i].disconnect(report); } catch (e) {}
  }
}
function activated(w) {
  watch(current, false);
  current = w;
  watch(current, true);
  report();
}
(workspace.windowActivated || workspace.clientActivated).connect(activated);
activated(workspace.activeWindow || workspace.activeClient);
"#;

const LIST_SCRIPT_BODY: &str = r#"
var list = workspace.windowList ? workspace.windowList() : workspace.clientList(), out = [];
for (var i = 0; i < list.length; i++) {
  var w = list[i];
  if (!w.normalWindow || w.skipTaskbar) continue;
  out.push({appId: String(w.resourceClass || ""), pid: w.pid, title: String(w.caption || "")});
}
callDBus(SERVICE, PATH, IFACE, "Windows", JSON.stringify(out));
"#;

/// What the script reported last; `None` before the first report.
#[derive(Default)]
struct Focus {
    current: Option<String>,
    external: Option<String>,
}

impl Focus {
    fn update(&mut self, value: &serde_json::Value) {
        self.current = value
            .get("windowId")
            .and_then(|id| id.as_str())
            .filter(|id| !id.is_empty())
            .map(str::to_owned);
        if value.get("appId").and_then(|id| id.as_str()) != Some(crate::storage::POPUP_APP_ID) {
            self.external = self.current.clone();
        }
    }
}

static FOCUS: Mutex<Focus> = Mutex::new(Focus {
    current: None,
    external: None,
});

pub(super) fn text_target() -> Option<String> {
    FOCUS.lock().ok()?.external.clone()
}

pub(super) fn target_is_current(target: &str) -> bool {
    FOCUS
        .lock()
        .ok()
        .is_some_and(|focus| focus.current.as_deref() == Some(target))
}

static PUSHED: Mutex<Option<Option<ActiveWindow>>> = Mutex::new(None);
/// Receiver of the next window list.
static LIST_REPLY: Mutex<Option<mpsc::Sender<String>>> = Mutex::new(None);
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

struct Session {
    connection: Connection,
    /// Whether the loaded script reports titles.
    titles: bool,
    file: PathBuf,
    owner: String,
    checked: Instant,
}

struct Service {
    /// Unique bus name of KWin, the only accepted caller; known once the
    /// connection is up.
    kwin: Arc<OnceLock<String>>,
}

// Calls must apply in arrival order: a focus change sends `{}` and the new
// window back to back, and a task per call could leave `{}` as the last state.
#[zbus::interface(name = "dev.bozonx.LeftHandControl.KWin", spawn = false)]
impl Service {
    fn active(&self, json: String, #[zbus(header)] header: Header<'_>) {
        if !self.sent_by_kwin(&header) {
            return;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json)
            && let Ok(mut focus) = FOCUS.lock()
        {
            focus.update(&value);
        }
        let window = serde_json::from_str::<serde_json::Value>(&json)
            .ok()
            .and_then(|value| super::linux::window_from_json(&value, "title", "appId"));
        if let Ok(mut pushed) = PUSHED.lock() {
            *pushed = Some(window);
        }
        super::wake();
    }

    fn windows(&self, json: String, #[zbus(header)] header: Header<'_>) {
        if !self.sent_by_kwin(&header) {
            return;
        }
        if let Ok(mut reply) = LIST_REPLY.lock()
            && let Some(reply) = reply.take()
        {
            let _ = reply.send(json);
        }
    }
}

impl Service {
    fn sent_by_kwin(&self, header: &Header<'_>) -> bool {
        header
            .sender()
            .is_some_and(|sender| self.kwin.get().is_some_and(|kwin| sender.as_str() == kwin))
    }
}

/// Load the script and start receiving focus changes.
pub(super) fn start(titles: bool) -> bool {
    match open(titles) {
        Ok(session) => {
            if let Ok(mut slot) = SESSION.lock() {
                *slot = Some(session);
            }
            true
        }
        Err(error) => {
            log::info!("[active-window] KWin script unavailable, polling instead: {error}");
            false
        }
    }
}

fn open(titles: bool) -> Result<Session, String> {
    open_connection(connect()?, script_path("active"), titles)
}

fn open_connection(
    (connection, owner): (Connection, String),
    file: PathBuf,
    titles: bool,
) -> Result<Session, String> {
    load_script(
        &connection,
        &file,
        ACTIVE_SCRIPT,
        ACTIVE_SCRIPT_BODY,
        titles,
    )?;
    Ok(Session {
        connection,
        titles,
        file,
        owner,
        checked: Instant::now(),
    })
}

/// A session-bus connection serving the callback object.
fn connect() -> Result<(Connection, String), String> {
    connect_builder(
        zbus::blocking::connection::Builder::session().map_err(|error| error.to_string())?,
    )
}

fn connect_builder(
    builder: zbus::blocking::connection::Builder<'_>,
) -> Result<(Connection, String), String> {
    let kwin = Arc::new(OnceLock::new());
    // Serving through the builder keeps zbus inside its own runtime when
    // another crate enabled its `tokio` feature.
    let connection = builder
        .serve_at(OBJECT_PATH, Service { kwin: kwin.clone() })
        .and_then(|builder| builder.build())
        .map_err(|error| error.to_string())?;
    let name = BusName::try_from(dbus::KWIN_SERVICE).map_err(|error| error.to_string())?;
    let owner = DBusProxy::new(&connection)
        .map_err(|error| error.to_string())?
        .get_name_owner(name)
        .map_err(|error| format!("KWin is not on the session bus: {error}"))?;
    let _ = kwin.set(owner.to_string());
    Ok((connection, owner.to_string()))
}

/// Whether the script runs and reports.
pub(super) fn active() -> bool {
    SESSION.lock().is_ok_and(|session| session.is_some())
}

fn owner(connection: &Connection) -> Option<String> {
    DBusProxy::new(connection)
        .ok()?
        .get_name_owner(BusName::try_from(dbus::KWIN_SERVICE).ok()?)
        .ok()
        .map(|owner| owner.to_string())
}

fn reset_reports() {
    if let Ok(mut pushed) = PUSHED.lock() {
        *pushed = None;
    }
    if let Ok(mut focus) = FOCUS.lock() {
        *focus = Focus::default();
    }
}

/// Recover after a compositor/bus restart and retry failed initial loads.
pub(super) fn sync(titles: bool) {
    static RETRY: Mutex<Option<Instant>> = Mutex::new(None);
    let Ok(mut slot) = SESSION.lock() else { return };
    let Ok(mut retry) = RETRY.lock() else { return };
    sync_session(&mut slot, &mut retry, titles, Instant::now(), open);
}

fn sync_session(
    slot: &mut Option<Session>,
    retry: &mut Option<Instant>,
    titles: bool,
    now: Instant,
    open: impl FnOnce(bool) -> Result<Session, String>,
) {
    if let Some(session) = slot.as_mut()
        && now.duration_since(session.checked) >= SESSION_CHECK_INTERVAL
    {
        session.checked = now;
        if owner(&session.connection).as_deref() != Some(session.owner.as_str()) {
            // Never expose the old compositor's cached window or target.
            reset_reports();
            close(slot.take());
        }
    }
    if slot.is_none() {
        if retry.is_some_and(|at| now.duration_since(at) < SESSION_RETRY) {
            return;
        }
        *retry = Some(now);
        match open(titles) {
            Ok(session) => {
                *slot = Some(session);
                *retry = None;
            }
            Err(error) => log::debug!("[active-window] KWin reconnect: {error}"),
        }
        return;
    }
    let Some(session) = slot.as_mut() else { return };
    if session.titles == titles {
        return;
    }
    reset_reports();
    match load_script(
        &session.connection,
        &session.file,
        ACTIVE_SCRIPT,
        ACTIVE_SCRIPT_BODY,
        titles,
    ) {
        Ok(()) => session.titles = titles,
        Err(error) => {
            log::warn!("[active-window] KWin script reload failed: {error}");
            close(slot.take());
        }
    }
}

/// Latest report; `None` until the script reported once.
pub(super) fn current() -> Option<Option<ActiveWindow>> {
    PUSHED.lock().ok().and_then(|pushed| pushed.clone())
}

pub(super) fn stop() {
    let session = SESSION.lock().ok().and_then(|mut slot| slot.take());
    close(session);
    reset_reports();
}

fn close(session: Option<Session>) {
    if let Some(session) = session {
        unload_script(&session.connection, ACTIVE_SCRIPT);
        let _ = std::fs::remove_file(&session.file);
        let _ = session.connection.close();
    }
}

/// Applications with normal windows, via a one-shot script.
pub(super) fn open_windows() -> Option<Vec<ActiveWindow>> {
    let session_connection = SESSION
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|session| session.connection.clone()));
    let connection = match session_connection {
        Some(connection) => connection,
        // Without the watcher's session a temporary one serves the reply.
        None => connect().ok()?.0,
    };
    let (sender, receiver) = mpsc::channel();
    *LIST_REPLY.lock().ok()? = Some(sender);
    let file = script_path("list");
    let loaded = load_script(&connection, &file, LIST_SCRIPT, LIST_SCRIPT_BODY, true);
    let reply = loaded
        .is_ok()
        .then(|| receiver.recv_timeout(LIST_TIMEOUT).ok());
    unload_script(&connection, LIST_SCRIPT);
    let _ = std::fs::remove_file(&file);
    if let Ok(mut slot) = LIST_REPLY.lock() {
        slot.take();
    }
    let json = reply.flatten()?;
    let items: Vec<serde_json::Value> = serde_json::from_str(&json).ok()?;
    Some(
        items
            .iter()
            .filter_map(|item| super::linux::window_from_json(item, "title", "appId"))
            .collect(),
    )
}

fn script_path(kind: &str) -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("lhc-kwin-{kind}-{}.js", std::process::id()))
}

/// Write the script with its callback address and run it under `name`,
/// replacing a script left by a previous run.
fn load_script(
    connection: &Connection,
    file: &PathBuf,
    name: &str,
    body: &str,
    titles: bool,
) -> Result<(), String> {
    let service = connection
        .unique_name()
        .ok_or("no unique bus name")?
        .to_string();
    let header = format!(
        "var SERVICE = {}, PATH = {}, IFACE = {}, TITLES = {titles};",
        json_string(&service),
        json_string(OBJECT_PATH),
        json_string(INTERFACE),
    );
    std::fs::write(file, format!("{header}{body}")).map_err(|error| error.to_string())?;
    unload_script(connection, name);
    let scripting = scripting(connection)?;
    let path = file.to_string_lossy();
    let id: i32 = scripting
        .call(
            "loadScript",
            &(path.as_ref(), script_name(connection, name)?),
        )
        .map_err(|error| error.to_string())?;
    if id < 0 {
        return Err(format!("KWin refused script {name}"));
    }
    // `start` runs every loaded script that is not running yet. The
    // `Script<id>` object is not reliable: KWin numbers scripts by their
    // count, so after an unload the new id can name another live script.
    scripting.call::<_, _, ()>("start", &()).map_err(|error| {
        unload_script(connection, name);
        error.to_string()
    })
}

fn unload_script(connection: &Connection, name: &str) {
    if let Ok(name) = script_name(connection, name)
        && let Ok(scripting) = scripting(connection)
    {
        let _: Result<bool, _> = scripting.call("unloadScript", &name);
    }
}

fn script_name(connection: &Connection, name: &str) -> Result<String, String> {
    let service = connection.unique_name().ok_or("no unique bus name")?;
    // A second instance must not replace or unload the first one's watcher.
    Ok(format!("{name}-{service}"))
}

fn scripting(connection: &Connection) -> Result<Proxy<'static>, String> {
    Proxy::new(
        connection,
        dbus::KWIN_SERVICE,
        dbus::KWIN_SCRIPTING_PATH,
        dbus::KWIN_SCRIPTING_INTERFACE,
    )
    .map_err(|error| error.to_string())
}

fn json_string(text: &str) -> String {
    serde_json::Value::String(text.into()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn settings_accept_text_and_popups_preserve_the_destination() {
        let mut focus = super::Focus::default();
        let report = |app, id| serde_json::json!({"appId": app, "windowId": id});
        focus.update(&report("org.kde.kate", "editor"));
        focus.update(&report(crate::storage::APP_ID, "settings"));
        assert_eq!(focus.current.as_deref(), Some("settings"));
        assert_eq!(focus.external, focus.current);
        focus.update(&report(crate::storage::POPUP_APP_ID, "quick"));
        assert_eq!(focus.external.as_deref(), Some("settings"));
        assert_ne!(focus.current, focus.external);
        focus.update(&report(crate::storage::APP_ID, "settings"));
        assert_eq!(focus.external, focus.current);
        focus.update(&serde_json::json!({}));
        assert!(focus.current.is_none());
        assert!(focus.external.is_none());
    }

    struct Bus(std::process::Child);
    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    struct Scripting {
        loads: Arc<AtomicUsize>,
        names: Arc<Mutex<std::collections::HashSet<String>>>,
    }
    #[zbus::interface(name = "org.kde.kwin.Scripting")]
    impl Scripting {
        #[zbus(name = "loadScript")]
        fn load_script(&self, _file: &str, name: &str) -> i32 {
            self.loads.fetch_add(1, Ordering::SeqCst);
            assert!(self.names.lock().unwrap().insert(name.into()));
            0
        }
        #[zbus(name = "unloadScript")]
        fn unload_script(&self, name: &str) -> bool {
            self.names.lock().unwrap().remove(name)
        }
        #[zbus(name = "start")]
        fn start(&self) {}
    }

    #[test]
    fn compositor_owner_restart_recovers_and_discards_old_reports() {
        let directory = tempfile::tempdir().unwrap();
        let mut daemon = Bus(Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is needed for the private bus test"));
        let mut address = String::new();
        BufReader::new(daemon.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim();
        let loads = Arc::new(AtomicUsize::new(0));
        let names = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let server = || {
            zbus::blocking::connection::Builder::address(address)
                .unwrap()
                .name(dbus::KWIN_SERVICE)
                .unwrap()
                .serve_at(
                    "/Scripting",
                    Scripting {
                        loads: loads.clone(),
                        names: names.clone(),
                    },
                )
                .unwrap()
                .build()
                .unwrap()
        };
        let open = |titles| {
            let connection =
                connect_builder(zbus::blocking::connection::Builder::address(address).unwrap())?;
            open_connection(connection, directory.path().join("active.js"), titles)
        };
        let now = Instant::now();
        let mut slot = None;
        let mut retry = None;
        // A missing compositor is retried rather than disabling the source forever.
        sync_session(&mut slot, &mut retry, false, now, open);
        assert!(slot.is_none());
        let first = server();
        sync_session(
            &mut slot,
            &mut retry,
            false,
            now + Duration::from_secs(4),
            open,
        );
        let old_owner = slot.as_ref().unwrap().owner.clone();
        *PUSHED.lock().unwrap() = Some(Some(ActiveWindow {
            app_id: "old-game".into(),
            fullscreen: Some(true),
            ..ActiveWindow::default()
        }));
        FOCUS.lock().unwrap().current = Some("old-window".into());
        first.close().unwrap();
        let second = server();
        sync_session(
            &mut slot,
            &mut retry,
            false,
            now + Duration::from_secs(8),
            open,
        );
        assert_ne!(slot.as_ref().unwrap().owner, old_owner);
        assert!(current().is_none());
        assert!(!target_is_current("old-window"));
        assert_eq!(loads.load(Ordering::SeqCst), 2);
        // Two application connections coexist; closing the second cannot
        // remove the first connection's watcher or its one-shot list script.
        let primary = slot.as_ref().unwrap().connection.clone();
        let primary_name = script_name(&primary, ACTIVE_SCRIPT).unwrap();
        let other = open_connection(
            connect_builder(zbus::blocking::connection::Builder::address(address).unwrap())
                .unwrap(),
            directory.path().join("second.js"),
            false,
        )
        .unwrap();
        let other_name = script_name(&other.connection, ACTIVE_SCRIPT).unwrap();
        assert_ne!(primary_name, other_name);
        assert!(names.lock().unwrap().contains(&primary_name));
        assert!(names.lock().unwrap().contains(&other_name));
        load_script(
            &primary,
            &directory.path().join("list.js"),
            LIST_SCRIPT,
            LIST_SCRIPT_BODY,
            true,
        )
        .unwrap();
        load_script(
            &other.connection,
            &directory.path().join("second-list.js"),
            LIST_SCRIPT,
            LIST_SCRIPT_BODY,
            true,
        )
        .unwrap();
        unload_script(&other.connection, LIST_SCRIPT);
        assert!(
            names
                .lock()
                .unwrap()
                .contains(&script_name(&primary, LIST_SCRIPT).unwrap())
        );
        close(Some(other));
        assert!(names.lock().unwrap().contains(&primary_name));
        assert!(!names.lock().unwrap().contains(&other_name));
        unload_script(&primary, LIST_SCRIPT);
        // Fresh owner callbacks work, while other senders are still rejected.
        let connection = slot.as_ref().unwrap().connection.clone();
        let proxy = Proxy::new(
            &second,
            connection.unique_name().unwrap().as_str(),
            OBJECT_PATH,
            INTERFACE,
        )
        .unwrap();
        proxy
            .call::<_, _, ()>(
                "Active",
                &(r#"{"appId":"new-game","fullscreen":true,"windowId":"new-window"}"#,),
            )
            .unwrap();
        assert_eq!(current().unwrap().unwrap().app_id, "new-game");
        let impostor = zbus::blocking::connection::Builder::address(address)
            .unwrap()
            .build()
            .unwrap();
        let proxy = Proxy::new(
            &impostor,
            connection.unique_name().unwrap().as_str(),
            OBJECT_PATH,
            INTERFACE,
        )
        .unwrap();
        proxy
            .call::<_, _, ()>("Active", &(r#"{"appId":"impostor"}"#,))
            .unwrap();
        assert_eq!(current().unwrap().unwrap().app_id, "new-game");
        close(slot.take());
        reset_reports();
    }
}
