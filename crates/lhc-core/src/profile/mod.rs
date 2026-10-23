//! The editable configuration model: settings, layouts, actions and the
//! rules that turn them into mapper input. UI-independent; the shell
//! presents and edits it through [`crate::config_document::ConfigDocument`].

pub mod actions;
pub mod app_match;
pub mod auto_switch;
pub mod bundled;
pub mod diagnostics;
pub mod ids;
pub mod layout_file;
pub mod macros;
pub mod model;
pub mod names;
pub mod settings;
pub mod timing;

pub mod menus;

pub mod library;

pub mod key_catalog;
