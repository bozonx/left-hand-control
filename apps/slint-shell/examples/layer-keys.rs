//! Dragging a key of the layer keyboard onto another key swaps their assignments;
//! keys that keep their modifiers held are marked.
mod support;
use lhc_core::{config_document::KeyAssignment, storage::StoragePaths};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use slint_shell::{
    Document, bind_document,
    document::View,
    ui::{DragDrop, LayerDialog, LayersEditor, MenuKind, Page, SettingsWindow},
};
use std::{cell::Cell, rc::Rc, time::Duration};
use support::snapshot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Document::load(paths)?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    let layer = document
        .edit(View::Library, |config| {
            let id = config.create_layer("Drag", "")?;
            config.set_layer_key(&id, "KeyQ", KeyAssignment::Action("ArrowUp".into()))?;
            config.set_layer_key(&id, "KeyJ", KeyAssignment::Swallow)?;
            config.set_layer_key(&id, "KeyW", KeyAssignment::Action("Alt+Tab".into()))?;
            config.set_layer_key_holds_modifiers(&id, "KeyW", true)?;
            config.set_extra_key(&id, None, "F13", KeyAssignment::Action("Alt+Tab".into()))?;
            config.set_layer_key_holds_modifiers(&id, "F13", true)?;
            Ok(id)
        })?
        .value;
    ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
    ui.global::<LayersEditor>().invoke_choose(0);
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));

    let weak = ui.as_weak();
    let step = Rc::new(Cell::new(0u32));
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        let ui = weak.unwrap();
        let window = ui.window();
        let drag = ui.global::<DragDrop>();
        let key = |code: &str| document.read().layer_key(&layer, code);
        let press = |at: LogicalPosition| {
            window.dispatch_event(WindowEvent::PointerMoved { position: at });
            window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
        };
        let release = |at: LogicalPosition| {
            window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
        };
        let (q, j) = (cell_center(true, 1, 1), cell_center(false, 2, 1));
        match step.get() {
            0..=4 => {}
            5 => {
                snapshot(&ui, "layer-keys");
                let editor = ui.global::<LayersEditor>();
                let marked: Vec<_> = editor
                    .get_left_cells()
                    .iter()
                    .filter(|cell| cell.holds)
                    .map(|cell| cell.code.to_string())
                    .collect();
                assert_eq!(marked, ["KeyW"]);
                assert!(editor.get_extras().iter().all(|row| row.holds == (row.key == "F13")));
                // From the left hand to the right one: the swap crosses both grids.
                press(q);
                window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(q.x + 40.0, q.y) });
                assert!(drag.get_dragging());
                window.dispatch_event(WindowEvent::PointerMoved { position: j });
            }
            6 => {
                snapshot(&ui, "layer-keys-dragging");
                release(j);
                assert!(!drag.get_dragging());
                assert_eq!(key("KeyQ"), KeyAssignment::Swallow);
                assert_eq!(key("KeyJ"), KeyAssignment::Action("ArrowUp".into()));
                assert_eq!(ui.global::<LayersEditor>().get_dialog(), LayerDialog::None);
            }
            7 => {
                // Onto an unassigned key: the action moves, the source becomes transparent.
                let empty = cell_center(false, 2, 2);
                press(j);
                window.dispatch_event(WindowEvent::PointerMoved { position: empty });
                release(empty);
                assert_eq!(key("KeyJ"), KeyAssignment::Transparent);
                assert_eq!(key("KeyK"), KeyAssignment::Action("ArrowUp".into()));
                // Dropped back on itself or outside the keys: nothing changes.
                let k = empty;
                press(k);
                window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(k.x + 30.0, k.y + 2.0) });
                release(k);
                press(k);
                window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(k.x, 20.0) });
                release(LogicalPosition::new(k.x, 20.0));
                assert_eq!(key("KeyK"), KeyAssignment::Action("ArrowUp".into()));
                assert_eq!(key("KeyQ"), KeyAssignment::Swallow);
                assert!(!drag.get_dragging());
            }
            8 => {
                // A plain click still opens the action picker.
                let k = cell_center(false, 2, 2);
                press(k);
                release(k);
                assert!(ui.global::<slint_shell::ui::ActionPicker>().get_opened());
                println!("Layer keys passed: held-modifier marks, drag swaps assignments across hands, drop on empty key, cancel, click");
                slint::quit_event_loop().unwrap();
            }
            _ => {}
        }
        step.set(step.get() + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}

/// Window position of grid cell (`row`, `col`) of one hand at the 1120×760 window size.
fn cell_center(left: bool, row: usize, col: usize) -> LogicalPosition {
    let (x, y, cell) = (if left { LEFT_X } else { RIGHT_X }, TOP_Y, CELL);
    LogicalPosition::new(
        x + col as f32 * (cell + 6.0) + cell / 2.0,
        y + row as f32 * 52.0 + 23.0,
    )
}

const LEFT_X: f32 = 40.0;
const RIGHT_X: f32 = 564.0;
const TOP_Y: f32 = 452.0;
const CELL: f32 = 78.0;
