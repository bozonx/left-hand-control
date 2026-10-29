//! Background metadata snapshots for the library cards.

use crate::{
    i18n::Msg,
    ui::{LayoutLibrary, LayoutSummary, SettingsWindow},
};
use lhc_core::{
    profile::{library::load_library_entries, model::LayoutPreset},
    storage::StoragePaths,
};
use slint::{ComponentHandle, Model};

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
/// while files load, including an inline draft after a failed save.
pub(super) fn load(ui: &SettingsWindow, paths: StoragePaths, names: Vec<String>) {
    let library = ui.global::<LayoutLibrary>();
    let request = library.get_metadata_revision().wrapping_add(1);
    library.set_metadata_revision(request);
    let ui = ui.as_weak();
    std::thread::spawn(move || {
        let loaded = load_library_entries(&paths, &names);
        let _ = ui.upgrade_in_event_loop(move |ui| {
            let library = ui.global::<LayoutLibrary>();
            if library.get_metadata_revision() != request {
                return;
            }
            let rows = library.get_rows();
            for (index, result) in loaded.into_iter().enumerate() {
                let Some(mut row) = rows.row_data(index) else {
                    continue;
                };
                row.loaded = true;
                match result {
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
        });
    });
}
