//! Slint shell of Left Hand Control.
//!
//! One binary runs in three roles: the settings process (`app`), the Spell
//! popup worker (`--spell-worker`, Linux + feature `spell`) and a CLI client
//! that forwards a command to a running instance over IPC.

pub mod ui {
    slint::include_modules!();
}

mod app;
pub mod command;
pub mod editor;
mod i18n;
mod ipc;
pub mod macro_editor;
pub mod menu_editor;
mod metrics;
mod platform;
pub mod popup_model;
#[cfg(all(feature = "spell", target_os = "linux"))]
mod spell;
#[cfg(all(feature = "spell", target_os = "linux"))]
pub mod test_keyboard;

use std::time::Instant;

/// Entry point shared by all roles; `args` excludes the program name.
pub fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,zbus=warn,tracing=warn"),
    )
    .init();
    #[cfg(all(feature = "spell", target_os = "linux"))]
    if args == ["--spell-worker"] {
        return spell::run(start);
    }
    if !args.is_empty() {
        let command = command::Command::parse(&args.join(" "))?;
        return ipc::client(&command);
    }
    app::run(start)
}
