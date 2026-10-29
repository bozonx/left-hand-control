//! Modal keyboard traversal and restoration of each dialog's initiator.

use crate::ui::{KeyboardFocus, SettingsWindow};
use i_slint_core::{
    accessibility::{AccessibleStringProperty, nearest_accessible_item},
    input::FocusReason,
    item_tree::{ItemRc, ItemWeak, ParentItemTraversalMode},
    window::{PopupWindowLocation, WindowInner},
};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

struct Dialog {
    token: i32,
    boundary: ItemWeak,
    initiator: ItemWeak,
    neighbors: [ItemWeak; 2],
}

#[derive(Default)]
struct State {
    next_token: i32,
    pending: ItemWeak,
    neighbors: [ItemWeak; 2],
    dialogs: Vec<Dialog>,
    timer: slint::Timer,
}

fn restore(ui: &SettingsWindow, state: &mut State) {
    let mut target = None;
    let mut neighbors = None;
    while state.dialogs.last().is_some_and(|dialog| {
        dialog
            .boundary
            .upgrade()
            .is_none_or(|item| !item.is_visible())
    }) {
        if let Some(dialog) = state.dialogs.pop() {
            target = Some(dialog.initiator);
            neighbors = Some(dialog.neighbors);
        }
    }
    ui.global::<KeyboardFocus>()
        .set_active(!state.dialogs.is_empty());
    let window = WindowInner::from_pub(ui.window());
    window.set_modal_focus_boundaries(
        state
            .dialogs
            .iter()
            .map(|dialog| dialog.boundary.clone())
            .collect(),
    );
    if let Some(item) = target
        .and_then(|item| item.upgrade())
        .filter(ItemRc::is_visible)
    {
        window.set_focus_item(&item, true, FocusReason::Programmatic);
    } else if let Some(neighbors) = neighbors {
        let boundary = state
            .dialogs
            .last()
            .and_then(|dialog| dialog.boundary.upgrade());
        for item in neighbors
            .into_iter()
            .filter_map(|item| item.upgrade())
            .filter(ItemRc::is_visible)
        {
            let mut parent = Some(item.clone());
            while let Some(current) = parent {
                if boundary
                    .as_ref()
                    .is_none_or(|boundary| *boundary == current)
                {
                    window.set_focus_item(&item, true, FocusReason::TabNavigation);
                    return;
                }
                parent = current.parent_item(ParentItemTraversalMode::FindAllParents);
            }
        }
        if let Some(boundary) = boundary {
            window.focus_within(&boundary, false);
        } else {
            window.focus_next_item();
        }
    }
}

pub(super) fn bind(ui: &SettingsWindow) {
    let state = Rc::new(RefCell::new(State::default()));
    let focus = ui.global::<KeyboardFocus>();
    on!(ui, focus.on_has_focus => |ui| {
        let window = WindowInner::from_pub(ui.window());
        window.ensure_tree_instantiated();
        window.focus_item.borrow().upgrade().is_some_and(|item| item.is_visible())
    });
    on!(ui, focus.on_describe => |ui| {
        let Some(item) = WindowInner::from_pub(ui.window()).focus_item.borrow().upgrade() else {
            return Default::default();
        };
        let item = nearest_accessible_item(item);
        let mut lines = Vec::new();
        for property in [AccessibleStringProperty::Label, AccessibleStringProperty::Value, AccessibleStringProperty::Description] {
            if let Some(value) = item.accessible_string_property(property).filter(|value| !value.is_empty()) {
                let value = value.to_string();
                if !lines.contains(&value) { lines.push(value); }
            }
        }
        lines.join("\n").into()
    });
    on!(ui, focus.on_save, state = state => |ui| {
        let mut state = state.borrow_mut();
        restore(&ui, &mut state);
        state.pending = WindowInner::from_pub(ui.window()).focus_item.borrow().clone();
        state.neighbors = state.pending.upgrade().map(|item| [item.next_focus_item().downgrade(), item.previous_focus_item().downgrade()]).unwrap_or_default();
    });
    on!(ui, focus.on_enter, state = state => |ui| {
        let mut focus_state = state.borrow_mut();
        let boundary = WindowInner::from_pub(ui.window()).focus_item.borrow().clone();
        let initiator = focus_state.pending.clone();
        let neighbors = focus_state.neighbors.clone();
        focus_state.next_token += 1;
        let token = focus_state.next_token;
        focus_state.dialogs.push(Dialog { token, boundary, initiator, neighbors });
        ui.global::<KeyboardFocus>().set_active(true);
        WindowInner::from_pub(ui.window()).set_modal_focus_boundaries(focus_state.dialogs.iter().map(|dialog| dialog.boundary.clone()).collect());
        drop(focus_state);
        let weak_ui = ui.as_weak();
        let weak_state = Rc::downgrade(&state);
        state.borrow().timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                if let (Some(ui), Some(state)) = (weak_ui.upgrade(), weak_state.upgrade()) {
                    let mut state = state.borrow_mut();
                    restore(&ui, &mut state);
                    if state.dialogs.is_empty() { state.timer.stop(); }
                }
            },
        );
        token
    });
    on!(ui, focus.on_is_top, state = state => |_ui, token| {
        state.borrow().dialogs.last().is_some_and(|dialog| dialog.token == token)
    });
    on!(ui, focus.on_cycle, state = state => |ui, backwards| {
        let mut state = state.borrow_mut();
        restore(&ui, &mut state);
        if let Some(boundary) = state.dialogs.last().and_then(|dialog| dialog.boundary.upgrade()) {
            WindowInner::from_pub(ui.window()).focus_within(&boundary, backwards);
        }
    });
    on!(ui, focus.on_focus_checked => |ui| {
        let window = WindowInner::from_pub(ui.window());
        let root = window.active_popups.borrow().last().map(|popup| ItemRc::new_root(popup.component.clone()));
        let mut pending: Vec<_> = root.into_iter().collect();
        while let Some(item) = pending.pop() {
            if item.accessible_string_property(AccessibleStringProperty::Checked).as_deref() == Some("true") {
                window.set_focus_item(&item, true, FocusReason::Programmatic);
                return;
            }
            let mut child = item.first_child();
            while let Some(item) = child {
                child = item.next_sibling();
                pending.push(item);
            }
        }
    });
    on!(ui, focus.on_step => |ui, backwards| {
        let window = WindowInner::from_pub(ui.window());
        let popup = window.active_popups.borrow().last().and_then(|popup| match &popup.location {
            PopupWindowLocation::TopLevel(adapter) => Some(adapter.clone()),
            PopupWindowLocation::ChildWindow(_) => None,
        });
        let window = popup.as_ref().map_or(window, |adapter| WindowInner::from_pub(adapter.window()));
        if backwards { window.focus_previous_item(); } else { window.focus_next_item(); }
    });
}
