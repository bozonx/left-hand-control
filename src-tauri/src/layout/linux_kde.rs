// KDE Plasma keyboard layout backend (Wayland + X11).
//
// Talks to the DBus service `org.kde.keyboard` at `/Layouts` with interface
// `org.kde.KeyboardLayouts`:
//   * getLayout() -> u             current index
//   * getLayoutsList() -> a(sss)   list of (short, display, long)
//   * signal layoutChanged(u)
//   * signal layoutListChanged()
//
// We shell out to `gdbus` to avoid pulling in a DBus crate. `gdbus` ships
// with glib2 which is a hard dependency of KDE itself, so it is always
// present on a Plasma desktop.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use super::LayoutInfo;

const SERVICE: &str = "org.kde.keyboard";
const OBJECT: &str = "/Layouts";
const IFACE: &str = "org.kde.KeyboardLayouts";

pub fn current() -> Result<Option<LayoutInfo>, String> {
    let list = match call_list()? {
        Some(v) => v,
        None => return Ok(None),
    };
    let idx = call_index()?.unwrap_or(0);
    let entry = list
        .get(idx as usize)
        .or_else(|| list.first())
        .cloned()
        .ok_or_else(|| "no layouts configured".to_string())?;
    Ok(Some(LayoutInfo {
        short: entry.0,
        display: entry.1,
        long: entry.2,
        index: idx,
        backend: "linux-kde",
    }))
}

pub fn start_watcher(app: AppHandle) {
    thread::spawn(move || loop {
        // Emit initial state if available.
        if let Ok(Some(info)) = current() {
            let _ = app.emit("layout-changed", info);
        }

        // Block on `gdbus monitor` until it exits (service restart / logout).
        match run_monitor(&app) {
            Ok(()) => {
                // Monitor exited cleanly — retry after a short pause.
            }
            Err(e) => {
                eprintln!("[layout] monitor error: {e}");
            }
        }
        thread::sleep(Duration::from_secs(2));
    });
}

fn run_monitor(app: &AppHandle) -> Result<(), String> {
    let mut child = Command::new("gdbus")
        .args([
            "monitor",
            "--session",
            "--dest",
            SERVICE,
            "--object-path",
            OBJECT,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn gdbus monitor: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "no stdout from gdbus monitor".to_string())?;
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        // Typical line:
        //   /Layouts: org.kde.KeyboardLayouts.layoutChanged (uint32 1,)
        //   /Layouts: org.kde.KeyboardLayouts.layoutListChanged ()
        if line.contains("layoutChanged") || line.contains("layoutListChanged") {
            if let Ok(Some(info)) = current() {
                let _ = app.emit("layout-changed", info);
            }
        }
    }

    let _ = child.wait();
    Ok(())
}

// --- DBus helpers (via gdbus call) -----------------------------------------

fn gdbus_call(method: &str) -> Result<Option<String>, String> {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            SERVICE,
            "--object-path",
            OBJECT,
            "--method",
            &format!("{IFACE}.{method}"),
        ])
        .output()
        .map_err(|e| format!("spawn gdbus call {method}: {e}"))?;
    if !out.status.success() {
        // Service not present (e.g. non-KDE session) — not fatal.
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("ServiceUnknown") || stderr.contains("was not provided") {
            return Ok(None);
        }
        return Err(format!("gdbus {method} failed: {}", stderr.trim()));
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

fn call_index() -> Result<Option<u32>, String> {
    let raw = match gdbus_call("getLayout")? {
        Some(s) => s,
        None => return Ok(None),
    };
    // Format: "(uint32 0,)"
    let start = raw.find("uint32").ok_or("unexpected getLayout output")?;
    let tail = &raw[start + "uint32".len()..];
    let num: String = tail
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num.parse::<u32>()
        .map(Some)
        .map_err(|e| format!("parse index: {e}"))
}

fn call_list() -> Result<Option<Vec<(String, String, String)>>, String> {
    let raw = match gdbus_call("getLayoutsList")? {
        Some(s) => s,
        None => return Ok(None),
    };
    // Format: "([('ru', '', 'Russian'), ('us', '', 'English (US)'), ...],)"
    Ok(Some(parse_triples(&raw)))
}

fn parse_triples(s: &str) -> Vec<(String, String, String)> {
    // Minimal tolerant parser: find every `(...)` tuple inside the outer list
    // and split its three single-quoted strings.
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            // Skip the outer wrapper tuple `(`.
            // We consider a tuple as a candidate only if it starts with `('`.
            if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                if let Some((triple, end)) = take_triple(&s[i..]) {
                    out.push(triple);
                    i += end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn take_triple(s: &str) -> Option<((String, String, String), usize)> {
    // Expects s to start with `('a', 'b', 'c')` where strings can be empty.
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if first != '(' {
        return None;
    }
    let mut fields: Vec<String> = Vec::with_capacity(3);
    let mut end = 0;
    for _ in 0..3 {
        let (p, mut c) = chars.next()?;
        // Skip whitespace and commas between fields.
        while c == ' ' || c == ',' {
            let (_, nc) = chars.next()?;
            c = nc;
            let _ = p;
        }
        if c != '\'' {
            return None;
        }
        // Read until unescaped closing quote.
        let mut buf = String::new();
        loop {
            let (p2, ch) = chars.next()?;
            end = p2;
            if ch == '\\' {
                let (_, esc) = chars.next()?;
                buf.push(esc);
                continue;
            }
            if ch == '\'' {
                break;
            }
            buf.push(ch);
        }
        fields.push(buf);
    }
    // Consume up to and including the closing `)`.
    for (p, c) in chars.by_ref() {
        end = p;
        if c == ')' {
            break;
        }
    }
    Some((
        (
            fields.remove(0),
            fields.remove(0),
            fields.remove(0),
        ),
        end + 1,
    ))
}
