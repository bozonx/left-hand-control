use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use slint::winit_030::winit;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_surface::WlSurface},
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgActivationV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &XdgActivationV1,
        _: wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[derive(Default)]
pub struct Activation {
    wayland: Option<(Connection, EventQueue<State>, XdgActivationV1)>,
}

impl Activation {
    pub fn activate(
        &mut self,
        window: &winit::window::Window,
        token: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let RawWindowHandle::Wayland(surface) = window.window_handle()?.as_raw() else {
            window.focus_window();
            return Ok(());
        };
        let Some(token) = token else {
            if matches!(
                lhc_core::platform::linux::detect().desktop,
                lhc_core::platform::linux::Desktop::Gnome
            ) {
                lhc_core::platform::gnome::request_window_activation(window.title())?;
            }
            return Ok(());
        };
        let RawDisplayHandle::Wayland(display) = window.display_handle()?.as_raw() else {
            return Err("Wayland surface without Wayland display".into());
        };
        let wayland = match self.wayland.take() {
            Some(wayland) => wayland,
            None => {
                let backend = unsafe {
                    wayland_backend::client::Backend::from_foreign_display(
                        display.display.as_ptr().cast(),
                    )
                };
                let connection = Connection::from_backend(backend);
                let (globals, queue) = registry_queue_init::<State>(&connection)?;
                let activation =
                    globals.bind::<XdgActivationV1, _, _>(&queue.handle(), 1..=1, ())?;
                (connection, queue, activation)
            }
        };
        let (connection, queue, activation) = self.wayland.insert(wayland);
        queue.dispatch_pending(&mut State)?;
        let id = unsafe {
            wayland_backend::client::ObjectId::from_ptr(
                WlSurface::interface(),
                surface.surface.as_ptr().cast(),
            )?
        };
        let surface = WlSurface::from_id(connection, id)?;
        activation.activate(token.to_owned(), &surface);
        connection.flush()?;
        log::info!("xdg_activation_v1.activate sent");
        Ok(())
    }
}
