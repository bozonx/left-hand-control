//! Contrast themes and responsive held-layer hint, using disposable configuration.
mod support;
use i_slint_core::{
    item_tree::ItemRc,
    items::{AccessibleRole, ComplexText, SimpleText, TextInput},
    window::WindowInner,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use slint_shell::ui::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use support::{Fixture, snapshot};

fn contrast(a: slint::Color, b: slint::Color) -> f64 {
    let luminance = |c: slint::Color| {
        let linear = |v: u8| {
            let v = f64::from(v) / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.red()) + 0.7152 * linear(c.green()) + 0.0722 * linear(c.blue())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn check_hint(hint: &LayerHintPopup) {
    let _ = hint.window().take_snapshot().unwrap();
    let inner = WindowInner::from_pub(hint.window());
    inner.ensure_tree_instantiated();
    let mut stack = vec![ItemRc::new_root(inner.component())];
    let mut labels = 0;
    let mut actions = 0;
    let size = hint
        .window()
        .size()
        .to_logical(hint.window().scale_factor());
    while let Some(item) = stack.pop() {
        let text = item
            .downcast::<SimpleText>()
            .map(|t| {
                SimpleText::FIELD_OFFSETS
                    .text()
                    .apply_pin(t.as_pin_ref())
                    .get()
            })
            .or_else(|| {
                item.downcast::<ComplexText>().map(|t| {
                    ComplexText::FIELD_OFFSETS
                        .text()
                        .apply_pin(t.as_pin_ref())
                        .get()
                })
            });
        if let Some(text) =
            text.filter(|t| t.starts_with("LH") || t.starts_with("RH") || t == "Ctrl+C")
        {
            if text == "Ctrl+C" {
                actions += 1;
            } else {
                labels += 1;
            }
            let point = item.map_to_window(item.geometry().origin);
            let geometry = item.geometry();
            assert!(
                point.x >= 0.
                    && point.y >= 0.
                    && point.x + geometry.size.width <= size.width + 1.
                    && point.y + geometry.size.height <= size.height + 1.,
                "hint label outside {size:?}: {point:?} {geometry:?}"
            );
            let parent = item
                .parent_item(i_slint_core::item_tree::ParentItemTraversalMode::FindAllParents)
                .unwrap();
            let parent_point = parent.map_to_window(parent.geometry().origin);
            let parent_size = parent.geometry().size;
            assert!(
                point.y >= parent_point.y - 1.
                    && point.y + geometry.size.height <= parent_point.y + parent_size.height + 1.,
                "hint text {text:?} clipped inside its cell: {point:?} {geometry:?}, parent {parent_point:?} {parent_size:?}"
            );
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
    assert_eq!(labels, 60, "both hands must remain visible");
    assert_eq!(actions, 60, "all action captions must remain visible");
}

fn show_widget_state(ui: &SettingsWindow, text_editor: bool) {
    use i_slint_core::{
        accessibility::{AccessibilityAction, nearest_accessible_item},
        input::FocusReason,
    };
    let inner = WindowInner::from_pub(ui.window());
    inner.ensure_tree_instantiated();
    let mut stack = vec![ItemRc::new_root(inner.component())];
    while let Some(item) = stack.pop() {
        if text_editor {
            if let Some(input) = item.downcast::<TextInput>()
                && TextInput::FIELD_OFFSETS
                    .text()
                    .apply_pin(input.as_pin_ref())
                    .get()
                    == "Selected text"
            {
                inner.set_focus_item(&item, true, FocusReason::Programmatic);
                nearest_accessible_item(item)
                    .accessible_action(&AccessibilityAction::SetSelectionOffsets(0, 13));
                return;
            }
        } else if item.is_visible()
            && item.accessible_role() == AccessibleRole::Combobox
            && item.map_to_window(item.geometry().origin).y > 100.
        {
            item.accessible_action(&AccessibilityAction::Expand);
            return;
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
    panic!("widget was not found, text_editor={text_editor}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture
        .document
        .edit(slint_shell::document::View::Library, |config| {
            config.update_layout(|layout| {
                let mut rule = lhc_core::profile::model::KeyRule::new("contrast".into(), "F13");
                rule.enabled = Some(false);
                layout.rules = vec![rule];
            })
        })?;
    let ui = fixture.ui.clone_strong();
    ui.global::<AppState>().set_in_development(true);
    ui.global::<SettingsEditor>().set_in_development(true);
    ui.window().set_size(slint::LogicalSize::new(1120., 760.));
    ui.show()?;
    let hint = LayerHintPopup::new()?;
    hint.set_name("Layer / Слой".into());
    let cells = |hand: &str| {
        ModelRc::new(VecModel::from(
            (0..30)
                .map(|index| LayerKeyCell {
                    index,
                    label: format!("{hand}{index:02}").into(),
                    kind: Assignment::Action,
                    action: "Ctrl+C".into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ))
    };
    hint.set_left_cells(cells("LH"));
    hint.set_right_cells(cells("RH"));
    let step = Rc::new(Cell::new(0));
    let progress = step.clone();
    let weak = ui.as_weak();
    let hint_weak = hint.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(180), move || {
        let ui = weak.unwrap();
        let hint = hint_weak.unwrap();
        let n = step.get();
        if n > 40 { return; }
        if n > 0 {
            if n <= 30 { snapshot(&ui, &format!("contrast-{}", n - 1)); }
            else if n <= 34 { check_hint(&hint); snapshot(&hint, &format!("hint-{}", [1290, 1280, 960, 640][n - 31])); }
            else { snapshot(&ui, &format!("widgets-{}", n - 35)); }
        }
        if n < 30 {
            let theme_index = (n / 3) % 5;
            let theme = ui.global::<Theme>();
            theme.set_dark(theme_index == 0 || theme_index == 2);
            theme.set_high_contrast(theme_index == 2 || theme_index == 3);
            theme.set_eink(theme_index == 4);
            slint_shell::ui::apply_theme(&theme);
            for background in [theme.get_surface(), theme.get_card(), theme.get_hover(), theme.get_selected()] {
                let ratio = contrast(theme.get_placeholder(), background);
                assert!(ratio >= 4.5, "placeholder contrast {ratio}");
            }
            slint_shell::select_ui_language(if n < 15 { "en" } else { "ru" }).unwrap();
            ui.set_page([Page::Settings, Page::Rules, Page::Layouts][n % 3]);
            ui.global::<SettingsEditor>().set_tab(SettingsTab::General);
        } else if n < 34 {
            let size = slint::LogicalSize::new([1290., 1280., 960., 640.][n - 30], 480.);
            hint.window().set_size(size);
            // Mirror the logical Resized event sent by the Spell configure handler.
            hint.window().dispatch_event(slint::platform::WindowEvent::Resized { size });
            hint.show().unwrap();
            ui.hide().unwrap();
        } else if n < 40 {
            ui.show().unwrap();
            hint.hide().unwrap();
            ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: slint::platform::Key::Escape.into() });
            ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: slint::platform::Key::Escape.into() });
            ui.global::<ActionPicker>().invoke_dismiss_picker();
            let variant = (n - 34) % 3;
            let theme = ui.global::<Theme>();
            theme.set_dark(variant == 0);
            theme.set_high_contrast(variant < 2);
            theme.set_eink(variant == 2);
            slint_shell::ui::apply_theme(&theme);
            if n < 37 { ui.set_page(Page::Settings); }
            else {
                ui.set_page(Page::Rules);
                ui.global::<ActionPicker>().invoke_open(PickerTarget::Rule, 0, "text:Selected text".into(), false);
            }
            show_widget_state(&ui, n >= 37);
        } else {
            println!("contrast-scaling: 30 theme/page/language states, 4 hint widths, 480 key/action bounds; HC/E-ink combo popups and focused text selection passed");
            slint::quit_event_loop().unwrap();
        }
        step.set(n + 1);
    });
    slint::run_event_loop()?;
    assert_eq!(progress.get(), 41, "scenario must complete all stages");
    Ok(())
}
