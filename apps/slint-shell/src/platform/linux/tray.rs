use crate::{
    command::{Command, Dispatch, Source},
    i18n::{Language, TrayItem},
};
use ksni::{blocking::TrayMethods, menu::StandardItem};
use std::time::Instant;

pub struct Tray {
    enabled: bool,
    english: bool,
    dispatch: Dispatch,
}

pub struct Handle(ksni::blocking::Handle<Tray>);

impl Handle {
    pub fn set_enabled(&self, enabled: bool) {
        self.0.update(|tray| tray.enabled = enabled);
    }

    pub fn set_english(&self, english: bool) {
        self.0.update(|tray| tray.english = english);
    }
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "lhc-slint-shell".into()
    }
    fn title(&self) -> String {
        "Left Hand Control".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let (rgba, width, height) = super::super::icon::rgba(self.enabled);
        // StatusNotifierItem pixmaps are ARGB32 in network byte order.
        let data = rgba
            .chunks_exact(4)
            .flat_map(|p| [p[3], p[0], p[1], p[2]])
            .collect();
        vec![ksni::Icon {
            width: width as i32,
            height: height as i32,
            data,
        }]
    }
    fn activate(&mut self, _: i32, _: i32) {
        (self.dispatch)(Command::ToggleSettings, Source::Tray, Instant::now(), None);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        TrayItem::ALL
            .into_iter()
            .map(|item| {
                StandardItem {
                    label: item.label(if self.english {
                        Language::English
                    } else {
                        Language::Russian
                    }),
                    activate: Box::new(move |tray: &mut Self| {
                        (tray.dispatch)(item.command(), Source::Tray, Instant::now(), None)
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
        enabled: false,
        english: false,
        dispatch,
    }
    .spawn()
    .map(Handle)
}
