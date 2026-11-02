//! Working copy context, dirty indicators and explicit saves.

use super::*;

pub(super) fn refresh_context(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let library = ui.global::<LayoutLibrary>();
    let name = current_name(&config);
    library.set_current_label(name.clone().unwrap_or_default().into());
    let current = config.layout();
    let flags = match &name {
        Some(name) => document.with_saved_layout(name, |saved| {
            DirtyFlags::compare(
                saved.map(|saved| (&saved.layout, saved.text.as_str())),
                current,
            )
        }),
        None => {
            let initial = LayoutPreset::initial();
            let text = layout_file::serialize(&initial);
            DirtyFlags::compare(Some((&initial, &text)), current)
        }
    };
    drop(config);
    library.set_dirty(flags.any);
    library.set_rules_dirty(flags.rules);
    library.set_layers_dirty(flags.layers);
    library.set_macros_dirty(flags.macros);
    library.set_quick_dirty(flags.quick);
    library.set_emoji_dirty(flags.emoji);
    library.set_commands_dirty(flags.commands);
}

/// Which parts of the working copy differ from its saved library file.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct DirtyFlags {
    pub any: bool,
    pub rules: bool,
    pub layers: bool,
    pub macros: bool,
    pub quick: bool,
    pub emoji: bool,
    pub commands: bool,
}

impl DirtyFlags {
    /// Compare the working copy with the saved layout and its serialized
    /// text. Both sides go through the same parser, so formatting
    /// differences in the file do not count as changes.
    pub(super) fn compare(saved: Option<(&LayoutPreset, &str)>, current: &LayoutPreset) -> Self {
        let Some((saved, saved_text)) = saved else {
            // No saved file: everything counts as unsaved.
            return Self {
                any: true,
                rules: true,
                layers: true,
                macros: true,
                quick: true,
                emoji: true,
                commands: true,
            };
        };
        // Fast path for the common clean state: no serialization at all.
        if saved == current {
            return Self::default();
        }
        let text = layout_file::serialize(current);
        let normalized = layout_file::parse(&text).ok().flatten();
        let current = normalized.as_ref().unwrap_or(current);
        let any = match &normalized {
            Some(normalized) => layout_file::serialize(normalized) != saved_text,
            None => text != saved_text,
        };
        let base = crate::keyboard::BASE
            .iter()
            .any(|key| saved.base_tap_action(key) != current.base_tap_action(key));
        Self {
            any,
            rules: saved.rules != current.rules,
            layers: base
                || saved.layers != current.layers
                || saved.layer_keymaps != current.layer_keymaps,
            macros: saved.macros != current.macros,
            quick: saved.quick_action_pages != current.quick_action_pages,
            emoji: saved.emoji_pages != current.emoji_pages,
            commands: saved.commands != current.commands,
        }
    }
}

/// Name of the layout the mapper uses now and the automatic rule that
/// chose it.
pub fn refresh_active(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let (label, matched, message) = active_context(&config, &AutoSwitchContext::current());
    drop(config);
    let library = ui.global::<LayoutLibrary>();
    library.set_active_error(message.to_ui());
    library.set_active_label(label.into());
    library.set_auto_matched(matched);
    let game = lhc_core::gamemode::status();
    library.set_game_detection(
        game.detection_enabled || game.control != lhc_core::gamemode::GameModeControl::Auto,
    );
}

/// Presentation state keeps a failed lookup distinct from intentional passthrough.
fn active_context(config: &ConfigDocument, context: &AutoSwitchContext) -> (String, i32, Msg) {
    let active = config.active_layout_id(context);
    let choice = config.auto_choice(context);
    let error = active.as_ref().err().or_else(|| choice.as_ref().err());
    let message = error.map_or(Msg::None, |error| {
        log::warn!("determining the active layout: {error}");
        Msg::LayoutLoadFailed(error.to_string())
    });
    let label = active
        .ok()
        .flatten()
        .and_then(|id| user_layout_name(&id).map(str::to_owned))
        .unwrap_or_default();
    let matched = choice
        .ok()
        .and_then(|choice| choice.matched_rule_index)
        .map_or(-1, |index| index as i32);
    (label, matched, message)
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
    metadata::remember(&name, Some(text.clone()));
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
    metadata::remember(&saved, Some(text.clone()));
    state.borrow_mut().baseline = Some((saved.clone(), text));
    ui.global::<LayoutLibrary>().set_save_as_open(false);
    refresh(ui, document, &mut state.borrow_mut())?;
    Ok(Msg::LayoutSaved(saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_context_reports_library_failure_and_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let paths = lhc_core::storage::StoragePaths::new(
            dir.path().join("config"),
            dir.path().join("data"),
        );
        let mut config = ConfigDocument::load(paths.clone()).unwrap();
        config
            .update_settings(|settings| settings.layout_mode = LayoutMode::Auto)
            .unwrap();
        let context = AutoSwitchContext::default();
        let (label, matched, message) = active_context(&config, &context);
        assert!(label.is_empty() && matched == -1 && message.is_none());
        let layouts = paths.layouts_dir();
        let backup = dir.path().join("layouts-backup");
        std::fs::create_dir_all(&layouts).unwrap();
        std::fs::rename(&layouts, &backup).unwrap();
        std::fs::write(&layouts, "not a directory").unwrap();
        let (label, matched, message) = active_context(&config, &context);
        assert!(label.is_empty() && matched == -1);
        assert!(matches!(message, Msg::LayoutLoadFailed(_)));
        std::fs::remove_file(&layouts).unwrap();
        std::fs::rename(&backup, &layouts).unwrap();
        assert!(active_context(&config, &context).2.is_none());
    }

    fn flags(saved: &LayoutPreset, current: &LayoutPreset) -> DirtyFlags {
        DirtyFlags::compare(Some((saved, &layout_file::serialize(saved))), current)
    }

    #[test]
    fn dirty_flags_follow_changed_sections() {
        let saved = LayoutPreset::initial();
        assert_eq!(flags(&saved, &saved), DirtyFlags::default());

        let mut described = saved.clone();
        described.description = Some("changed".into());
        let result = flags(&saved, &described);
        assert!(result.any);
        assert!(!result.rules && !result.layers && !result.macros);

        let mut no_rules = saved.clone();
        no_rules.rules.clear();
        if no_rules != saved {
            let result = flags(&saved, &no_rules);
            assert!(result.any && result.rules && !result.emoji);
        }

        let missing = DirtyFlags::compare(None, &saved);
        assert!(missing.any && missing.rules && missing.commands);
    }
}
