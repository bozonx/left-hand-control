// Linux-side active window detection.
//
// Dispatches by detected desktop / session type:
//   * KDE Wayland     -> KWin script over D-Bus (`kwin`), `kdotool` fallback
//   * Hyprland        -> hyprctl activewindow -j, events from socket2
//   * Sway            -> swaymsg get_tree, events from `swaymsg subscribe`
//   * X11 (any DE)    -> xprop (+ xdotool for titles), events from `xprop -spy`
//   * everything else -> None (condition will not match)

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use crate::exec::run_cmd_with_timeout;
use crate::platform::linux::{Desktop, SessionType, command_available};

use super::{ActiveWindow, OpenWindow, kwin};

static KDOTOOL_WARN_ONCE: AtomicBool = AtomicBool::new(false);
const COMMAND_TIMEOUT_MS: u64 = 1000;
static LAST_QUERY: AtomicU8 = AtomicU8::new(0);
/// KWin script printing the active window as JSON, for `kdotool` when the
/// D-Bus script cannot run. Borderless windows covering their output
/// count as fullscreen: many games use them instead of real fullscreen.
const KDE_ACTIVE_WINDOW_SCRIPT: &str = "var w=workspace.activeWindow;var r={};\
if(w){r.title=w.caption;r.appId=String(w.resourceClass);r.pid=w.pid;r.fullscreen=w.fullScreen;\
try{var g=w.frameGeometry,o=w.output.geometry;r.fullscreen=r.fullscreen||(w.normalWindow&&w.noBorder&&\
g.x<=o.x&&g.y<=o.y&&g.x+g.width>=o.x+o.width&&g.y+g.height>=o.y+o.height);}catch(e){}}\
output_result(JSON.stringify(r));";

pub(super) fn availability() -> crate::gamemode::DetectorAvailability {
    use crate::gamemode::DetectorAvailability;
    let session = crate::platform::linux::detect();
    let available = match (session.desktop, session.session_type) {
        (_, SessionType::X11) => {
            command_available("xprop") && (!super::titles_needed() || command_available("xdotool"))
        }
        (Desktop::Kde, SessionType::Wayland) => kwin::active() || command_available("kdotool"),
        (Desktop::Hyprland, SessionType::Wayland) => command_available("hyprctl"),
        (Desktop::Sway, SessionType::Wayland) => command_available("swaymsg"),
        _ => return DetectorAvailability::Unsupported,
    };
    if available && LAST_QUERY.load(Ordering::Relaxed) != 2 {
        DetectorAvailability::Available
    } else {
        DetectorAvailability::Unavailable
    }
}

/// Tool that window detection needs in this session but cannot find.
pub(super) fn missing_tool() -> Option<&'static str> {
    let session = crate::platform::linux::detect();
    let tools: &[&'static str] = match (session.desktop, session.session_type) {
        (_, SessionType::X11) if super::titles_needed() => &["xprop", "xdotool"],
        (_, SessionType::X11) => &["xprop"],
        (Desktop::Kde, SessionType::Wayland) if kwin::active() => &[],
        (Desktop::Kde, SessionType::Wayland) => &["kdotool"],
        (Desktop::Hyprland, SessionType::Wayland) => &["hyprctl"],
        (Desktop::Sway, SessionType::Wayland) => &["swaymsg"],
        _ => &[],
    };
    tools.iter().copied().find(|tool| !command_available(tool))
}

pub fn detect(titles: bool) -> Option<ActiveWindow> {
    let session = crate::platform::linux::detect();

    let result = match (session.desktop.clone(), session.session_type) {
        (Desktop::Hyprland, SessionType::Wayland) => detect_hyprland(),
        (Desktop::Kde, SessionType::Wayland) => {
            kwin::sync(titles);
            match kwin::current() {
                Some(window) => Ok(window),
                // Not reported yet, or the script did not load.
                None => detect_kde_wayland(),
            }
        }
        (Desktop::Sway, SessionType::Wayland) => detect_sway(),
        (_, SessionType::X11) => detect_x11(titles),
        _ => Err(()),
    };
    LAST_QUERY.store(if result.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
    result.ok().flatten()
}

/// A running focus-event source.
enum Events {
    Kwin,
    /// A long-running command printing a line per event.
    Command(Child, Arc<AtomicBool>),
    /// Hyprland's event socket, read by a thread until `stop`.
    Socket {
        stop: Arc<AtomicBool>,
        alive: Arc<AtomicBool>,
    },
}

static EVENTS: Mutex<Option<Events>> = Mutex::new(None);

/// Start reporting focus changes through [`super::wake`]. Called on the
/// watcher thread, which outlives the sources it starts.
pub(super) fn start_events() -> bool {
    let session = crate::platform::linux::detect();
    let events = match (session.desktop, session.session_type) {
        (Desktop::Kde, SessionType::Wayland) => {
            kwin::start(super::titles_needed()).then_some(Events::Kwin)
        }
        (Desktop::Hyprland, SessionType::Wayland) => hyprland_events(),
        (Desktop::Sway, SessionType::Wayland) => command_events(
            Command::new("swaymsg").args(["-m", "-r", "-t", "subscribe", "[\"window\"]"]),
            |line| {
                !(line.contains("\"change\":\"title\"") || line.contains("\"change\": \"title\""))
                    || super::titles_needed()
            },
        ),
        (_, SessionType::X11) => command_events(
            Command::new("xprop").args(["-root", "-spy", "_NET_ACTIVE_WINDOW"]),
            |_| true,
        ),
        _ => None,
    };
    let started = events.is_some();
    if let Ok(mut slot) = EVENTS.lock() {
        *slot = events;
    }
    started
}

/// Whether the started source still delivers events.
pub(super) fn events_active() -> bool {
    match EVENTS.lock().ok().as_deref() {
        Some(Some(Events::Kwin)) => kwin::active(),
        Some(Some(Events::Command(_, alive) | Events::Socket { alive, .. })) => {
            alive.load(Ordering::SeqCst)
        }
        _ => false,
    }
}

pub(super) fn stop_events() {
    let events = EVENTS.lock().ok().and_then(|mut slot| slot.take());
    match events {
        Some(Events::Kwin) => kwin::stop(),
        Some(Events::Command(mut child, _)) => {
            let _ = child.kill();
            let _ = child.wait();
        }
        Some(Events::Socket { stop, .. }) => stop.store(true, Ordering::SeqCst),
        None => {}
    }
}

/// Run `command` and wake the watcher for every output line `relevant`
/// accepts. The child dies with the watcher thread.
fn command_events(command: &mut Command, relevant: fn(&str) -> bool) -> Option<Events> {
    // SAFETY: prctl is async-signal-safe and touches no parent state.
    let mut child = unsafe {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            })
            .spawn()
    }
    .inspect_err(|error| log::debug!("[active-window] event source: {error}"))
    .ok()?;
    let stdout = child.stdout.take()?;
    let alive = Arc::new(AtomicBool::new(true));
    let thread_alive = alive.clone();
    let spawned = std::thread::Builder::new()
        .name("active-window-events".into())
        .spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) if relevant(&line) => super::wake(),
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            thread_alive.store(false, Ordering::SeqCst);
        });
    if spawned.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    Some(Events::Command(child, alive))
}

fn hyprland_events() -> Option<Events> {
    use std::os::unix::net::UnixStream;
    let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let path = dirs::runtime_dir()
        .map(|dir| dir.join("hypr").join(&signature).join(".socket2.sock"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| {
            std::path::PathBuf::from(format!("/tmp/hypr/{signature}/.socket2.sock"))
        });
    let stream = UnixStream::connect(&path)
        .inspect_err(|error| log::debug!("[active-window] Hyprland events: {error}"))
        .ok()?;
    // Bounded reads let the thread notice `stop`.
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    let stop = Arc::new(AtomicBool::new(false));
    let alive = Arc::new(AtomicBool::new(true));
    let (thread_stop, thread_alive) = (stop.clone(), alive.clone());
    std::thread::Builder::new()
        .name("active-window-events".into())
        .spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            while !thread_stop.load(Ordering::SeqCst) {
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let event = line.split(">>").next().unwrap_or("");
                        let relevant = match event {
                            "activewindowv2" | "fullscreen" | "closewindow" => true,
                            "windowtitlev2" => super::titles_needed(),
                            _ => false,
                        };
                        if relevant {
                            super::wake();
                        }
                        line.clear();
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(_) => break,
                }
            }
            thread_alive.store(false, Ordering::SeqCst);
        })
        .ok()?;
    Some(Events::Socket { stop, alive })
}

fn detect_hyprland() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("hyprctl").args(["activewindow", "-j"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<serde_json::Value>(&stdout).map_err(|_| ())?;
    Ok(parse_hyprctl_json(&stdout))
}

fn detect_kde_wayland() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("kdotool").args(["kwinscript", "--inline", KDE_ACTIVE_WINDOW_SCRIPT]),
        COMMAND_TIMEOUT_MS,
    );
    let Some(output) = output else {
        if !KDOTOOL_WARN_ONCE.swap(true, Ordering::SeqCst) {
            log::debug!("[active-window] KDE Wayland active-window detection requires 'kdotool'");
        }
        return Err(());
    };
    if !output.status.success() {
        return Err(());
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|_| ())?;
    Ok(window_from_json(&value, "title", "appId"))
}

fn detect_sway() -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("swaymsg").args(["-t", "get_tree", "-r"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    let tree: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|_| ())?;
    Ok(find_focused_window(&tree).map(|window| ActiveWindow {
        fullscreen: Some(find_focused_fullscreen(&tree, false)),
        ..window
    }))
}

/// Whether the focused Sway node, or a container holding it, is
/// fullscreen.
fn find_focused_fullscreen(node: &serde_json::Value, ancestor_fullscreen: bool) -> bool {
    let fullscreen = ancestor_fullscreen
        || node
            .get("fullscreen_mode")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|mode| mode > 0);
    if node.get("focused").and_then(serde_json::Value::as_bool) == Some(true) {
        return fullscreen;
    }
    ["nodes", "floating_nodes"].iter().any(|key| {
        node.get(key)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|nodes| {
                nodes
                    .iter()
                    .any(|node| find_focused_fullscreen(node, fullscreen))
            })
    })
}

fn find_focused_window(node: &serde_json::Value) -> Option<ActiveWindow> {
    if node.get("focused").and_then(serde_json::Value::as_bool) == Some(true) {
        let title = node
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let app_id = node
            .get("app_id")
            .and_then(serde_json::Value::as_str)
            .or_else(|| node.get("window_properties")?.get("class")?.as_str())
            .unwrap_or_default()
            .to_string();
        if !title.is_empty() || !app_id.is_empty() {
            return Some(ActiveWindow {
                title,
                app_id,
                process_name: process_name(node.get("pid").and_then(serde_json::Value::as_u64)),
                fullscreen: None,
            });
        }
    }
    ["nodes", "floating_nodes"].iter().find_map(|key| {
        node.get(key)?
            .as_array()?
            .iter()
            .find_map(find_focused_window)
    })
}

fn process_name(pid: Option<u64>) -> Option<String> {
    crate::gamemode::process::proc_display_name(pid.filter(|pid| *pid > 0)?)
}

/// KWin reports `fullscreen` as a boolean, Hyprland as a mode number
/// whose bit 2 is real fullscreen (1 is maximized).
fn fullscreen_from_json(value: &serde_json::Value) -> Option<bool> {
    match value.get("fullscreen")? {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::Number(value) => value.as_u64().map(|value| value & 2 != 0),
        _ => None,
    }
}

pub(super) fn window_from_json(
    value: &serde_json::Value,
    title_key: &str,
    app_key: &str,
) -> Option<ActiveWindow> {
    let title = value
        .get(title_key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let app_id = value
        .get(app_key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if title.is_empty() && app_id.is_empty() {
        return None;
    }
    Some(ActiveWindow {
        title,
        app_id,
        process_name: process_name(value.get("pid").and_then(serde_json::Value::as_u64)),
        fullscreen: fullscreen_from_json(value),
    })
}

fn detect_x11(titles: bool) -> Result<Option<ActiveWindow>, ()> {
    let output = run_cmd_with_timeout(
        Command::new("xprop").args(["-root", "_NET_ACTIVE_WINDOW"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let window_id = stdout.split_whitespace().last().ok_or(())?;
    if window_id == "0x0" {
        return Ok(None);
    }
    if !window_id.starts_with("0x") {
        return Err(());
    }
    x11_window(window_id, titles)
}

fn x11_window(window_id: &str, titles: bool) -> Result<Option<ActiveWindow>, ()> {
    let title = if titles {
        x11_title(window_id)?
    } else {
        String::new()
    };
    let class_output = run_cmd_with_timeout(
        Command::new("xprop").args(["-id", window_id, "WM_CLASS", "_NET_WM_PID", "_NET_WM_STATE"]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !class_output.status.success() {
        return Err(());
    }
    let stdout = String::from_utf8_lossy(&class_output.stdout);
    let app_id = parse_wm_class(&stdout);
    let pid = stdout
        .lines()
        .find(|line| line.starts_with("_NET_WM_PID"))
        .and_then(|line| line.split_once('='))
        .and_then(|(_, value)| value.trim().parse().ok());
    let process_name = process_name(pid);
    if title.is_empty() && app_id.is_empty() && process_name.is_none() {
        return Ok(None);
    }
    Ok(Some(ActiveWindow {
        title,
        app_id,
        process_name,
        fullscreen: Some(stdout.contains("_NET_WM_STATE_FULLSCREEN")),
    }))
}

fn x11_title(window_id: &str) -> Result<String, ()> {
    let output = run_cmd_with_timeout(
        Command::new("xdotool").args(["getwindowname", window_id]),
        COMMAND_TIMEOUT_MS,
    )
    .ok_or(())?;
    if !output.status.success() {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Applications with open windows in this session.
pub(super) fn open_windows() -> Vec<OpenWindow> {
    let session = crate::platform::linux::detect();
    let windows = match (session.desktop, session.session_type) {
        (Desktop::Kde, SessionType::Wayland) => kwin::open_windows().unwrap_or_default(),
        (Desktop::Hyprland, SessionType::Wayland) => json_command(&["hyprctl", "clients", "-j"])
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|client| window_from_json(client, "title", "class"))
            .collect(),
        (Desktop::Sway, SessionType::Wayland) => {
            let mut windows = Vec::new();
            if let Some(tree) = json_command(&["swaymsg", "-t", "get_tree", "-r"]) {
                collect_sway_windows(&tree, &mut windows);
            }
            windows
        }
        (_, SessionType::X11) => x11_client_ids()
            .iter()
            .filter_map(|id| x11_window(id, command_available("xdotool")).ok().flatten())
            .collect(),
        _ => Vec::new(),
    };
    windows
        .into_iter()
        .map(|window| OpenWindow {
            app_id: window.app_id,
            process_name: window.process_name,
            title: window.title,
        })
        .collect()
}

fn json_command(command: &[&str]) -> Option<serde_json::Value> {
    let output = run_cmd_with_timeout(
        Command::new(command[0]).args(&command[1..]),
        COMMAND_TIMEOUT_MS,
    )?;
    output
        .status
        .success()
        .then(|| serde_json::from_slice(&output.stdout).ok())
        .flatten()
}

/// Sway windows: leaves with a pid.
fn collect_sway_windows(node: &serde_json::Value, out: &mut Vec<ActiveWindow>) {
    if node
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|pid| pid > 0)
    {
        let mut focused = node.clone();
        focused["focused"] = true.into();
        out.extend(find_focused_window(&focused));
    }
    for key in ["nodes", "floating_nodes"] {
        for child in node
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            collect_sway_windows(child, out);
        }
    }
}

fn x11_client_ids() -> Vec<String> {
    let Some(output) = run_cmd_with_timeout(
        Command::new("xprop").args(["-root", "_NET_CLIENT_LIST"]),
        COMMAND_TIMEOUT_MS,
    ) else {
        return Vec::new();
    };
    parse_client_list(&String::from_utf8_lossy(&output.stdout))
}

/// Window ids of `_NET_CLIENT_LIST(WINDOW): window id # 0x1, 0x2`.
fn parse_client_list(stdout: &str) -> Vec<String> {
    stdout
        .split_once('#')
        .map(|(_, ids)| {
            ids.split(',')
                .map(str::trim)
                .filter(|id| id.starts_with("0x"))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

// Parses `xprop WM_CLASS` output of the form:
//   WM_CLASS(STRING) = "instance", "Class"
// Returns the class (second value) when present, otherwise the instance,
// otherwise empty.
pub(crate) fn parse_wm_class(stdout: &str) -> String {
    let line = stdout
        .lines()
        .find(|l| l.contains("WM_CLASS"))
        .unwrap_or("");
    let Some((_, rhs)) = line.split_once('=') else {
        return String::new();
    };
    let mut parts: Vec<String> = rhs
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() >= 2 {
        parts.remove(1)
    } else if !parts.is_empty() {
        parts.remove(0)
    } else {
        String::new()
    }
}

pub(crate) fn parse_hyprctl_json(stdout: &str) -> Option<ActiveWindow> {
    let value: serde_json::Value = serde_json::from_str(stdout).ok()?;
    window_from_json(&value, "title", "class")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sway_finds_native_and_xwayland_focused_windows() {
        let native = serde_json::json!({"nodes": [{"nodes": [{"focused": true, "name": "Game", "app_id": "game"}]}]});
        assert_eq!(find_focused_window(&native).unwrap().app_id, "game");
        let xwayland = serde_json::json!({"floating_nodes": [{"focused": true, "name": "Game", "app_id": null, "window_properties": {"class": "Game.exe"}}]});
        assert_eq!(find_focused_window(&xwayland).unwrap().app_id, "Game.exe");
        assert!(find_focused_window(&serde_json::json!({"focused": true, "name": null})).is_none());
    }

    #[test]
    fn sway_fullscreen_can_belong_to_a_focused_childs_parent() {
        let tree =
            serde_json::json!({"nodes": [{"fullscreen_mode": 1, "nodes": [{"focused": true}]}]});
        assert!(find_focused_fullscreen(&tree, false));
        let tree = serde_json::json!({"nodes": [{"fullscreen_mode": 1, "focused": false}, {"focused": true}]});
        assert!(!find_focused_fullscreen(&tree, false));
        let tree = serde_json::json!({"floating_nodes": [{"fullscreen_mode": 1, "focused": true}]});
        assert!(find_focused_fullscreen(&tree, false));
    }

    #[test]
    fn reads_kwin_and_hyprland_fullscreen() {
        let kde = serde_json::json!({"title": "Game", "appId": "steam_app_1", "fullscreen": true});
        assert_eq!(
            window_from_json(&kde, "title", "appId").unwrap().fullscreen,
            Some(true)
        );
        for (mode, fullscreen) in [(0, false), (1, false), (2, true), (3, true)] {
            let hyprland = format!(r#"{{"title":"Game","class":"game","fullscreen":{mode}}}"#);
            assert_eq!(
                parse_hyprctl_json(&hyprland).unwrap().fullscreen,
                Some(fullscreen)
            );
        }
        let unknown = serde_json::json!({"title": "Game", "appId": "game"});
        assert_eq!(
            window_from_json(&unknown, "title", "appId")
                .unwrap()
                .fullscreen,
            None
        );
    }

    #[test]
    fn parses_x11_client_list() {
        assert_eq!(
            parse_client_list("_NET_CLIENT_LIST(WINDOW): window id # 0x1a00003, 0x2c00004\n"),
            ["0x1a00003", "0x2c00004"]
        );
        assert!(parse_client_list("_NET_CLIENT_LIST:  not found.\n").is_empty());
    }

    #[test]
    fn sway_lists_every_window() {
        let tree = serde_json::json!({"nodes": [{"pid": 0, "nodes": [
            {"pid": 10, "name": "A", "app_id": "a"},
            {"pid": 11, "name": "B", "app_id": null, "window_properties": {"class": "B"}},
        ]}]});
        let mut windows = Vec::new();
        collect_sway_windows(&tree, &mut windows);
        let ids: Vec<_> = windows
            .iter()
            .map(|window| window.app_id.as_str())
            .collect();
        assert_eq!(ids, ["a", "B"]);
    }

    #[test]
    fn parses_wm_class_class_value() {
        let s = "WM_CLASS(STRING) = \"firefox\", \"firefox\"\n";
        assert_eq!(parse_wm_class(s), "firefox");
    }

    #[test]
    fn parses_wm_class_with_distinct_values() {
        let s = "WM_CLASS(STRING) = \"navigator\", \"Firefox\"\n";
        assert_eq!(parse_wm_class(s), "Firefox");
    }

    #[test]
    fn parses_wm_class_single_value() {
        let s = "WM_CLASS(STRING) = \"only\"\n";
        assert_eq!(parse_wm_class(s), "only");
    }

    #[test]
    fn parses_hyprctl_json_basic() {
        let s =
            r#"{"address":"0x1","title":"My Doc - Editor","class":"editor","fullscreen":false}"#;
        let aw = parse_hyprctl_json(s).unwrap();
        assert_eq!(aw.title, "My Doc - Editor");
        assert_eq!(aw.app_id, "editor");
    }

    #[test]
    fn parses_hyprctl_json_missing_fields_returns_none() {
        assert!(parse_hyprctl_json("{}").is_none());
    }

    #[test]
    fn parses_hyprctl_json_escaped_unicode() {
        let s = r#"{"title":"\u0422\u0435\u0441\u0442","class":"kitty"}"#;
        let aw = parse_hyprctl_json(s).unwrap();
        assert_eq!(aw.title, "Тест");
        assert_eq!(aw.app_id, "kitty");
    }
}
