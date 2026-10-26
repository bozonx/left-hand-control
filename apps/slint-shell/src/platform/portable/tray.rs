//! System tray icon and menu through `tray-icon`.

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
    menu: Menu,
    in_development: Cell<bool>,
    items: Vec<(TrayItem, MenuItem)>,
    game: Submenu,
    game_items: Vec<CheckMenuItem>,
    language: Cell<Language>,
    game_active: Cell<bool>,
}

fn icon(enabled: bool) -> Result<Icon, tray_icon::BadIcon> {
    let (rgba, width, height) = super::super::icon::rgba(enabled);
    Icon::from_rgba(rgba, width, height)
}

impl Handle {
    pub fn set_in_development(&self, enabled: bool) {
        if self.in_development.replace(enabled) != enabled {
            let result = if enabled {
                self.menu.insert(&self.game, self.items.len() - 1)
            } else {
                self.menu.remove(&self.game)
            };
            if let Err(error) = result {
                log::warn!("tray game menu: {error}");
            }
        }
        self.set_game_title();
    }

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

    pub fn set_language(&self, language: Language) {
        self.language.set(language);
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
        let title = i18n::tray_game_title(self.language.get(), self.game_active.get());
        self.game.set_text(&title);
        let name = i18n::application_name(self.language.get());
        let tooltip = if self.in_development.get() {
            format!("{name}\n{title}")
        } else {
            name
        };
        if let Err(error) = self.icon.set_tooltip(Some(tooltip)) {
            log::warn!("tray tooltip: {error}");
        }
    }
}

pub fn start(dispatch: Dispatch) -> Result<Handle, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let mut commands = HashMap::new();
    let mut items = Vec::new();
    for entry in TrayItem::ALL {
        let item = MenuItem::new(entry.label(Language::English), true, None);
        commands.insert(item.id().clone(), entry.command());
        items.push((entry, item));
    }
    let game = Submenu::new(i18n::tray_game_title(Language::English, false), true);
    let mut game_items = Vec::new();
    for (index, mode) in i18n::TRAY_GAME_MODES.into_iter().enumerate() {
        let item = CheckMenuItem::new(
            i18n::tray_game_choice(Language::English, mode),
            true,
            index == 0,
            None,
        );
        commands.insert(item.id().clone(), Command::GameMode(mode));
        game.append(&item)?;
        game_items.push(item);
    }
    for (_, item) in &items {
        menu.append(item)?;
    }
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(command) = commands.get(&event.id) {
            dispatch(command.clone(), Source::Tray, Instant::now(), None);
        }
    }));
    let icon = TrayIconBuilder::new()
        .with_tooltip(i18n::application_name(Language::English))
        .with_icon(icon(false)?)
        .with_menu(Box::new(menu.clone()))
        .build()?;
    Ok(Handle {
        icon,
        menu,
        in_development: Cell::new(false),
        items,
        game,
        game_items,
        language: Cell::new(Language::English),
        game_active: Cell::new(false),
    })
}
