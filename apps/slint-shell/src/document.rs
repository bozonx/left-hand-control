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
    profile::auto_switch::AutoSwitchContext,
    storage::StoragePaths,
};
use std::{
    cell::{Ref, RefCell},
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

type Listener = Rc<dyn Fn(&Document)>;

pub struct Document {
    config: RefCell<ConfigDocument>,
    ui_state: RefCell<lhc_core::ui_state::UiState>,
    listeners: RefCell<Vec<(View, Listener)>>,
    /// Layout last given to the running mapper; `None` before the first push.
    pushed_layout: RefCell<Option<Option<String>>>,
}

/// An applied edit and whether the running mapper took it.
#[derive(Debug)]
pub struct Saved<T> {
    pub value: T,
    pub runtime: Result<(), String>,
}

impl<T> Saved<T> {
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
        let ui_state = lhc_core::ui_state::UiState::default();
        Rc::new(Self {
            ui_state: RefCell::new(ui_state),
            config: RefCell::new(config),
            listeners: RefCell::default(),
            pushed_layout: RefCell::default(),
        })
    }

    pub fn load(paths: StoragePaths) -> Result<Rc<Self>, ConfigError> {
        ConfigDocument::load(paths).map(Self::new)
    }

    pub fn selected_layer_id(&self) -> String {
        self.ui_state.borrow().selected_layer_id().to_owned()
    }

    pub fn label_mode(&self) -> i32 {
        self.ui_state.borrow().label_mode()
    }

    pub fn update_ui_state(&self, layer: Option<&str>, mode: Option<i32>) {
        self.ui_state.borrow_mut().update(layer, mode)
    }

    /// Read access. Do not keep the guard across UI calls that may edit.
    pub fn read(&self) -> Ref<'_, ConfigDocument> {
        self.config.borrow()
    }

    /// Call `refresh` whenever the document changes outside `view`.
    pub fn subscribe(&self, view: View, refresh: impl Fn(&Document) + 'static) {
        self.listeners.borrow_mut().push((view, Rc::new(refresh)));
    }

    /// Change the document on behalf of `origin`, update a running mapper
    /// and refresh every other view.
    /// When another process changed the files first, the document reloads
    /// them, refreshes every view and returns [`ConfigError::ExternalChange`].
    pub fn edit<T>(
        &self,
        origin: View,
        edit: impl FnOnce(&mut ConfigDocument) -> Result<T, ConfigError>,
    ) -> Result<Saved<T>, ConfigError> {
        let result = edit(&mut self.config.borrow_mut());
        match result {
            Ok(value) => {
                let runtime = self.sync_runtime(true);
                self.notify(Some(origin));
                Ok(Saved { value, runtime })
            }
            Err(ConfigError::ExternalChange) => {
                if let Err(error) = self.reload() {
                    log::warn!("reload after external change: {error}");
                }
                Err(ConfigError::ExternalChange)
            }
            Err(error) => Err(error),
        }
    }

    /// Pick up changes other processes made. `Ok(None)` when nothing
    /// changed; otherwise every view was refreshed.
    pub fn reload(&self) -> Result<Option<Saved<()>>, ConfigError> {
        let changed = self.config.borrow_mut().reload_if_changed()?;
        if !changed {
            return Ok(None);
        }
        let runtime = self.sync_runtime(true);
        self.notify(None);
        Ok(Some(Saved { value: (), runtime }))
    }

    /// Mapper input for the current system state.
    pub fn runtime_config(&self) -> Result<RuntimeConfig, ConfigError> {
        self.read().runtime_config(&AutoSwitchContext::current())
    }

    /// Bring a running mapper up to date. With `force` the configuration is
    /// always sent; otherwise only when the active layout changed (system
    /// context changes). A stopped mapper is left alone. A failed edit
    /// stops the mapper, which would run outdated rules; a failed context
    /// switch (e.g. a game started) keeps the current layout running and
    /// is retried on the next change.
    pub fn sync_runtime(&self, force: bool) -> Result<(), String> {
        if !lhc_core::mapper::runtime::status().running {
            self.pushed_layout.borrow_mut().take();
            return Ok(());
        }
        let result = (|| {
            let runtime = self.runtime_config().map_err(|error| error.to_string())?;
            if !force && self.pushed_layout.borrow().as_ref() == Some(&runtime.layout_id) {
                return Ok(());
            }
            lhc_core::mapper::runtime::update_config_if_running(&runtime.json)?;
            *self.pushed_layout.borrow_mut() = Some(runtime.layout_id);
            Ok(())
        })();
        if result.is_err() && force {
            self.pushed_layout.borrow_mut().take();
            if let Err(error) = lhc_core::mapper::runtime::stop() {
                log::warn!("stop outdated mapper: {error}");
            }
        }
        result
    }

    /// Record the layout a freshly started mapper runs.
    pub fn mapper_started(&self, layout_id: Option<String>) {
        *self.pushed_layout.borrow_mut() = Some(layout_id);
    }

    /// Refresh every view, for example after the language changed.
    pub fn refresh_all(&self) {
        self.notify(None);
    }

    fn notify(&self, origin: Option<View>) {
        let listeners: Vec<Listener> = self
            .listeners
            .borrow()
            .iter()
            .filter(|(view, _)| Some(*view) != origin)
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
    fn external_changes_reload_and_refresh_the_origin_too() {
        let (_dir, document) = document();
        let other = ConfigDocument::load(document.read().paths().clone()).unwrap();
        let mut other = other;
        other.create_layer("Elsewhere", "").unwrap();
        other.save_current_layout_as("Elsewhere").unwrap();
        let refreshed = Rc::new(Cell::new(0));
        let counter = refreshed.clone();
        document.subscribe(View::Rules, move |_| counter.set(counter.get() + 1));
        let error = document
            .edit(View::Rules, |config| config.create_layer("Here", ""))
            .unwrap_err();
        assert_eq!(error, ConfigError::ExternalChange);
        assert_eq!(refreshed.get(), 1);
        assert_eq!(document.read().layout().layers[0].name, "Elsewhere");
    }

    #[test]
    fn external_library_edits_refresh_views_and_runtime_configuration() {
        let (_dir, document) = document();
        let paths = document.read().paths().clone();
        let layout = "layers: []\nrules:\n  - key: KeyQ\n    tap: Escape\n";
        paths.save_user_layout("Nav", layout, false).unwrap();
        document
            .edit(View::Library, |config| {
                config.update_settings(|settings| {
                    settings.manual_active_layout_id = Some("user:Nav".into());
                })
            })
            .unwrap();
        document.reload().unwrap();
        let refreshed = Rc::new(Cell::new(0));
        let count = refreshed.clone();
        document.subscribe(View::Shell, move |_| count.set(count.get() + 1));
        paths
            .save_user_layout("Nav", &layout.replace("Escape", "Enter"), true)
            .unwrap();
        assert!(document.reload().unwrap().unwrap().runtime.is_ok());
        assert_eq!(refreshed.get(), 1);
        let config: serde_json::Value =
            serde_json::from_str(&document.runtime_config().unwrap().json).unwrap();
        assert_eq!(config["rules"][0]["tapAction"], "Enter");
        assert!(document.reload().unwrap().is_none());
    }

    #[test]
    fn reload_reports_unchanged_files() {
        let (_dir, document) = document();
        assert!(document.reload().unwrap().is_none());
    }
}
