use evdev::{AttributeSet, InputEvent, KeyCode, uinput::VirtualDevice};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::{
    self, OrgKdeKwinFakeInput,
};

pub fn isolated() -> bool {
    std::env::var("SLINT_SHELL_ISOLATED_INPUT").as_deref() == Ok("1")
}
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
impl Dispatch<OrgKdeKwinFakeInput, ()> for State {
    fn event(
        _: &mut Self,
        _: &OrgKdeKwinFakeInput,
        _: org_kde_kwin_fake_input::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
pub enum Keyboard {
    Evdev(VirtualDevice),
    Isolated(Connection, OrgKdeKwinFakeInput),
}
impl Keyboard {
    pub fn new(
        name: &str,
        keys: &AttributeSet<KeyCode>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !isolated() {
            return Ok(Self::Evdev(
                VirtualDevice::builder()?
                    .name(name)
                    .with_keys(keys)?
                    .build()?,
            ));
        }
        if !std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| v.starts_with("lhc-stage3a-")) {
            return Err("isolated test input requires dedicated lhc-stage3a-* compositor".into());
        }
        let connection = Connection::connect_to_env()?;
        let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
        let input: OrgKdeKwinFakeInput = globals.bind(&queue.handle(), 4..=5, ())?;
        input.authenticate(
            "Slint prototype tests".into(),
            "Keyboard tests in private virtual compositor".into(),
        );
        queue.roundtrip(&mut State)?;
        Ok(Self::Isolated(connection, input))
    }
    pub fn emit(&mut self, events: &[InputEvent]) -> std::io::Result<()> {
        match self {
            Self::Evdev(device) => device.emit(events),
            Self::Isolated(connection, input) => {
                for event in events {
                    input.keyboard_key(event.code() as u32, event.value() as u32);
                }
                connection.flush().map_err(std::io::Error::other)
            }
        }
    }
}
impl Drop for Keyboard {
    fn drop(&mut self) {
        use wayland_client::Proxy;
        if let Self::Isolated(connection, input) = self
            && input.version() >= 5
        {
            input.destroy();
            let _ = connection.flush();
        }
    }
}
