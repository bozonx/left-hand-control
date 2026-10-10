use crate::{
    command::{Command, Dispatch, Source},
    i18n::{self, Language, TrayItem},
};
use lhc_core::gamemode::GameModeStatus;
use std::{cell::Cell, collections::HashMap, time::Instant};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu},
};

pub struct Handle {
    icon: TrayIcon,
    items: Vec<(TrayItem, MenuItem)>,
    game: Submenu,
    game_items: Vec<CheckMenuItem>,
    english: Cell<bool>,
    game_active: Cell<bool>,
}

fn icon(enabled: bool) -> Result<Icon, tray_icon::BadIcon> {
    let (rgba, width, height) = super::super::icon::rgba(enabled);
    Icon::from_rgba(rgba, width, height)
}

fn language(english: bool) -> Language {
    if english {
        Language::English
    } else {
        Language::Russian
    }
}

impl Handle {
    pub fn set_enabled(&self, enabled: bool) {
        match icon(enabled) {
            Ok(icon) => {
                if let Err(error) = self.icon.set_icon(Some(icon)) {
                    log::warn!("tray icon: {error}");
                }
            }
            Err(error) => log::warn!("tray icon: {error}"),
        }
    }

    pub fn set_english(&self, english: bool) {
        self.english.set(english);
        let language = language(english);
        for (entry, item) in &self.items {
            item.set_text(entry.label(language));
        }
        for (mode, item) in i18n::TRAY_GAME_MODES.iter().zip(&self.game_items) {
            item.set_text(i18n::tray_game_choice(language, *mode));
        }
        self.set_game_title();
    }

    pub fn set_game(&self, status: &GameModeStatus) {
        let selected = i18n::tray_game_index(status.control);
        for (index, item) in self.game_items.iter().enumerate() {
            item.set_checked(index == selected);
        }
        self.game_active.set(status.active);
        self.set_game_title();
    }

    fn set_game_title(&self) {
        let title = i18n::tray_game_title(language(self.english.get()), self.game_active.get());
        self.game.set_text(&title);
        if let Err(error) = self
            .icon
            .set_tooltip(Some(format!("Left Hand Control\n{title}")))
        {
            log::warn!("tray tooltip: {error}");
        }
    }
}

pub fn start(dispatch: Dispatch) -> Result<Handle, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let mut commands = HashMap::new();
    let mut items = Vec::new();
    for entry in TrayItem::ALL {
        let item = MenuItem::new(entry.label(Language::Russian), true, None);
        commands.insert(item.id().clone(), entry.command());
        items.push((entry, item));
    }
    let game = Submenu::new(i18n::tray_game_title(Language::Russian, false), true);
    let mut game_items = Vec::new();
    for (index, mode) in i18n::TRAY_GAME_MODES.into_iter().enumerate() {
        let item = CheckMenuItem::new(
            i18n::tray_game_choice(Language::Russian, mode),
            true,
            index == 0,
            None,
        );
        commands.insert(item.id().clone(), Command::GameMode(mode));
        game.append(&item)?;
        game_items.push(item);
    }
    for (entry, item) in &items {
        if *entry == TrayItem::Quit {
            menu.append(&game)?;
        }
        menu.append(item)?;
    }
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(command) = commands.get(&event.id) {
            dispatch(command.clone(), Source::Tray, Instant::now(), None);
        }
    }));
    let icon = TrayIconBuilder::new()
        .with_tooltip("Left Hand Control")
        .with_icon(icon(false)?)
        .with_menu(Box::new(menu))
        .build()?;
    Ok(Handle {
        icon,
        items,
        game,
        game_items,
        english: Cell::new(false),
        game_active: Cell::new(false),
    })
}
