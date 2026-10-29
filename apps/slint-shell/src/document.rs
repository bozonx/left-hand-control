//! The editable configuration of a shell process and its single write path.
//!
//! Pages never touch [`ConfigDocument`] mutably themselves. They edit
//! through [`Document::edit`], which applies the change, pushes it to a
//! running mapper and tells the other pages to refresh. No borrow is held
//! while listeners run, so a refresh that leads to another edit cannot hit a
//! `RefCell` conflict.

use crate::i18n::Msg;
use lhc_core::{
    config_document::{ConfigDocument, ConfigError, RuntimeConfig},
    profile::{
        auto_switch::AutoSwitchContext,
        model::{AppSettings, LayoutPreset},
    },
    storage::StoragePaths,
};
use std::{
    cell::{Cell, Ref, RefCell},
    rc::Rc,
};

/// A part of the UI that shows the document and refreshes on changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Keys,
    Rules,
    Layers,
    Library,
    Macros,
    Menus,
    Settings,
    /// Status bar, popups and everything outside the pages.
    Shell,
}

/// Sections changed by an edit, used to refresh only dependent views.
#[derive(Default)]
struct Changes {
    settings: bool,
    rules: bool,
    layers: bool,
    macros: bool,
    commands: bool,
    library: bool,
    quick: bool,
}

impl Changes {
    fn between(
        layout: &LayoutPreset,
        settings: &AppSettings,
        next: &ConfigDocument,
        origin: View,
    ) -> Self {
        let current = next.layout();
        Self {
            settings: settings != next.settings(),
            rules: layout.rules != current.rules,
            layers: layout.layers != current.layers
                || layout.layer_keymaps != current.layer_keymaps,
            macros: layout.macros != current.macros,
            commands: layout.commands != current.commands,
            library: origin == View::Library,
            quick: layout.quick_action_pages != current.quick_action_pages,
        }
    }

    fn affects(&self, view: View) -> bool {
        if self.settings || self.library {
            return true;
        }
        match view {
            View::Keys => self.rules,
            View::Rules => self.rules || self.layers,
            View::Layers => self.layers || self.rules || self.macros || self.commands,
            View::Settings => {
                self.rules || self.layers || self.macros || self.commands || self.quick
            }
            View::Menus | View::Macros | View::Library | View::Shell => true,
        }
    }
}

struct LibrarySnapshot {
    name: String,
    layout: Option<LayoutPreset>,
}

type Listener = Rc<dyn Fn(&Document)>;

pub struct Document {
    config: RefCell<ConfigDocument>,
    revision: Cell<u64>,
    command_revision: Cell<u64>,
    library_snapshot: RefCell<Option<LibrarySnapshot>>,
    listeners: RefCell<Vec<(View, Listener)>>,
    /// Document revision and layout last given to the running mapper.
    pushed_layout: RefCell<Option<(u64, Option<String>)>>,
}

/// An applied edit and whether the running mapper took it.
#[derive(Debug)]
pub struct Saved<T> {
    pub value: T,
    pub runtime: Result<(), String>,
}

impl<T> Saved<T> {
    /// Keep the applied value even when the mapper could not take the change.
    pub fn into_parts(self) -> (T, Option<Msg>) {
        let warning = self.runtime.err().map(Msg::SavedMapperNotUpdated);
        (self.value, warning)
    }

    /// `ok` when the mapper is up to date, otherwise a warning that the
    /// change was applied to the document but not to the mapper.
    pub fn message(&self, ok: Msg) -> Msg {
        match &self.runtime {
            Ok(()) => ok,
            Err(error) => Msg::SavedMapperNotUpdated(error.clone()),
        }
    }
}

impl Document {
    pub fn new(config: ConfigDocument) -> Rc<Self> {
        Rc::new(Self {
            config: RefCell::new(config),
            revision: Cell::new(0),
            command_revision: Cell::new(0),
            library_snapshot: RefCell::default(),
            listeners: RefCell::default(),
            pushed_layout: RefCell::default(),
        })
    }

    pub fn load(paths: StoragePaths) -> Result<Rc<Self>, ConfigError> {
        ConfigDocument::load(paths).map(Self::new)
    }

    /// Read access. Do not keep the guard across UI calls that may edit.
    pub fn read(&self) -> Ref<'_, ConfigDocument> {
        self.config.borrow()
    }

    /// Call `refresh` when another view changes this view's dependencies.
    pub fn subscribe(&self, view: View, refresh: impl Fn(&Document) + 'static) {
        self.listeners.borrow_mut().push((view, Rc::new(refresh)));
    }

    /// Change the document on behalf of `origin`, update a running mapper
    /// and refresh other views that depend on the changed sections.
    pub fn edit<T>(
        &self,
        origin: View,
        edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
    ) -> Result<Saved<T>, ConfigError> {
        self.apply_operation(origin, |config| config.transaction(edit))
    }

    /// Apply one core library operation, which owns its file rollback.
    /// Do not compose several fallible operations here; use a core operation
    /// that stages the compound change (for example create-and-select).
    pub fn edit_library<T>(
        &self,
        edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
    ) -> Result<Saved<T>, ConfigError> {
        self.apply_operation(View::Library, edit)
    }

    pub fn sync_autostart(&self) -> Result<Saved<bool>, ConfigError> {
        self.apply_operation(View::Shell, ConfigDocument::sync_autostart)
    }

    fn apply_operation<T>(
        &self,
        origin: View,
        edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
    ) -> Result<Saved<T>, ConfigError> {
        let (value, changes) = {
            let mut config = self.config.borrow_mut();
            let layout = config.layout().clone();
            let settings = config.settings().clone();
            let value = edit(&mut config);
            if let Err(error) = &value
                && layout == *config.layout()
                && settings == *config.settings()
            {
                return Err(error.clone());
            }
            (value, Changes::between(&layout, &settings, &config, origin))
        };
        self.revision.set(self.revision.get().wrapping_add(1));
        if changes.affects(View::Settings) {
            self.command_revision
                .set(self.command_revision.get().wrapping_add(1));
        }
        if changes.library {
            self.invalidate_library();
        }
        let runtime = self.sync_runtime(true);
        self.notify(Some(origin), Some(&changes));
        value.map(|value| Saved { value, runtime })
    }

    /// Re-read library contents on the next request (library edits or explicit refresh).
    pub(crate) fn invalidate_library(&self) {
        self.command_revision
            .set(self.command_revision.get().wrapping_add(1));
        self.library_snapshot.borrow_mut().take();
    }

    /// Saved library layout used by dirty indicators; disk reads happen once per refresh.
    pub(crate) fn saved_layout(&self, name: &str) -> Option<LayoutPreset> {
        let mut snapshot = self.library_snapshot.borrow_mut();
        if snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.name != name)
        {
            *snapshot = Some(LibrarySnapshot {
                name: name.to_owned(),
                layout: self
                    .read()
                    .load_layout(&lhc_core::profile::model::user_layout_id(name))
                    .ok(),
            });
        }
        snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.layout.clone())
    }

    pub(crate) fn command_revision(&self) -> u64 {
        self.command_revision.get()
    }

    /// Revision captured together with a mapper startup snapshot.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Mapper input for the current system state.
    pub fn runtime_config(&self) -> Result<RuntimeConfig, ConfigError> {
        self.read().runtime_config(&AutoSwitchContext::current())
    }

    /// Bring a running mapper up to date. With `force` the configuration is
    /// always sent; otherwise only when the document or active layout changed (system
    /// context changes). A stopped mapper is left alone. A failed edit
    /// stops the mapper, which would run outdated rules; a failed context
    /// switch (e.g. a game started) keeps the current layout running and
    /// is retried on the next change.
    pub fn sync_runtime(&self, force: bool) -> Result<(), String> {
        if !lhc_core::mapper::runtime::status().running {
            self.pushed_layout.borrow_mut().take();
            return Ok(());
        }
        let document_changed = self
            .pushed_layout
            .borrow()
            .as_ref()
            .is_some_and(|(revision, _)| *revision != self.revision.get());
        let result = (|| {
            let runtime = self.runtime_config().map_err(|error| error.to_string())?;
            lhc_core::active_window::set_titles_needed(runtime.uses_titles);
            if !force && self.runtime_is_current(&runtime.layout_id) {
                return Ok(());
            }
            lhc_core::mapper::runtime::update_config_if_running(runtime.mapper)?;
            *self.pushed_layout.borrow_mut() = Some((self.revision.get(), runtime.layout_id));
            Ok(())
        })();
        if result.is_err() && (force || document_changed) {
            self.pushed_layout.borrow_mut().take();
            if let Err(error) = lhc_core::mapper::runtime::stop() {
                log::warn!("stop outdated mapper: {error}");
            }
        }
        result
    }

    fn runtime_is_current(&self, layout_id: &Option<String>) -> bool {
        self.pushed_layout
            .borrow()
            .as_ref()
            .is_some_and(|(revision, id)| *revision == self.revision.get() && id == layout_id)
    }

    /// Record the layout a freshly started mapper runs.
    pub fn mapper_started(&self, revision: u64, layout_id: Option<String>) {
        *self.pushed_layout.borrow_mut() = Some((revision, layout_id));
    }

    /// Refresh every view, for example after the language changed.
    pub fn refresh_all(&self) {
        self.command_revision
            .set(self.command_revision.get().wrapping_add(1));
        self.invalidate_library();
        self.notify(None, None);
    }

    fn notify(&self, origin: Option<View>, changes: Option<&Changes>) {
        let listeners: Vec<Listener> = self
            .listeners
            .borrow()
            .iter()
            .filter(|(view, _)| {
                Some(*view) != origin && changes.is_none_or(|changes| changes.affects(*view))
            })
            .map(|(_, listener)| listener.clone())
            .collect();
        for listener in listeners {
            listener(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn document() -> (tempfile::TempDir, Rc<Document>) {
        let dir = tempfile::tempdir().unwrap();
        let paths = StoragePaths::new(dir.path().join("settings"), dir.path().join("data"));
        let document = Document::load(paths).unwrap();
        (dir, document)
    }

    #[test]
    fn startup_snapshot_does_not_hide_edits_with_the_same_layout_id() {
        let (_dir, document) = document();
        let started_revision = document.revision();
        let layout_id = document.runtime_config().unwrap().layout_id;
        document.mapper_started(started_revision, layout_id.clone());
        assert!(document.runtime_is_current(&layout_id));
        document
            .edit(View::Keys, |config| {
                config.set_base_tap_action("KeyA", "KeyB")
            })
            .unwrap();
        document.mapper_started(started_revision, layout_id.clone());
        assert!(!document.runtime_is_current(&layout_id));
        assert_eq!(document.runtime_config().unwrap().layout_id, layout_id);
        document.mapper_started(document.revision(), layout_id.clone());
        assert!(document.runtime_is_current(&layout_id));
        assert!(!document.runtime_is_current(&Some("user:other".into())));
        let revision = document.revision();
        assert!(
            document
                .edit(View::Settings, |config| {
                    config.update_settings(|settings| settings.default_hold_timeout_ms = 0)
                })
                .is_err()
        );
        assert_eq!(document.revision(), revision);
        assert!(document.runtime_is_current(&layout_id));
    }

    #[test]
    fn mapper_warning_does_not_discard_an_applied_edit() {
        let saved = Saved {
            value: "saved file contents",
            runtime: Err("mapper failed".into()),
        };
        let (value, warning) = saved.into_parts();
        assert_eq!(value, "saved file contents");
        assert!(
            matches!(warning, Some(Msg::SavedMapperNotUpdated(error)) if error == "mapper failed")
        );
    }

    #[test]
    fn command_revision_skips_emoji_edits_and_tracks_assignments_and_library_refresh() {
        let (_dir, document) = document();
        let initial = document.command_revision();
        document
            .edit(View::Menus, |config| {
                config.update_layout(|layout| layout.emoji_pages[0].name = "Emoji".into())
            })
            .unwrap();
        assert_eq!(document.command_revision(), initial);
        document
            .edit(View::Menus, |config| {
                config.update_layout(|layout| layout.quick_action_pages[0].name = "Quick".into())
            })
            .unwrap();
        assert_ne!(document.command_revision(), initial);
        let before_refresh = document.command_revision();
        document.refresh_all();
        assert_ne!(document.command_revision(), before_refresh);
    }

    #[test]
    fn menu_edits_skip_unrelated_keyboard_views() {
        let (_dir, document) = document();
        let counters: Vec<_> = [
            View::Keys,
            View::Rules,
            View::Layers,
            View::Macros,
            View::Library,
        ]
        .into_iter()
        .map(|view| {
            let count = Rc::new(Cell::new(0));
            let next = count.clone();
            document.subscribe(view, move |_| next.set(next.get() + 1));
            count
        })
        .collect();
        document
            .edit(View::Menus, |config| {
                config.update_layout(|layout| {
                    layout.emoji_pages[0].name = "Edited".into();
                })
            })
            .unwrap();
        assert_eq!(
            counters.iter().map(|count| count.get()).collect::<Vec<_>>(),
            [0, 0, 0, 1, 1]
        );
        document.refresh_all();
        assert_eq!(
            counters.iter().map(|count| count.get()).collect::<Vec<_>>(),
            [1, 1, 1, 2, 2]
        );
    }

    #[test]
    fn library_snapshot_refreshes_after_library_edits_and_explicit_refresh() {
        let (_dir, document) = document();
        document
            .edit_library(|config| config.save_current_layout_as("Saved"))
            .unwrap();
        let before = document.saved_layout("Saved").unwrap();
        document
            .edit(View::Layers, |config| {
                config.update_layout(|layout| layout.description = Some("working".into()))
            })
            .unwrap();
        assert_eq!(document.saved_layout("Saved"), Some(before.clone()));
        document
            .edit_library(|config| config.overwrite_library_layout("Saved"))
            .unwrap();
        assert_eq!(
            document
                .saved_layout("Saved")
                .unwrap()
                .description
                .as_deref(),
            Some("working")
        );
        let text = lhc_core::profile::layout_file::serialize(&before);
        document
            .read()
            .paths()
            .save_user_layout("Saved", &text, true)
            .unwrap();
        document.refresh_all();
        assert_eq!(document.saved_layout("Saved"), Some(before));
    }

    #[test]
    fn edits_notify_every_other_view() {
        let (_dir, document) = document();
        let rules = Rc::new(Cell::new(0));
        let layers = Rc::new(Cell::new(0));
        let counter = rules.clone();
        document.subscribe(View::Rules, move |_| counter.set(counter.get() + 1));
        let counter = layers.clone();
        document.subscribe(View::Layers, move |_| counter.set(counter.get() + 1));
        let saved = document
            .edit(View::Rules, |config| config.create_layer("Nav", ""))
            .unwrap();
        assert!(saved.runtime.is_ok());
        assert_eq!((rules.get(), layers.get()), (0, 1));
        assert_eq!(document.read().layout().layers.len(), 1);
    }

    #[test]
    fn listeners_may_edit_again() {
        let (_dir, document) = document();
        let done = Rc::new(Cell::new(false));
        let flag = done.clone();
        document.subscribe(View::Layers, move |document| {
            if !flag.replace(true) {
                document
                    .edit(View::Layers, |config| config.create_layer("Second", ""))
                    .unwrap();
            }
        });
        document
            .edit(View::Rules, |config| config.create_layer("First", ""))
            .unwrap();
        assert_eq!(document.read().layout().layers.len(), 2);
    }
    #[test]
    fn failed_compound_edit_does_not_advance_revision_or_notify_views() {
        let (_dir, document) = document();
        let revision = document.revision();
        let before = document.read().layout().clone();
        let notified = Rc::new(Cell::new(false));
        let flag = notified.clone();
        document.subscribe(View::Shell, move |_| flag.set(true));
        let result: Result<Saved<()>, ConfigError> = document.edit(View::Layers, |config| {
            config.create_layer("Staged", "")?;
            Err(ConfigError::EditConflict)
        });
        assert!(matches!(result, Err(ConfigError::EditConflict)));
        assert_eq!(document.revision(), revision);
        assert_eq!(document.read().layout(), &before);
        assert!(!notified.get());
    }
}
