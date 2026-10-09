//! Slint shell of Left Hand Control.
//!
//! One binary runs in three roles: the settings process (`app`), the Spell
//! popup worker (`--spell-worker`, Linux + feature `spell`) and a CLI client
//! that forwards a command to a running instance over IPC.

pub mod ui {
    slint::include_modules!();

    pub fn apply_theme(theme: &Theme<'_>) {
        thread_local! {
            static ACCENT: std::cell::Cell<Option<slint::Color>> = const { std::cell::Cell::new(None) };
        }
        i_slint_core::context::with_global_context(
            || Err(slint::PlatformError::NoPlatform),
            |context| {
                ACCENT.with(|accent| {
                    let original = accent.get().unwrap_or_else(|| {
                        let color = context.accent_color();
                        accent.set(Some(color));
                        color
                    });
                    context.set_accent_color(if theme.get_eink() {
                        slint::Color::from_rgb_u8(0, 0, 0)
                    } else {
                        original
                    });
                });
            },
        )
        .expect("Slint theme requires an initialized platform");
        theme.invoke_apply();
    }
}

mod app;
pub mod command;
pub mod document;
mod game_mode;
mod i18n;
mod ipc;
pub mod keyboard;
mod metrics;
mod notifications;
pub mod pages;
mod platform;
pub mod popup_model;
#[cfg(all(feature = "spell", target_os = "linux"))]
mod spell;
#[cfg(all(feature = "probes", target_os = "linux"))]
pub mod test_keyboard;
pub mod text_editing;

pub use document::Document;
pub use game_mode::bind as bind_game_mode;
pub use i18n::select_ui_language;
pub use pages::bind_document;

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
        let command = command::Command::parse_args(&args)?;
        return ipc::client(&command);
    }
    app::run(start)
}

#[cfg(test)]
#[path = "../vendor/i-slint-core/text_history.rs"]
mod text_history_tests;
