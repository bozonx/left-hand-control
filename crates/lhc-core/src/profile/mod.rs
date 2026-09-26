//! The editable configuration model: settings, layouts, actions and the
//! rules that turn them into mapper input. UI-independent; both shells
//! present and edit it through [`crate::config_document::ConfigDocument`].

pub mod actions;
pub mod auto_switch;
pub mod diagnostics;
pub mod ids;
pub mod layout_file;
pub mod macros;
pub mod model;
pub mod settings;

pub mod menus;
