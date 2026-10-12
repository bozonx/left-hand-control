use lhc_core::storage::StoragePaths;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use slint_shell::{Document, bind_document, ui::{MacroEditor, Page, SettingsWindow, Theme}};
use std::{cell::Cell, rc::Rc, time::Duration};

fn snap(ui: &SettingsWindow, path: &str) {
    let pixels = ui.window().take_snapshot().unwrap();
    let mut data = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
    for p in pixels.as_slice() { data.extend([p.r, p.g, p.b]); }
    std::fs::write(path, data).unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = match std::env::var("DEV") { Ok(d) => StoragePaths::new(format!("{d}/.config/dev.bozonx.left-hand-control").into(), format!("{d}/.local/share/dev.bozonx.left-hand-control").into()), Err(_) => StoragePaths::new(dir.path().join("config"), dir.path().join("data")) };
    let document = Document::load(paths)?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    ui.set_page(Page::Macros);
    ui.global::<Theme>().invoke_apply();
    if std::env::var("SETUP").is_ok() {
        let editor = ui.global::<MacroEditor>();
        for _ in 0..4 { editor.invoke_add("X".into()); }
        editor.invoke_add("M".into());
        editor.invoke_add_step(0, "pause:100".into());
    }
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    let out = std::env::var("PROBE_OUT").unwrap();
    // Script: "c x y" click, "w x y dy [phase]" wheel, "m x y" move, "s name" snapshot, "k t" key, "d" dump.
    let script: Vec<Vec<String>> = std::env::var("SCRIPT").unwrap().split(';').map(|c| c.split_whitespace().map(String::from).collect()).collect();
    let weak = ui.as_weak();
    let doc = document.clone();
    let step = Rc::new(Cell::new(0usize));
    let timer = slint::Timer::default();
    let tick = std::env::var("TICK").ok().and_then(|v| v.parse().ok()).unwrap_or(150);
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(tick), move || {
        let ui = weak.upgrade().unwrap();
        let w = ui.window();
        let n = step.get();
        step.set(n + 1);
        if let Ok(Some(_)) = doc.reload() { println!("{n}: RELOADED"); }
        if n < 3 { return; }
        let Some(cmd) = script.get(n - 3) else { slint::quit_event_loop().unwrap(); return; };
        let f = |i: usize| cmd[i].parse::<f32>().unwrap();
        match cmd[0].as_str() {
            "c" => { let at = LogicalPosition::new(f(1), f(2));
                w.dispatch_event(WindowEvent::PointerMoved { position: at });
                w.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
                w.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left }); }
            "m" => w.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(f(1), f(2)) }),
            "w" => { use i_slint_core::input::TouchPhase::*; let phase = match cmd.get(4).map(String::as_str) { Some("s") => Started, Some("e") => Ended, Some("c") => Cancelled, _ => Moved };
                i_slint_core::window::WindowInner::from_pub(w).process_mouse_input(i_slint_core::input::MouseEvent::Wheel { position: i_slint_core::lengths::LogicalPoint::new(f(1), f(2)), delta_x: 0.0, delta_y: f(3), phase }); }
            "s" => snap(&ui, &format!("{out}/{}.ppm", cmd[1])),
            "k" => { w.dispatch_event(WindowEvent::KeyPressed { text: cmd[1].clone().into() }); w.dispatch_event(WindowEvent::KeyReleased { text: cmd[1].clone().into() }); }
            "d" => println!("{n}: {:?}", doc.read().layout().macros.iter().map(|m| m.steps.iter().map(|s| s.action.clone()).collect::<Vec<_>>()).collect::<Vec<_>>()),
            _ => {}
        }
    });
    slint::run_event_loop()?;
    Ok(())
}
