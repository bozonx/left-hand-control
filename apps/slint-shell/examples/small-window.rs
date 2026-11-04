//! Pages and dialogs at the minimum window size, using disposable configuration.
//!
//! `LHC_WINDOW_SIZE=WxH` overrides the size (default: the window minimum),
//! `LHC_SMALL_LANGUAGE=<code>` the UI language, `LHC_SMALL_RELEASE=1` hides development-only UI;
//! `LHC_EDITOR_SNAPSHOTS=<dir>` saves a snapshot of every state.
//! Every language checks toolbar bounds, overlaps and non-scrolling dialog actions.
mod support;
use lhc_core::config_document::KeyAssignment;
use slint::ComponentHandle;
use slint_shell::ui::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use support::{Fixture, snapshot};

type Step = (&'static str, fn(&SettingsWindow));

/// Debug element identities are independent of translated accessibility labels.
fn element_matches(item: &i_slint_core::item_tree::ItemRc, name: &str) -> bool {
    (0..item.element_count().unwrap_or(0)).any(|index| {
        item.element_type_names_and_ids(index)
            .is_some_and(|entries| {
                entries.iter().any(|(kind, id)| {
                    kind == name || id.as_str().split("::").any(|part| part == name)
                })
            })
    })
}

/// Scroll the actual automatic-default picker into the small panel's viewport.
fn scroll_auto_default(ui: &SettingsWindow) {
    use i_slint_core::{item_tree::ItemRc, window::WindowInner};
    let window = WindowInner::from_pub(ui.window());
    window.ensure_tree_instantiated();
    let mut pending = vec![(ItemRc::new_root(window.component()), None)];
    while let Some((item, mut viewport)) = pending.pop() {
        let origin = item.map_to_window(item.geometry().origin);
        let rect = (
            origin.x,
            origin.y,
            item.geometry().width(),
            item.geometry().height(),
        );
        if element_matches(&item, "Flickable") {
            viewport = Some(rect);
        }
        if element_matches(&item, "auto-default-picker") {
            let (x, y, width, height) = viewport.expect("default picker must scroll");
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                    position: slint::LogicalPosition::new(x + width / 2., y + height / 2.),
                    delta_x: 0.,
                    delta_y: y + height / 2. - (rect.1 + rect.3 / 2.),
                });
            return;
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push((next, viewport));
        }
    }
    panic!("automatic default picker not found");
}

/// Check toolbar controls against their container and each other. Dialog actions
/// outside scrolling bodies must also remain reachable in every language.
fn controls_fit(ui: &SettingsWindow, state: &str) {
    use i_slint_core::{item_tree::ItemRc, items::AccessibleRole, window::WindowInner};
    let window = WindowInner::from_pub(ui.window());
    window.ensure_tree_instantiated();
    let size = ui.window().size().to_logical(ui.window().scale_factor());
    let mut pending = vec![(ItemRc::new_root(window.component()), None, false, None)];
    let mut toolbar_controls = Vec::new();
    let mut toolbars = 0;
    let mut auto_controls = false;
    let mut auto_rules = false;
    let mut auto_default_visible = false;
    while let Some((item, mut toolbar, mut dialog, mut scrolling)) = pending.pop() {
        if !item.is_visible() {
            continue;
        }
        auto_controls |= element_matches(&item, "auto-mode-controls");
        auto_rules |= element_matches(&item, "auto-rules-panel");
        let origin = item.map_to_window(item.geometry().origin);
        let geometry = item.geometry();
        let rect = (origin.x, origin.y, geometry.width(), geometry.height());
        if element_matches(&item, "main-toolbar")
            || element_matches(&item, "editor-toolbar")
            || element_matches(&item, "library-toolbar")
        {
            toolbar = Some(rect);
            toolbars += 1;
        }
        dialog |= element_matches(&item, "DialogFrame");
        if element_matches(&item, "Flickable") {
            scrolling = Some(rect);
        }
        if element_matches(&item, "auto-default-picker")
            && let Some((x, y, width, height)) = scrolling
        {
            auto_default_visible = rect.0 >= x - 0.5
                && rect.1 >= y - 0.5
                && rect.0 + rect.2 <= x + width + 0.5
                && rect.1 + rect.3 <= y + height + 0.5;
        }
        if matches!(
            item.accessible_role(),
            AccessibleRole::Button | AccessibleRole::Tab
        ) && (toolbar.is_some() || (dialog && scrolling.is_none()))
        {
            let rect = if let Some((cx, cy, cw, ch)) = scrolling {
                let (x, y, w, h) = rect;
                let left = x.max(cx);
                let top = y.max(cy);
                (
                    left,
                    top,
                    (x + w).min(cx + cw) - left,
                    (y + h).min(cy + ch) - top,
                )
            } else {
                rect
            };
            let (x, y, w, h) = rect;
            if scrolling.is_some() && (w <= 0. || h <= 0.) {
                continue;
            }
            assert!(
                w > 0.
                    && h > 0.
                    && x >= -0.5
                    && y >= -0.5
                    && x + w <= size.width + 0.5
                    && y + h <= size.height + 0.5,
                "{state}: control outside window: {rect:?}"
            );
            if let Some((bx, by, bw, bh)) = toolbar {
                assert!(
                    x >= bx - 0.5
                        && y >= by - 0.5
                        && x + w <= bx + bw + 0.5
                        && y + h <= by + bh + 0.5,
                    "{state}: control clipped by toolbar: {rect:?}"
                );
                for &(px, py, pw, ph) in &toolbar_controls {
                    assert!(
                        (x + w).min(px + pw) - x.max(px) <= 0.5
                            || (y + h).min(py + ph) - y.max(py) <= 0.5,
                        "{state}: overlapping toolbar controls: {rect:?} and {:?}",
                        (px, py, pw, ph)
                    );
                }
                toolbar_controls.push(rect);
            }
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push((next, toolbar, dialog, scrolling));
        }
    }
    if state == "auto-default-scroll" {
        assert!(
            auto_default_visible,
            "automatic default picker must remain reachable"
        );
    }
    if state == "auto-without-development" {
        assert!(
            auto_controls && auto_rules,
            "automatic-mode controls disappeared"
        );
    }
    assert!(
        toolbars > 0,
        "{state}: toolbar identities missing; enable Slint debug info"
    );
    assert!(
        toolbar_controls.len() >= 4,
        "{state}: toolbar controls were not checked"
    );
}

/// Check input widths even below the fold; vertical scrolling cannot repair clipping on the right.
fn settings_inputs_fit(ui: &SettingsWindow, state: &str) {
    use i_slint_core::{
        accessibility::AccessibleStringProperty, item_tree::ItemRc, items::AccessibleRole,
        window::WindowInner,
    };
    if !state.starts_with("settings-") {
        return;
    }
    let window = WindowInner::from_pub(ui.window());
    window.ensure_tree_instantiated();
    let width = ui
        .window()
        .size()
        .to_logical(ui.window().scale_factor())
        .width;
    let mut pending = vec![ItemRc::new_root(window.component())];
    let mut checked = 0;
    while let Some(item) = pending.pop() {
        if item.is_visible()
            && matches!(
                item.accessible_role(),
                AccessibleRole::Combobox | AccessibleRole::Spinbox
            )
        {
            let origin = item.map_to_window(item.geometry().origin);
            let geometry = item.geometry();
            assert!(
                origin.x >= -0.5 && origin.x + geometry.width() <= width + 0.5,
                "{state}: {:?} clips horizontally: x={}, width={}, window={width}",
                item.accessible_string_property(AccessibleStringProperty::Label),
                origin.x,
                geometry.width()
            );
            checked += 1;
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
    }
    if matches!(state, "settings-general" | "settings-behavior") {
        assert!(checked > 0, "{state}: no inputs were checked");
    }
}

const STEPS: &[Step] = &[
    ("long-layout-names", |ui| {
        let library = ui.global::<LayoutLibrary>();
        library.set_active_label(
            "A very long active layout name with an identical prefix — active".into(),
        );
        library.set_current_label(
            "A very long active layout name with an identical prefix — editing".into(),
        );
        library.set_automatic(true);
        ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
    }),
    ("library", |ui| {
        ui.invoke_navigate(Page::Layouts, MenuKind::Emoji)
    }),
    ("auto-rule-dialog", |ui| {
        ui.global::<LayoutLibrary>().invoke_set_automatic(true);
        ui.global::<LayoutLibrary>().invoke_open_rule(-1, 0);
    }),
    ("auto-without-development", |ui| {
        ui.global::<LayoutLibrary>().invoke_cancel_dialog();
        ui.global::<AppState>().set_in_development(false);
        ui.invoke_navigate(Page::Layouts, MenuKind::Emoji);
    }),
    ("auto-default-scroll", scroll_auto_default),
    ("settings-general", |ui| {
        ui.global::<AppState>().set_in_development(true);
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
                if state == "macros" {
                    snapshot(&ui, "macros-top");
                    ui.window()
                        .dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                            position: slint::LogicalPosition::new(320.0, 300.0),
                            delta_x: 0.0,
                            delta_y: -600.0,
                        });
                }
                snapshot(&ui, state);
                settings_inputs_fit(&ui, state);
                controls_fit(&ui, state);
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
