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

fn last_dismiss_position(ui: &SettingsWindow) -> slint::LogicalPosition {
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
        .max_by(|a, b| a.y.total_cmp(&b.y))
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
    let center = ui.global::<NotificationCenter>();
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
    ui.show()?;
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(500), move || {
        let ui = weak.unwrap();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: last_dismiss_position(&ui),
            });
        if let Some(dir) = std::env::var_os("LHC_EDITOR_SNAPSHOTS") {
            let pixels = ui.window().take_snapshot().unwrap();
            let mut bytes =
                format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
            for pixel in pixels.as_slice() {
                bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b]);
            }
            std::fs::write(
                std::path::PathBuf::from(dir).join(format!("notifications-{appearance}.ppm")),
                bytes,
            )
            .unwrap();
        }
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(1500), move || {
        let ui = weak.unwrap();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                position: last_dismiss_position(&ui),
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
            ["copied", "layout-saved", "config-edit-conflict", "error"]
        );
        let position = last_dismiss_position(&ui);
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
    });
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(9500), move || {
        let ui = weak.unwrap();
        let center = ui.global::<NotificationCenter>();
        assert_eq!(
            center
                .get_items()
                .iter()
                .map(|item| item.message.id)
                .collect::<Vec<_>>(),
            ["error"]
        );
        assert_eq!(center.get_items().row_data(0).unwrap().message.id, "error");
        println!(
            "Notifications passed: stack, field validation, independent expiry, hover pause, dismissal and repeated errors"
        );
        slint::quit_event_loop().unwrap();
    });
    ui.run()?;
    Ok(())
}
