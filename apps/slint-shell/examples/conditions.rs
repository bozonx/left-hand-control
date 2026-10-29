//! Conditions dialog regression: add, reject duplicates, edit, remove and save.

mod support;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use slint_shell::ui::*;
use std::time::Duration;
use support::{Fixture, snapshot, wait_until};

fn items(choices: &ConditionChoices<'_>, value: slint::SharedString) -> Vec<String> {
    choices
        .invoke_items(value)
        .iter()
        .map(|item| item.to_string())
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Fixture {
        dir: _dir,
        paths: _,
        document,
        ui,
    } = Fixture::new()?;
    document.edit(slint_shell::document::View::Library, |config| {
        config.update_layout(|layout| {
            layout.rules = vec![lhc_core::profile::model::KeyRule::new("r".into(), "F13")];
        })
    })?;
    ui.set_page(Page::Rules);
    let choices = ui.global::<ConditionChoices>();
    // Supply stable window choices; the scenario does not query the compositor.
    choices.on_refresh_open_windows(|| {});
    choices.set_open_windows(ModelRc::new(VecModel::from(vec![WindowChoice {
        pattern: "org.kde.konsole".into(),
        app_id: "org.kde.konsole".into(),
        count: 1,
        titles: ModelRc::new(VecModel::from(vec!["Terminal".into()])),
        ..Default::default()
    }])));
    choices.set_open_windows_ready(true);
    let rules = ui.global::<RulesEditor>();
    rules.invoke_open_dialog(0, RuleDialog::Conditions);
    ui.window().set_size(slint::LogicalSize::new(1100., 800.));
    ui.show()?;
    // Instantiate the visible condition-list bindings before editing its dialog.
    let _ = ui.window().take_snapshot()?;
    let dialog = ui.global::<AppEntryDialog>();
    let names = [
        "org.kde.konsole",
        "firefox",
        "a-very-long-application-identifier.with.many.parts",
    ];
    for (index, name) in names.iter().enumerate() {
        dialog.invoke_open(rules.get_apps_include(), false, -1);
        dialog.set_text((*name).into());
        assert!(dialog.invoke_valid());
        dialog.invoke_save();
        wait_until("added application condition", || {
            items(&choices, rules.get_apps_include()).len() == index + 1
        });
        assert_eq!(items(&choices, rules.get_apps_include()), names[..=index]);
    }
    dialog.invoke_open(rules.get_apps_include(), false, -1);
    dialog.set_text("FIREFOX".into());
    assert!(!dialog.invoke_valid(), "duplicates are case insensitive");
    let revision = dialog.get_revision();
    dialog.invoke_save();
    assert_eq!(dialog.get_revision(), revision);
    dialog.invoke_close();

    dialog.invoke_open(rules.get_apps_include(), false, 1);
    dialog.set_text("org.mozilla.firefox".into());
    assert!(dialog.invoke_valid());
    dialog.invoke_save();
    wait_until("edited application condition", || {
        items(&choices, rules.get_apps_include())[1] == "org.mozilla.firefox"
    });
    dialog.invoke_confirm_remove(rules.get_apps_include(), false, 2);
    dialog.invoke_remove();
    wait_until("removed application condition", || {
        items(&choices, rules.get_apps_include()).len() == 2
    });
    assert_eq!(
        items(&choices, rules.get_apps_include()),
        ["org.kde.konsole", "org.mozilla.firefox"]
    );
    rules.invoke_apply_conditions();
    assert_eq!(
        document.read().layout().rules[0]
            .condition_window_patterns
            .as_deref(),
        Some(
            [
                "org.kde.konsole".to_string(),
                "org.mozilla.firefox".to_string()
            ]
            .as_slice()
        )
    );
    rules.invoke_open_dialog(0, RuleDialog::Conditions);
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(100), move || {
        let ui = weak.unwrap();
        snapshot(&ui, "conditions");
        // Keep compatibility with the diagnostic stand's explicit output path.
        if let Some(path) = std::env::var_os("SNAP") {
            support::write_snapshot(&ui, std::path::Path::new(&path));
        }
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
