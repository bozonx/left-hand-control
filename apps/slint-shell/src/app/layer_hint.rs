//! Layer hint: while a mapper layer is held, the Spell worker shows its keys
//! above all windows without taking focus.

use super::App;
use crate::{
    command::{Command, Source},
    pages,
};
use std::time::{Duration, Instant};

/// How long a layer must stay held before its hint appears, so quick layer
/// use by someone who knows the layout does not flash it.
const HINT_DELAY: Duration = Duration::from_millis(100);

impl App {
    /// Apply the saved hint setting: start or stop the worker it needs.
    pub(super) fn sync_layer_hint_setting(&self) {
        let enabled = self
            .document()
            .is_some_and(|document| document.read().settings().layer_hint);
        {
            let mut supervisor = self.supervisor.borrow_mut();
            if supervisor.hint == enabled {
                return;
            }
            supervisor.hint = enabled;
        }
        if enabled {
            self.start_worker(true);
            if self.held_layer.borrow().is_some() {
                self.schedule_layer_hint();
            }
        } else {
            self.hide_layer_hint();
            self.supervisor.borrow_mut().stop_unwanted();
        }
    }

    /// The mapper's topmost held layer changed.
    pub(super) fn layer_changed(&self, layer: Option<String>) {
        self.layer_hint_timer.stop();
        let held = layer.is_some();
        *self.held_layer.borrow_mut() = layer;
        if !held {
            self.hide_layer_hint();
        } else if self.layer_hint_shown.get() {
            // Another layer took over while the hint is up.
            self.show_layer_hint();
        } else {
            self.start_worker(true);
            self.schedule_layer_hint();
        }
    }

    fn schedule_layer_hint(&self) {
        self.layer_hint_timer
            .start(slint::TimerMode::SingleShot, HINT_DELAY, || {
                super::with_app(|app| app.show_layer_hint())
            });
    }

    fn show_layer_hint(&self) {
        let Some(id) = self.held_layer.borrow().clone() else {
            return;
        };
        if !self.supervisor.borrow().hint {
            return;
        }
        let Some(document) = self.document() else {
            return;
        };
        let layout = self.menus();
        let hint = pages::layers::hint(&self.settings, &document.read(), &layout.layout, &id);
        let Some(hint) = hint else {
            self.hide_layer_hint();
            return;
        };
        // Marked before sending: a send that fails after the worker took
        // the command must not leave the hint up with nothing to hide it.
        // A worker that is still starting gets the hint once it is up.
        self.layer_hint_shown.set(true);
        if !self.supervisor.borrow().running() {
            return;
        }
        if let Err(error) = self.send_worker(
            &Command::LayerHint(Some(hint)),
            Source::Mapper,
            Instant::now(),
            None,
        ) {
            log::warn!("layer hint: {error:?}");
            self.schedule_layer_hint();
        }
    }

    /// Send the hint again to a worker that just started.
    pub(super) fn resend_layer_hint(&self) {
        if self.layer_hint_shown.get() {
            self.show_layer_hint();
        }
    }

    pub(super) fn hide_layer_hint(&self) {
        self.layer_hint_timer.stop();
        self.layer_hint_shown.set(false);
        let running = self.supervisor.borrow().running();
        if running
            && let Err(error) = self.send_worker(
                &Command::LayerHint(None),
                Source::Mapper,
                Instant::now(),
                None,
            )
        {
            log::warn!("hide layer hint: {error:?}");
            self.layer_hint_timer
                .start(slint::TimerMode::SingleShot, HINT_DELAY, || {
                    super::with_app(|app| app.hide_layer_hint())
                });
        }
    }
}
