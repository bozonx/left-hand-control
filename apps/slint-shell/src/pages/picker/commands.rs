//! Command drafts and file chooser callbacks for the action picker.

use super::Session;
use crate::{
    document::Document,
    i18n::Msg,
    ui::{ActionPicker, Locale, SettingsWindow},
};
use lhc_core::profile::actions::Action;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

/// Warning for the command being edited: a missing working directory or
/// a script file that cannot be executed.
pub(super) fn command_notice(picker: &ActionPicker) -> Msg {
    use lhc_core::mapper::commands::{DraftIssue, draft_issue};
    if !matches!(
        Action::parse(Some(picker.get_value().as_str())),
        Action::Command(_)
    ) {
        return Msg::None;
    }
    let directory = picker.get_command_directory();
    match draft_issue(
        &picker.get_command_script(),
        Some(directory.as_str()).filter(|d| !d.trim().is_empty()),
    ) {
        Some(DraftIssue::MissingDirectory) => Msg::CommandDirectoryMissing,
        Some(DraftIssue::NotExecutable) => Msg::ScriptNotExecutable,
        None => Msg::None,
    }
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>, session: &Rc<RefCell<Session>>) {
    let picker = ui.global::<ActionPicker>();
    on!(ui, picker.on_edit_command, doc = document, state = session => |ui| {
        if !doc.read().settings().commands_enabled {
            return;
        }
        let picker = ui.global::<ActionPicker>();
        let value = picker.get_value().to_string();
        let id = match Action::parse(Some(value.as_str())) {
            Action::Command(id) => id,
            _ => lhc_core::profile::ids::generate("cmd-"),
        };
        // Keep the script of the other operating systems.
        let mut command = doc
            .read()
            .layout()
            .commands
            .iter()
            .find(|command| command.id == id)
            .cloned()
            .unwrap_or_else(|| lhc_core::profile::model::Command {
                id: id.clone(),
                name: String::new(),
                posix_script: String::new(),
                powershell_script: String::new(),
                working_directory: None,
            });
        command.name = picker.get_command_name().to_string();
        command.set_script(picker.get_command_script().to_string());
        command.working_directory = (!picker.get_command_directory().trim().is_empty())
            .then(|| picker.get_command_directory().trim().to_owned());
        state.borrow_mut().command = Some(command);
        picker.set_value(Action::Command(id).format().unwrap_or_default().into());
        picker.set_touched(true);
        picker.invoke_refresh();
    });

    for directory in [false, true] {
        let weak = ui.as_weak();
        let choose = move || {
            let Some(ui) = weak.upgrade() else { return };
            let title = ui
                .global::<Locale>()
                .invoke_text(
                    if directory {
                        Msg::ChooseCommandDirectory
                    } else {
                        Msg::ChooseCommandScript
                    }
                    .to_ui(),
                )
                .to_string();
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let picked = lhc_core::platform::file_chooser::pick(&title, directory);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let picker = ui.global::<ActionPicker>();
                    match picked {
                        Ok(Some(path)) => {
                            let path = path.to_string_lossy();
                            if directory {
                                picker.set_command_directory(path.as_ref().into());
                            } else {
                                let quoted = lhc_core::mapper::commands::invoke_path(&path);
                                let script = picker.get_command_script();
                                picker.set_command_script(if script.trim().is_empty() {
                                    quoted.into()
                                } else {
                                    format!("{} {quoted}", script.trim_end()).into()
                                });
                            }
                            picker.invoke_edit_command();
                        }
                        Ok(None) => {}
                        Err(error) => picker.set_error(Msg::ActionFailed(error).to_ui()),
                    }
                });
            });
        };
        if directory {
            picker.on_choose_command_directory(choose);
        } else {
            picker.on_choose_command_file(choose);
        }
    }

    on!(ui, picker.on_open_command_settings => |ui| {
        ui.global::<ActionPicker>().invoke_dismiss_picker();
        ui.global::<crate::ui::SettingsEditor>()
            .set_tab(crate::ui::SettingsTab::Commands);
        ui.invoke_navigate(crate::ui::Page::Settings, crate::ui::MenuKind::Emoji);
    });
}
