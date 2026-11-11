//! Command drafts and file chooser callbacks for the action picker.

use super::{Drafts, Session};
use crate::{
    document::Document,
    i18n::Msg,
    ui::{ActionPicker, Locale, SettingsWindow},
};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};
use tecloid_core::config::actions::Action;

/// The picker session a file chooser belongs to and the action it edits.
struct ChoiceRequest {
    session: u64,
    value: String,
}

impl ChoiceRequest {
    /// The chosen path still applies: same open session, same command.
    /// Edits of the script or folder meanwhile keep it, the path is added
    /// to what is there now.
    fn accepts(&self, opened: bool, session: u64, value: &str) -> bool {
        opened && self.session == session && self.value == value
    }
}

/// Warning for the command being edited: a missing working directory or
/// a script file that cannot be executed.
pub(super) fn command_notice(picker: &ActionPicker) -> Msg {
    use tecloid_core::mapper::commands::{DraftIssue, draft_issue};
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

pub(super) fn bind(
    ui: &SettingsWindow,
    document: &Rc<Document>,
    session: &Rc<RefCell<Session>>,
    drafts: &Rc<Drafts>,
) {
    let picker = ui.global::<ActionPicker>();
    on!(ui, picker.on_edit_command, doc = document, state = session, drafts = drafts => |ui| {
        if !doc.read().settings().commands_enabled {
            return;
        }
        let picker = ui.global::<ActionPicker>();
        let value = picker.get_value().to_string();
        let id = match Action::parse(Some(value.as_str())) {
            Action::Command(id) => id,
            _ => tecloid_core::config::ids::generate("cmd-"),
        };
        // Keep the script of the other operating systems.
        let mut command = drafts.command(&id).or_else(|| doc
            .read()
            .keymap()
            .commands
            .iter()
            .find(|command| command.id == id)
            .cloned())
            .unwrap_or_else(|| tecloid_core::config::model::Command {
                id: id.clone(),
                name: String::new(),
                posix_script: String::new(),
                powershell_script: String::new(),
                working_directory: None,
            });
        picker.set_command_error(Msg::None.to_ui());
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
        let session = session.clone();
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
            let picker = ui.global::<ActionPicker>();
            if session.borrow().target.is_none() || picker.get_choosing_file() {
                return;
            }
            picker.set_choosing_file(true);
            picker.set_command_error(Msg::None.to_ui());
            let request = ChoiceRequest {
                session: session.borrow().current_generation(),
                value: picker.get_value().to_string(),
            };
            let weak = ui.as_weak();
            let generation = session.borrow().generation.clone();
            std::thread::spawn(move || {
                let picked = tecloid_core::platform::file_chooser::pick(&title, directory);
                if let Err(error) = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let picker = ui.global::<ActionPicker>();
                    let generation = generation.load(std::sync::atomic::Ordering::Relaxed);
                    if generation != request.session {
                        return;
                    }
                    picker.set_choosing_file(false);
                    if !request.accepts(picker.get_opened(), generation, &picker.get_value()) {
                        return;
                    }
                    match picked {
                        Ok(Some(path)) => {
                            let path = path.to_string_lossy();
                            if directory {
                                picker.set_command_directory(path.as_ref().into());
                            } else {
                                let quoted = tecloid_core::mapper::commands::invoke_path(&path);
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
                        Err(error) => picker.set_command_error(Msg::ActionFailed(error).to_ui()),
                    }
                }) {
                    log::warn!("[picker] file choice arrived after the event loop ended: {error}");
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_choice_ignores_closed_reopened_and_changed_actions() {
        let request = ChoiceRequest {
            session: 3,
            value: "cmd:test".into(),
        };
        assert!(request.accepts(true, 3, "cmd:test"));
        assert!(!request.accepts(false, 3, "cmd:test"));
        assert!(!request.accepts(true, 3, "cmd:other"));
        assert!(!request.accepts(true, 4, "cmd:test"));
    }
}
