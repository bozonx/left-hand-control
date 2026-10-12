use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use slint_shell::{Document, bind_document, ui::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Document::load(paths)?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    document.edit(slint_shell::document::View::Library, |config| {
        config.update_layout(|layout| {
            layout.rules = vec![lhc_core::profile::model::LayerRule::new("r".to_string(), "F13")];
        })
    })?;
    ui.set_page(Page::Rules);
    let rules = ui.global::<RulesEditor>();
    println!("rules: {}", rules.get_rows().row_count());
    rules.invoke_open_dialog(0, RuleDialog::Conditions);
    let choices = ui.global::<ConditionChoices>();
    ui.window().set_size(slint::LogicalSize::new(1100., 800.));
    ui.show()?;
    let weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(1500), move || {
        let ui = weak.unwrap();
        let choices = ui.global::<ConditionChoices>();
        choices.set_open_windows(ModelRc::new(VecModel::from(vec![WindowChoice {
            pattern: "org.kde.konsole".into(),
            app_id: "org.kde.konsole".into(),
            count: 1,
            titles: ModelRc::new(VecModel::from(vec![slint::SharedString::from("t")])),
            ..Default::default()
        }])));
        choices.set_open_windows_ready(true);
    });
    for (i, name) in ["org.kde.konsole", "firefox", "a-very-long-application-identifier.with.many.parts"].into_iter().enumerate() {
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(1700 + 400 * i as u64), move || {
            let ui = weak.unwrap();
            let d = ui.global::<AppEntryDialog>();
            let rules = ui.global::<RulesEditor>();
            d.invoke_open(rules.get_apps_include(), false, -1);
            d.set_text(name.into());
            println!("valid {name}: {} rev {}", d.invoke_valid(), d.get_revision());
            d.invoke_save();
        });
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(1900 + 400 * i as u64), move || {
            let ui = weak.unwrap();
            println!("after {name}: {:?}", ui.global::<RulesEditor>().get_apps_include());
        });
    }
    let weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(3000), move || {
        let ui = weak.unwrap();
        println!("final: {:?}", ui.global::<RulesEditor>().get_apps_include());
        let pixels = ui.window().take_snapshot().unwrap();
        let mut bytes = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
        for p in pixels.as_slice() {
            bytes.extend_from_slice(&[p.r, p.g, p.b]);
        }
        std::fs::write(std::env::var("SNAP").unwrap(), bytes).unwrap();
        slint::quit_event_loop().unwrap();
    });
    let _ = choices;
    slint::run_event_loop()?;
    Ok(())
}
