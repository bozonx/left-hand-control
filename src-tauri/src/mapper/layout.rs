// Detect the current keyboard layout on KDE Plasma (X11/Wayland).
//
// Strategy:
//   * Parse ~/.config/kxkbrc to learn the ordered list of layouts/variants,
//     the keyboard model and xkb options the user configured.
//   * Ask `qdbus org.kde.keyboard /Layouts getLayout` for the currently
//     active layout short name (e.g. "us", "ru"). Match it against the
//     layout list to pick the corresponding variant.
//   * If any of the above fails, fall back to a plain "us" layout so the
//     mapper still boots.
//
// We also provide a poll-based watcher that emits a `LayoutInfo` on a
// channel whenever the active layout changes. Polling (every ~1s) is
// simple, dependency-free and good enough for a user-facing action.

#![cfg(target_os = "linux")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutInfo {
    pub layout: String,
    pub variant: String,
    pub model: String,
    pub options: Option<String>,
}

impl Default for LayoutInfo {
    fn default() -> Self {
        Self {
            layout: "us".into(),
            variant: String::new(),
            model: "pc105".into(),
            options: None,
        }
    }
}

/// Detect the currently active layout. Never fails — returns a sensible
/// default ("us", pc105) if detection does not work on this machine.
pub fn current() -> LayoutInfo {
    let cfg = read_kxkbrc();
    let active = active_short_name().or_else(|| cfg.layouts.first().cloned());

    let (layout, variant) = match active {
        Some(short) => {
            let variant = cfg
                .layouts
                .iter()
                .position(|l| l == &short)
                .and_then(|idx| cfg.variants.get(idx).cloned())
                .unwrap_or_default();
            (short, variant)
        }
        None => ("us".to_string(), String::new()),
    };

    let model = if cfg.model.is_empty() {
        "pc105".to_string()
    } else {
        cfg.model
    };
    let options = if cfg.options.is_empty() {
        None
    } else {
        Some(cfg.options)
    };

    LayoutInfo {
        layout,
        variant,
        model,
        options,
    }
}

/// Spawn a thread that polls the active layout and sends a new `LayoutInfo`
/// on `tx` whenever it changes compared to `initial`. The thread exits when
/// `stop` becomes true or when the receiver is dropped.
pub fn spawn_watcher(initial: LayoutInfo, tx: Sender<LayoutInfo>, stop: Arc<AtomicBool>) {
    thread::Builder::new()
        .name("lhc-layout-watch".into())
        .spawn(move || {
            let mut last = initial;
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1000));
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let cur = current();
                if cur != last {
                    eprintln!(
                        "[mapper] layout changed: {}({}) -> {}({})",
                        last.layout, last.variant, cur.layout, cur.variant
                    );
                    if tx.send(cur.clone()).is_err() {
                        // receiver dropped: mapper shutting down
                        break;
                    }
                    last = cur;
                }
            }
        })
        .expect("spawn layout watcher");
}

// ---------------- internals ----------------

#[derive(Default)]
struct KxkbConfig {
    layouts: Vec<String>,
    variants: Vec<String>,
    model: String,
    options: String,
}

fn read_kxkbrc() -> KxkbConfig {
    let Some(home) = std::env::var_os("HOME") else {
        return KxkbConfig::default();
    };
    let path = PathBuf::from(home).join(".config").join("kxkbrc");
    let Ok(text) = fs::read_to_string(&path) else {
        return KxkbConfig::default();
    };

    let mut in_layout_section = false;
    let mut cfg = KxkbConfig::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_layout_section = line.eq_ignore_ascii_case("[Layout]");
            continue;
        }
        if !in_layout_section || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim();
        match key {
            "LayoutList" => {
                cfg.layouts = split_csv(val);
            }
            "VariantList" => {
                cfg.variants = split_csv(val);
            }
            "Model" => cfg.model = val.to_string(),
            "Options" => cfg.options = val.to_string(),
            _ => {}
        }
    }
    cfg
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',').map(|p| p.trim().to_string()).collect()
}

/// Ask KDE's keyboard kded module for the active layout short name.
/// Returns None if the call fails (e.g. running outside KDE Plasma).
fn active_short_name() -> Option<String> {
    if !is_kde() {
        return None;
    }
    let out = Command::new("qdbus")
        .args(["org.kde.keyboard", "/Layouts", "getLayout"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn is_kde() -> bool {
    if let Ok(v) = std::env::var("XDG_CURRENT_DESKTOP") {
        if v.to_ascii_uppercase().split(':').any(|s| s == "KDE") {
            return true;
        }
    }
    std::env::var("KDE_FULL_SESSION").is_ok()
}
