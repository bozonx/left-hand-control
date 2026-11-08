//! Trigger-page regression scenario with real popup, pointer and keyboard input.
mod support;

use i_slint_core::{
    accessibility::AccessibleStringProperty, item_tree::ItemRc, items::AccessibleRole,
    window::WindowInner,
};
use lhc_core::config::model::{KeyRule, Layer};
use slint::{ComponentHandle, LogicalPosition, platform::WindowEvent};
use slint_shell::{document::View, ui::*};
use support::{Fixture, accessible_position};

fn click(ui: &SettingsWindow, position: LogicalPosition) {
    for event in [
        WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        },
        WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        },
    ] {
        ui.window().dispatch_event(event);
    }
    ui.window().take_snapshot().unwrap();
}

fn click_control(ui: &SettingsWindow, role: AccessibleRole, label: &str) {
    click(ui, accessible_position(ui, role, label, false));
}

fn assert_suppressed_hold_choices(ui: &SettingsWindow) {
    ui.window().take_snapshot().unwrap();
    let mut pending = vec![ItemRc::new_root(
        WindowInner::from_pub(ui.window()).component(),
    )];
    let mut suppressed = false;
    while let Some(item) = pending.pop() {
        if item.is_visible() && item.accessible_role() == AccessibleRole::RadioButton {
            let label = item.accessible_string_property(AccessibleStringProperty::Label);
            assert_ne!(label.as_deref(), Some("Trigger itself"));
            if label.as_deref() == Some("Don't press") {
                assert_eq!(
                    item.accessible_string_property(AccessibleStringProperty::Checked)
                        .as_deref(),
                    Some("true")
                );
                suppressed = true;
            }
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
    }
    assert!(suppressed, "the effective hold output must be shown");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    slint_shell::select_ui_language("en")?;
    fixture.document.edit(View::Library, |config| {
        config.update_profile(|profile| {
            profile.rules = vec![KeyRule::new("rule".into(), "KeyA")];
            profile.layers = vec![Layer {
                id: "nav".into(),
                name: "Navigation".into(),
                description: None,
            }];
        })
    })?;
    let ui = fixture.ui;
    ui.global::<AppState>().set_in_development(true);
    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
    ui.window().set_size(slint::LogicalSize::new(1120.0, 760.0));
    ui.show()?;
    let weak = ui.as_weak();
    let document = fixture.document;
    slint::Timer::single_shot(std::time::Duration::from_millis(200), move || {
        let ui = weak.upgrade().unwrap();
        let editor = ui.global::<RulesEditor>();
        for (dark, eink, contrast) in [
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let theme = ui.global::<Theme>();
            theme.set_dark(dark);
            theme.set_eink(eink);
            theme.set_high_contrast(contrast);
            slint_shell::ui::apply_theme(&theme);
            for escape in [false, true] {
                click_control(&ui, AccessibleRole::Combobox, "When held");
                assert_eq!(editor.get_dialog(), RuleDialog::Layer);
                if escape {
                    ui.window().dispatch_event(WindowEvent::KeyPressed {
                        text: slint::platform::Key::Escape.into(),
                    });
                    ui.window().dispatch_event(WindowEvent::KeyReleased {
                        text: slint::platform::Key::Escape.into(),
                    });
                    ui.window().take_snapshot().unwrap();
                } else {
                    click(&ui, LogicalPosition::new(20.0, 20.0));
                }
                assert_eq!(
                    editor.get_dialog(),
                    RuleDialog::None,
                    "dismissal must release modal state"
                );
            }
        }
        document
            .edit(View::Library, |config| {
                config.update_profile(|profile| {
                    profile.rules[0].tap_action = None;
                    profile.rules[0].hold_action = Some(String::new());
                })
            })
            .unwrap();
        click_control(&ui, AccessibleRole::Button, "Advanced rule options");
        assert_eq!(editor.get_dialog(), RuleDialog::Advanced);
        assert_suppressed_hold_choices(&ui);
        click_control(&ui, AccessibleRole::Button, "Cancel");
        document
            .edit(View::Library, |config| {
                config.update_profile(|profile| {
                    profile.rules[0].key.clear();
                })
            })
            .unwrap();
        click_control(&ui, AccessibleRole::Button, "Advanced rule options");
        assert_eq!(
            editor.get_dialog(),
            RuleDialog::Advanced,
            "empty drafts must remain editable"
        );
        click_control(&ui, AccessibleRole::Checkbox, "Enabled");
        click_control(&ui, AccessibleRole::Button, "Apply");
        assert!(!document.read().profile().rules[0].is_enabled());
        click_control(&ui, AccessibleRole::Button, "Advanced rule options");
        click_control(&ui, AccessibleRole::Button, "Delete trigger");
        click_control(&ui, AccessibleRole::Button, "Delete");
        assert!(document.read().profile().rules.is_empty());
        println!(
            "Triggers passed: popup outside/Escape dismissal in four themes, effective suppressed hold, empty draft disable/delete"
        );
        slint::quit_event_loop().unwrap();
    });
    ui.run()?;
    drop(fixture.dir);
    Ok(())
}
