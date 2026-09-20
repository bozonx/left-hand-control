use crate::{ActionRow, SettingsWindow};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
struct Action {
    kind: i32,
    value: String,
}

impl Action {
    fn error(&self) -> &'static str {
        match self.kind {
            0 if self.value.trim().is_empty() => "Введите непустой текст",
            1 if !self
                .value
                .parse::<u32>()
                .is_ok_and(|n| (1..=10000).contains(&n)) =>
            {
                "Пауза: целое число от 1 до 10000 мс"
            }
            2 if !["Копировать", "Вставить", "Следующее окно"].contains(&self.value.as_str()) => {
                "Выберите системное действие"
            }
            3 if self.value.is_empty() => "Захватите сочетание клавиш",
            0..=3 => "",
            _ => "Неизвестный тип действия",
        }
    }
}

struct Editor {
    saved: Vec<Option<Action>>,
    catalog: Vec<Action>,
    revisions: Vec<u32>,
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
    ui.set_validation(action.error().into());
    ui.set_capturing(false);
}

impl Editor {
    fn new() -> Self {
        Self {
            saved: vec![None; 80],
            catalog: (0..500)
                .map(|i| Action {
                    kind: i % 4,
                    value: match i % 4 {
                        0 => format!("Привет! Текст {} 👋", i + 1),
                        1 => (i + 1).to_string(),
                        2 => ["Копировать", "Вставить", "Следующее окно"][(i / 4) as usize % 3]
                            .into(),
                        _ => format!("Ctrl+{}", (b'A' + (i / 4 % 26) as u8) as char),
                    },
                })
                .collect(),
            revisions: vec![0; 500],
            rows: Rc::new(VecModel::default()),
            assignments: Rc::new(VecModel::from(vec![SharedString::from("—"); 80])),
        }
    }

    fn row(&self, id: usize) -> ActionRow {
        let action = &self.catalog[id];
        let category = ["Текст", "Пауза", "Система", "Сочетание"][action.kind as usize];
        ActionRow {
            id: id as i32,
            label: format!(
                "{:03} · {category} · {}{}",
                id + 1,
                action.value,
                if self.revisions[id] == 0 {
                    String::new()
                } else {
                    format!(" · обновлено {}", self.revisions[id])
                }
            )
            .into(),
        }
    }

    fn filter(&self, query: &str, category: i32) {
        let query = query.to_lowercase();
        self.rows.set_vec(
            (0..self.catalog.len())
                .filter(|&id| {
                    (category == 0 || self.catalog[id].kind == category - 1)
                        && self.row(id).label.to_lowercase().contains(&query)
                })
                .map(|id| self.row(id))
                .collect::<Vec<_>>(),
        );
    }

    fn save(&mut self, key: usize, action: Action) -> bool {
        if key >= self.saved.len() || !action.error().is_empty() {
            return false;
        }
        self.assignments
            .set_row_data(key, action.value.clone().into());
        self.saved[key] = Some(action);
        true
    }

    fn update(&mut self, id: usize) {
        if id >= self.catalog.len() {
            return;
        }
        self.revisions[id] += 1;
        if let Some(index) = self.rows.iter().position(|row| row.id == id as i32) {
            self.rows.set_row_data(index, self.row(id));
        }
    }
}

pub fn bind(ui: &SettingsWindow) {
    let labels = "Esc F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 PrtSc ScrL Pause ` 1 2 3 4 5 6 7 8 9 0 - = Back Ins Home Tab Q W E R T Y U I O P [ ] \\ Del End Caps A S D F G H J K L ; ' Enter PgUp PgDn ↑ Shift Z X C V B N M , . / Alt Space ← ↓ →";
    let keys: Vec<SharedString> = labels.split_whitespace().map(Into::into).collect();
    assert_eq!(keys.len(), 80);
    ui.set_keys(ModelRc::new(VecModel::from(keys)));
    let state = Rc::new(RefCell::new(Editor::new()));
    ui.set_actions(state.borrow().rows.clone().into());
    ui.set_assignments(state.borrow().assignments.clone().into());
    state.borrow().filter("", 0);
    let weak = ui.as_weak();
    let state_copy = state.clone();
    ui.on_edit_key(move |key| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let state = state_copy.borrow();
        if !(0..80).contains(&key) {
            return;
        }
        ui.set_selected_key(key);
        ui.set_selected_action(-1);
        present(
            &ui,
            &state.saved[key as usize].clone().unwrap_or(Action {
                kind: 0,
                value: String::new(),
            }),
        );
        ui.set_editing(true);
    });
    let weak = ui.as_weak();
    ui.on_change_kind(move |kind| {
        if let Some(ui) = weak.upgrade() {
            present(
                &ui,
                &Action {
                    kind,
                    value: match kind {
                        1 => "100",
                        2 => "Копировать",
                        _ => "",
                    }
                    .into(),
                },
            );
            ui.set_selected_action(-1);
        }
    });
    let weak = ui.as_weak();
    ui.on_validate(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_validation(draft(&ui).error().into());
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
        if let Some(ui) = weak.upgrade() {
            if let Some(action) = state_copy.borrow().catalog.get(id as usize) {
                present(&ui, action);
                ui.set_selected_action(id);
            }
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    ui.on_update_action(move || {
        if let Some(ui) = weak.upgrade() {
            state_copy
                .borrow_mut()
                .update(ui.get_selected_action() as usize);
        }
    });
    let weak = ui.as_weak();
    ui.on_capture(move |text, ctrl, alt, shift, meta| {
        if let Some(ui) = weak.upgrade() {
            let key = match text.chars().next() {
                Some(c)
                    if c == char::from(slint::platform::Key::Control)
                        || c == char::from(slint::platform::Key::Shift)
                        || c == char::from(slint::platform::Key::Alt)
                        || c == char::from(slint::platform::Key::Meta) =>
                {
                    return;
                }
                Some(c) if c == char::from(slint::platform::Key::Return) => "Enter".into(),
                Some(c) if c == char::from(slint::platform::Key::Escape) => "Esc".into(),
                Some(c) if c == char::from(slint::platform::Key::Tab) => "Tab".into(),
                Some(c) if c == char::from(slint::platform::Key::Backspace) => "Backspace".into(),
                Some(c) if c == char::from(slint::platform::Key::LeftArrow) => "Left".into(),
                Some(c) if c == char::from(slint::platform::Key::RightArrow) => "Right".into(),
                Some(c) if c == char::from(slint::platform::Key::UpArrow) => "Up".into(),
                Some(c) if c == char::from(slint::platform::Key::DownArrow) => "Down".into(),
                Some(c)
                    if (char::from(slint::platform::Key::F1)
                        ..=char::from(slint::platform::Key::F24))
                        .contains(&c) =>
                {
                    format!(
                        "F{}",
                        c as u32 - char::from(slint::platform::Key::F1) as u32 + 1
                    )
                }
                Some(c) if c == char::from(slint::platform::Key::Delete) => "Delete".into(),
                Some(c) if c == char::from(slint::platform::Key::Insert) => "Insert".into(),
                Some(c) if c == char::from(slint::platform::Key::Home) => "Home".into(),
                Some(c) if c == char::from(slint::platform::Key::End) => "End".into(),
                Some(c) if c == char::from(slint::platform::Key::PageUp) => "PageUp".into(),
                Some(c) if c == char::from(slint::platform::Key::PageDown) => "PageDown".into(),
                Some(' ') => "Space".into(),
                Some(c) if !c.is_control() && !(0xe000..=0xf8ff).contains(&(c as u32)) => {
                    text.to_uppercase()
                }
                _ => return,
            };
            let mut parts: Vec<&str> = Vec::new();
            if ctrl {
                parts.push("Ctrl");
            }
            if alt {
                parts.push("Alt");
            }
            if shift {
                parts.push("Shift");
            }
            if meta {
                parts.push("Meta");
            }
            parts.push(&key);
            ui.set_value(parts.join("+").into());
            ui.set_validation(draft(&ui).error().into());
            ui.set_capturing(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_save(move || {
        if let Some(ui) = weak.upgrade() {
            let action = draft(&ui);
            ui.set_validation(action.error().into());
            if !ui.get_capturing()
                && state
                    .borrow_mut()
                    .save(ui.get_selected_key() as usize, action)
            {
                ui.set_status(
                    "Действие сохранено. Повторное открытие покажет сохранённое значение.".into(),
                );
                ui.set_editing(false);
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_cancel(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_capturing(false);
            ui.set_editing(false);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_cancel_save_and_invalid_values() {
        let mut editor = Editor::new();
        let saved = Action {
            kind: 0,
            value: "Привет 👋".into(),
        };
        assert!(editor.save(0, saved.clone()));
        let mut draft = editor.saved[0].clone().unwrap();
        draft.value = "отменено".into();
        assert_eq!(editor.saved[0], Some(saved));
        assert!(!editor.save(
            0,
            Action {
                kind: 1,
                value: "10001".into()
            }
        ));
        assert!(editor.save(0, draft.clone()));
        assert_eq!(editor.saved[0], Some(draft));
        for value in ["", "0", "-1", "1.5", "abc"] {
            assert!(
                !Action {
                    kind: 1,
                    value: value.into()
                }
                .error()
                .is_empty()
            );
        }
    }

    #[test]
    fn catalog_filter_and_update_keep_row_identity() {
        let mut editor = Editor::new();
        editor.filter("", 0);
        assert_eq!(editor.rows.row_count(), 500);
        let rows = editor.rows.clone();
        let before: Vec<_> = rows.iter().map(|row| row.id).collect();
        editor.update(321);
        assert!(Rc::ptr_eq(&rows, &editor.rows));
        assert_eq!(before, rows.iter().map(|row| row.id).collect::<Vec<_>>());
        assert!(rows.row_data(321).unwrap().label.contains("обновлено 1"));
        editor.filter("ПРИВЕТ", 1);
        assert_eq!(rows.row_count(), 125);
        editor.filter("Привет", 2);
        assert_eq!(rows.row_count(), 0);
        editor.filter("500", 0);
        assert_eq!(rows.row_count(), 1);
    }
}
