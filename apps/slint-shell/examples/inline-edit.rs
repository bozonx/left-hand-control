use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, SharedString};
use std::{cell::Cell, rc::Rc, time::Duration};

slint::slint! {
    import { InlineTextField, InlineTextArea, ConfirmDialog, InfoTip } from "../ui/controls.slint";
    export component ControlsWindow inherits Window {
        width: 520px; height: 320px;
        in-out property <string> value: "Original";
        in-out property <string> description: "Description";
        in-out property <bool> confirming;
        in-out property <bool> confirmed;
        public function edit() { field.begin(); }
        public function edit-description() { description-field.begin(); }
        VerticalLayout { padding: 20px; spacing: 12px; alignment: start;
            field := InlineTextField { text: root.value; saved(value) => { root.value = value; } }
            description-field := InlineTextArea { text: root.description; saved(value) => { root.description = value; } }
            InfoTip { text: "First line\nSecond line\nA longer explanation that should wrap onto several lines without extending beyond the tooltip."; }
        }
        if root.confirming: ConfirmDialog {
            title: "Delete?"; body: "Confirm deletion."; confirm-text: "Delete";
            confirm => { root.confirmed = true; root.confirming = false; }
            cancel => { root.confirming = false; }
        }
    }
}

fn key(ui: &ControlsWindow, text: impl Into<SharedString>) {
    let text = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn shortcut(ui: &ControlsWindow, text: &str) {
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(ui, text);
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
}

fn redo(ui: &ControlsWindow) {
    if cfg!(target_os = "windows") {
        shortcut(ui, "y");
    } else {
        ui.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Control.into() });
        ui.window().dispatch_event(WindowEvent::KeyPressed { text: Key::Shift.into() });
        key(ui, "Z");
        ui.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Shift.into() });
        ui.window().dispatch_event(WindowEvent::KeyReleased { text: Key::Control.into() });
    }
}

fn snapshot(ui: &ControlsWindow, name: &str) {
    let Some(dir) = std::env::var_os("LHC_EDITOR_SNAPSHOTS") else {
        return;
    };
    let pixels = ui.window().take_snapshot().unwrap();
    let mut bytes = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
    for pixel in pixels.as_slice() {
        bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
    }
    std::fs::write(
        std::path::PathBuf::from(dir).join(format!("{name}.ppm")),
        bytes,
    )
    .unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = ControlsWindow::new()?;
    ui.show()?;
    let timer = slint::Timer::default();
    let step = Rc::new(Cell::new(0));
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        match step.get() {
            0 => { snapshot(&ui, "inline-view"); ui.invoke_edit(); },
            1 => { snapshot(&ui, "inline-edit"); key(&ui, Key::End); key(&ui, "Draft"); shortcut(&ui, "z"); key(&ui, Key::Return); assert_eq!(ui.get_value(), "Original"); }
            2 => ui.invoke_edit(),
            3 => { key(&ui, "Saved"); shortcut(&ui, "z"); redo(&ui); key(&ui, Key::Return); assert_eq!(ui.get_value(), "Saved"); }
            4 => ui.invoke_edit(),
            5 => { key(&ui, "Discarded"); key(&ui, Key::Escape); assert_eq!(ui.get_value(), "Saved"); }
            6 => ui.invoke_edit_description(),
            7 => { shortcut(&ui, "a"); key(&ui, "Line one"); key(&ui, Key::Return); key(&ui, "Line two"); shortcut(&ui, "\n"); assert_eq!(ui.get_description(), "Line one\nLine two"); }
            8 => ui.set_confirming(true),
            9 => { snapshot(&ui, "confirm"); key(&ui, Key::Return); assert!(ui.get_confirmed()); println!("Inline editing passed: native undo/redo, save, cancel, multiline, confirmation focus"); }
            10 => ui.window().dispatch_event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(29.0, 120.0),
            }),
            11..=17 => {},
            18 => { snapshot(&ui, "info-tip"); slint::quit_event_loop().unwrap(); },
            _ => unreachable!(),
        }
        step.set(step.get() + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
