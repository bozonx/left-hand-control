//! Pages and dialogs at the minimum window size, using disposable configuration.
//!
//! `LHC_WINDOW_SIZE=WxH` overrides the size (default: the window minimum),
//! `LHC_SMALL_LANGUAGE=<code>` the UI language, `LHC_SMALL_RELEASE=1` hides development-only UI;
//! `LHC_EDITOR_SNAPSHOTS=<dir>` saves a snapshot of every state.
//! In English, every state checks that the header and dialog buttons lie inside the window.
mod support;
use lhc_core::config_document::KeyAssignment;
use slint::ComponentHandle;
use slint_shell::ui::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use support::{Fixture, snapshot};

type Step = (&'static str, fn(&SettingsWindow));

/// Header controls present in every state; `Active:` is a label prefix.
const HEADER: &[&str] = &["Layouts", "Settings", "Active:", "Start"];

/// Dialog buttons that must stay reachable in each state.
fn dialog_buttons(state: &str) -> &'static [&'static str] {
    match state {
        "auto-rule-dialog" | "nested-app-dialog" => &["Cancel", "Save"],
        "device-dialog" => &["Refresh", "Close"],
        "new-layer-dialog" | "conditions" | "advanced" | "picker" => &["Cancel", "Apply"],
        _ => &[],
    }
}

/// Whether a visible control named `label` lies entirely inside the window.
fn inside(ui: &SettingsWindow, label: &str) -> bool {
    use i_slint_core::{
        accessibility::AccessibleStringProperty, item_tree::ItemRc, window::WindowInner,
    };
    let size = ui.window().size().to_logical(ui.window().scale_factor());
    let mut pending = vec![ItemRc::new_root(
        WindowInner::from_pub(ui.window()).component(),
    )];
    while let Some(item) = pending.pop() {
        let named = item
            .accessible_string_property(AccessibleStringProperty::Label)
            .is_some_and(|value| match label.strip_suffix(':') {
                Some(prefix) => value.starts_with(prefix),
                None => value == label,
            });
        if named && item.is_visible() {
            let origin = item.map_to_window(item.geometry().origin);
            let geometry = item.geometry();
            if origin.x >= -0.5
                && origin.y >= -0.5
                && origin.x + geometry.width() <= size.width + 0.5
                && origin.y + geometry.height() <= size.height + 0.5
            {
                return true;
            }
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
    }
    false
}

const STEPS: &[Step] = &[
    ("library", |ui| {
        ui.invoke_navigate(Page::Layouts, MenuKind::Emoji)
    }),
    ("auto-rule-dialog", |ui| {
        ui.global::<LayoutLibrary>().invoke_set_automatic(true);
        ui.global::<LayoutLibrary>().invoke_open_rule(-1, 0);
    }),
    ("settings-general", |ui| {
        ui.global::<LayoutLibrary>().invoke_cancel_dialog();
        ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
        ui.global::<SettingsEditor>().set_tab(SettingsTab::General);
    }),
    ("settings-mapper", |ui| {
        ui.global::<SettingsEditor>().set_tab(SettingsTab::Mapper)
    }),
    ("settings-behavior", |ui| {
        ui.global::<SettingsEditor>()
            .set_tab(SettingsTab::KeyBehavior)
    }),
    ("settings-game", |ui| {
        ui.global::<SettingsEditor>().set_tab(SettingsTab::GameMode)
    }),
    ("settings-commands", |ui| {
        ui.global::<SettingsEditor>().set_tab(SettingsTab::Commands)
    }),
    ("settings-accessibility", |ui| {
        ui.global::<SettingsEditor>()
            .set_tab(SettingsTab::Accessibility)
    }),
    ("device-dialog", |ui| {
        ui.global::<SettingsEditor>().set_tab(SettingsTab::Mapper);
        ui.global::<DeviceDialog>().set_open(true);
    }),
    ("layers", |ui| {
        ui.global::<DeviceDialog>().set_open(false);
        ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
        ui.global::<LayersEditor>().invoke_choose(0);
    }),
    ("new-layer-dialog", |ui| {
        ui.global::<LayersEditor>()
            .invoke_open_dialog(LayerDialog::Create, -1)
    }),
    ("rules", |ui| {
        ui.global::<LayersEditor>().set_dialog(LayerDialog::None);
        ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
    }),
    ("conditions", |ui| {
        ui.global::<RulesEditor>()
            .invoke_open_dialog(0, RuleDialog::Conditions)
    }),
    ("nested-app-dialog", |ui| {
        ui.global::<AppEntryDialog>()
            .invoke_open("firefox".into(), false, 0)
    }),
    ("advanced", |ui| {
        ui.global::<AppEntryDialog>().invoke_close();
        ui.global::<RulesEditor>()
            .invoke_open_dialog(0, RuleDialog::Advanced);
    }),
    ("picker", |ui| {
        ui.global::<RulesEditor>().set_dialog(RuleDialog::None);
        ui.global::<ActionPicker>()
            .invoke_open(PickerTarget::Rule, 0, "KeyA".into(), false);
    }),
    ("macros", |ui| {
        ui.global::<ActionPicker>().invoke_dismiss_picker();
        ui.invoke_navigate(Page::Macros, MenuKind::Emoji);
    }),
    ("emoji-editor", |ui| {
        ui.invoke_navigate(Page::Menus, MenuKind::Emoji)
    }),
    ("quick-editor", |ui| {
        ui.invoke_navigate(Page::Menus, MenuKind::Quick)
    }),
    ("extra-keys", |ui| {
        ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
        ui.global::<LayersEditor>().invoke_choose(5);
    }),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let ui = fixture.ui.clone_strong();
    ui.global::<AppState>().set_is_linux(true);
    ui.global::<AppState>().set_mapper_supported(true);
    ui.global::<AppState>().set_autostart_supported(true);
    let development = std::env::var_os("LHC_SMALL_RELEASE").is_none();
    ui.global::<AppState>().set_in_development(development);
    ui.global::<SettingsEditor>()
        .set_in_development(development);
    ui.global::<LayoutLibrary>().invoke_create(
        "Small window".into(),
        "".into(),
        LayoutSource::Bundled,
        0,
    );
    fixture
        .document
        .edit(slint_shell::document::View::Library, |config| {
            let id = config.create_layer("Small layer", "")?;
            config.set_extra_key(&id, None, "F13", KeyAssignment::Action("Alt+Tab".into()))?;
            Ok(())
        })?;
    ui.global::<MacroEditor>().invoke_add("Small macro".into());
    let language = std::env::var("LHC_SMALL_LANGUAGE").unwrap_or_else(|_| "en".into());
    slint_shell::select_ui_language(&language)?;
    let english = language == "en";
    let size = std::env::var("LHC_WINDOW_SIZE")
        .ok()
        .and_then(|s| {
            s.split_once('x')
                .map(|(w, h)| (w.parse().unwrap(), h.parse().unwrap()))
        })
        .unwrap_or((ui.get_min_window_width(), ui.get_min_window_height()));
    ui.show()?;
    ui.window()
        .set_size(slint::LogicalSize::new(size.0, size.1));
    let step = Rc::new(Cell::new(0usize));
    let progress = step.clone();
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    // Two ticks per state: apply, then snapshot after layout and animations settle.
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let ui = weak.unwrap();
            let n = step.get();
            if n == 2 * STEPS.len() {
                println!(
                    "small-window: {} states at {}x{}",
                    STEPS.len(),
                    size.0,
                    size.1
                );
                slint::quit_event_loop().unwrap();
            } else if n.is_multiple_of(2) {
                // The compositor's first configure may arrive after `show()` and override the size.
                ui.window()
                    .set_size(slint::LogicalSize::new(size.0, size.1));
                (STEPS[n / 2].1)(&ui);
            } else {
                let logical = ui.window().size().to_logical(ui.window().scale_factor());
                assert!(
                    (logical.width - size.0).abs() < 1. && (logical.height - size.1).abs() < 1.,
                    "window must accept {size:?}, got {logical:?}"
                );
                let state = STEPS[n / 2].0;
                snapshot(&ui, state);
                if english {
                    for label in HEADER.iter().chain(dialog_buttons(state)) {
                        assert!(
                            inside(&ui, label),
                            "{state}: {label:?} is outside the window"
                        );
                    }
                }
            }
            step.set(n + 1);
        },
    );
    slint::run_event_loop()?;
    assert_eq!(
        progress.get(),
        2 * STEPS.len() + 1,
        "scenario must complete all states"
    );
    Ok(())
}
