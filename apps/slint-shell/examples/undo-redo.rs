use slint::platform::{Key, WindowEvent};
use slint::winit_030::{
    WinitWindowAccessor,
    winit::keyboard::{KeyCode, ModifiersState},
};
use slint::{ComponentHandle, SharedString};
use std::{cell::Cell, rc::Rc, time::Duration};

slint::slint! {
    import { LineEdit, TextEdit, VerticalBox } from "std-widgets.slint";
    import { InlineTextField, InlineTextArea } from "../ui/controls.slint";
    export component EditingWindow inherits Window {
        width: 500px; height: 480px;
        in-out property <int> active;
        in-out property <bool> read-only;
        in-out property <string> raw-text;
        in-out property <string> line-text;
        in-out property <string> area-text;
        in-out property <string> inline-text: "исходный";
        in-out property <string> inline-area: "строка";
        in-out property <bool> clearable;
        public function begin-inline() { inline.begin(); }
        public function begin-inline-area() { inline-area.begin(); }
        out property <int> cursor: raw.cursor-position-byte-offset;
        out property <int> anchor: raw.anchor-position-byte-offset;
        public function focus-editor() {
            if root.active == 0 { raw.focus(); }
            if root.active == 1 { line.focus(); }
            if root.active == 2 { area.focus(); }
        }
        VerticalBox {
            raw := TextInput { text <=> root.raw-text; single-line: true; read-only: root.read-only; height: 36px; }
            line := LineEdit { text <=> root.line-text; read-only: root.read-only; }
            area := TextEdit { text <=> root.area-text; read-only: root.read-only; }
            inline := InlineTextField { text: root.inline-text; clearable: root.clearable; saved(value) => { root.inline-text = value; } }
            inline-area := InlineTextArea { text: root.inline-area; saved(value) => { root.inline-area = value; } }
        }
    }
}

fn key(ui: &EditingWindow, text: impl Into<SharedString>) {
    let text = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn shortcut(ui: &EditingWindow, logical: &str, physical: KeyCode, shift: bool) {
    let primary = if cfg!(target_os = "macos") {
        ModifiersState::SUPER
    } else {
        ModifiersState::CONTROL
    };
    let modifiers = if shift {
        primary | ModifiersState::SHIFT
    } else {
        primary
    };
    let text = slint_shell::text_editing::shortcut_text(logical, physical, modifiers).unwrap();
    key_pressed(ui, Key::Control);
    if shift {
        key_pressed(ui, Key::Shift);
    }
    key(ui, text);
    if shift {
        key_released(ui, Key::Shift);
    }
    key_released(ui, Key::Control);
}

fn key_pressed(ui: &EditingWindow, key: Key) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
}

fn key_released(ui: &EditingWindow, key: Key) {
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
}

fn text(ui: &EditingWindow) -> SharedString {
    match ui.get_active() {
        0 => ui.get_raw_text(),
        1 => ui.get_line_text(),
        _ => ui.get_area_text(),
    }
}

fn reset(ui: &EditingWindow, value: &str) {
    ui.set_read_only(false);
    match ui.get_active() {
        0 => ui.set_raw_text(value.into()),
        1 => ui.set_line_text(value.into()),
        _ => ui.set_area_text(value.into()),
    }
    ui.invoke_focus_editor();
}

fn clipboard(value: Option<&str>) -> Option<String> {
    i_slint_core::context::with_global_context(
        || Err(slint::PlatformError::NoPlatform),
        |context| {
            let platform = context.platform();
            if let Some(value) = value {
                platform.set_clipboard_text(value, slint::platform::Clipboard::DefaultClipboard);
            }
            platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard)
        },
    )
    .unwrap()
}

fn verify(ui: &EditingWindow) {
    shortcut(ui, "ф", KeyCode::KeyA, false);
    key(ui, "новый🙂");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "исходный");
    if ui.get_active() == 0 {
        assert_eq!((ui.get_cursor(), ui.get_anchor()), (16, 0));
    }
    shortcut(ui, "Я", KeyCode::KeyZ, true);
    assert_eq!(text(ui), "новый🙂");
    if ui.get_active() == 0 {
        assert_eq!((ui.get_cursor(), ui.get_anchor()), (14, 14));
    }
    shortcut(ui, "Я", KeyCode::KeyZ, false);
    key(ui, "ветка");
    shortcut(ui, "н", KeyCode::KeyY, false);
    assert_eq!(text(ui), "ветка");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "исходный");
    shortcut(ui, "н", KeyCode::KeyY, false);
    assert_eq!(text(ui), "ветка");
    key(ui, Key::End);
    key(ui, "!");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "ветка");
    shortcut(ui, "Я", KeyCode::KeyZ, true);
    assert_eq!(text(ui), "ветка!");
    ui.set_read_only(true);
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "ветка!");
    ui.set_read_only(false);
    shortcut(ui, "я", KeyCode::KeyZ, false);
    ui.set_read_only(true);
    shortcut(ui, "н", KeyCode::KeyY, false);
    assert_eq!(text(ui), "ветка");
    reset(ui, "abc");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "abc");
    key(ui, Key::End);
    key(ui, Key::Backspace);
    key(ui, Key::Backspace);
    assert_eq!(text(ui), "a");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "abc");
    if ui.get_active() == 0 {
        assert_eq!((ui.get_cursor(), ui.get_anchor()), (3, 3));
    }
    key(ui, Key::Home);
    key(ui, Key::Delete);
    key(ui, Key::Delete);
    assert_eq!(text(ui), "c");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "abc");
    let old_clipboard = clipboard(None);
    clipboard(Some("вставка🙂"));
    shortcut(ui, "ф", KeyCode::KeyA, false);
    shortcut(ui, "м", KeyCode::KeyV, false);
    assert_eq!(text(ui), "вставка🙂");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "abc");
    shortcut(ui, "н", KeyCode::KeyY, false);
    assert_eq!(text(ui), "вставка🙂");
    shortcut(ui, "ф", KeyCode::KeyA, false);
    shortcut(ui, "ч", KeyCode::KeyX, false);
    assert_eq!(text(ui), "");
    shortcut(ui, "я", KeyCode::KeyZ, false);
    assert_eq!(text(ui), "вставка🙂");
    if ui.get_active() == 0 {
        assert_eq!((ui.get_cursor(), ui.get_anchor()), (18, 0));
    }
    shortcut(ui, "с", KeyCode::KeyC, false);
    let copied = clipboard(None);
    clipboard(Some(old_clipboard.as_deref().unwrap_or_default()));
    assert_eq!(
        copied.as_deref(),
        Some("вставка🙂"),
        "copy in editor {}",
        ui.get_active()
    );
    if ui.get_active() == 2 {
        reset(ui, "строка");
        key(ui, Key::End);
        key(ui, Key::Return);
        key(ui, "вторая🙂");
        assert_eq!(text(ui), "строка\nвторая🙂");
        shortcut(ui, "я", KeyCode::KeyZ, false);
        assert_eq!(text(ui), "строка\n");
        shortcut(ui, "я", KeyCode::KeyZ, false);
        assert_eq!(text(ui), "строка");
        shortcut(ui, "н", KeyCode::KeyY, false);
        shortcut(ui, "Я", KeyCode::KeyZ, true);
        assert_eq!(text(ui), "строка\nвторая🙂");
    }
}

fn snapshot(ui: &EditingWindow) {
    let Some(dir) = std::env::var_os("LHC_EDITOR_SNAPSHOTS") else {
        return;
    };
    let pixels = ui.window().take_snapshot().unwrap();
    let mut bytes = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
    for pixel in pixels.as_slice() {
        bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
    }
    std::fs::write(
        std::path::PathBuf::from(dir).join("undo-redo-scroll.ppm"),
        bytes,
    )
    .unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = EditingWindow::new()?;
    let modifiers = Cell::new(ModifiersState::empty());
    ui.window().on_winit_window_event(move |window, event| {
        if let slint::winit_030::winit::event::WindowEvent::ModifiersChanged(next) = event {
            modifiers.set(next.state());
        }
        if slint_shell::text_editing::dispatch_shortcut(window, event, modifiers.get()) {
            slint::winit_030::EventResult::PreventDefault
        } else {
            slint::winit_030::EventResult::Propagate
        }
    });
    ui.show()?;
    let timer = slint::Timer::default();
    let step = Rc::new(Cell::new(0));
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        let current = step.get();
        match current {
            0 | 2 | 4 => { ui.set_active(current / 2); reset(&ui, "исходный"); }
            1 | 3 | 5 => verify(&ui),
            6 | 8 | 10 | 12 => {
                ui.set_clearable(current >= 10);
                ui.invoke_begin_inline();
            }
            7 | 11 => {
                let original = ui.get_inline_text();
                shortcut(&ui, "ф", KeyCode::KeyA, false);
                key(&ui, "новый🙂");
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                key(&ui, Key::Return);
                assert_eq!(ui.get_inline_text(), original);
            }
            9 | 13 => {
                shortcut(&ui, "ф", KeyCode::KeyA, false);
                key(&ui, "новый🙂");
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                shortcut(&ui, "Я", KeyCode::KeyZ, true);
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                key(&ui, "ветка");
                shortcut(&ui, "н", KeyCode::KeyY, false);
                key(&ui, Key::Return);
                assert_eq!(ui.get_inline_text(), "ветка");
            }
            14 => ui.invoke_begin_inline_area(),
            15 => {
                shortcut(&ui, "ф", KeyCode::KeyA, false);
                key(&ui, "первая");
                key(&ui, Key::Return);
                key(&ui, "вторая🙂");
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                shortcut(&ui, "Я", KeyCode::KeyZ, true);
                shortcut(&ui, "н", KeyCode::KeyY, false);
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                key(&ui, "замена🙂");
                shortcut(&ui, "н", KeyCode::KeyY, false);
                key_pressed(&ui, Key::Control);
                key(&ui, Key::Return);
                key_released(&ui, Key::Control);
                assert_eq!(ui.get_inline_area(), "первая\nзамена🙂");
            }
            16 => {
                ui.set_clearable(false);
                ui.set_inline_text(format!("{}END", "длинный текст ".repeat(24)).into());
                ui.invoke_begin_inline();
            }
            17 => key(&ui, Key::End),
            18 => {
                snapshot(&ui);
                key(&ui, Key::Backspace);
                shortcut(&ui, "я", KeyCode::KeyZ, false);
                shortcut(&ui, "н", KeyCode::KeyY, false);
                key(&ui, Key::Return);
                assert!(ui.get_inline_text().ends_with("EN"));
            }
            19 => {
                println!("Undo/redo passed: TextInput, LineEdit, TextEdit, both inline fields, inline textarea, Russian shortcuts, atomic replacement, redo invalidation, selection, deletion, clipboard, read-only, multiline");
                slint::quit_event_loop().unwrap();
            }
            _ => unreachable!(),
        }
        step.set(current + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
