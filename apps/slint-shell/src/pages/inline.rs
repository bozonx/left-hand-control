//! Window events that drive the inline editors of every page: window
//! activation and pointer presses, which commit an open editor when they land
//! outside it.

use crate::ui::{InlineEditors, SettingsWindow};
use slint::ComponentHandle;
use slint::winit_030::winit::event::{ElementState, TouchPhase, WindowEvent};
use std::cell::Cell;

thread_local! {
    /// Last cursor position in physical pixels; winit reports presses without it.
    static CURSOR: Cell<(f64, f64)> = const { Cell::new((0.0, 0.0)) };
}

/// Forward `event` of the settings window to [`InlineEditors`].
pub fn observe(ui: &SettingsWindow, event: &WindowEvent) {
    let editors = ui.global::<InlineEditors>();
    let pressed = match event {
        WindowEvent::Focused(active) => {
            editors.set_window_active(*active);
            None
        }
        WindowEvent::CursorMoved { position, .. } => {
            CURSOR.set((position.x, position.y));
            None
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            ..
        } => Some(CURSOR.get()),
        WindowEvent::Touch(touch) if touch.phase == TouchPhase::Started => {
            Some((touch.location.x, touch.location.y))
        }
        _ => None,
    };
    if let Some((x, y)) = pressed {
        let scale = f64::from(ui.window().scale_factor());
        editors.invoke_pointer_pressed((x / scale) as f32, (y / scale) as f32);
    }
}
