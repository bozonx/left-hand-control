mod support;
use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model};
use slint_shell::{Document, bind_document, ui::*};
use support::snapshot;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    ui.set_page(Page::Keyboard);
    if std::env::args().any(|arg| arg == "--smoke") {
        let keys = ui.global::<KeyEditor>();
        assert_eq!(keys.get_keys().row_count(), 80);
        assert_eq!(keys.get_keys().row_data(33).unwrap().label, "Q");
        let loaded = |count| Message {
            id: "config-loaded".into(),
            arg: "".into(),
            count,
        };
        let locale = ui.global::<Locale>();
        // Exercise every bundled language through Slint before the detailed RU/EN assertions.
        for code in [
            "en", "ru", "de", "fr", "es", "it", "pt", "pl", "tr", "uk", "cs", "hu", "ro", "el",
            "hr", "ka", "kk", "zh-Hans", "zh-Hant", "ja", "ko",
        ] {
            slint_shell::select_ui_language(code)?;
            for count in [0, 1, 2, 5, 11, 21] {
                let text = locale.invoke_text(loaded(count));
                assert!(!text.is_empty(), "{code}: missing translation");
                assert!(
                    text.contains(&count.to_string()),
                    "{code}: missing count: {text}"
                );
            }
        }
        slint_shell::select_ui_language("ru")?;
        assert_eq!(
            locale.invoke_text(loaded(1)),
            "Конфигурация загружена: 1 правило"
        );
        assert_eq!(
            locale.invoke_text(loaded(5)),
            "Конфигурация загружена: 5 правил"
        );
        assert_eq!(
            locale.invoke_text(loaded(2)),
            "Конфигурация загружена: 2 правила"
        );
        slint_shell::select_ui_language("en")?;
        assert_eq!(
            locale.invoke_text(loaded(1)),
            "Configuration loaded: 1 rule"
        );
        slint_shell::select_ui_language("ru")?;
        assert_eq!(
            locale.invoke_text(loaded(21)),
            "Конфигурация загружена: 21 правило"
        );
        for id in [
            "action-invalid-syntax",
            "action-pause-outside-macro",
            "action-unknown-macro",
            "action-unknown-command",
            "action-unknown-system",
            "action-unknown-app",
            "action-unknown-type",
            "action-failed",
            "error",
            "mapper-failed",
            "config-io",
            "config-parse",
            "config-invalid",
        ] {
            let message = Message {
                id: id.into(),
                arg: "untranslated technical detail".into(),
                count: 0,
            };
            let russian = locale.invoke_text(message.clone());
            assert!(!russian.is_empty(), "missing Russian message: {id}");
            assert!(!russian.contains("untranslated technical detail"));
            slint_shell::select_ui_language("en")?;
            let english = locale.invoke_text(message);
            assert!(!english.is_empty(), "missing English message: {id}");
            assert_ne!(russian, english, "untranslated message: {id}");
            slint_shell::select_ui_language("ru")?;
        }
        let action_messages: Vec<_> =
            lhc_core::profile::actions::catalog(&document.read().config())
                .into_iter()
                .filter_map(|entry| {
                    use lhc_core::profile::actions::ActionName;
                    let (id, arg, count) = match entry.name {
                        ActionName::BuiltinMacro { id } => ("builtin-macro", id, 0),
                        ActionName::System { id, n } | ActionName::App { id, n } => {
                            ("picker-action", id, n)
                        }
                        ActionName::Verbatim(_) => return None,
                    };
                    Some(Message {
                        id: id.into(),
                        arg: arg.into(),
                        count: i32::try_from(count).unwrap(),
                    })
                })
                .collect();
        let russian: Vec<_> = action_messages
            .iter()
            .map(|message| locale.invoke_text(message.clone()))
            .collect();
        slint_shell::select_ui_language("en")?;
        for (message, russian) in action_messages.into_iter().zip(russian) {
            let english = locale.invoke_text(message.clone());
            assert!(
                !russian.is_empty() && !english.is_empty(),
                "missing catalog label: {message:?}"
            );
            assert_ne!(russian, english, "untranslated catalog label: {message:?}");
        }
        slint_shell::select_ui_language("ru")?;
        assert_eq!(
            locale.invoke_text(Message {
                id: "unknown-id".into(),
                arg: "untranslated technical detail".into(),
                count: 0
            }),
            ""
        );
        slint_shell::select_ui_language("en")?;

        // Editing a key opens the picker; applying saves the tap action.
        let picker = ui.global::<ActionPicker>();
        keys.invoke_edit(33);
        assert!(picker.get_opened());
        assert_eq!(keys.get_selected(), 33);
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_value("text:Привет 👋".into());
        picker.invoke_apply();
        assert!(!picker.get_opened());
        assert_eq!(
            document.read().base_tap_action("KeyQ"),
            Some("text:Привет 👋")
        );
        assert_eq!(
            keys.get_keys().row_data(33).unwrap().action,
            "text:Привет 👋"
        );
        assert_eq!(ui.global::<AppState>().get_status().id, "");

        // Cancel keeps the saved value.
        keys.invoke_edit(33);
        assert_eq!(picker.get_value(), "text:Привет 👋");
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_value("Ctrl+KeyC".into());
        picker.invoke_dismiss_picker();
        assert_eq!(
            document.read().base_tap_action("KeyQ"),
            Some("text:Привет 👋")
        );

        // Pauses are only valid inside macros.
        keys.invoke_edit(33);
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_value("pause:250".into());
        picker.invoke_apply();
        assert!(picker.get_opened());
        assert!(!picker.get_valid());
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_value("Ctrl+Shift+KeyK".into());
        picker.invoke_apply();
        assert_eq!(
            document.read().base_tap_action("KeyQ"),
            Some("Ctrl+Shift+KeyK")
        );
        ui.show()?;
        let weak = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(500), move || {
            let ui = weak.unwrap();
            snapshot(&ui, "keyboard");
            println!(
                "Editor smoke: passed (80 keys, picker assignment, cancel, validation, translations)"
            );
            slint::quit_event_loop().unwrap();
        });
    }
    ui.run()?;
    Ok(())
}
