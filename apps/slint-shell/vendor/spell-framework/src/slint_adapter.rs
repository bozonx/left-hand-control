//! This module contains relevent structs for slint side backend configurations.
//! All structs mentioned are either internal or not used anymore. Still their
//! implementation is public because they had to be set by the user of library
//! in intial iterations of spell_framework.
use slint::platform::{EventLoopProxy, Platform, WindowAdapter};
use smithay_client_toolkit::reexports::calloop;
use std::{cell::RefCell, io::Read, rc::Rc};
use tracing::{Level, info, span, warn};
use wl_clipboard_rs::{
    copy::{MimeType as CopyMimeType, Options, Source},
    paste::{ClipboardType, Error, MimeType as PasteMimeType, Seat, get_contents},
};

thread_local! {
    pub(crate) static ADAPTERS: RefCell<Vec<Rc<SpellSkiaWinAdapter>>> = const { RefCell::new(Vec::new()) };
}

#[cfg(not(docsrs))]
#[cfg(feature = "i-slint-renderer-skia")]
use crate::skia_non_docs::SpellSkiaWinAdapterReal;

/// It is the main struct handling the rendering of pixels in the wayland window. It implements slint's
/// [WindowAdapter](https://docs.rs/slint/latest/slint/platform/trait.WindowAdapter.html) trait.
/// It is used internally by [SpellMultiWinHandler] and previously by [SpellLayerShell]. This
/// adapter internally uses [Skia](https://skia.org/) 2D graphics library for rendering.
#[cfg(not(docsrs))]
#[cfg(feature = "i-slint-renderer-skia")]
pub type SpellSkiaWinAdapter = SpellSkiaWinAdapterReal;

#[cfg(docsrs)]
use crate::dummy_skia_docs::SpellSkiaWinAdapterDummy;

/// It is the main struct handling the rendering of pixels in the wayland window. It implements slint's
/// [WindowAdapter](https://docs.rs/slint/latest/slint/platform/trait.WindowAdapter.html) trait.
/// It is used internally by [SpellMultiWinHandler] and previously by [SpellLayerShell]. This
/// adapter internally uses [Skia](https://skia.org/) 2D graphics library for rendering.
#[cfg(docsrs)]
pub type SpellSkiaWinAdapter = SpellSkiaWinAdapterDummy;

/// Previously needed to be implemented, now this struct is called and set internally
/// when [`invoke_spell`](crate::wayland_adapter::SpellWin::invoke_spell) is called.
pub struct SpellLayerShell {
    /// Span storing the logging context for `debug`` statements of slint.
    pub span: span::Span,
    slint_event_sender: calloop::channel::Sender<Box<dyn FnOnce() + Send>>,
}

impl SpellLayerShell {
    /// Creates an instance of this Platform implementation, for internal use.
    pub(crate) fn new(
        slint_event_sender: calloop::channel::Sender<Box<dyn FnOnce() + Send>>,
    ) -> Self {
        Self {
            span: span!(Level::INFO, "slint-log",),
            slint_event_sender,
        }
    }
}

// impl Default for SpellLayerShell {
//     /// Creates an instance of this Platform implementation, for internal use.
//     fn default() -> Self {
//         SpellLayerShell {
//             span: span!(Level::INFO, "slint-log",),
//         }
//     }
// }

impl Platform for SpellLayerShell {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let adapter = ADAPTERS.with(|v| v.borrow().last().unwrap().clone());
        Ok(adapter)
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        self.span.in_scope(|| {
            if let Some(val) = arguments.as_str() {
                info!(val);
            } else {
                info!("{}", arguments.to_string());
            }
        })
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(SlintEventProxy(self.slint_event_sender.clone())))
    }

    // FIXME: this implementation should be by smithay-clipboard.
    // fix after state management moved from window to platform.
    fn set_clipboard_text(&self, text: &str, _clipboard: slint::platform::Clipboard) {
        // if let DefaultClipboard = clipboard {
        let opts = Options::new();
        if let Err(err) = opts.copy(
            Source::Bytes(text.to_string().into_bytes().into()),
            CopyMimeType::Autodetect,
        ) {
            warn!("[Clipboard]: Error in setting clipboard value: {}", err);
        } else {
            info!("[Clipboard]: Successfully copied text");
        }
        //}
    }

    // FIXME: this implementation should be by smithay-clipboard.
    // fix after state management moved from window to platform.
    fn clipboard_text(&self, _clipboard: slint::platform::Clipboard) -> Option<String> {
        let result = get_contents(
            ClipboardType::Regular,
            Seat::Unspecified,
            PasteMimeType::Text,
        );
        match result {
            Ok((mut pipe, _)) => {
                let mut contents = vec![];
                // TODO: handle the below unwrap properly.
                pipe.read_to_end(&mut contents).unwrap();
                let text = String::from_utf8_lossy(&contents).to_string();
                info!("[Clipboard]: Successfully pasted text");
                Some(text)
            }

            // In this cases, an empty string is returned.
            Err(Error::NoSeats) | Err(Error::ClipboardEmpty) | Err(Error::NoMimeType) => {
                warn!("[Clipboard]: Clipboard was either empty or didn't have text type data");
                Some("".to_string())
            }

            Err(err) => {
                warn!("[Clipboard]: error getting clipboard text: {}", err);
                None
            }
        }
    }
}

struct SlintEventProxy(calloop::channel::Sender<Box<dyn FnOnce() + Send>>);

impl EventLoopProxy for SlintEventProxy {
    fn quit_event_loop(&self) -> Result<(), i_slint_core::api::EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), i_slint_core::api::EventLoopError> {
        self.0
            .send(event)
            .map_err(|_| i_slint_core::api::EventLoopError::EventLoopTerminated)
    }
}
