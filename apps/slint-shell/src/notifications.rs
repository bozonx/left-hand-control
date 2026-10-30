//! Toast notifications of the settings window: which messages are shown, for
//! how long, and the visible queue.

use crate::{
    i18n::Msg,
    ui::{Message, NotificationCenter, SettingsWindow, ToastItem, ToastKind},
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};

/// Tick of the notification expiry timer.
const EXPIRY_TICK: Duration = Duration::from_millis(100);

const VISIBLE: usize = 4;
const CAPACITY: usize = 32;

/// Success lifetimes; warning/error durations are retained for policy metadata.
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
            "auto-rule-deleted",
            "layout-activated",
            "layout-discarded",
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
            "action-unavailable-named",
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
        if let Some(index) = self.entries.iter().position(|entry| {
            let old = &entry.item.message;
            old == &message
                || (kind == ToastKind::Error
                    && entry.item.kind == kind
                    && matches!(message.id.as_str(), "error" | "action-failed")
                    && matches!(old.id.as_str(), "error" | "action-failed")
                    && !message.diagnostic_key.is_empty()
                    && old.diagnostic_key == message.diagnostic_key)
        }) {
            self.entries[index].remaining = remaining;
            if index >= VISIBLE && kind == ToastKind::Error {
                if let Some(entry) = self.entries.remove(index) {
                    self.entries.push_front(entry);
                }
                return true;
            }
            return false;
        }
        // A worker incident has one evolving notification, including its final failure.
        if worker_message(&message) {
            if self
                .entries
                .iter()
                .any(|entry| entry.item.message.id == "worker-restart-limit")
            {
                return false;
            }
            self.entries
                .retain(|entry| !worker_message(&entry.item.message));
        }
        // New problems must be visible even when older problems await dismissal.
        if self.entries.len() == CAPACITY {
            if kind == ToastKind::Success
                || (kind == ToastKind::Warning
                    && self
                        .entries
                        .back()
                        .is_some_and(|entry| entry.item.kind == ToastKind::Error))
            {
                return false;
            }
            self.entries.pop_back();
        }
        self.token = self.token.wrapping_add(1);
        let entry = Entry {
            item: ToastItem {
                token: self.token,
                message,
                kind,
            },
            remaining,
        };
        if kind == ToastKind::Success {
            self.entries.push_back(entry);
        } else {
            let rank = |kind| match kind {
                ToastKind::Error => 2,
                ToastKind::Warning => 1,
                _ => 0,
            };
            let index = self
                .entries
                .iter()
                .position(|old| rank(old.item.kind) <= rank(kind))
                .unwrap_or(self.entries.len());
            self.entries.insert(index, entry);
        }
        true
    }

    fn advance(&mut self, elapsed: Duration) -> bool {
        if self.paused {
            return false;
        }
        for entry in self.entries.iter_mut().take(VISIBLE) {
            if entry.item.kind == ToastKind::Success {
                entry.remaining = entry.remaining.saturating_sub(elapsed);
            }
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

// Keep surviving rows alive so an arriving/expired toast cannot destroy keyboard focus.
fn publish_items(ui: &SettingsWindow, items: ModelRc<ToastItem>) {
    let center = ui.global::<NotificationCenter>();
    let current = center.get_items();
    let Some(model) = current.as_any().downcast_ref::<VecModel<ToastItem>>() else {
        center.set_items(items);
        return;
    };
    for index in (0..model.row_count()).rev() {
        if let Some(old) = model.row_data(index)
            && !items.iter().any(|item| item.token == old.token)
        {
            model.remove(index);
        }
    }
    for (index, item) in items.iter().enumerate() {
        if model
            .row_data(index)
            .is_some_and(|old| old.token == item.token)
        {
            continue;
        }
        model.insert(index, item);
    }
}

struct State {
    queue: Queue,
    last_tick: Instant,
    timer: slint::Timer,
}

fn worker_message(message: &Message) -> bool {
    message.id.starts_with("worker-")
}

pub(crate) fn clear_worker(ui: &SettingsWindow) {
    ui.global::<NotificationCenter>().invoke_worker_recovered();
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
            if !ui.window().is_visible()
                && policy(&message, backend).is_some_and(|(kind, _)| kind == ToastKind::Success)
            {
                return;
            }
            let items = {
                let mut state = state.borrow_mut();
                // Charge elapsed time to existing rows before adding a new one. Cold
                // renderer initialization can delay the timer's very first tick.
                let now = Instant::now();
                let elapsed = now.saturating_duration_since(state.last_tick);
                state.last_tick = now;
                let expired = !ui.global::<NotificationCenter>().get_focused()
                    && state.queue.advance(elapsed);
                let added = state.queue.add(message, backend);
                (expired || added).then(|| state.queue.items())
            };
            if let Some(items) = items {
                publish_items(&ui, items);
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
    center.on_worker_recovered(move || {
        let Some(ui) = weak.upgrade() else { return };
        let items = {
            let mut state = shared.borrow_mut();
            state
                .queue
                .entries
                .retain(|entry| !worker_message(&entry.item.message));
            state.queue.items()
        };
        publish_items(&ui, items);
    });
    let shared = state.clone();
    let weak = ui.as_weak();
    center.on_dismiss(move |token| {
        let Some(ui) = weak.upgrade() else { return };
        let items = {
            let mut state = shared.borrow_mut();
            state.queue.dismiss(token);
            state.queue.items()
        };
        if ui.global::<NotificationCenter>().get_focused_token() == token {
            ui.global::<NotificationCenter>().set_focused_token(0);
        }
        publish_items(&ui, items);
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
                let changed = if ui.window().is_visible() {
                    !ui.global::<NotificationCenter>().get_focused() && state.queue.advance(elapsed)
                } else {
                    let old = state.queue.entries.len();
                    state
                        .queue
                        .entries
                        .retain(|entry| entry.item.kind != ToastKind::Success);
                    old != state.queue.entries.len()
                };
                changed.then(|| state.queue.items())
            };
            if let Some(items) = items {
                publish_items(&ui, items);
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
        assert_eq!(queue.entries[1].remaining, Duration::from_secs(2));
        queue.paused = false;
        assert!(queue.advance(Duration::from_secs(2)));
        assert_eq!(queue.entries.len(), 1);
        assert!(!queue.advance(Duration::from_secs(80)));
        assert_eq!(queue.entries.len(), 1);
        queue.dismiss(queue.entries[0].item.token);
        assert!(queue.entries.is_empty());
    }

    #[test]
    fn warnings_require_dismissal() {
        let mut queue = Queue::default();
        queue.add(Msg::ConfigEditConflict.to_ui(), false);
        assert!(!queue.advance(Duration::from_secs(3600)));
        assert_eq!(queue.entries.len(), 1);
        queue.dismiss(queue.entries[0].item.token);
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
    fn new_errors_preempt_warnings_and_the_queue_is_bounded() {
        let mut queue = Queue::default();
        for slot in 0..(CAPACITY + 10) {
            queue.add(Msg::LayoutSlotEmpty(slot as u32).to_ui(), false);
        }
        assert_eq!(queue.entries.len(), CAPACITY);
        queue.add(Msg::Error("first failure".into()).to_ui(), false);
        assert_eq!(queue.items().row_data(0).unwrap().kind, ToastKind::Error);
        for index in 0..CAPACITY {
            queue.add(Msg::Error(format!("failure {index}")).to_ui(), false);
        }
        assert_eq!(queue.entries.len(), CAPACITY);
        assert!(!queue.add(Msg::Copied.to_ui(), false));
        assert!(!queue.add(Msg::SelectInputDevice.to_ui(), false));
        assert!(
            queue
                .entries
                .iter()
                .all(|entry| entry.item.kind == ToastKind::Error)
        );
    }

    #[test]
    fn worker_incidents_replace_each_other_and_final_failure_is_not_downgraded() {
        let mut queue = Queue::default();
        for message in [
            Msg::WorkerError("send failed".into()),
            Msg::WorkerUnavailable("exited".into()),
            Msg::WorkerRestarting,
            Msg::WorkerRestartLimit,
        ] {
            assert!(queue.add(message.to_ui(), true));
            assert_eq!(queue.entries.len(), 1);
        }
        assert!(!queue.add(Msg::WorkerError("late failure".into()).to_ui(), true));
        assert_eq!(queue.entries[0].item.message.id, "worker-restart-limit");
    }

    #[test]
    fn distinct_diagnostics_remain_separate_and_identical_failures_coalesce() {
        let mut queue = Queue::default();
        assert!(queue.add(Msg::Error("first".into()).to_ui(), false));
        assert!(queue.add(Msg::Error("second".into()).to_ui(), false));
        assert!(!queue.add(Msg::ActionFailed("first".into()).to_ui(), false));
        assert_eq!(queue.entries.len(), 2);
        assert!(
            queue
                .entries
                .iter()
                .all(|entry| entry.item.message.arg.is_empty())
        );
    }

    #[test]
    fn a_repeated_error_waiting_out_of_view_becomes_visible() {
        let mut queue = Queue::default();
        for index in 0..=VISIBLE {
            queue.add(Msg::Error(index.to_string()).to_ui(), false);
        }
        assert!(queue.add(Msg::Error("0".into()).to_ui(), false));
        assert_eq!(queue.entries.len(), VISIBLE + 1);
        assert_eq!(
            queue.entries[0].item.message,
            Msg::Error("0".into()).to_ui()
        );
    }

    #[test]
    fn automatic_rule_deletion_is_confirmed_and_command_success_is_silent() {
        assert!(policy(&Msg::AutoRuleDeleted.to_ui(), false).is_some());
        assert!(policy(&Msg::CommandStarted("script".into()).to_ui(), false).is_none());
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
