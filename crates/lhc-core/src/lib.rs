//! UI-independent core of Left Hand Control: configuration model and storage,
//! platform detection, system watchers, the mapper and the event bus that
//! carries core state to a shell.

// Runtime paths return errors instead of panicking; tests may unwrap (clippy.toml).
#![warn(clippy::unwrap_used)]

pub mod active_window;
pub mod autostart;
pub mod config_document;
pub mod env;
pub mod events;
#[cfg(target_os = "linux")]
pub mod exec;
pub mod gamemode;
pub mod layout;
pub mod mapper;
pub mod platform;
pub mod profile;
pub mod runtime_state;
pub mod storage;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod text_input;

pub use events::{CoreEvent, EventBus};
pub use storage::StoragePaths;
