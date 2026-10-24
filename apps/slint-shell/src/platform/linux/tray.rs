use crate::{
    command::{Command, Dispatch, Source, Window},
    i18n::{self, Language, TrayItem},
};
use ksni::{
    blocking::TrayMethods,
    menu::{RadioGroup, RadioItem, StandardItem, SubMenu},
};
use lhc_core::gamemode::GameModeStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long a missing tray host may take to appear before settings open
/// instead. At login the application can start before the host (GNOME
/// AppIndicators, a restarting Plasma shell).
const HOST_GRACE: Duration = Duration::from_secs(10);

pub struct Tray {
    enabled: bool,
    in_development: bool,
    language: Language,
    /// Index of the Auto/On/Off choice and the effective state.
    game: (usize, bool),
    dispatch: Dispatch,
    online: Arc<AtomicBool>,
}

pub struct Handle(ksni::blocking::Handle<Tray>);

impl Handle {
    pub fn set_in_development(&self, enabled: bool) {
        self.0.update(|tray| tray.in_development = enabled);
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.0.update(|tray| tray.enabled = enabled);
    }

    pub fn set_language(&self, language: Language) {
        self.0.update(|tray| tray.language = language);
    }

    pub fn set_game(&self, status: &GameModeStatus) {
        let game = (i18n::tray_game_index(status.control), status.active);
        self.0.update(|tray| tray.game = game);
    }
}

impl Tray {
    fn language(&self) -> Language {
        self.language
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
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        log::warn!("tray watcher unavailable, waiting for recovery: {reason:?}");
        // Keep settings reachable when the desktop does not supply a tray host.
        self.online.store(false, Ordering::SeqCst);
        let (online, dispatch) = (self.online.clone(), self.dispatch.clone());
        std::thread::spawn(move || {
            std::thread::sleep(HOST_GRACE);
            if !online.load(Ordering::SeqCst) {
                dispatch(
                    Command::Show(Window::Settings),
                    Source::Tray,
                    Instant::now(),
                    None,
                );
            }
        });
        true
    }

    fn watcher_online(&self) {
        log::info!("tray watcher online; registering the tray again");
        self.online.store(true, Ordering::SeqCst);
    }

    fn id(&self) -> String {
        crate::ipc::RUNTIME_NAME.into()
    }
    fn title(&self) -> String {
        i18n::application_name(self.language())
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let (rgba, width, height) = super::super::icon::rgba(self.enabled);
        // StatusNotifierItem pixmaps are ARGB32 in network byte order.
        let data = rgba
            .as_chunks::<4>()
            .0
            .iter()
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
            title: i18n::application_name(self.language()),
            description: if self.in_development {
                i18n::tray_game_title(self.language(), self.game.1)
            } else {
                String::new()
            },
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
            if item == TrayItem::Quit && self.in_development {
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
        in_development: false,
        language: Language::English,
        game: (0, false),
        dispatch,
        online: Arc::new(AtomicBool::new(true)),
    }
    // GNOME AppIndicators may appear after the application during autostart.
    .assume_sni_available(true)
    .spawn()
    .map(Handle)
}
