//! Window events that drive the inline editors of every page: window
//! activation and pointer presses, which commit an open editor when they land
//! outside it.

use crate::ui::{InlineEditors, SettingsWindow};
use slint::ComponentHandle;
use slint::winit_030::winit::event::{ElementState, TouchPhase, WindowEvent};
use std::cell::Cell;
use std::time::Duration;

/// Pointer state between window events, in physical pixels.
#[derive(Default)]
pub(super) struct Pointer {
    /// Last position; winit reports presses without it.
    cursor: Cell<(f64, f64)>,
    /// Position of the press waiting for its release.
    pressed: Cell<Option<(f64, f64)>>,
}

/// Forward `event` of the settings window to [`InlineEditors`].
///
/// The commit for a press outside an editor runs only after Slint has handled
/// the release: committing on press closes the editor and reflows the page
/// before the click, so the button under the pointer moves away and the click
/// is lost.
pub(super) fn observe(ui: &SettingsWindow, event: &WindowEvent, pointer: &Pointer) {
    let editors = ui.global::<InlineEditors>();
    let released = match event {
        WindowEvent::Focused(active) => {
            editors.set_window_active(*active);
            None
        }
        WindowEvent::CursorMoved { position, .. } => {
            pointer.cursor.set((position.x, position.y));
            None
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            ..
        } => {
            pointer.pressed.set(Some(pointer.cursor.get()));
            None
        }
        WindowEvent::MouseInput {
            state: ElementState::Released,
            ..
        } => pointer.pressed.take(),
        WindowEvent::Touch(touch) => match touch.phase {
            TouchPhase::Started => {
                pointer
                    .pressed
                    .set(Some((touch.location.x, touch.location.y)));
                None
            }
            TouchPhase::Ended | TouchPhase::Cancelled => pointer.pressed.take(),
            TouchPhase::Moved => None,
        },
        _ => None,
    };
    if let Some((x, y)) = released {
        let scale = f64::from(ui.window().scale_factor());
        let (x, y) = ((x / scale) as f32, (y / scale) as f32);
        let ui = ui.as_weak();
        // This hook runs before Slint sees the event; defer past its dispatch.
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(ui) = ui.upgrade() {
                ui.global::<InlineEditors>().invoke_pointer_pressed(x, y);
            }
        });
    }
}
