extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub cursor: usize,
    pub anchor: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditKind {
    Typing,
    Delete,
    Isolated,
}

struct Edit {
    pos: usize,
    removed: String,
    inserted: String,
    before: Selection,
    after: Selection,
    kind: EditKind,
    time: Duration,
}

#[derive(Default)]
pub(crate) struct TextHistory {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    expected: Option<String>,
    merge: bool,
}

impl TextHistory {
    pub fn break_group(&mut self) {
        self.merge = false;
    }

    fn synchronize(&mut self, text: &str) {
        if self
            .expected
            .as_deref()
            .is_some_and(|expected| expected != text)
        {
            self.undo.clear();
            self.redo.clear();
            self.merge = false;
        }
    }

    pub fn record(
        &mut self,
        text: &str,
        range: core::ops::Range<usize>,
        inserted: &str,
        before: Selection,
        kind: EditKind,
        time: Duration,
    ) {
        self.synchronize(text);
        if range.is_empty() && inserted.is_empty() {
            return;
        }
        let mut next = String::from(text);
        next.replace_range(range.clone(), inserted);
        let after = Selection {
            cursor: range.start + inserted.len(),
            anchor: range.start + inserted.len(),
        };
        let edit = Edit {
            pos: range.start,
            removed: String::from(&text[range.clone()]),
            inserted: String::from(inserted),
            before,
            after,
            kind,
            time,
        };
        let merged = self.merge
            && self.undo.last_mut().is_some_and(|last| {
                if last.kind != kind
                    || kind == EditKind::Isolated
                    || time.saturating_sub(last.time) > Duration::from_millis(1000)
                    || last.after != before
                    || before.cursor != before.anchor
                {
                    return false;
                }
                match kind {
                    EditKind::Typing
                        if last.removed.is_empty()
                            && edit.removed.is_empty()
                            && edit.pos == last.pos + last.inserted.len()
                            && edit.inserted.chars().count() == 1
                            && !edit.inserted.chars().any(char::is_whitespace)
                            && !last
                                .inserted
                                .chars()
                                .last()
                                .is_some_and(char::is_whitespace) =>
                    {
                        last.inserted.push_str(&edit.inserted);
                    }
                    EditKind::Delete
                        if last.inserted.is_empty()
                            && edit.inserted.is_empty()
                            && !edit.removed.chars().any(char::is_whitespace)
                            && !last.removed.chars().any(char::is_whitespace) =>
                    {
                        if edit.pos + edit.removed.len() == last.pos {
                            last.pos = edit.pos;
                            last.removed.insert_str(0, &edit.removed);
                        } else if edit.pos == last.pos {
                            last.removed.push_str(&edit.removed);
                        } else {
                            return false;
                        }
                    }
                    _ => return false,
                }
                last.after = after;
                last.time = time;
                true
            });
        if !merged {
            self.undo.push(edit);
        }
        self.redo.clear();
        self.expected = Some(next);
        self.merge = kind != EditKind::Isolated;
    }

    pub fn undo(&mut self, text: &str) -> Option<(String, Selection)> {
        self.synchronize(text);
        self.break_group();
        let edit = self.undo.pop()?;
        let mut next = String::from(text);
        next.replace_range(edit.pos..edit.pos + edit.inserted.len(), &edit.removed);
        let selection = edit.before;
        self.expected = Some(next.clone());
        self.redo.push(edit);
        Some((next, selection))
    }

    pub fn redo(&mut self, text: &str) -> Option<(String, Selection)> {
        self.synchronize(text);
        self.break_group();
        let edit = self.redo.pop()?;
        let mut next = String::from(text);
        next.replace_range(edit.pos..edit.pos + edit.removed.len(), &edit.inserted);
        let selection = edit.after;
        self.expected = Some(next.clone());
        self.undo.push(edit);
        Some((next, selection))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(cursor: usize, anchor: usize) -> Selection {
        Selection { cursor, anchor }
    }

    #[test]
    fn replacement_is_atomic_and_restores_both_selections() {
        let mut history = TextHistory::default();
        history.record(
            "привет",
            0..12,
            "🙂",
            selection(12, 0),
            EditKind::Typing,
            Duration::ZERO,
        );
        let (text, caret) = history.undo("🙂").unwrap();
        assert_eq!(text, "привет");
        assert_eq!(caret, selection(12, 0));
        let (text, caret) = history.redo(&text).unwrap();
        assert_eq!(text, "🙂");
        assert_eq!(caret, selection(4, 4));
    }

    #[test]
    fn new_edit_discards_redo_and_external_updates_discard_history() {
        let mut history = TextHistory::default();
        history.record(
            "",
            0..0,
            "old",
            selection(0, 0),
            EditKind::Typing,
            Duration::ZERO,
        );
        assert_eq!(history.undo("old").unwrap().0, "");
        history.record(
            "",
            0..0,
            "new",
            selection(0, 0),
            EditKind::Typing,
            Duration::ZERO,
        );
        assert!(history.redo("new").is_none());
        assert!(history.undo("external").is_none());
        assert!(history.redo("external").is_none());
    }

    #[test]
    fn words_pauses_navigation_and_paste_break_groups() {
        let mut history = TextHistory::default();
        history.record(
            "",
            0..0,
            "a",
            selection(0, 0),
            EditKind::Typing,
            Duration::ZERO,
        );
        history.record(
            "a",
            1..1,
            "b",
            selection(1, 1),
            EditKind::Typing,
            Duration::ZERO,
        );
        history.record(
            "ab",
            2..2,
            " ",
            selection(2, 2),
            EditKind::Typing,
            Duration::ZERO,
        );
        history.record(
            "ab ",
            3..3,
            "c",
            selection(3, 3),
            EditKind::Typing,
            Duration::ZERO,
        );
        history.record(
            "ab c",
            4..4,
            "d",
            selection(4, 4),
            EditKind::Typing,
            Duration::from_secs(2),
        );
        assert_eq!(history.undo("ab cd").unwrap().0, "ab c");
        assert_eq!(history.undo("ab c").unwrap().0, "ab ");
        assert_eq!(history.undo("ab ").unwrap().0, "ab");
        assert_eq!(history.undo("ab").unwrap().0, "");
        history.record(
            "",
            0..0,
            "a",
            selection(0, 0),
            EditKind::Typing,
            Duration::ZERO,
        );
        history.break_group();
        history.record(
            "a",
            1..1,
            "b",
            selection(1, 1),
            EditKind::Typing,
            Duration::ZERO,
        );
        history.record(
            "ab",
            2..2,
            "paste",
            selection(2, 2),
            EditKind::Isolated,
            Duration::ZERO,
        );
        assert_eq!(history.undo("abpaste").unwrap().0, "ab");
        assert_eq!(history.undo("ab").unwrap().0, "a");
    }

    #[test]
    fn backward_and_forward_deletion_are_grouped() {
        for (range, before, after) in [(2..3, selection(3, 3), "ab"), (0..1, selection(0, 0), "bc")]
        {
            let mut history = TextHistory::default();
            history.record("abc", range, "", before, EditKind::Delete, Duration::ZERO);
            let (range, before, next) = if after == "ab" {
                (1..2, selection(2, 2), "a")
            } else {
                (0..1, selection(0, 0), "c")
            };
            history.record(after, range, "", before, EditKind::Delete, Duration::ZERO);
            assert_eq!(history.undo(next).unwrap().0, "abc");
        }
    }
}
