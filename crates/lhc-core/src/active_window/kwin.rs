//! KDE Plasma: a KWin script loaded once reports every focus change over
//! D-Bus, so detection needs neither polling nor `kdotool`.
//!
//! The script calls back into a small D-Bus object this process serves
//! on its unique bus name; calls from anyone but KWin are ignored.

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use zbus::blocking::{Connection, Proxy, fdo::DBusProxy};
use zbus::message::Header;
use zbus::names::BusName;

use super::ActiveWindow;

const KWIN: &str = "org.kde.KWin";
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
static PUSHED: Mutex<Option<Option<ActiveWindow>>> = Mutex::new(None);
/// Receiver of the next window list.
static LIST_REPLY: Mutex<Option<mpsc::Sender<String>>> = Mutex::new(None);
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

struct Session {
    connection: Connection,
    /// Whether the loaded script reports titles.
    titles: bool,
    file: PathBuf,
}

struct Service {
    /// Unique bus name of KWin, the only accepted caller; known once the
    /// connection is up.
    kwin: Arc<OnceLock<String>>,
}

#[zbus::interface(name = "dev.bozonx.LeftHandControl.KWin")]
impl Service {
    fn active(&self, json: String, #[zbus(header)] header: Header<'_>) {
        if !self.sent_by_kwin(&header) {
            return;
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
    let connection = connect()?;
    let file = script_path("active");
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
    })
}

/// A session-bus connection serving the callback object.
fn connect() -> Result<Connection, String> {
    let kwin = Arc::new(OnceLock::new());
    // Serving through the builder keeps zbus inside its own runtime when
    // another crate enabled its `tokio` feature.
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.serve_at(OBJECT_PATH, Service { kwin: kwin.clone() }))
        .and_then(|builder| builder.build())
        .map_err(|error| error.to_string())?;
    let name = BusName::try_from(KWIN).map_err(|error| error.to_string())?;
    let owner = DBusProxy::new(&connection)
        .map_err(|error| error.to_string())?
        .get_name_owner(name)
        .map_err(|error| format!("KWin is not on the session bus: {error}"))?;
    let _ = kwin.set(owner.to_string());
    Ok(connection)
}

/// Whether the script runs and reports.
pub(super) fn active() -> bool {
    SESSION.lock().is_ok_and(|session| session.is_some())
}

/// Reload the script when whether titles are needed changed.
pub(super) fn sync(titles: bool) {
    let Ok(mut slot) = SESSION.lock() else {
        return;
    };
    let Some(session) = slot.as_mut() else {
        return;
    };
    if session.titles == titles {
        return;
    }
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
    if let Ok(mut pushed) = PUSHED.lock() {
        *pushed = None;
    }
}

fn close(session: Option<Session>) {
    if let Some(session) = session {
        unload_script(&session.connection, ACTIVE_SCRIPT);
        let _ = std::fs::remove_file(&session.file);
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
        None => connect().ok()?,
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
        .call("loadScript", &(path.as_ref(), name))
        .map_err(|error| error.to_string())?;
    if id < 0 {
        return Err(format!("KWin refused script {name}"));
    }
    // Plasma 6 path first, then Plasma 5.
    let run = |path: String| {
        Proxy::new(connection, KWIN, path, "org.kde.kwin.Script")
            .and_then(|script| script.call::<_, _, ()>("run", &()))
    };
    run(format!("/Scripting/Script{id}"))
        .or_else(|_| run(format!("/{id}")))
        .map_err(|error| {
            unload_script(connection, name);
            error.to_string()
        })
}

fn unload_script(connection: &Connection, name: &str) {
    if let Ok(scripting) = scripting(connection) {
        let _: Result<bool, _> = scripting.call("unloadScript", &name);
    }
}

fn scripting(connection: &Connection) -> Result<Proxy<'static>, String> {
    Proxy::new(connection, KWIN, "/Scripting", "org.kde.kwin.Scripting")
        .map_err(|error| error.to_string())
}

fn json_string(text: &str) -> String {
    serde_json::Value::String(text.into()).to_string()
}
