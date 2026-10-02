use crate::{
    command::{Dispatch, Source},
    i18n::{Language, TrayItem},
};
use std::{collections::HashMap, time::Instant};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

pub struct Handle {
    icon: TrayIcon,
    items: Vec<(TrayItem, MenuItem)>,
}

fn icon(enabled: bool) -> Result<Icon, tray_icon::BadIcon> {
    let (rgba, width, height) = super::super::icon::rgba(enabled);
    Icon::from_rgba(rgba, width, height)
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
        for (entry, item) in &self.items {
            item.set_text(entry.label(if english {
                Language::English
            } else {
                Language::Russian
            }));
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
    menu.append_items(
        &items
            .iter()
            .map(|(_, item)| item as &dyn tray_icon::menu::IsMenuItem)
            .collect::<Vec<_>>(),
    )?;
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
    Ok(Handle { icon, items })
}
