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
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use support::snapshot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::log_navigation();
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
    let focused_key = RefCell::new(None);
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
        let (q, j) = (cell_center(&ui, true, 1, 1), cell_center(&ui, false, 2, 1));
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
                let empty = cell_center(&ui, false, 2, 2);
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
                let k = cell_center(&ui, false, 2, 2);
                press(k);
                release(k);
                assert!(ui.global::<slint_shell::ui::ActionPicker>().get_opened());
                ui.global::<slint_shell::ui::ActionPicker>().invoke_dismiss_picker();
            }
            9 => {
                use i_slint_core::{accessibility::{nearest_accessible_item, AccessibleStringProperty}, window::WindowInner};
                let inner = WindowInner::from_pub(window);
                for _ in 0..300 {
                    let focus = inner.focus_item.borrow().upgrade();
                    if focus.as_ref().is_some_and(|item| nearest_accessible_item(item.clone()).accessible_string_property(AccessibleStringProperty::Value).as_deref() == Some("KeyK")) {
                        *focused_key.borrow_mut() = focus.map(|item| item.downgrade());
                        break;
                    }
                    send_key(&ui, slint::platform::Key::Tab);
                }
                assert!(focused_key.borrow().is_some(), "Tab reaches a layer key");
                send_key(&ui, slint::platform::Key::Return);
                let picker = ui.global::<slint_shell::ui::ActionPicker>();
                assert!(picker.get_opened());
                picker.invoke_select_behavior(slint_shell::ui::PickerBehavior::Action);
                picker.set_value("ArrowDown".into());
                picker.invoke_apply();
                assert!(!picker.get_opened());
            }
            10 => {
                let inner = i_slint_core::window::WindowInner::from_pub(window);
                let expected = focused_key.borrow().as_ref().unwrap().upgrade().expect("key repeater survives assignment");
                assert_eq!(inner.focus_item.borrow().upgrade(), Some(expected.clone()), "picker restores its keyboard initiator");
                document.refresh_all();
                let _ = window.take_snapshot().unwrap();
                assert_eq!(inner.focus_item.borrow().upgrade(), Some(expected), "refresh keeps key focus");
                assert_eq!(key("KeyK"), KeyAssignment::Action("ArrowDown".into()));
                send_key(&ui, slint::platform::Key::LeftArrow);
                let focused = inner.focus_item.borrow().upgrade().unwrap();
                let focused_value = i_slint_core::accessibility::nearest_accessible_item(focused).accessible_string_property(i_slint_core::accessibility::AccessibleStringProperty::Value).unwrap();
                assert_eq!(focused_value.as_str(), "KeyJ");
                let control = slint::platform::Key::Control;
                window.dispatch_event(WindowEvent::KeyPressed { text: control.into() });
                window.dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
                window.dispatch_event(WindowEvent::KeyReleased { text: "x".into() });
                window.dispatch_event(WindowEvent::KeyReleased { text: control.into() });
                send_key(&ui, slint::platform::Key::RightArrow);
                window.dispatch_event(WindowEvent::KeyPressed { text: control.into() });
                window.dispatch_event(WindowEvent::KeyPressed { text: "v".into() });
                window.dispatch_event(WindowEvent::KeyReleased { text: "v".into() });
                window.dispatch_event(WindowEvent::KeyReleased { text: control.into() });
                assert_eq!(key("KeyJ"), KeyAssignment::Action("ArrowDown".into()));
                assert_eq!(key("KeyK"), KeyAssignment::Transparent);
                println!("Layer keys passed: arrows and keyboard swaps; held-modifier marks, drag swaps, cancel, click, keyboard assignment and focus restoration");
                slint::quit_event_loop().unwrap();
            }
            _ => {}
        }
        step.set(step.get() + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}

/// Resolve the actual key position so toolbar height, fonts and scale may change.
fn cell_center(ui: &SettingsWindow, left: bool, row: usize, col: usize) -> LogicalPosition {
    let editor = ui.global::<LayersEditor>();
    let cells = if left {
        editor.get_left_cells()
    } else {
        editor.get_right_cells()
    };
    let cell = cells.row_data(row * 6 + col).unwrap();
    support::accessible_position(
        ui,
        i_slint_core::items::AccessibleRole::Button,
        &format!("{} ({})", cell.label, cell.code),
        false,
    )
}

fn send_key(ui: &SettingsWindow, key: slint::platform::Key) {
    let text: slint::SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
