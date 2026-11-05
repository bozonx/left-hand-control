//! Background metadata snapshots for the library cards.
//!
//! The file bytes read for each card are remembered: edits from a card
//! (rename, description, delete) pass it as the expected contents, so a
//! changes detected since the card was shown are reported as conflicts.

use crate::{
    i18n::Msg,
    ui::{LayoutLibrary, LayoutSummary, SettingsWindow},
};
use lhc_core::{
    profile::{
        library::{LibraryEntry, load_library_entries},
        model::LayoutPreset,
    },
    storage::StoragePaths,
};
use slint::{ComponentHandle, Model};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

type Request = (slint::Weak<SettingsWindow>, i32, StoragePaths, Vec<String>);

/// A loader belongs to one library page, including its remembered snapshots
/// and pending work. Separate documents never share baselines or requests.
#[derive(Clone, Default)]
pub(super) struct Loader {
    known: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    queue: Arc<Mutex<Requests>>,
}

#[derive(Default)]
struct Requests {
    busy: bool,
    pending: Option<Request>,
}

impl Requests {
    fn push(&mut self, request: Request) -> Option<Request> {
        if self.busy {
            self.pending = Some(request);
            None
        } else {
            self.busy = true;
            Some(request)
        }
    }

    fn next(&mut self) -> Option<Request> {
        let next = self.pending.take();
        self.busy = next.is_some();
        next
    }
}

impl Loader {
    pub(super) fn known(&self, name: &str) -> Option<Vec<u8>> {
        self.known
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .cloned()
    }

    pub(super) fn remember(&self, name: &str, bytes: Option<Vec<u8>>) {
        let mut known = self.known.lock().unwrap_or_else(|e| e.into_inner());
        match bytes {
            Some(bytes) => {
                known.insert(name.to_owned(), bytes);
            }
            None => {
                known.remove(name);
            }
        }
    }
}

fn layout_summary(preset: &LayoutPreset) -> LayoutSummary {
    LayoutSummary {
        rules: preset.rules.len() as i32,
        layers: preset.layers.len() as i32,
        emoji_pages: if preset
            .emoji_pages
            .iter()
            .any(|page| page.cells.values().any(|value| !value.is_empty()))
        {
            preset.emoji_pages.len() as i32
        } else {
            0
        },
        action_pages: if preset
            .quick_action_pages
            .iter()
            .any(|page| page.cells.values().any(|cell| !cell.action.is_empty()))
        {
            preset.quick_action_pages.len() as i32
        } else {
            0
        },
        macros: preset.macros.len() as i32,
        commands: preset.used_command_count() as i32,
    }
}

/// Only the latest request may publish metadata. Rows keep their models
/// while files load, including an inline draft after a failed save. At most
/// one read runs at a time; requests arriving meanwhile collapse into one.
impl Loader {
    pub(super) fn load(&self, ui: &SettingsWindow, paths: StoragePaths, names: Vec<String>) {
        let library = ui.global::<LayoutLibrary>();
        let revision = library.get_metadata_revision().wrapping_add(1);
        library.set_metadata_revision(revision);
        let request = (ui.as_weak(), revision, paths, names);
        let request = self
            .queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(request);
        let Some(request) = request else {
            return;
        };
        let loader = self.clone();
        std::thread::spawn(move || {
            let mut next = Some(request);
            while let Some((weak, revision, paths, names)) = next {
                let loaded = load_library_entries(&paths, &names);
                let publisher = loader.clone();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    publisher.publish(&ui, revision, &names, loaded)
                });
                next = loader
                    .queue
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .next();
            }
        });
    }

    pub(super) fn publish(
        &self,
        ui: &SettingsWindow,
        request: i32,
        names: &[String],
        loaded: Vec<LibraryEntry>,
    ) {
        let library = ui.global::<LayoutLibrary>();
        if library.get_metadata_revision() != request {
            return;
        }
        {
            let mut known = self.known.lock().unwrap_or_else(|e| e.into_inner());
            known.clear();
            for (name, entry) in names.iter().zip(&loaded) {
                if let Some(bytes) = &entry.bytes {
                    known.insert(name.clone(), bytes.clone());
                }
            }
        }
        let rows = library.get_rows();
        for (index, entry) in loaded.into_iter().enumerate() {
            let Some(mut row) = rows.row_data(index) else {
                continue;
            };
            row.loaded = true;
            match entry.layout {
                Ok(preset) => {
                    row.summary = layout_summary(&preset);
                    row.description = preset.description.unwrap_or_default().into();
                    row.error = Msg::None.to_ui();
                }
                Err(error) => {
                    row.error = Msg::LayoutLoadFailed(format!("{}: {error}", row.name)).to_ui();
                }
            }
            if rows.row_data(index).as_ref() != Some(&row) {
                rows.set_row_data(index, row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_are_isolated_between_library_pages() {
        let first = Loader::default();
        let second = Loader::default();
        first.remember("A", Some(vec![0xff]));
        second.remember("A", Some(b"another document".to_vec()));
        first.remember("A", None);
        assert_eq!(first.known("A"), None);
        assert_eq!(second.known("A"), Some(b"another document".to_vec()));
    }

    #[test]
    fn requests_coalesce_without_leaving_the_loader_busy() {
        let request = |revision| {
            (
                slint::Weak::default(),
                revision,
                StoragePaths::new("config".into(), "data".into()),
                Vec::new(),
            )
        };
        let mut queue = Requests::default();
        assert_eq!(queue.push(request(1)).unwrap().1, 1);
        assert!(queue.push(request(2)).is_none());
        assert!(queue.push(request(3)).is_none());
        assert_eq!(queue.next().unwrap().1, 3);
        assert!(queue.next().is_none());
        assert_eq!(queue.push(request(4)).unwrap().1, 4);
    }
}
