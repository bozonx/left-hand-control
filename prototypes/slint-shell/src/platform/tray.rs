use crate::{Command, Dispatch};
use std::{collections::HashMap, sync::Mutex, time::Instant};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

pub struct Handle {
    _icon: TrayIcon,
    enabled: Mutex<bool>,
}

impl Handle {
    pub fn toggle_enabled(&self) {
        if let Ok(mut enabled) = self.enabled.lock() {
            *enabled = !*enabled;
        }
    }
}

pub fn start(dispatch: Dispatch) -> Result<Handle, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let entries = [
        ("settings", "Настройки", Command::Show("settings")),
        ("emoji", "Эмодзи", Command::Show("emoji")),
        ("quick", "Быстрые действия", Command::Show("quick")),
        ("mapper", "Mapper вкл / выкл", Command::ToggleMapper),
        ("quit", "Выход", Command::Quit),
    ];
    let mut commands = HashMap::new();
    let mut items = Vec::new();
    for (id, label, command) in entries {
        let item = MenuItem::with_id(id, label, true, None);
        commands.insert(item.id().clone(), command);
        items.push(item);
    }
    menu.append_items(
        &items
            .iter()
            .map(|item| item as &dyn tray_icon::menu::IsMenuItem)
            .collect::<Vec<_>>(),
    )?;
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(command) = commands.get(&event.id) {
            dispatch(command.clone(), "tray", Instant::now(), None);
        }
    }));
    let icon = Icon::from_rgba(vec![70, 180, 110, 255].repeat(22 * 22), 22, 22)?;
    let icon = TrayIconBuilder::new()
        .with_tooltip("Left Hand Control Slint")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()?;
    Ok(Handle {
        _icon: icon,
        enabled: Mutex::new(true),
    })
}
