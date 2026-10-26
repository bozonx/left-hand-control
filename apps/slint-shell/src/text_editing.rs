//! Text field shortcuts: their display text and dispatch to the focused editor.

use slint::winit_030::winit::{
    event::{ElementState, WindowEvent},
    keyboard::{Key, KeyCode, ModifiersState, PhysicalKey},
};

pub fn shortcut_text(
    logical: &str,
    physical: KeyCode,
    modifiers: ModifiersState,
) -> Option<&'static str> {
    let primary = if cfg!(target_os = "macos") {
        modifiers.super_key() && !modifiers.control_key()
    } else {
        modifiers.control_key() && !modifiers.super_key()
    };
    if !primary || modifiers.alt_key() {
        return None;
    }
    if !modifiers.shift_key()
        && (logical.eq_ignore_ascii_case("s")
            || (physical == KeyCode::KeyS
                && logical
                    .chars()
                    .next()
                    .is_some_and(|letter| letter.is_alphabetic() && !letter.is_ascii())))
    {
        return Some("s");
    }
    let physical = match physical {
        KeyCode::KeyA => Some('a'),
        KeyCode::KeyC => Some('c'),
        KeyCode::KeyX => Some('x'),
        KeyCode::KeyV => Some('v'),
        KeyCode::KeyZ => Some('z'),
        KeyCode::KeyY => Some('y'),
        _ => None,
    };
    i_slint_core::input::text_editing_shortcut(
        logical,
        physical,
        primary,
        modifiers.shift_key(),
        modifiers.alt_key(),
        false,
    )
}

pub fn dispatch_shortcut(
    window: &slint::Window,
    event: &WindowEvent,
    modifiers: ModifiersState,
) -> bool {
    let WindowEvent::KeyboardInput { event, .. } = event else {
        return false;
    };
    let PhysicalKey::Code(physical) = event.physical_key else {
        return false;
    };
    let logical = match &event.logical_key {
        Key::Character(text) => text.as_str(),
        _ => return false,
    };
    let Some(text) = shortcut_text(logical, physical, modifiers) else {
        return false;
    };
    let text = text.into();
    window.dispatch_event(match (event.state, event.repeat) {
        (ElementState::Released, _) => slint::platform::WindowEvent::KeyReleased { text },
        (ElementState::Pressed, true) => slint::platform::WindowEvent::KeyPressRepeated { text },
        (ElementState::Pressed, false) => slint::platform::WindowEvent::KeyPressed { text },
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary() -> ModifiersState {
        if cfg!(target_os = "macos") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        }
    }

    #[test]
    fn shortcuts_work_in_russian_and_with_caps_lock() {
        for (logical, physical, expected) in [
            ("я", KeyCode::KeyZ, "z"),
            ("Я", KeyCode::KeyZ, "z"),
            ("н", KeyCode::KeyY, "y"),
            ("ф", KeyCode::KeyA, "a"),
            ("с", KeyCode::KeyC, "c"),
            ("ч", KeyCode::KeyX, "x"),
            ("м", KeyCode::KeyV, "v"),
            ("Z", KeyCode::KeyZ, "z"),
            ("ы", KeyCode::KeyS, "s"),
            ("S", KeyCode::KeyS, "s"),
        ] {
            assert_eq!(shortcut_text(logical, physical, primary()), Some(expected));
        }
        assert_eq!(
            shortcut_text("Я", KeyCode::KeyZ, primary() | ModifiersState::SHIFT),
            Some("z")
        );
    }

    #[test]
    fn preserves_latin_layouts_and_leaves_typing_altgr_and_other_chords_alone() {
        assert_eq!(shortcut_text("z", KeyCode::KeyY, primary()), Some("z"));
        assert_eq!(shortcut_text("q", KeyCode::KeyA, primary()), None);
        assert_eq!(shortcut_text("o", KeyCode::KeyS, primary()), None);
        assert_eq!(shortcut_text("s", KeyCode::KeyO, primary()), Some("s"));
        assert_eq!(
            shortcut_text("я", KeyCode::KeyZ, ModifiersState::empty()),
            None
        );
        assert_eq!(
            shortcut_text("я", KeyCode::KeyZ, primary() | ModifiersState::ALT),
            None
        );
        assert_eq!(
            shortcut_text("м", KeyCode::KeyV, primary() | ModifiersState::SHIFT),
            None
        );
        assert_eq!(shortcut_text("й", KeyCode::KeyQ, primary()), None);
    }
}
