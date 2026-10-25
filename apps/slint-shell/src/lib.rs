//! Slint shell of Left Hand Control.
//!
//! One binary runs the settings process (`app`), the Spell
//! popup worker (`--spell-worker`, Linux + feature `spell`) and a CLI client
//! that forwards a command to a running instance over IPC.
//! On Windows it also hosts the private input guard, before UI or config startup.

pub mod ui {
    slint::include_modules!();

    /// Application sky accent; the Fluent style derives its primary
    /// buttons and check boxes from it, matching `Theme.accent`.
    const ACCENT: slint::Color = slint::Color::from_rgb_u8(0x0e, 0xa5, 0xe9);

    thread_local! {
        static WANTED: std::cell::Cell<slint::Color> = const { std::cell::Cell::new(ACCENT) };
        static GUARD: i_slint_core::properties::ChangeTracker = Default::default();
    }

    fn with_context<T>(f: impl FnOnce(&i_slint_core::SlintContext) -> T) -> T {
        i_slint_core::context::with_global_context(|| Err(slint::PlatformError::NoPlatform), f)
            .expect("Slint theme requires an initialized platform")
    }

    pub fn apply_theme(theme: &Theme<'_>) {
        i_slint_core::animations::set_animations_enabled(!theme.get_reduce_motion());
        let wanted = if theme.get_eink() {
            slint::Color::from_rgb_u8(0, 0, 0)
        } else {
            ACCENT
        };
        WANTED.with(|cell| cell.set(wanted));
        with_context(|context| context.set_accent_color(wanted));
        // winit's XDG settings watcher pushes the desktop accent (e.g. KDE)
        // into the context after the window shows; put ours back.
        GUARD.with(|guard| {
            guard.init(
                (),
                |_| with_context(|context| context.accent_color()),
                |_, color| {
                    let wanted = WANTED.with(std::cell::Cell::get);
                    if *color != wanted {
                        with_context(|context| context.set_accent_color(wanted));
                    }
                },
            )
        });
        theme.invoke_apply();
    }
}

mod app;
pub mod command;
pub mod document;
pub mod env;
mod game_mode;
mod i18n;
mod ipc;
pub mod keyboard;
mod layer_hint;
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

const HELP: &str = concat!(
    "Left Hand Control ",
    env!("CARGO_PKG_VERSION"),
    "\n\nWithout arguments starts the application, or shows the settings window of the\n\
     instance that is already running. With a command, sends it to the running instance.\n\n"
);

/// Entry point shared by all roles; `args` excludes the program name.
pub fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    if lhc_core::mapper::windows::dispatch_input_guard()? {
        return Ok(());
    }
    let start = Instant::now();
    match args.as_slice() {
        [flag] if flag == "--version" || flag == "-V" => {
            println!("left-hand-control {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{HELP}{}", command::USAGE);
            return Ok(());
        }
        _ => {}
    }
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("info,zbus=warn,tracing=warn,spell_framework=warn"),
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

#[cfg(test)]
mod animation_tests {
    #[test]
    fn disabling_animations_finishes_an_active_transition() {
        use i_slint_core::{Property, animations, items::PropertyAnimation};

        animations::set_animations_enabled(true);
        let value = Box::pin(Property::new(0.0_f32));
        value.as_ref().set_animated_value(
            1.0,
            PropertyAnimation {
                duration: 10_000,
                ..Default::default()
            },
        );
        assert!(value.as_ref().get() < 1.0);
        animations::set_animations_enabled(false);
        assert_eq!(value.as_ref().get(), 1.0);
        assert_eq!(animations::animation_tick(), 0);
        animations::set_animations_enabled(true);
    }
}

#[cfg(test)]
#[path = "../build-support/i18n.rs"]
mod i18n_checks;
