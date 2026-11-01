//! Background metadata snapshots for the library cards.
//!
//! The file text read for each card is remembered: edits from a card
//! (rename, description, delete) pass it as the expected contents, so a
//! file changed elsewhere since the card was shown is never overwritten.

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
use std::{cell::RefCell, collections::HashMap, sync::Mutex};

thread_local! {
    // File text per layout name as last read or written by this page.
    // Only touched from the UI thread.
    static KNOWN: RefCell<HashMap<String, String>> = RefCell::default();
}

type Request = (slint::Weak<SettingsWindow>, i32, StoragePaths, Vec<String>);

/// Worker state: whether a reader thread runs, and the newest request
/// that arrived meanwhile. Kept off the UI thread so a window closed
/// before its result arrives cannot leave the reader marked busy.
static QUEUE: Mutex<(bool, Option<Request>)> = Mutex::new((false, None));

/// Text of `name` as the user last saw it, if it was read.
pub(super) fn known(name: &str) -> Option<String> {
    KNOWN.with_borrow(|known| known.get(name).cloned())
}

/// Record the file text after this page wrote or deleted (`None`) it.
pub(super) fn remember(name: &str, text: Option<String>) {
    KNOWN.with_borrow_mut(|known| match text {
        Some(text) => {
            known.insert(name.to_owned(), text);
        }
        None => {
            known.remove(name);
        }
    });
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
pub(super) fn load(ui: &SettingsWindow, paths: StoragePaths, names: Vec<String>) {
    let library = ui.global::<LayoutLibrary>();
    let request = library.get_metadata_revision().wrapping_add(1);
    library.set_metadata_revision(request);
    let request = (ui.as_weak(), request, paths, names);
    let mut queue = QUEUE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if queue.0 {
        queue.1 = Some(request);
        return;
    }
    queue.0 = true;
    drop(queue);
    std::thread::spawn(move || {
        let mut next = Some(request);
        while let Some((weak, request, paths, names)) = next {
            let loaded = load_library_entries(&paths, &names);
            let _ = weak.upgrade_in_event_loop(move |ui| publish(&ui, request, &names, loaded));
            let mut queue = QUEUE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            next = queue.1.take();
            queue.0 = next.is_some();
        }
    });
}

fn publish(ui: &SettingsWindow, request: i32, names: &[String], loaded: Vec<LibraryEntry>) {
    let library = ui.global::<LayoutLibrary>();
    if library.get_metadata_revision() != request {
        return;
    }
    KNOWN.with_borrow_mut(|known| {
        known.clear();
        for (name, entry) in names.iter().zip(&loaded) {
            if let Some(text) = &entry.text {
                known.insert(name.clone(), text.clone());
            }
        }
    });
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
