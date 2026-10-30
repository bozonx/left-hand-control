mod support;
use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model};
use slint_shell::{Document, bind_document, ui::*};
use std::time::Duration;

fn message(id: &str, arg: &str) -> Message {
    Message {
        id: id.into(),
        arg: arg.into(),
        ..Default::default()
    }
}

fn first_dismiss_position(ui: &SettingsWindow) -> slint::LogicalPosition {
    use i_slint_core::{
        accessibility::AccessibleStringProperty, item_tree::ItemRc, items::AccessibleRole,
        window::WindowInner,
    };
    // Toast height depends on the translation and font metrics.
    let _ = ui.window().take_snapshot().unwrap();
    let mut pending = vec![ItemRc::new_root(
        WindowInner::from_pub(ui.window()).component(),
    )];
    let mut positions = Vec::new();
    while let Some(item) = pending.pop() {
        if item.is_visible()
            && item.accessible_role() == AccessibleRole::Button
            && item
                .accessible_string_property(AccessibleStringProperty::Label)
                .as_deref()
                == Some("Закрыть уведомление")
        {
            let center = item.map_to_window(item.geometry().center());
            positions.push(slint::LogicalPosition::new(center.x, center.y));
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
    }
    positions
        .into_iter()
        .min_by(|a, b| a.y.total_cmp(&b.y))
        .expect("visible notification dismiss button")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    let appearance = std::env::var("LHC_TOAST_APPEARANCE").unwrap_or_default();
    if appearance == "light" || appearance == "eink" {
        let theme = ui.global::<Theme>();
        theme.set_dark(false);
        theme.set_eink(appearance == "eink");
        slint_shell::ui::apply_theme(&theme);
    }
    bind_document(&ui, &document);
    slint_shell::select_ui_language("ru")?;
    ui.show()?;
    // Cold renderer/font initialization must not consume the short toast's lifetime.
    let _ = ui.window().take_snapshot()?;
    // Hover pauses expiry only in the active window; the compositor may not focus a test window.
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
    let center = ui.global::<NotificationCenter>();
    // Time before the first arrival must not count against its lifetime.
    std::thread::sleep(Duration::from_millis(2100));
    center.invoke_send(message("timeout-invalid", ""));
    assert_eq!(center.get_items().row_count(), 0);
    for (id, arg) in [
        ("copied", ""),
        ("layout-saved", "Тест"),
        ("config-edit-conflict", ""),
        ("error", "Проверьте доступ к устройству ввода."),
    ] {
        center.invoke_send(message(id, arg));
    }
    assert_eq!(center.get_items().row_count(), 4);
    support::snapshot(&ui, "notifications-initial");
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::PointerMoved {
            position: first_dismiss_position(&ui),
        });
    slint::platform::update_timers_and_animations();
    let weak = ui.as_weak();
    let snapshot_appearance = appearance.clone();
    slint::Timer::single_shot(Duration::from_millis(500), move || {
        let ui = weak.unwrap();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: first_dismiss_position(&ui),
            });
        support::snapshot(&ui, &format!("notifications-{snapshot_appearance}"));
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(1500), move || {
        let ui = weak.unwrap();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: first_dismiss_position(&ui),
            });
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(3500), move || {
        let ui = weak.unwrap();
        let center = ui.global::<NotificationCenter>();
        let ids = center
            .get_items()
            .iter()
            .map(|item| item.message.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["error", "config-edit-conflict", "copied", "layout-saved"]
        );
        let position = first_dismiss_position(&ui);
        for event in [
            slint::platform::WindowEvent::PointerMoved { position },
            slint::platform::WindowEvent::PointerPressed {
                position,
                button: slint::platform::PointerEventButton::Left,
            },
            slint::platform::WindowEvent::PointerReleased {
                position,
                button: slint::platform::PointerEventButton::Left,
            },
        ] {
            ui.window().dispatch_event(event);
        }
        assert_eq!(center.get_items().row_count(), 3);
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(0.0, 0.0),
            });
        center.invoke_send(message("error", "Проверьте доступ к устройству ввода."));
        assert_eq!(center.get_items().row_count(), 4);
        center.invoke_send(message("error", "Проверьте доступ к устройству ввода."));
        assert_eq!(center.get_items().row_count(), 4);
        for _ in 0..400 {
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::Tab.into(),
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: slint::platform::Key::Tab.into(),
                });
            if center.get_focused() {
                break;
            }
        }
        assert!(
            center.get_focused(),
            "notification dismissal must be reachable by Tab"
        );
        let focused = i_slint_core::window::WindowInner::from_pub(ui.window())
            .focus_item
            .borrow()
            .upgrade()
            .expect("notification focus");
        center.invoke_send(message("layout-created", "Queued while reading"));
        let _ = ui.window().take_snapshot().unwrap();
        assert_eq!(
            i_slint_core::window::WindowInner::from_pub(ui.window())
                .focus_item
                .borrow()
                .upgrade(),
            Some(focused),
            "arriving notification preserves focus"
        );
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(9500), move || {
        let ui = weak.unwrap();
        let center = ui.global::<NotificationCenter>();
        assert_eq!(
            center.get_items().row_count(),
            4,
            "keyboard focus pauses expiry"
        );
        for _ in 0..20 {
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::Tab.into(),
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: slint::platform::Key::Tab.into(),
                });
            if !center.get_focused() {
                break;
            }
        }
        assert!(!center.get_focused());
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(17000), move || {
        let ui = weak.unwrap();
        let center = ui.global::<NotificationCenter>();
        support::wait_until("success and warning notifications to expire", || {
            let items = center.get_items();
            items.row_count() == 1 && items.row_data(0).unwrap().message.id == "error"
        });
        assert_eq!(
            center
                .get_items()
                .iter()
                .map(|item| item.message.id)
                .collect::<Vec<_>>(),
            ["error"]
        );
        assert_eq!(center.get_items().row_data(0).unwrap().message.id, "error");
        for item in center.get_items().iter().collect::<Vec<_>>() {
            center.invoke_dismiss(item.token);
        }
        for slot in 1..=4 {
            center.invoke_send(Message {
                id: "layout-slot-empty".into(),
                count: slot,
                ..Default::default()
            });
        }
        center.invoke_backend(message("mapper-failed", ""));
        assert_eq!(
            center.get_items().row_data(0).unwrap().message.id,
            "mapper-failed"
        );
        for id in ["worker-error", "worker-unavailable", "worker-restart-limit"] {
            center.invoke_backend(message(id, ""));
            assert_eq!(
                center
                    .get_items()
                    .iter()
                    .filter(|item| item.message.id.starts_with("worker-"))
                    .count(),
                1
            );
        }
        center.invoke_worker_recovered();
        assert!(
            !center
                .get_items()
                .iter()
                .any(|item| item.message.id.starts_with("worker-"))
        );
        ui.hide().unwrap();
        center.invoke_send(message("layout-created", "Hidden success"));
        assert!(
            !center
                .get_items()
                .iter()
                .any(|item| item.message.arg == "Hidden success")
        );
        ui.show().unwrap();
        while let Some(item) = center.get_items().row_data(0) {
            center.invoke_dismiss(item.token);
        }
        document
            .edit(slint_shell::document::View::Library, |config| {
                config.update_layout(|layout| {
                    layout.rules = vec![
                        lhc_core::profile::model::KeyRule::new("draft".into(), ""),
                        lhc_core::profile::model::KeyRule::new("other".into(), "F13"),
                    ];
                })
            })
            .unwrap();
        let rules = ui.global::<RulesEditor>();
        assert_eq!(
            rules.get_rows().row_data(0).unwrap().diagnostic.id,
            "rule-missing-trigger"
        );
        rules.invoke_select(1);
        rules.invoke_set_enabled(false);
        assert_eq!(
            center.get_items().row_count(),
            0,
            "an unrelated edit must not repeat an existing rule warning"
        );
        assert_eq!(
            rules.get_rows().row_data(0).unwrap().diagnostic.id,
            "rule-missing-trigger"
        );
        ui.set_page(Page::Rules);
        support::snapshot(&ui, &format!("notifications-rule-diagnostic-{appearance}"));
        rules.invoke_select(0);
        rules.set_field(RuleDialog::Key);
        rules.invoke_choose("F14".into());
        assert!(
            rules
                .get_rows()
                .row_data(0)
                .unwrap()
                .diagnostic
                .id
                .is_empty()
        );
        println!(
            "Notifications passed: stack, field validation, independent expiry, hover pause, dismissal and repeated errors"
        );
        slint::quit_event_loop().unwrap();
    });
    ui.run()?;
    Ok(())
}
