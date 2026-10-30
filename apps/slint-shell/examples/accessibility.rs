//! Accessible focus, numeric text interface and selection regression scenario.
use i_slint_core::accessibility::AccessibilityAction;
use i_slint_core::{
    accessibility::{
        AccessibleStringProperty as P, find_exposed_text_input, nearest_accessible_item,
    },
    item_tree::ItemRc,
    items::AccessibleRole,
    window::WindowInner,
};
use slint::ComponentHandle;

slint::slint! {
    import { NumericInput, InlineTextField, ButtonGroup, NavTab, Section } from "../ui/controls.slint";
    export component Probe inherits Window {
        title: "LHC accessibility fixture";
        width: 640px; height: 400px;
        in-out property <string> number: "20";
        in-out property <string> numeric-placeholder;
        in-out property <bool> numeric-read-only;
        in-out property <bool> numeric-enabled: true;
        in-out property <string> name: "Original";
        in-out property <int> selection: 0;
        VerticalLayout {
            Section { title: "Fixture section";
                NumericInput { placeholder-text: root.numeric-placeholder; read-only: root.numeric-read-only; enabled: root.numeric-enabled; label: "Fixture number"; text <=> root.number; minimum: 10; maximum: 30; step: 5; }
                InlineTextField { label: "Fixture name"; text: root.name; saved(value) => { root.name = value; return true; } }
                ButtonGroup { labels: ["First", "Second"]; selected-index: root.selection; selected(index) => { root.selection = index; } }
                NavTab { text: "Fixture tab"; selected: true; }
            }
        }
    }
}

fn key(ui: &Probe, text: impl Into<slint::SharedString>) {
    let text = text.into();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
}
fn focus(ui: &Probe) -> ItemRc {
    nearest_accessible_item(
        WindowInner::from_pub(ui.window())
            .focus_item
            .borrow()
            .upgrade()
            .expect("focused item"),
    )
}
fn tab_to(ui: &Probe, name: &str) -> ItemRc {
    for _ in 0..40 {
        key(ui, slint::platform::Key::Tab);
        let item = focus(ui);
        if item.accessible_string_property(P::Label).as_deref() == Some(name) {
            return item;
        }
    }
    panic!("unreachable field: {name}");
}
fn nodes(ui: &Probe) -> Vec<ItemRc> {
    let _ = ui.window().take_snapshot().unwrap();
    let mut pending = vec![ItemRc::new_root(
        WindowInner::from_pub(ui.window()).component(),
    )];
    let mut result = vec![];
    while let Some(item) = pending.pop() {
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
        if item.is_visible() && item.is_accessible() {
            result.push(item);
        }
    }
    result
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = Probe::new()?;
    ui.show()?;
    if std::env::var_os("LHC_A11Y_INSPECT").is_some() {
        slint::Timer::single_shot(std::time::Duration::from_secs(20), || {
            slint::quit_event_loop().unwrap();
        });
        ui.run()?;
        return Ok(());
    }
    let weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(250), move || {
        let ui = weak.unwrap();
        let numeric = tab_to(&ui, "Fixture number");
        assert_eq!(numeric.accessible_role(), AccessibleRole::Spinbox);
        assert!(
            find_exposed_text_input(&numeric).is_some(),
            "spinbox must retain native text selection/cursor"
        );
        assert_eq!(
            nodes(&ui)
                .iter()
                .filter(
                    |i| i.accessible_string_property(P::Label).as_deref() == Some("Fixture number")
                )
                .count(),
            1
        );
        key(&ui, slint::platform::Key::UpArrow);
        assert_eq!(ui.get_number(), "25");
        key(&ui, slint::platform::Key::DownArrow);
        assert_eq!(ui.get_number(), "20");
        numeric.accessible_action(&AccessibilityAction::Increment);
        assert_eq!(ui.get_number(), "25");
        numeric.accessible_action(&AccessibilityAction::SetValue("30".into()));
        assert_eq!(ui.get_number(), "30");
        ui.set_numeric_read_only(true);
        numeric.accessible_action(&AccessibilityAction::Increment);
        assert_eq!(ui.get_number(), "30");
        assert_eq!(
            numeric.accessible_string_property(P::ReadOnly).as_deref(),
            Some("true")
        );
        ui.set_numeric_read_only(false);
        ui.set_numeric_enabled(false);
        numeric.accessible_action(&AccessibilityAction::Decrement);
        assert_eq!(ui.get_number(), "30");
        assert_eq!(
            numeric.accessible_string_property(P::Enabled).as_deref(),
            Some("false")
        );
        ui.set_numeric_enabled(true);
        ui.set_number("5".into());
        assert!(
            !numeric
                .accessible_string_property(P::Description)
                .unwrap_or_default()
                .is_empty()
        );
        ui.set_number("".into());
        assert!(
            !numeric
                .accessible_string_property(P::Description)
                .unwrap_or_default()
                .is_empty(),
            "an empty required numeric field describes its error"
        );
        ui.set_numeric_placeholder("20".into());
        assert!(
            numeric
                .accessible_string_property(P::Description)
                .unwrap_or_default()
                .is_empty(),
            "an inherited default remains valid"
        );
        ui.set_number("20".into());
        let editor = tab_to(&ui, "Fixture name");
        assert_eq!(editor.accessible_role(), AccessibleRole::Button);
        assert_eq!(
            nodes(&ui)
                .iter()
                .filter(|i| i.accessible_role() == AccessibleRole::Button
                    && i.accessible_string_property(P::Label).as_deref() == Some("Fixture name"))
                .count(),
            1
        );
        key(&ui, slint::platform::Key::Return);
        assert_eq!(focus(&ui).accessible_role(), AccessibleRole::TextInput);
        assert_eq!(
            focus(&ui).accessible_string_property(P::Label).as_deref(),
            Some("Fixture name")
        );
        key(&ui, slint::platform::Key::Escape);
        assert_eq!(focus(&ui).accessible_role(), AccessibleRole::Button);
        key(&ui, slint::platform::Key::Return);
        key(&ui, "Renamed");
        key(&ui, slint::platform::Key::Return);
        assert_eq!(ui.get_name(), "Renamed");
        assert_eq!(
            focus(&ui).accessible_string_property(P::Label).as_deref(),
            Some("Fixture name")
        );
        tab_to(&ui, "First");
        key(&ui, slint::platform::Key::RightArrow);
        assert_eq!(ui.get_selection(), 1);
        assert_eq!(
            focus(&ui).accessible_string_property(P::Label).as_deref(),
            Some("Second")
        );
        let tab = tab_to(&ui, "Fixture tab");
        assert_eq!(
            tab.accessible_string_property(P::ItemSelected).as_deref(),
            Some("true")
        );
        println!(
            "PASS accessible numeric focus/text interface, inline edit/save/cancel, radio and tab selection"
        );
        slint::quit_event_loop().unwrap();
    });
    ui.run()?;
    Ok(())
}
