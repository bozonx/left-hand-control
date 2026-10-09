use crate::{
    slint_adapter::SpellSkiaWinAdapter,
    wayland_adapter::{SpellWin, common, common::get_string},
};
use slint::{SharedString, platform::WindowEvent};
use smithay_client_toolkit::{
    reexports::client::{Connection, QueueHandle, protocol::wl_pointer},
    seat::{
        keyboard::KeyboardHandler,
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        touch::TouchHandler,
    },
    shell::WaylandSurface,
};
use tracing::{info, trace, warn};

// Slint doesn't hve very specific
// APIs for touch support (I think). I am talking with them on what
// can be done so that things like multi-touch support, gestures etc
// can be made possible. For now I am going to place empty value in here.
impl TouchHandler for SpellWin {
    fn up(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        _serial: u32,
        _time: u32,
        _id: i32,
    ) {
        info!("Up event from touch");
    }
    fn down(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        _serial: u32,
        _time: u32,
        _surface: smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        _id: i32,
        position: (f64, f64),
    ) {
        info!("Down event produced with posaition: {position:?}");
    }

    fn motion(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        _time: u32,
        _id: i32,
        position: (f64, f64),
    ) {
        self.adapter
            .as_ref()
            .unwrap()
            .try_dispatch_event(WindowEvent::PointerMoved {
                position: slint::LogicalPosition {
                    x: position.0 as f32,
                    y: position.1 as f32,
                },
            })
            .unwrap_or_else(|err| warn!("Touch move event failed with error: {:?}", err));
    }

    fn shape(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        _id: i32,
        major: f64,
        minor: f64,
    ) {
        info!("Shape data released. Major: {major}, Minor: {minor}");
    }
    fn orientation(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
        _id: i32,
        orientation: f64,
    ) {
        info!("Orientation data released: {orientation}.")
    }
    fn cancel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch,
    ) {
        info!("Active touch sequence cancelled");
    }
}

impl PointerHandler for SpellWin {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            let adapter: &std::rc::Rc<SpellSkiaWinAdapter> =
                if let Some(popup) = self.popup_manager.return_adapter(&event.surface) {
                    popup
                } else if &event.surface == self.layer.as_ref().unwrap().wl_surface() {
                    self.adapter.as_ref().unwrap()
                } else {
                    continue;
                };

            match event.kind {
                Enter { serial } => {
                    trace!(
                        "Pointer entered with serial {:?} at: {:?}",
                        serial, event.position
                    );

                    adapter
                        .try_dispatch_event(WindowEvent::PointerMoved {
                            position: slint::LogicalPosition {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                            },
                        })
                        .unwrap_or_else(|err| {
                            warn!(
                                "Pointer move event after entry failed with error: {:?}",
                                err
                            )
                        });
                    self.states.pointer_state.last_cursor_enter_serial = Some(serial);
                }
                Leave { .. } => {
                    trace!("Pointer left: {:?}", event.position);

                    adapter
                        .try_dispatch_event(WindowEvent::PointerExited)
                        .unwrap_or_else(|err| {
                            warn!("Pointer exit event failed with error: {:?}", err)
                        });
                }
                Motion { .. } => {
                    trace!("Pointer entered @{:?}", event.position);

                    adapter
                        .try_dispatch_event(WindowEvent::PointerMoved {
                            position: slint::LogicalPosition {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                            },
                        })
                        .unwrap_or_else(|err| {
                            warn!("Pointer move event failed with error: {:?}", err)
                        });
                }
                Press { button, .. } => {
                    trace!("Press {:?} @ {:?}", button, event.position);

                    adapter
                        .try_dispatch_event(WindowEvent::PointerPressed {
                            position: slint::LogicalPosition {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                            },
                            button: common::map_pointer_button(button),
                        })
                        .unwrap_or_else(|err| {
                            warn!("Pointer press event failed with error: {:?}", err)
                        });
                }
                Release { button, .. } => {
                    trace!("Release {:?} @ {:?}", button, event.position);

                    adapter
                        .try_dispatch_event(WindowEvent::PointerReleased {
                            position: slint::LogicalPosition {
                                x: event.position.0 as f32,
                                y: event.position.1 as f32,
                            },
                            button: common::map_pointer_button(button),
                        })
                        .unwrap_or_else(|err| {
                            warn!("Pointer release event failed with error: {:?}", err)
                        });
                }
                Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    trace!("Scroll H:{horizontal:?}, V:{vertical:?}");

                    if !self.natural_scroll {
                        adapter
                            .try_dispatch_event(WindowEvent::PointerScrolled {
                                position: slint::LogicalPosition {
                                    x: event.position.0 as f32,
                                    y: event.position.1 as f32,
                                },
                                delta_x: horizontal.absolute as f32,
                                delta_y: vertical.absolute as f32,
                            })
                            .unwrap_or_else(|err| {
                                warn!("Pointer scroll event failed with error: {:?}", err)
                            });
                    } else {
                        adapter
                            .try_dispatch_event(WindowEvent::PointerScrolled {
                                position: slint::LogicalPosition {
                                    x: event.position.0 as f32,
                                    y: event.position.1 as f32,
                                },
                                delta_x: -horizontal.absolute as f32,
                                delta_y: -vertical.absolute as f32,
                            })
                            .unwrap_or_else(|err| {
                                warn!("Pointer scroll event failed with error: {:?}", err)
                            });
                    }
                }
            }
        }
    }
}

impl KeyboardHandler for SpellWin {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
        if _surface != self.layer.as_ref().unwrap().wl_surface() { return; }
        self.adapter.as_ref().unwrap().window.dispatch_event(WindowEvent::WindowActiveChanged(true));
        self.emit(super::WindowEvent::Focus(true));
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _surface: &smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface,
        _serial: u32,
    ) {
        if _surface != self.layer.as_ref().unwrap().wl_surface() { return; }
        self.consumed_keys.clear();
        for (_, text) in self.pressed.drain() {
            self.adapter.as_ref().unwrap().window.dispatch_event(WindowEvent::KeyReleased { text });
        }
        self.sync_modifiers(Default::default());
        self.adapter.as_ref().unwrap().window.dispatch_event(WindowEvent::WindowActiveChanged(false));
        self.emit(super::WindowEvent::Focus(false));
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        let code = event.raw_code;
        if self.key_handler.as_mut().is_some_and(|handler| handler(code, self.modifiers.shift, self.modifiers.ctrl)) {
            self.consumed_keys.insert(code);
            return;
        }
        let text = self.editing_text(event);
        if is_modifier(&text) { return; }
        self.pressed.insert(code, text.clone());
        self.adapter.as_ref().unwrap().window.dispatch_event(WindowEvent::KeyPressed { text });
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        /*mut*/ event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        if self.consumed_keys.remove(&event.raw_code) { return; }
        if let Some(text) = self.pressed.remove(&event.raw_code) {
            self.adapter.as_ref().unwrap().window.dispatch_event(WindowEvent::KeyReleased { text });
        }
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        _raw_modifiers: smithay_client_toolkit::seat::keyboard::RawModifiers,
        _layout: u32,
    ) {
        self.sync_modifiers(_modifiers);
    }
    fn update_repeat_info(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _info: smithay_client_toolkit::seat::keyboard::RepeatInfo,
    ) {
        trace!("Key repeat info updated");
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        self.repeat_input(_event);
    }
}

fn is_modifier(text: &SharedString) -> bool {
    use slint::platform::Key;
    [Key::Shift, Key::ShiftR, Key::Control, Key::ControlR, Key::Alt, Key::AltGr, Key::Meta, Key::MetaR]
        .into_iter().any(|key| *text == SharedString::from(key))
}

impl SpellWin {
    fn editing_text(&self, event: smithay_client_toolkit::seat::keyboard::KeyEvent) -> SharedString {
        let physical = match event.raw_code {
            30 => Some('a'), 46 => Some('c'), 45 => Some('x'),
            47 => Some('v'), 44 => Some('z'), 21 => Some('y'),
            _ => None,
        };
        let shortcut_logical = event.keysym.key_char().map(|character| character.to_string());
        let logical = get_string(event);
        i_slint_core::input::text_editing_shortcut(
            shortcut_logical.as_deref().unwrap_or(logical.as_str()), physical, self.modifiers.ctrl, self.modifiers.shift,
            self.modifiers.alt, self.modifiers.logo,
        ).map(SharedString::from).unwrap_or(logical)
    }

    pub(super) fn repeat_input(&mut self, event: smithay_client_toolkit::seat::keyboard::KeyEvent) {
        if self.is_hidden.get() || self.consumed_keys.contains(&event.raw_code) { return; }
        let text = self.editing_text(event);
        if !is_modifier(&text) {
            self.adapter.as_ref().unwrap().window.dispatch_event(WindowEvent::KeyPressRepeated { text });
        }
    }

    fn sync_modifiers(&mut self, next: smithay_client_toolkit::seat::keyboard::Modifiers) {
        use slint::platform::Key;
        for (before, after, key) in [
            (self.modifiers.shift, next.shift, Key::Shift),
            (self.modifiers.ctrl, next.ctrl, Key::Control),
            (self.modifiers.alt, next.alt, Key::Alt),
            (self.modifiers.logo, next.logo, Key::Meta),
        ] {
            if before != after {
                let text = key.into();
                self.adapter.as_ref().unwrap().window.dispatch_event(if after {
                    WindowEvent::KeyPressed { text }
                } else {
                    WindowEvent::KeyReleased { text }
                });
            }
        }
        self.modifiers = next;
    }
}
