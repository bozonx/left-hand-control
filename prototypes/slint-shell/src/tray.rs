use crate::{Command, Dispatch};
use ksni::{blocking::TrayMethods, menu::StandardItem};
use std::time::Instant;

pub struct Tray {
    pub enabled: bool,
    dispatch: Dispatch,
}

pub struct Handle(ksni::blocking::Handle<Tray>);

impl Handle {
    pub fn set_enabled(&self, enabled: bool) {
        self.0.update(|tray| tray.enabled = enabled);
    }
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "lhc-slint-shell".into()
    }
    fn title(&self) -> String {
        "Slint Shell".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let color = if self.enabled {
            [255, 70, 180, 110]
        } else {
            [255, 130, 130, 130]
        };
        vec![ksni::Icon {
            width: 22,
            height: 22,
            data: color.repeat(22 * 22),
        }]
    }
    fn activate(&mut self, _: i32, _: i32) {
        (self.dispatch)(Command::ToggleSettings, "tray", Instant::now(), None);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        [
            ("Настройки", Command::Show("settings")),
            ("Эмодзи", Command::Show("emoji")),
            ("Быстрые действия", Command::Show("quick")),
            ("Mapper вкл / выкл", Command::ToggleMapper),
            ("Выход", Command::Quit),
        ]
        .into_iter()
        .map(|(label, command)| {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |tray: &mut Self| {
                    (tray.dispatch)(command.clone(), "tray", Instant::now(), None)
                }),
                ..Default::default()
            }
            .into()
        })
        .collect()
    }
}

pub fn start(dispatch: Dispatch) -> Result<Handle, ksni::Error> {
    Tray {
        enabled: true,
        dispatch,
    }
    .spawn()
    .map(Handle)
}
