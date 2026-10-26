//! Working copy context, dirty indicators and explicit saves.

use super::*;

pub(super) fn refresh_context(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let library = ui.global::<LayoutLibrary>();
    let name = current_name(&config);
    library.set_current_label(name.clone().unwrap_or_default().into());
    let named = name.is_some();
    let saved = name
        .and_then(|name| document.saved_layout(&name))
        .or_else(|| (!named).then(LayoutPreset::initial));
    let current = config.layout();
    let normalized = layout_file::parse(&layout_file::serialize(current))
        .ok()
        .flatten();
    let current = normalized.as_ref().unwrap_or(current);
    let dirty = saved
        .as_ref()
        .is_none_or(|saved| layout_file::serialize(saved) != layout_file::serialize(current));
    library.set_dirty(dirty);
    let changed = |section: fn(&LayoutPreset, &LayoutPreset) -> bool| {
        saved
            .as_ref()
            .map_or(dirty, |saved| section(saved, current))
    };
    library.set_rules_dirty(changed(|saved, current| saved.rules != current.rules));
    let base_dirty = saved.as_ref().map_or(dirty, |saved| {
        crate::keyboard::BASE
            .iter()
            .any(|key| saved.base_tap_action(key) != current.base_tap_action(key))
    });
    library.set_layers_dirty(
        base_dirty
            || changed(|saved, current| {
                saved.layers != current.layers || saved.layer_keymaps != current.layer_keymaps
            }),
    );
    library.set_macros_dirty(changed(|saved, current| saved.macros != current.macros));
    library.set_quick_dirty(changed(|saved, current| {
        saved.quick_action_pages != current.quick_action_pages
    }));
    library.set_emoji_dirty(changed(|saved, current| {
        saved.emoji_pages != current.emoji_pages
    }));
    library.set_commands_dirty(changed(|saved, current| saved.commands != current.commands));
}

/// Name of the layout the mapper uses now and the automatic rule that
/// chose it.
pub fn refresh_active(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let context = AutoSwitchContext::current();
    let label = config
        .active_layout_id(&context)
        .ok()
        .flatten()
        .and_then(|id| user_layout_name(&id).map(str::to_owned))
        .unwrap_or_default();
    let matched = config
        .auto_choice(&context)
        .ok()
        .and_then(|choice| choice.rule)
        .map_or(-1, |index| index as i32);
    drop(config);
    let library = ui.global::<LayoutLibrary>();
    library.set_active_label(label.into());
    library.set_auto_matched(matched);
    let game = lhc_core::gamemode::status();
    library.set_game_detection(
        game.detection_enabled || game.control != lhc_core::gamemode::GameModeControl::Auto,
    );
}

/// Remember the library file of the working copy as its saved state.
pub(super) fn reset_context(ui: &SettingsWindow, document: &Document, state: &mut State) {
    let config = document.read();
    state.baseline = current_name(&config).and_then(|name| {
        let text = config.paths().load_user_layout(&name).ok()?;
        Some((name, text))
    });
    drop(config);
    refresh_context(ui, document);
}

/// Save the working copy back to its library file. When the file was
/// changed elsewhere, ask whether to overwrite, save a copy or reload.
pub(super) fn save_current(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
) -> Result<Msg, Msg> {
    let library = ui.global::<LayoutLibrary>();
    let target = document.read().library_save_target(
        state
            .borrow()
            .baseline
            .as_ref()
            .map(|(name, text)| (name.as_str(), text.as_str())),
    );
    match target {
        LibrarySaveTarget::NeedsName => {
            library.set_save_as_name("".into());
            library.set_save_as_open(true);
            Ok(Msg::None)
        }
        LibrarySaveTarget::Conflict => {
            library.set_dialog(LibraryDialog::Conflict);
            Ok(Msg::None)
        }
        LibrarySaveTarget::Existing(name) => write_current(ui, document, state, name),
    }
}

/// Write the working copy over its library file.
pub(super) fn overwrite_current(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
) -> Result<Msg, Msg> {
    let name = current_name(&document.read()).ok_or(Msg::LibrarySelect)?;
    ui.global::<LayoutLibrary>().set_dialog(LibraryDialog::None);
    write_current(ui, document, state, name)
}

fn write_current(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    name: String,
) -> Result<Msg, Msg> {
    let text = edit(ui, document, state, |config| {
        config.overwrite_library_layout(&name)
    })?;
    state.borrow_mut().baseline = Some((name.clone(), text));
    refresh_context(ui, document);
    Ok(Msg::LayoutSaved(name))
}

/// Save the working copy as a new library layout and continue editing it.
pub(super) fn save_as(
    ui: &SettingsWindow,
    document: &Document,
    state: &Shared,
    name: &str,
) -> Result<Msg, Msg> {
    let (saved, text) = edit(ui, document, state, |config| {
        config.save_working_layout_as(name)
    })?;
    state.borrow_mut().baseline = Some((saved.clone(), text));
    ui.global::<LayoutLibrary>().set_save_as_open(false);
    refresh(ui, document, &mut state.borrow_mut())?;
    Ok(Msg::LayoutSaved(saved))
}
