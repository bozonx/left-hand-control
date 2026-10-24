use crate::{
    i18n::Msg,
    ui::{Message, NotificationCenter, SettingsWindow, ToastItem, ToastKind},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};

/// Tick of the notification expiry timer.
const EXPIRY_TICK: Duration = Duration::from_millis(100);

const VISIBLE: usize = 4;

/// How long each kind of notification stays on screen.
const SHORT: Duration = Duration::from_secs(2);
const SUCCESS: Duration = Duration::from_secs(3);
const WARNING: Duration = Duration::from_secs(6);
const LONG: Duration = Duration::from_secs(10);

/// Message ids shown as notifications; ids missing here stay silent unless
/// they come from the backend.
const POLICY: &[(&[&str], ToastKind, Duration)] = &[
    (&["copied"], ToastKind::Success, SHORT),
    (
        &[
            "layout-created",
            "layout-saved",
            "layout-deleted",
            "layout-activated",
            "layout-discarded",
            "command-started",
            "mapper-running",
            "mapper-stopped",
        ],
        ToastKind::Success,
        SUCCESS,
    ),
    (
        &[
            "rule-duplicate-trigger",
            "rule-shadowed-trigger",
            "rule-missing-trigger",
            "config-edit-conflict",
            "library-changed",
            "worker-restarting",
            "commands-disabled",
            "action-unavailable",
            "save-layout-first",
            "layout-slot-empty",
            "select-input-device",
        ],
        ToastKind::Warning,
        WARNING,
    ),
    (
        &[crate::i18n::SAVED_MAPPER_NOT_UPDATED],
        ToastKind::Warning,
        LONG,
    ),
    (
        &[
            "error",
            "action-failed",
            "config-unavailable",
            "worker-unavailable",
            "worker-restart-limit",
            "worker-not-started",
            "worker-error",
            "mapper-failed",
            "mapper-running-error",
            "mapper-stopped-error",
            "config-io",
            "config-parse",
            "config-invalid",
        ],
        ToastKind::Error,
        LONG,
    ),
];

fn policy(message: &Message, backend: bool) -> Option<(ToastKind, Duration)> {
    let id = message.id.as_str();
    if id.is_empty() {
        return None;
    }
    POLICY
        .iter()
        .find(|(ids, _, _)| ids.contains(&id))
        .map(|&(_, kind, duration)| (kind, duration))
        .or(backend.then_some((ToastKind::Error, LONG)))
}

struct Entry {
    item: ToastItem,
    remaining: Duration,
}

#[derive(Default)]
struct Queue {
    entries: VecDeque<Entry>,
    token: i32,
    paused: bool,
}

impl Queue {
    fn add(&mut self, message: Message, backend: bool) -> bool {
        let Some((kind, remaining)) = policy(&message, backend) else {
            return false;
        };
        if let Some(entry) = self.entries.iter_mut().find(|entry| {
            let old = &entry.item.message;
            (old.id == message.id && old.arg == message.arg && old.count == message.count)
                || (kind == ToastKind::Error
                    && entry.item.kind == kind
                    && !message.arg.is_empty()
                    && old.arg == message.arg)
        }) {
            entry.remaining = remaining;
            return false;
        }
        self.token = self.token.wrapping_add(1);
        self.entries.push_back(Entry {
            item: ToastItem {
                token: self.token,
                message,
                kind,
            },
            remaining,
        });
        true
    }

    fn advance(&mut self, elapsed: Duration) -> bool {
        if self.paused {
            return false;
        }
        for entry in self.entries.iter_mut().take(VISIBLE) {
            entry.remaining = entry.remaining.saturating_sub(elapsed);
        }
        let old = self.entries.len();
        self.entries.retain(|entry| !entry.remaining.is_zero());
        old != self.entries.len()
    }

    fn dismiss(&mut self, token: i32) {
        self.entries.retain(|entry| entry.item.token != token);
    }

    fn items(&self) -> ModelRc<ToastItem> {
        ModelRc::new(VecModel::from(
            self.entries
                .iter()
                .take(VISIBLE)
                .map(|entry| entry.item.clone())
                .collect::<Vec<_>>(),
        ))
    }
}

struct State {
    queue: Queue,
    last_tick: Instant,
    timer: slint::Timer,
}

pub(crate) fn report(ui: &SettingsWindow, message: &Msg) -> Message {
    // A successful start or stop is reported by the status line alone.
    let silent = matches!(message, Msg::MapperRunning(None) | Msg::MapperStopped(None));
    let message = message.to_ui();
    if !silent {
        ui.global::<NotificationCenter>()
            .invoke_send(message.clone());
    }
    message
}

pub(crate) fn report_changed(ui: &SettingsWindow, message: &Msg, previous: Message) -> Message {
    let message = message.to_ui();
    if message != previous {
        ui.global::<NotificationCenter>()
            .invoke_send(message.clone());
    }
    message
}

pub(crate) fn send(ui: &SettingsWindow, message: Msg) {
    ui.global::<NotificationCenter>()
        .invoke_send(message.to_ui());
}

pub(crate) fn bind(ui: &SettingsWindow) {
    let center = ui.global::<NotificationCenter>();
    if center.get_initialized() {
        return;
    }
    center.set_initialized(true);
    let state = Rc::new(RefCell::new(State {
        queue: Queue::default(),
        last_tick: Instant::now(),
        timer: slint::Timer::default(),
    }));
    for backend in [false, true] {
        let state = state.clone();
        let weak = ui.as_weak();
        let callback = move |message| {
            let Some(ui) = weak.upgrade() else { return };
            let items = {
                let mut state = state.borrow_mut();
                state
                    .queue
                    .add(message, backend)
                    .then(|| state.queue.items())
            };
            if let Some(items) = items {
                ui.global::<NotificationCenter>().set_items(items);
            }
        };
        if backend {
            center.on_backend(callback);
        } else {
            center.on_send(callback);
        }
    }
    let shared = state.clone();
    let weak = ui.as_weak();
    center.on_dismiss(move |token| {
        let Some(ui) = weak.upgrade() else { return };
        let items = {
            let mut state = shared.borrow_mut();
            state.queue.dismiss(token);
            state.queue.items()
        };
        ui.global::<NotificationCenter>().set_items(items);
    });
    let shared = state.clone();
    center.on_hover(move |hovered| {
        shared.borrow_mut().queue.paused = hovered;
    });
    let weak_state = Rc::downgrade(&state);
    let weak = ui.as_weak();
    state
        .borrow()
        .timer
        .start(slint::TimerMode::Repeated, EXPIRY_TICK, move || {
            let (Some(ui), Some(state)) = (weak.upgrade(), weak_state.upgrade()) else {
                return;
            };
            let items = {
                let mut state = state.borrow_mut();
                let now = Instant::now();
                let elapsed = now.saturating_duration_since(state.last_tick);
                state.last_tick = now;
                (ui.window().is_visible() && state.queue.advance(elapsed))
                    .then(|| state.queue.items())
            };
            if let Some(items) = items {
                ui.global::<NotificationCenter>().set_items(items);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_policy_id_has_a_translated_message() {
        let catalog = include_str!("../ui/i18n.slint");
        for id in POLICY.iter().flat_map(|(ids, _, _)| ids.iter()) {
            assert!(
                catalog.contains(&format!("m.id == \"{id}\"")),
                "{id} is not handled by Locale.text()"
            );
        }
    }

    #[test]
    fn localized_diagnostic_ids_remain_error_notifications() {
        for message in [
            Msg::ConfigIo,
            Msg::ConfigParse,
            Msg::ConfigInvalid,
            Msg::MapperFailed("details".into()),
            Msg::MapperRunning(Some("details".into())),
            Msg::MapperStopped(Some("details".into())),
        ] {
            assert_eq!(
                policy(&message.to_ui(), false),
                Some((ToastKind::Error, Duration::from_secs(10)))
            );
        }
        assert_eq!(
            policy(&Msg::MapperRunning(None).to_ui(), false),
            Some((ToastKind::Success, Duration::from_secs(3)))
        );
    }

    #[test]
    fn errors_outlive_success_and_hover_pauses_the_stack() {
        let mut queue = Queue::default();
        queue.add(Msg::Copied.to_ui(), false);
        queue.add(Msg::Error("failure".into()).to_ui(), false);
        queue.paused = true;
        assert!(!queue.advance(Duration::from_secs(3)));
        assert_eq!(queue.entries[0].remaining, Duration::from_secs(2));
        queue.paused = false;
        assert!(queue.advance(Duration::from_secs(2)));
        assert_eq!(queue.entries.len(), 1);
        assert!(queue.advance(Duration::from_secs(8)));
        assert!(queue.entries.is_empty());
    }

    #[test]
    fn repeats_refresh_and_dismissed_errors_can_reappear() {
        let mut queue = Queue::default();
        let error = Msg::Error("failure".into());
        let first = error.to_ui();
        let repeated = error.to_ui();
        assert_eq!(first, repeated);
        assert!(queue.add(first, false));
        queue.advance(Duration::from_secs(9));
        assert!(!queue.add(repeated, false));
        queue.advance(Duration::from_secs(2));
        assert_eq!(queue.entries.len(), 1);
        queue.dismiss(queue.entries[0].item.token);
        assert!(queue.add(error.to_ui(), false));
        assert_eq!(queue.entries.len(), 1);
    }

    #[test]
    fn waiting_notifications_get_their_full_duration_when_visible() {
        let mut queue = Queue::default();
        for index in 0..=VISIBLE {
            queue.add(Msg::LayoutSaved(index.to_string()).to_ui(), false);
        }
        assert!(queue.advance(Duration::from_secs(3)));
        assert_eq!(queue.entries.len(), 1);
        assert_eq!(queue.entries[0].remaining, Duration::from_secs(3));
    }

    #[test]
    fn form_statuses_are_silent_but_blocked_actions_report_errors() {
        assert!(policy(&Msg::TimeoutInvalid.to_ui(), false).is_none());
        assert!(policy(&Msg::RuleSaved.to_ui(), false).is_none());
        assert!(policy(&Msg::ActionRequired.to_ui(), false).is_none());
        assert_eq!(
            policy(&Msg::LoadConfigFirst.to_ui(), true),
            Some((ToastKind::Error, Duration::from_secs(10)))
        );
    }
}
