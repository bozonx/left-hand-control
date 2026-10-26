//! Applies the `Theme` global: animation preference and the Fluent accent
//! colour, which is kept against the desktop accent pushed by winit.

use crate::ui::Theme;

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
