//! Native popup shortcuts are released while game mode is active.

use crate::command::{Command, Dispatch, Source, Window};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

/// Pause between retries of a failed hotkey registration.
const REGISTRATION_RETRY: Duration = Duration::from_secs(3);

struct Hotkeys {
    manager: GlobalHotKeyManager,
    keys: [HotKey; 2],
    registered: [bool; 2],
    _retry: slint::Timer,
}

thread_local! {
    static HOTKEYS: RefCell<Option<Hotkeys>> = const { RefCell::new(None) };
}

pub fn set_game_active(active: bool) {
    HOTKEYS.with_borrow_mut(|state| {
        let Some(state) = state else { return };
        for (key, registered) in state.keys.iter().zip(&mut state.registered) {
            if *registered != active {
                continue;
            }
            let result = if active {
                state.manager.unregister(*key)
            } else {
                state.manager.register(*key)
            };
            match result {
                Ok(()) => *registered = !active,
                Err(error) => log::warn!("native popup shortcut: {error}"),
            }
        }
    });
}

pub fn start(dispatch: Dispatch) {
    if !lhc_core::env::popup_hotkeys_enabled() {
        return;
    }
    let modifiers = Modifiers::CONTROL | Modifiers::ALT;
    let emoji = HotKey::new(Some(modifiers), Code::F11);
    let quick = HotKey::new(Some(modifiers), Code::F12);
    let emoji_id = emoji.id();
    let quick_id = quick.id();
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            log::error!("native hotkeys unavailable: {error}");
            return;
        }
    };
    // Retry temporary OS registration failures, on the UI thread.
    let retry = slint::Timer::default();
    retry.start(slint::TimerMode::Repeated, REGISTRATION_RETRY, || {
        set_game_active(lhc_core::runtime_state::game_mode_active());
    });
    HOTKEYS.with_borrow_mut(|state| {
        *state = Some(Hotkeys {
            manager,
            keys: [emoji, quick],
            registered: [false; 2],
            _retry: retry,
        })
    });
    set_game_active(lhc_core::runtime_state::game_mode_active());
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
        let _ = dispatch(command, Source::Hotkey, Instant::now(), None);
    }));
}
