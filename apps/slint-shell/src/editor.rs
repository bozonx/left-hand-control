use crate::{
    i18n::{Language, Msg},
    ui::{ActionRow, Locale, Message, SettingsWindow, Theme},
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError},
    profile::auto_switch::AutoSwitchContext,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

/// Key codes of the on-screen keyboard, row by row (5 × 16).
pub const KEY_CODES: [&str; 80] = [
    "Escape",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "PrintScreen",
    "ScrollLock",
    "Pause",
    "Backquote",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Digit0",
    "Minus",
    "Equal",
    "Backspace",
    "Insert",
    "Home",
    "Tab",
    "KeyQ",
    "KeyW",
    "KeyE",
    "KeyR",
    "KeyT",
    "KeyY",
    "KeyU",
    "KeyI",
    "KeyO",
    "KeyP",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Delete",
    "End",
    "CapsLock",
    "KeyA",
    "KeyS",
    "KeyD",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyJ",
    "KeyK",
    "KeyL",
    "Semicolon",
    "Quote",
    "Enter",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ShiftLeft",
    "KeyZ",
    "KeyX",
    "KeyC",
    "KeyV",
    "KeyB",
    "KeyN",
    "KeyM",
    "Comma",
    "Period",
    "Slash",
    "AltLeft",
    "Space",
    "ArrowLeft",
    "ArrowDown",
    "ArrowRight",
];

const KEY_LABELS: &str = "Esc F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 PrtSc ScrL Pause ` 1 2 3 4 5 6 7 8 9 0 - = Back Ins Home Tab Q W E R T Y U I O P [ ] \\ Del End Caps A S D F G H J K L ; ' Enter PgUp PgDn ↑ Shift Z X C V B N M , . / Alt Space ← ↓ →";

/// System actions offered by the pilot: canonical chord, Russian, English.
const SYSTEM_ACTIONS: [(&str, &str, &str); 3] = [
    ("Ctrl+KeyC", "Копировать", "Copy"),
    ("Ctrl+KeyV", "Вставить", "Paste"),
    ("Alt+Tab", "Следующее окно", "Next window"),
];

const KIND_TEXT: i32 = 0;
const KIND_DELAY: i32 = 1;
const KIND_SYSTEM: i32 = 2;
const KIND_SHORTCUT: i32 = 3;

#[derive(Clone, Debug, PartialEq)]
struct Action {
    kind: i32,
    value: String,
}

impl Action {
    fn error(&self) -> Msg {
        match self.kind {
            KIND_TEXT if self.value.trim().is_empty() => Msg::TextEmpty,
            KIND_DELAY
                if !self
                    .value
                    .parse::<u32>()
                    .is_ok_and(|n| (1..=10000).contains(&n)) =>
            {
                Msg::DelayRange
            }
            KIND_SYSTEM if system_action(&self.value).is_none() => Msg::SystemActionRequired,
            KIND_TEXT..=KIND_SHORTCUT => Msg::None,
            _ => Msg::UnknownKind,
        }
    }

    fn from_config(value: &str) -> Self {
        let (kind, value) = if let Some(text) = value.strip_prefix("text:") {
            (KIND_TEXT, text)
        } else if let Some(delay) = value.strip_prefix("pause:") {
            (KIND_DELAY, delay)
        } else if system_action(value).is_some() {
            (KIND_SYSTEM, value)
        } else {
            (KIND_SHORTCUT, value)
        };
        Self {
            kind,
            value: value.into(),
        }
    }

    fn to_config(&self) -> String {
        match self.kind {
            KIND_TEXT => format!("text:{}", self.value),
            KIND_DELAY => format!("pause:{}", self.value),
            _ => self.value.clone(),
        }
    }

    /// Short label for the key cap; system actions show their chord.
    fn summary(&self) -> String {
        self.value.clone()
    }
}

fn system_action(value: &str) -> Option<(&'static str, &'static str, &'static str)> {
    SYSTEM_ACTIONS
        .into_iter()
        .find(|(chord, _, _)| *chord == value)
}

struct Editor {
    saved: Vec<Option<Action>>,
    catalog: Vec<Action>,
    rows: Rc<VecModel<ActionRow>>,
    assignments: Rc<VecModel<SharedString>>,
}

fn draft(ui: &SettingsWindow) -> Action {
    Action {
        kind: ui.get_kind(),
        value: ui.get_value().into(),
    }
}

fn present(ui: &SettingsWindow, action: &Action) {
    ui.set_kind(action.kind);
    ui.set_value(action.value.clone().into());
    ui.set_validation(action.error().to_ui());
    ui.set_capturing(false);
}

impl Editor {
    fn new() -> Self {
        Self {
            saved: vec![None; KEY_CODES.len()],
            catalog: SYSTEM_ACTIONS
                .iter()
                .map(|(value, _, _)| Action::from_config(value))
                .chain(KEY_CODES.iter().map(|value| Action::from_config(value)))
                .collect(),
            rows: Rc::new(VecModel::default()),
            assignments: Rc::new(VecModel::from(vec![
                SharedString::from("—");
                KEY_CODES.len()
            ])),
        }
    }

    fn load_assignments(&mut self, config: &ConfigDocument) {
        self.catalog = SYSTEM_ACTIONS
            .iter()
            .map(|(value, _, _)| Action::from_config(value))
            .chain(KEY_CODES.iter().map(|value| Action::from_config(value)))
            .chain(
                lhc_core::profile::actions::catalog(&config.config())
                    .iter()
                    .filter_map(|entry| entry.action.format())
                    .map(|value| Action::from_config(&value)),
            )
            .collect();
        self.filter("", 0);
        for (index, key) in KEY_CODES.iter().enumerate() {
            let action = config.base_tap_action(key).map(Action::from_config);
            let summary = action.as_ref().map_or("—".into(), Action::summary);
            self.assignments.set_row_data(index, summary.into());
            self.saved[index] = action;
        }
    }

    fn row(&self, id: usize) -> ActionRow {
        let action = &self.catalog[id];
        let kind = action.kind as usize;
        let (value_ru, value_en) = match system_action(&action.value) {
            Some((_, ru, en)) => (ru, en),
            None => (action.value.as_str(), action.value.as_str()),
        };
        ActionRow {
            id: id as i32,
            label_en: format!(
                "{} · {value_en}",
                ["Text", "Delay", "System", "Action"][kind]
            )
            .into(),
            label: format!(
                "{} · {value_ru}",
                ["Текст", "Пауза", "Система", "Действие"][kind]
            )
            .into(),
        }
    }

    fn filter(&self, query: &str, category: i32) {
        let query = query.to_lowercase();
        self.rows.set_vec(
            (0..self.catalog.len())
                .filter(|&id| {
                    (category == 0 || self.catalog[id].kind == category - 1) && {
                        let row = self.row(id);
                        row.label.to_lowercase().contains(&query)
                            || row.label_en.to_lowercase().contains(&query)
                    }
                })
                .map(|id| self.row(id))
                .collect::<Vec<_>>(),
        );
    }

    fn save(&mut self, key: usize, action: Action) -> bool {
        if key >= self.saved.len() || !action.error().is_none() {
            return false;
        }
        self.assignments.set_row_data(key, action.summary().into());
        self.saved[key] = Some(action);
        true
    }
}

/// Lets the application refresh key assignments after the config changed
/// on disk.
#[derive(Clone)]
pub struct EditorHandle(Rc<RefCell<Editor>>);

impl EditorHandle {
    pub fn reload(&self, config: &ConfigDocument) {
        self.0.borrow_mut().load_assignments(config);
    }
}

fn apply_preferences(ui: &SettingsWindow, dark: bool, english: bool) {
    ui.global::<Theme>().set_dark(dark);
    ui.global::<Theme>().invoke_apply();
    ui.global::<Locale>().set_english(english);
    if english {
        Language::English
    } else {
        Language::Russian
    }
    .select_bundled();
}

fn config_message(error: &ConfigError) -> Message {
    Msg::from(error).to_ui()
}

/// Bind the editor to `ui`. The standalone examples use the built-in
/// preference handler; the application replaces it with its own.
pub fn bind_with_config(
    ui: &SettingsWindow,
    config: Option<Rc<RefCell<ConfigDocument>>>,
) -> EditorHandle {
    crate::action_picker::bind(ui, config.clone());
    use slint::winit_030::{EventResult, WinitWindowAccessor};
    let weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_, event| {
        if weak
            .upgrade()
            .is_some_and(|ui| crate::action_picker::capture(&ui, event))
        {
            EventResult::PreventDefault
        } else {
            EventResult::Propagate
        }
    });
    apply_preferences(ui, true, false);
    let weak = ui.as_weak();
    ui.on_preferences(move |dark, english| {
        if let Some(ui) = weak.upgrade() {
            apply_preferences(&ui, dark, english);
        }
    });
    let keys: Vec<SharedString> = KEY_LABELS.split_whitespace().map(Into::into).collect();
    debug_assert_eq!(keys.len(), KEY_CODES.len());
    ui.set_keys(ModelRc::new(VecModel::from(keys)));
    let mut editor = Editor::new();
    if let Some(config) = config.as_ref() {
        editor.load_assignments(&config.borrow());
    }
    let state = Rc::new(RefCell::new(editor));
    ui.set_actions(state.borrow().rows.clone().into());
    ui.set_assignments(state.borrow().assignments.clone().into());
    state.borrow().filter("", 0);

    let weak = ui.as_weak();
    let state_copy = state.clone();
    let key_config = config.clone();
    ui.on_edit_key(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if let Some(config) = &key_config {
            state_copy.borrow_mut().load_assignments(&config.borrow());
        }
        let Some(saved) = usize::try_from(key)
            .ok()
            .and_then(|key| state_copy.borrow().saved.get(key).cloned())
        else {
            return;
        };
        ui.set_selected_key(key);
        ui.set_selected_action(-1);
        present(
            &ui,
            &saved.unwrap_or(Action {
                kind: KIND_TEXT,
                value: String::new(),
            }),
        );
        ui.set_editing(true);
        ui.invoke_focus_editor();
    });
    let weak = ui.as_weak();
    ui.on_change_kind(move |kind| {
        if let Some(ui) = weak.upgrade() {
            let value = match kind {
                KIND_DELAY => "100",
                KIND_SYSTEM => SYSTEM_ACTIONS[0].0,
                _ => "",
            };
            present(
                &ui,
                &Action {
                    kind,
                    value: value.into(),
                },
            );
            ui.set_selected_action(-1);
        }
    });
    let weak = ui.as_weak();
    ui.on_validate(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_validation(draft(&ui).error().to_ui());
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    ui.on_filter(move || {
        if let Some(ui) = weak.upgrade() {
            state_copy
                .borrow()
                .filter(&ui.get_query(), ui.get_category());
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    ui.on_pick_action(move |id| {
        if let Some(ui) = weak.upgrade()
            && let Some(action) = state_copy.borrow().catalog.get(id as usize)
        {
            present(&ui, action);
            ui.set_selected_action(id);
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    ui.on_navigate_action(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let state = state_copy.borrow();
            let count = state.rows.row_count();
            if count == 0 {
                return;
            }
            let index = state
                .rows
                .iter()
                .position(|row| row.id == ui.get_selected_action());
            let index = index.map_or(0, |i| {
                (i as i32 + delta).clamp(0, count as i32 - 1) as usize
            });
            let Some(row) = state.rows.row_data(index) else {
                return;
            };
            present(&ui, &state.catalog[row.id as usize]);
            ui.set_selected_action(row.id);
            ui.set_catalog_scroll_y(-(index as f32 * 36.0));
        }
    });
    let weak = ui.as_weak();
    ui.on_capture(move |text, ctrl, alt, shift, meta| {
        if let Some(ui) = weak.upgrade()
            && let Some(chord) = capture_chord(&text, ctrl, alt, shift, meta)
        {
            ui.set_value(chord.into());
            ui.set_validation(draft(&ui).error().to_ui());
            ui.set_capturing(false);
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    ui.on_save(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let action = draft(&ui);
        ui.set_validation(action.error().to_ui());
        if ui.get_capturing() || !action.error().is_none() {
            return;
        }
        let Some(key) = usize::try_from(ui.get_selected_key())
            .ok()
            .filter(|key| *key < KEY_CODES.len())
        else {
            return;
        };
        let mut status = Msg::ActionSaved;
        if let Some(config) = &config {
            let mut document = config.borrow_mut();
            if let Err(error) = document.set_base_tap_action(KEY_CODES[key], &action.to_config()) {
                if error == ConfigError::ExternalChange {
                    match document.reload_if_changed() {
                        Ok(_) => state_copy.borrow_mut().load_assignments(&document),
                        Err(reload) => log::warn!("reload config: {reload}"),
                    }
                }
                ui.set_validation(config_message(&error));
                return;
            }
            match document.runtime_config(&AutoSwitchContext::current()) {
                Ok(runtime) => {
                    if let Err(error) =
                        lhc_core::mapper::runtime::update_config_if_running(&runtime.json)
                    {
                        status = Msg::SavedMapperNotUpdated(error);
                    }
                }
                Err(error) => status = Msg::SavedMapperNotUpdated(error.to_string()),
            }
            ui.set_config_status(Msg::ConfigSaved(document.layout().rules.len()).to_ui());
        }
        if state_copy.borrow_mut().save(key, action) {
            ui.set_status(status.to_ui());
            ui.set_editing(false);
            ui.invoke_restore_focus();
        }
    });
    let weak = ui.as_weak();
    ui.on_cancel(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_capturing(false);
            ui.set_editing(false);
            ui.invoke_restore_focus();
        }
    });
    EditorHandle(state)
}

/// Convert a captured key event into the config chord format.
fn capture_chord(text: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Option<String> {
    use slint::platform::Key;
    let c = text.chars().next()?;
    let named = |key: Key| c == char::from(key);
    if [Key::Control, Key::Shift, Key::Alt, Key::Meta]
        .into_iter()
        .any(named)
    {
        return None;
    }
    let names = [
        (Key::Return, "Enter"),
        (Key::Escape, "Escape"),
        (Key::Tab, "Tab"),
        (Key::Backspace, "Backspace"),
        (Key::LeftArrow, "ArrowLeft"),
        (Key::RightArrow, "ArrowRight"),
        (Key::UpArrow, "ArrowUp"),
        (Key::DownArrow, "ArrowDown"),
        (Key::Delete, "Delete"),
        (Key::Insert, "Insert"),
        (Key::Home, "Home"),
        (Key::End, "End"),
        (Key::PageUp, "PageUp"),
        (Key::PageDown, "PageDown"),
    ];
    let f1 = char::from(Key::F1);
    let key = if let Some((_, name)) = names.iter().find(|(key, _)| named(*key)) {
        (*name).to_owned()
    } else if (f1..=char::from(Key::F24)).contains(&c) {
        format!("F{}", c as u32 - f1 as u32 + 1)
    } else if c == ' ' {
        "Space".into()
    } else if c.is_ascii_alphabetic() {
        format!("Key{}", c.to_ascii_uppercase())
    } else if c.is_ascii_digit() {
        format!("Digit{c}")
    } else if !c.is_control() && !(0xe000..=0xf8ff).contains(&(c as u32)) {
        text.to_uppercase()
    } else {
        return None;
    };
    let mut parts: Vec<&str> = Vec::new();
    for (active, name) in [
        (ctrl, "Ctrl"),
        (alt, "Alt"),
        (shift, "Shift"),
        (meta, "Meta"),
    ] {
        if active {
            parts.push(name);
        }
    }
    parts.push(&key);
    Some(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_cancel_save_and_invalid_values() {
        let mut editor = Editor::new();
        let saved = Action {
            kind: KIND_TEXT,
            value: "Привет 👋".into(),
        };
        assert!(editor.save(0, saved.clone()));
        let mut draft = editor.saved[0].clone().unwrap();
        draft.value = "отменено".into();
        assert_eq!(editor.saved[0], Some(saved));
        assert!(!editor.save(
            0,
            Action {
                kind: KIND_DELAY,
                value: "10001".into()
            }
        ));
        assert!(editor.save(0, draft.clone()));
        assert_eq!(editor.saved[0], Some(draft));
        for value in ["", "0", "-1", "1.5", "abc"] {
            assert_eq!(
                Action {
                    kind: KIND_DELAY,
                    value: value.into()
                }
                .error(),
                Msg::DelayRange
            );
        }
    }

    #[test]
    fn config_values_round_trip() {
        for value in ["text:Привет", "pause:250", "Ctrl+KeyV", "Ctrl+Shift+KeyK"] {
            assert_eq!(Action::from_config(value).to_config(), value);
        }
        assert_eq!(Action::from_config("Alt+Tab").kind, KIND_SYSTEM);
        assert_eq!(
            Action {
                kind: KIND_SYSTEM,
                value: "Копировать".into()
            }
            .error(),
            Msg::SystemActionRequired
        );
    }

    #[test]
    fn catalog_contains_real_actions() {
        let editor = Editor::new();
        editor.filter("копировать", 3);
        assert_eq!(editor.rows.row_count(), 1);
        let id = editor.rows.row_data(0).unwrap().id;
        assert_eq!(editor.catalog[id as usize].to_config(), "Ctrl+KeyC");
        editor.filter("KeyQ", 0);
        assert_eq!(editor.rows.row_count(), 1);
    }

    #[test]
    fn capture_builds_config_chords() {
        assert_eq!(
            capture_chord("k", true, false, true, false).as_deref(),
            Some("Ctrl+Shift+KeyK")
        );
        assert_eq!(
            capture_chord("7", false, true, false, false).as_deref(),
            Some("Alt+Digit7")
        );
        let control = SharedString::from(slint::platform::Key::Control);
        assert_eq!(capture_chord(&control, true, false, false, false), None);
    }
}
