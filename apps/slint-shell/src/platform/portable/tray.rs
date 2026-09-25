use crate::{
    command::{Dispatch, Source},
    i18n::TrayItem,
};
use std::{collections::HashMap, time::Instant};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

pub struct Handle {
    _icon: TrayIcon,
    items: Vec<(TrayItem, MenuItem)>,
}

impl Handle {
    pub fn set_enabled(&self, _enabled: bool) {}

    pub fn set_english(&self, english: bool) {
        for (entry, item) in &self.items {
            item.set_text(entry.label(english));
        }
    }
}

pub fn start(dispatch: Dispatch) -> Result<Handle, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let mut commands = HashMap::new();
    let mut items = Vec::new();
    for entry in TrayItem::ALL {
        let item = MenuItem::new(entry.label(false), true, None);
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
    let icon = Icon::from_rgba(vec![70, 180, 110, 255].repeat(22 * 22), 22, 22)?;
    let icon = TrayIconBuilder::new()
        .with_tooltip("Left Hand Control Slint")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()?;
    Ok(Handle { _icon: icon, items })
}
