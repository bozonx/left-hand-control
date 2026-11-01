//! Modal keyboard traversal and restoration of each dialog's initiator.

use crate::ui::{KeyboardFocus, NavigationKind, NavigationMove, SettingsWindow, Theme};
use i_slint_core::{
    accessibility::{AccessibleStringProperty, nearest_accessible_item},
    input::FocusReason,
    item_tree::{ItemRc, ItemTreeRc, ItemWeak, ParentItemTraversalMode},
    items::{AccessibleRole, FocusScope},
    window::WindowAdapter,
    window::{PopupWindowLocation, WindowInner},
};
use slint::ComponentHandle;
use slint::winit_030::winit::{
    event::{ElementState, TouchPhase, WindowEvent},
    keyboard::{Key, NamedKey},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

/// Remember whether the last input was keyboard navigation or the pointer, so
/// focus rings appear only for keyboard users (like CSS `:focus-visible`).
/// Typing text does not count: it keeps the current mode.
pub(super) fn track_input_mode(ui: &SettingsWindow, event: &WindowEvent) {
    if let Some(keyboard) = navigation_input_mode(event) {
        ui.global::<Theme>().set_keyboard_navigation(keyboard);
    }
}

fn navigation_key(key: &Key) -> bool {
    matches!(
        key,
        Key::Named(
            NamedKey::Tab
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::ArrowLeft
                | NamedKey::ArrowRight
                | NamedKey::Home
                | NamedKey::End
                | NamedKey::PageUp
                | NamedKey::PageDown
                | NamedKey::Enter
                | NamedKey::Space
                | NamedKey::F1
        )
    ) || matches!(key, Key::Character(text) if text == " ")
}

pub(crate) fn navigation_input_mode(event: &WindowEvent) -> Option<bool> {
    match event {
        WindowEvent::KeyboardInput { event, .. }
            if event.state == ElementState::Pressed && navigation_key(&event.logical_key) =>
        {
            Some(true)
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            ..
        } => Some(false),
        WindowEvent::Touch(touch) if touch.phase == TouchPhase::Started => Some(false),
        _ => None,
    }
}

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

/// Use the native popup's focus tree when a dropdown lives in its own window.
fn focus_window(adapter: Rc<dyn WindowAdapter>) -> Rc<dyn WindowAdapter> {
    let window = WindowInner::from_pub(adapter.window());
    let popup = window
        .active_popups
        .borrow()
        .last()
        .and_then(|popup| match &popup.location {
            PopupWindowLocation::TopLevel(adapter) => Some(adapter.clone()),
            PopupWindowLocation::ChildWindow(_) => None,
        });
    popup.unwrap_or(adapter)
}

fn group_boundary(item: &ItemRc, kind: NavigationKind) -> Option<ItemRc> {
    let role = match kind {
        NavigationKind::HorizontalTabs | NavigationKind::VerticalTabs => AccessibleRole::TabList,
        NavigationKind::Radio => AccessibleRole::RadioGroup,
        _ => AccessibleRole::List,
    };
    let mut parent = item.parent_item(ParentItemTraversalMode::StopAtPopups);
    while let Some(item) = parent {
        if item.accessible_role() == role {
            return Some(item);
        }
        parent = item.parent_item(ParentItemTraversalMode::StopAtPopups);
    }
    None
}

fn descendants(root: &ItemRc) -> Vec<ItemRc> {
    let mut result = Vec::new();
    let mut next = root.first_child();
    while let Some(item) = next {
        result.push(item.clone());
        result.extend(descendants(&item));
        next = item.next_sibling();
    }
    result
}

fn group_targets(boundary: &ItemRc, kind: NavigationKind) -> Vec<ItemRc> {
    descendants(boundary)
        .into_iter()
        .filter(|item| {
            item.is_visible_or_clipped_by_flickable()
                && item
                    .downcast::<FocusScope>()
                    .is_some_and(|scope| scope.as_pin_ref().enabled())
                && if matches!(
                    kind,
                    NavigationKind::HorizontalTabs | NavigationKind::VerticalTabs
                ) {
                    nearest_accessible_item(item.clone()).accessible_role() == AccessibleRole::Tab
                } else {
                    matches!(
                        nearest_accessible_item(item.clone()).accessible_role(),
                        AccessibleRole::Button
                            | AccessibleRole::RadioButton
                            | AccessibleRole::ListItem
                    )
                }
        })
        .collect()
}

fn center(item: &ItemRc) -> (f32, f32) {
    let item = nearest_accessible_item(item.clone());
    let position = item.map_to_window(Default::default());
    let size = item.geometry().size;
    (position.x + size.width / 2., position.y + size.height / 2.)
}

fn grid_distance(along: f32, across: f32, movement: NavigationMove) -> Option<(f32, f32)> {
    let horizontal = matches!(movement, NavigationMove::Previous | NavigationMove::Next);
    // Left/Right stay in the same row. Up/Down choose the next row and then
    // its closest column, including incomplete rows and the gap between hands.
    (along > 1. && (!horizontal || across < 1.)).then_some((along, across))
}

fn navigate(adapter: Rc<dyn WindowAdapter>, kind: NavigationKind, movement: NavigationMove) {
    let adapter = focus_window(adapter);
    let window = WindowInner::from_pub(adapter.window());
    window.ensure_tree_instantiated();
    let Some(current) = window.focus_item.borrow().upgrade() else {
        return;
    };
    let Some(boundary) = group_boundary(&current, kind) else {
        return;
    };
    let targets = group_targets(&boundary, kind);
    let Some(index) = targets.iter().position(|item| *item == current) else {
        return;
    };
    let target = match movement {
        NavigationMove::First => targets.first(),
        NavigationMove::Last => targets.last(),
        _ if kind == NavigationKind::Grid => {
            let (x, y) = center(&current);
            targets
                .iter()
                .filter_map(|item| {
                    let (tx, ty) = center(item);
                    let (along, across) = match movement {
                        NavigationMove::Previous => (x - tx, (y - ty).abs()),
                        NavigationMove::Next => (tx - x, (y - ty).abs()),
                        NavigationMove::Up => (y - ty, (x - tx).abs()),
                        NavigationMove::Down => (ty - y, (x - tx).abs()),
                        _ => return None,
                    };
                    grid_distance(along, across, movement).map(|distance| (item, distance))
                })
                .min_by(|a, b| {
                    a.1.0
                        .total_cmp(&b.1.0)
                        .then_with(|| a.1.1.total_cmp(&b.1.1))
                })
                .map(|(item, _)| item)
        }
        _ => {
            let backwards = matches!(movement, NavigationMove::Previous | NavigationMove::Up);
            let next = if matches!(
                kind,
                NavigationKind::HorizontalTabs
                    | NavigationKind::VerticalTabs
                    | NavigationKind::Menu
                    | NavigationKind::Radio
            ) {
                if backwards {
                    (index + targets.len() - 1) % targets.len()
                } else {
                    (index + 1) % targets.len()
                }
            } else if backwards {
                index.saturating_sub(1)
            } else {
                (index + 1).min(targets.len() - 1)
            };
            targets.get(next)
        }
    };
    if let Some(target) = target {
        window.set_focus_item(target, true, FocusReason::Programmatic);
    }
}

/// Shared by settings, winit popups and the Spell worker. Weak window references
/// avoid keeping a closed component alive through its global callbacks.
pub(crate) fn bind_navigation(focus: &KeyboardFocus<'_>, window: &slint::Window) {
    let window = WindowInner::from_pub(window).window_adapter();
    let weak = Rc::downgrade(&window);
    focus.on_navigate(move |kind, movement| {
        if let Some(window) = weak.upgrade() {
            navigate(window, kind, movement);
        }
    });
    let weak = Rc::downgrade(&window);
    focus.on_leave_group(move |kind, backwards| {
        let Some(window) = weak.upgrade() else { return };
        if kind == NavigationKind::Menu {
            let window = WindowInner::from_pub(window.window());
            window.close_top_popup();
            if backwards {
                window.focus_previous_item();
            } else {
                window.focus_next_item();
            }
            return;
        }
        let adapter = focus_window(window);
        let window = WindowInner::from_pub(adapter.window());
        let Some(current) = window.focus_item.borrow().upgrade() else {
            return;
        };
        let Some(boundary) = group_boundary(&current, kind) else {
            return;
        };
        let targets = group_targets(&boundary, kind);
        if let Some(edge) = if backwards {
            targets.first()
        } else {
            targets.last()
        } {
            window.focus_outside(edge, backwards);
        }
    });
    let weak = Rc::downgrade(&window);
    focus.on_ensure_focus(move || {
        let weak = weak.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            let Some(adapter) = weak.upgrade() else {
                return;
            };
            let window = WindowInner::from_pub(adapter.window());
            window.ensure_tree_instantiated();
            if window
                .focus_item
                .borrow()
                .upgrade()
                .is_none_or(|item| !item.is_visible())
            {
                window.focus_next_item();
            }
        });
    });
    let weak = Rc::downgrade(&window);
    focus.on_remember_reorder(move |from, to| {
        let Some(adapter) = weak.upgrade() else {
            return;
        };
        let window = WindowInner::from_pub(adapter.window());
        let Some(current) = window.focus_item.borrow().upgrade() else {
            return;
        };
        // Rebuild the path from the window root: replacing an outer model can
        // destroy every delegate, including the moved row's former parent.
        let row = ItemRc::new_root(current.item_tree().clone());
        let index_count = ItemTreeRc::borrow_pin(current.item_tree())
            .as_ref()
            .get_item_tree()
            .len();
        let role = nearest_accessible_item(current.clone()).accessible_role();
        let old_focus = current.downgrade();
        let mut path = Vec::new();
        let mut item = current;
        while let Some(parent) = item.parent_item(ParentItemTraversalMode::FindAllParents) {
            let mut sibling = parent.first_child();
            let mut ordinal = 0i32;
            while let Some(candidate) = sibling {
                if candidate == item {
                    break;
                }
                ordinal += 1;
                sibling = candidate.next_sibling();
            }
            if item == row {
                ordinal += to - from;
            }
            let Ok(ordinal) = usize::try_from(ordinal) else {
                return;
            };
            path.push(ordinal);
            item = parent;
        }
        let weak = weak.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            let Some(adapter) = weak.upgrade() else {
                return;
            };
            let window = WindowInner::from_pub(adapter.window());
            window.ensure_tree_instantiated();
            if !window.active_popups.borrow().is_empty() {
                return;
            }
            if let Some(current) = window.focus_item.borrow().upgrade()
                && old_focus.upgrade().as_ref() != Some(&current)
            {
                return;
            }
            let mut target = Some(ItemRc::new_root(window.component()));
            for ordinal in path.into_iter().rev() {
                target = target.and_then(|item| item.first_child());
                for _ in 0..ordinal {
                    target = target.and_then(|item| item.next_sibling());
                }
            }
            let Some(target) = target else { return };
            if ItemTreeRc::borrow_pin(target.item_tree())
                .as_ref()
                .get_item_tree()
                .len()
                != index_count
            {
                return;
            }
            if target.is_visible_or_clipped_by_flickable()
                && target.downcast::<FocusScope>().is_some()
                && nearest_accessible_item(target.clone()).accessible_role() == role
            {
                window.set_focus_item(&target, true, FocusReason::Programmatic);
            }
        });
    });
    let weak = Rc::downgrade(&window);
    focus.on_focus_grid(move || {
        let Some(adapter) = weak.upgrade().map(focus_window) else {
            return;
        };
        let window = WindowInner::from_pub(adapter.window());
        window.ensure_tree_instantiated();
        let root = ItemRc::new_root(window.component());
        let boundary = descendants(&root)
            .into_iter()
            .find(|item| item.accessible_role() == AccessibleRole::List);
        if let Some(boundary) = boundary {
            let targets = group_targets(&boundary, NavigationKind::Grid);
            let target = targets
                .iter()
                .find(|item| {
                    item.downcast::<FocusScope>()
                        .is_some_and(|scope| scope.as_pin_ref().focus_on_tab_navigation())
                })
                .or_else(|| targets.first());
            if let Some(target) = target {
                window.set_focus_item(target, true, FocusReason::Programmatic);
            }
        }
    });
    let weak = Rc::downgrade(&window);
    focus.on_focus_index(move |index| {
        let Some(adapter) = weak.upgrade().map(focus_window) else {
            return;
        };
        let window = WindowInner::from_pub(adapter.window());
        let Some(current) = window.focus_item.borrow().upgrade() else {
            return;
        };
        let Some(boundary) = group_boundary(&current, NavigationKind::Grid) else {
            return;
        };
        if let Ok(index) = usize::try_from(index)
            && let Some(target) = group_targets(&boundary, NavigationKind::Grid).get(index)
        {
            window.set_focus_item(target, true, FocusReason::Programmatic);
        }
    });
    let weak = Rc::downgrade(&window);
    focus.on_find_option(move |prefix| {
        let Some(adapter) = weak.upgrade().map(focus_window) else {
            return;
        };
        let window = WindowInner::from_pub(adapter.window());
        let Some(current) = window.focus_item.borrow().upgrade() else {
            return;
        };
        let Some(boundary) = group_boundary(&current, NavigationKind::Menu) else {
            return;
        };
        let prefix = prefix.to_lowercase();
        if let Some(target) = group_targets(&boundary, NavigationKind::Menu)
            .into_iter()
            .find(|item| {
                nearest_accessible_item(item.clone())
                    .accessible_string_property(AccessibleStringProperty::Label)
                    .is_some_and(|label| label.to_lowercase().starts_with(&prefix))
            })
        {
            window.set_focus_item(&target, true, FocusReason::Programmatic);
        }
    });
}

pub(super) fn bind(ui: &SettingsWindow) {
    let state = Rc::new(RefCell::new(State::default()));
    let focus = ui.global::<KeyboardFocus>();
    bind_navigation(&focus, ui.window());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_keys_include_activation_but_not_typing() {
        for key in [
            NamedKey::Tab,
            NamedKey::ArrowUp,
            NamedKey::ArrowDown,
            NamedKey::ArrowLeft,
            NamedKey::ArrowRight,
            NamedKey::Home,
            NamedKey::End,
            NamedKey::PageUp,
            NamedKey::PageDown,
            NamedKey::Enter,
            NamedKey::Space,
            NamedKey::F1,
        ] {
            assert!(navigation_key(&Key::Named(key)));
        }
        assert!(navigation_key(&Key::Character(" ".into())));
        assert!(!navigation_key(&Key::Character("a".into())));
        assert!(!navigation_key(&Key::Named(NamedKey::Control)));
    }

    #[test]
    fn grid_navigation_does_not_wrap_or_move_diagonally_to_another_row() {
        assert_eq!(grid_distance(-10., 0., NavigationMove::Next), None);
        assert_eq!(grid_distance(50., 46., NavigationMove::Next), None);
        assert_eq!(
            grid_distance(300., 0., NavigationMove::Next),
            Some((300., 0.))
        );
        assert_eq!(
            grid_distance(50., 20., NavigationMove::Down),
            Some((50., 20.))
        );
    }
}
