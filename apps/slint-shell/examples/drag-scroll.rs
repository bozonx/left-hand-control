//! Scrolling a list while a DragHandle drag is in progress: wheel and edge auto-scroll.
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::{cell::Cell, rc::Rc, time::Duration};

slint::slint! {
    import { ScrollView } from "../ui/scroll.slint";
    import { DragDrop, DragHandle, DropMarker } from "../ui/reorder.slint";
    export { DragDrop }
    export component DragWindow inherits Window {
        width: 400px; height: 300px;
        out property <length> scroll-y: list.viewport-y;
        out property <length> other-y: other.viewport-y;
        // A second ScrollView on the page also observes the wheel and must leave it alone.
        other := ScrollView {
            x: 300px; width: 50px; height: 100%;
            VerticalLayout { for i in 30: Rectangle { height: 40px; } }
        }
        list := ScrollView {
            x: 0px; width: 300px; height: 100%;
            drag-scroll: true;
            viewport-width: self.visible-width;
            VerticalLayout {
                for i in 30: Rectangle {
                    height: 40px;
                    HorizontalLayout { DragHandle { group: 7; index: i; count: 30; } Text { text: i; } }
                    DropMarker { width: parent.width; group: 7; index: i; row-y: parent.y; row-height: parent.height; }
                }
            }
        }
        // `changed` handlers run in an unspecified order, so put a passive ScrollView on both sides.
        ScrollView {
            x: 350px; width: 50px; height: 100%;
            VerticalLayout { for i in 30: Rectangle { height: 40px; } }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = DragWindow::new()?;
    ui.global::<DragDrop>().on_locate(|_, source, offset, _| source + (offset / 40.0).round() as i32);
    ui.show()?;
    let weak = ui.as_weak();
    let step = Rc::new(Cell::new(0u32));
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(50), move || {
        let ui = weak.unwrap();
        let window = ui.window();
        let drag = ui.global::<DragDrop>();
        let at = |y: f32| LogicalPosition::new(14.0, y);
        match step.get() {
            0..=4 => {}
            5 => {
                window.dispatch_event(WindowEvent::PointerMoved { position: at(20.0) });
                window.dispatch_event(WindowEvent::PointerPressed { position: at(20.0), button: PointerEventButton::Left });
                window.dispatch_event(WindowEvent::PointerMoved { position: at(140.0) });
                assert!(drag.get_dragging());
                assert_eq!(drag.get_target(), 3);
            }
            6 => window.dispatch_event(WindowEvent::PointerScrolled { position: at(140.0), delta_x: 0.0, delta_y: -60.0 }),
            7 => {
                assert_eq!(ui.get_scroll_y(), -60.0);
                assert_eq!(ui.get_other_y(), 0.0);
                assert!(drag.get_dragging());
                // The list moved 60px under the pointer: 180px from the source row.
                assert_eq!(drag.get_target(), 5);
                window.dispatch_event(WindowEvent::PointerMoved { position: at(295.0) });
            }
            8..=17 => {}
            18 => {
                assert!(ui.get_scroll_y() < -200.0, "edge auto-scroll: {}", ui.get_scroll_y());
                assert!(drag.get_dragging());
                assert!(drag.get_target() > 10);
                window.dispatch_event(WindowEvent::PointerMoved { position: at(150.0) });
                let y = ui.get_scroll_y();
                window.dispatch_event(WindowEvent::PointerReleased { position: at(150.0), button: PointerEventButton::Left });
                assert!(!drag.get_dragging());
                assert_eq!(ui.get_scroll_y(), y);
                println!("Drag scroll passed: wheel during drag, edge auto-scroll, drop target follows the list");
                slint::quit_event_loop().unwrap();
            }
            _ => {}
        }
        step.set(step.get() + 1);
    });
    slint::run_event_loop()?;
    Ok(())
}
