use crate::{
    command::{Command, Dispatch, Source},
    i18n::{self, Language, TrayItem},
};
use ksni::{
    blocking::TrayMethods,
    menu::{RadioGroup, RadioItem, StandardItem, SubMenu},
};
use lhc_core::gamemode::GameModeStatus;
use std::time::Instant;

pub struct Tray {
    enabled: bool,
    english: bool,
    /// Index of the Auto/On/Off choice and the effective state.
    game: (usize, bool),
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

    pub fn set_game(&self, status: &GameModeStatus) {
        let game = (i18n::tray_game_index(status.control), status.active);
        self.0.update(|tray| tray.game = game);
    }
}

impl Tray {
    fn language(&self) -> Language {
        if self.english {
            Language::English
        } else {
            Language::Russian
        }
    }

    fn game_menu(&self) -> ksni::MenuItem<Self> {
        let language = self.language();
        SubMenu {
            label: i18n::tray_game_title(language, self.game.1),
            submenu: vec![
                RadioGroup {
                    selected: self.game.0,
                    select: Box::new(|tray: &mut Self, index: usize| {
                        let Some(mode) = i18n::TRAY_GAME_MODES.get(index) else {
                            return;
                        };
                        tray.game.0 = index;
                        (tray.dispatch)(
                            Command::GameMode(*mode),
                            Source::Tray,
                            Instant::now(),
                            None,
                        )
                    }),
                    options: i18n::TRAY_GAME_MODES
                        .into_iter()
                        .map(|mode| RadioItem {
                            label: i18n::tray_game_choice(language, mode),
                            ..Default::default()
                        })
                        .collect(),
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
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
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Left Hand Control".into(),
            description: i18n::tray_game_title(self.language(), self.game.1),
            ..Default::default()
        }
    }
    fn activate(&mut self, _: i32, _: i32) {
        (self.dispatch)(Command::ToggleSettings, Source::Tray, Instant::now(), None);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let language = self.language();
        let mut items = Vec::new();
        for item in TrayItem::ALL {
            if item == TrayItem::Quit {
                items.push(self.game_menu());
            }
            items.push(
                StandardItem {
                    label: item.label(language),
                    activate: Box::new(move |tray: &mut Self| {
                        (tray.dispatch)(item.command(), Source::Tray, Instant::now(), None)
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        items
    }
}

pub fn start(dispatch: Dispatch) -> Result<Handle, ksni::Error> {
    Tray {
        enabled: false,
        english: false,
        game: (0, false),
        dispatch,
    }
    .spawn()
    .map(Handle)
}
