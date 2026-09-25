use crate::command::{Command, Dispatch, Source, Window};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use std::time::Instant;

pub fn start(dispatch: Dispatch) {
    if std::env::var("SLINT_SHELL_HOTKEYS").as_deref() == Ok("off") {
        return;
    }
    let modifiers = Modifiers::CONTROL | Modifiers::ALT;
    let emoji = HotKey::new(Some(modifiers), Code::F11);
    let quick = HotKey::new(Some(modifiers), Code::F12);
    let emoji_id = emoji.id();
    let quick_id = quick.id();
    let manager = match GlobalHotKeyManager::new()
        .and_then(|manager| manager.register_all(&[emoji, quick]).map(|_| manager))
    {
        Ok(manager) => manager,
        Err(error) => {
            log::error!("native hotkeys unavailable: {error}");
            return;
        }
    };
    Box::leak(Box::new(manager));
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state != HotKeyState::Pressed {
            return;
        }
        let command = if event.id == emoji_id {
            Command::Show(Window::EMOJI)
        } else if event.id == quick_id {
            Command::Show(Window::QUICK)
        } else {
            return;
        };
        dispatch(command, Source::Hotkey, Instant::now(), None);
    }));
}
