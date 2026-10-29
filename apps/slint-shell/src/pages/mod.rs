//! Pages of the settings window. Each module binds one Slint global to the
//! shared [`Document`] and refreshes it when the document changes.

/// Bind a callback that needs the settings window, skipping it once the
/// window is gone: `on!(ui, global.on_x, doc = document => |ui, arg| { … })`
/// clones each capture and passes the upgraded window as `ui`.
macro_rules! on {
    ($ui:ident, $target:ident . $method:ident $(, $cap:ident = $src:expr)* => |$w:ident $(, $arg:pat_param)*| $body:block) => {{
        let weak = $ui.as_weak();
        $(let $cap = $src.clone();)*
        $target.$method(move |$($arg),*| {
            let Some($w) = weak.upgrade() else { return Default::default() };
            $body
        });
    }};
}

mod autosave;
pub(crate) mod conditions;
mod focus;
mod inline;
mod keys;
pub(crate) mod layers;
mod library;
mod macros;
mod menus;
pub(crate) mod picker;
mod reorder;
mod rules;
mod settings;
mod timing;

pub use library::refresh_active;
pub(crate) use settings::check_input_access;

use crate::{
    document::{Document, Saved, View},
    i18n::Msg,
    ui::{ActionKind, GameCondition, Message, SettingsWindow},
};
use lhc_core::config_document::{ConfigDocument, ConfigError};
use lhc_core::profile::actions::Action;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Input state of the settings window that pages keep between window events:
/// the pointer position for inline editors and the action picker's key capture.
#[derive(Default)]
pub struct WindowInput {
    cursor: std::cell::Cell<(f64, f64)>,
    capture: std::cell::RefCell<picker::CaptureState>,
}

impl WindowInput {
    /// Route a settings-window event to inline editors and key capture;
    /// returns `true` to withhold the event from Slint.
    pub fn observe(
        &self,
        ui: &SettingsWindow,
        event: &slint::winit_030::winit::event::WindowEvent,
    ) -> bool {
        inline::observe(ui, event, &self.cursor);
        picker::capture(ui, event, &self.capture)
    }

    fn reset_capture(&self) {
        *self.capture.borrow_mut() = picker::CaptureState::default();
    }
}

/// Bind every page of `ui` to `document`.
pub fn bind_document(ui: &SettingsWindow, document: &Rc<Document>) {
    bind_document_with_input(ui, document, &Rc::default());
}

/// [`bind_document`] with the window input state that the window event
/// handler feeds through [`WindowInput::observe`].
pub fn bind_document_with_input(
    ui: &SettingsWindow,
    document: &Rc<Document>,
    input: &Rc<WindowInput>,
) {
    focus::bind(ui);
    timing::bind(ui);
    crate::notifications::bind(ui);
    let inline = ui.global::<crate::ui::InlineEditors>();
    inline.on_nonempty(|value| !value.trim().is_empty());
    // Slint defers `changed` handlers. Explicit saves must finish inline
    // commits before flushing page drafts or replacing the current page.
    inline.on_synchronize(i_slint_core::properties::ChangeTracker::run_change_handlers);
    reorder::bind(ui);
    conditions::bind(ui);
    keys::bind(ui, document);
    library::bind(ui, document);
    rules::bind(ui, document);
    let layers = layers::bind(ui, document);
    let macros = macros::bind(ui, document);
    let menus = menus::bind(ui, document);
    picker::bind(
        ui,
        document,
        input,
        picker::Drafts {
            layers,
            macros,
            menus,
        },
    );
    settings::bind(ui, document);
    let library = ui.global::<crate::ui::LayoutLibrary>();
    on!(ui, library.on_discard_drafts => |ui| {
        ui.global::<crate::ui::MacroEditor>().invoke_refresh();
        ui.global::<crate::ui::MenuEditor>().invoke_refresh();
    });
    on!(ui, library.on_flush_drafts => |ui| {
        let inline = ui.global::<crate::ui::InlineEditors>();
        let inline_saved = inline.invoke_flush();
        let macros = ui.global::<crate::ui::MacroEditor>().invoke_flush_pending();
        let menus = ui.global::<crate::ui::MenuEditor>().invoke_flush_pending();
        inline_saved && macros && menus
    });
}

/// Replace a command draft by identity, preserving the other commands.
fn stage_command(
    commands: &mut Vec<lhc_core::profile::model::Command>,
    command: lhc_core::profile::model::Command,
) {
    if let Some(current) = commands.iter_mut().find(|current| current.id == command.id) {
        *current = command;
    } else {
        commands.push(command);
    }
}

pub(crate) fn strings(
    values: impl IntoIterator<Item = impl Into<SharedString>>,
) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}

/// Comma-separated list as typed in the UI; empty items are dropped.
pub(crate) fn parse_list(value: &str) -> Vec<String> {
    if let Ok(items) = serde_json::from_str::<Vec<String>>(value) {
        return items;
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn condition_list(items: &[String]) -> String {
    if items
        .iter()
        .any(|item| item.contains(',') || item.starts_with('['))
    {
        serde_json::to_string(items).unwrap_or_default()
    } else {
        items.join(", ")
    }
}

/// Game-mode condition as stored in the config (`"on"`, `"off"` or none).
pub(crate) fn game_condition(value: Option<&str>) -> GameCondition {
    match value {
        Some("on") => GameCondition::On,
        Some("off") => GameCondition::Off,
        _ => GameCondition::Any,
    }
}

pub(crate) fn game_condition_value(condition: GameCondition) -> Option<String> {
    match condition {
        GameCondition::On => Some("on".into()),
        GameCondition::Off => Some("off".into()),
        GameCondition::Any => None,
    }
}

/// Message for the result of a page edit: `ok`, a warning that the mapper
/// was not updated, or the error.
pub(crate) fn outcome<T>(result: &Result<Saved<T>, ConfigError>, ok: Msg) -> Msg {
    match result {
        Ok(saved) => saved.message(ok),
        Err(error) => Msg::from(error),
    }
}

/// Save `edit` from page `view`, refresh the page whether or not it was
/// saved, and show the outcome in the page's status line.
pub(crate) fn change<T>(
    ui: &SettingsWindow,
    document: &Document,
    view: View,
    edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
    refresh: impl FnOnce(),
    set_status: impl FnOnce(Message),
) -> Result<T, Msg> {
    let result = document.edit(view, edit);
    refresh();
    let message = outcome(&result, Msg::None);
    set_status(crate::notifications::report(ui, &message));
    result.map(|saved| saved.value).map_err(|_| message)
}

/// Message shown under a name field; empty when the name can be used.
pub(crate) fn name_error<T>(result: Result<T, ConfigError>) -> Message {
    result
        .err()
        .map_or(Msg::None, |error| Msg::from(&error))
        .to_ui()
}

/// `text` on one line for labels and previews: line breaks become ⏎ and
/// tabs spaces, so a multi-line `text:` action does not grow its row.
pub(crate) fn one_line(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\n', " ⏎ ")
        .replace('\t', " ")
}

/// Icon of an action on key caps.
pub(crate) fn action_kind(action: &str) -> ActionKind {
    match Action::parse(Some(action)) {
        Action::Keys(_) => ActionKind::Keys,
        Action::Macro(_) | Action::BuiltinMacro(_) => ActionKind::Macro,
        Action::Command(_) => ActionKind::Command,
        Action::System(_) | Action::KdeShortcut(_) => ActionKind::System,
        Action::App(_) => ActionKind::App,
        Action::Text(_) => ActionKind::Text,
        Action::Native | Action::Swallow | Action::Pause(_) | Action::Unknown(_) => {
            ActionKind::None
        }
    }
}

/// Index of `value` in `choices`, or 0.
pub(crate) fn choice_index<T: PartialEq>(choices: &[T], value: &T) -> i32 {
    choices
        .iter()
        .position(|choice| choice == value)
        .unwrap_or(0) as i32
}

/// UI index as a position in a list of `len` items.
pub(crate) fn index(value: i32, len: usize) -> Option<usize> {
    usize::try_from(value).ok().filter(|index| *index < len)
}

/// Positions of a drag-and-drop move in a list of `len` items, or `None`
/// when either index is out of range or the item stays in place.
pub(crate) fn move_indices(from: i32, to: i32, len: usize) -> Option<(usize, usize)> {
    let (from, to) = (index(from, len)?, index(to, len)?);
    (from != to).then_some((from, to))
}

/// Move the item at `from` so that it ends up at `to`; returns whether anything moved.
pub(crate) fn move_item<T>(items: &mut Vec<T>, from: i32, to: i32) -> bool {
    let Some((from, to)) = move_indices(from, to, items.len()) else {
        return false;
    };
    let item = items.remove(from);
    items.insert(to, item);
    true
}

/// Choice at a UI index, falling back to the first one.
pub(crate) fn choice<T: Copy>(choices: &[T], index: i32) -> T {
    usize::try_from(index)
        .ok()
        .and_then(|index| choices.get(index).copied())
        .unwrap_or(choices[0])
}

#[cfg(test)]
fn test_document() -> (tempfile::TempDir, Rc<Document>) {
    let dir = tempfile::tempdir().unwrap();
    let paths =
        lhc_core::storage::StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
    let document = Document::load(paths).unwrap();
    (dir, document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lhc_core::profile::model::LocalePreference;

    #[test]
    fn move_item_checks_bounds() {
        let mut items = vec![1, 2, 3];
        assert!(move_item(&mut items, 0, 2));
        assert_eq!(items, [2, 3, 1]);
        assert!(move_item(&mut items, 2, 0));
        assert_eq!(items, [1, 2, 3]);
        assert!(!move_item(&mut items, 1, 1));
        assert!(!move_item(&mut items, -1, 0));
        assert!(!move_item(&mut items, 0, 3));
        assert_eq!(items, [1, 2, 3]);
    }

    #[test]
    fn helpers_round_trip() {
        assert_eq!(parse_list(" us, , ru "), ["us", "ru"]);
        for condition in [GameCondition::Any, GameCondition::On, GameCondition::Off] {
            assert_eq!(
                game_condition(game_condition_value(condition).as_deref()),
                condition
            );
        }
        assert_eq!(choice(&LocalePreference::ALL, 2), LocalePreference::Russian);
        assert_eq!(choice(&LocalePreference::ALL, 99), LocalePreference::Auto);
        assert_eq!(
            choice_index(&LocalePreference::ALL, &LocalePreference::Russian),
            2
        );
        assert_eq!(action_kind("text:hi"), ActionKind::Text);
        assert_eq!(action_kind("Ctrl+KeyC"), ActionKind::Keys);
    }
}
