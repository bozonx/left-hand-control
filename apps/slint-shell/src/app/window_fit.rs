//! Keeps the settings window inside the screen at high display scales.
//!
//! The compositor does not shrink a window that opens larger than the
//! screen: at 200% on 1920×1080 the default 1120×760 window would hang off
//! the bottom edge. The window is shrunk once it knows its screen and again
//! whenever the scale changes; it never grows.

use slint::LogicalSize;

/// Room left for the title bar and desktop panels, in logical pixels.
const MARGIN: LogicalSize = LogicalSize::new(32., 100.);

/// Size that fits `current` on `screen`, or `None` when it already fits.
/// Never goes below `min`; a screen smaller than that is out of reach.
fn fitted(current: LogicalSize, min: LogicalSize, screen: LogicalSize) -> Option<LogicalSize> {
    let fit =
        |current: f32, min: f32, screen: f32, margin: f32| current.min(screen - margin).max(min);
    let size = LogicalSize::new(
        fit(current.width, min.width, screen.width, MARGIN.width),
        fit(current.height, min.height, screen.height, MARGIN.height),
    );
    // A fractional scale rounds the size to whole physical pixels; ignore that.
    (size.width < current.width - 1. || size.height < current.height - 1.).then_some(size)
}

/// Shrink `window` to its screen. The window's own scale factor converts the
/// screen to logical pixels: on Wayland the monitor reports only an integer scale.
pub(super) fn fit_to_screen(window: &slint::Window, min: LogicalSize) {
    use slint::winit_030::WinitWindowAccessor;
    if !window.is_visible() {
        return;
    }
    let scale = window.scale_factor();
    let screen = window
        .with_winit_window(|native| {
            // Before the surface enters an output there is no current monitor;
            // the smallest one is then a safe bound.
            let monitors: Vec<_> = match native.current_monitor() {
                Some(monitor) => vec![monitor],
                None => native.available_monitors().collect(),
            };
            monitors
                .iter()
                .map(|monitor| monitor.size())
                .filter(|size| size.width > 0 && size.height > 0)
                .map(|size| LogicalSize::new(size.width as f32 / scale, size.height as f32 / scale))
                .reduce(|a, b| LogicalSize::new(a.width.min(b.width), a.height.min(b.height)))
        })
        .flatten();
    let Some(screen) = screen else {
        return;
    };
    let current = window.size().to_logical(scale);
    if let Some(size) = fitted(current, min, screen) {
        log::info!(
            "settings window {}x{} does not fit screen {}x{}; resizing to {}x{}",
            current.width,
            current.height,
            screen.width,
            screen.height,
            size.width,
            size.height
        );
        window.set_size(size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: LogicalSize = LogicalSize::new(640., 400.);
    const DEFAULT: LogicalSize = LogicalSize::new(1120., 760.);

    #[test]
    fn keeps_a_window_that_fits() {
        assert_eq!(fitted(DEFAULT, MIN, LogicalSize::new(1920., 1080.)), None);
        assert_eq!(fitted(DEFAULT, MIN, LogicalSize::new(1536., 864.)), None);
    }

    #[test]
    fn shrinks_to_scaled_screens() {
        // 1920×1080 at 200%.
        assert_eq!(
            fitted(DEFAULT, MIN, LogicalSize::new(960., 540.)),
            Some(LogicalSize::new(928., 440.))
        );
        // 1366×768 at 150%: only the height is too large.
        assert_eq!(
            fitted(DEFAULT, MIN, LogicalSize::new(910.67, 512.)),
            Some(LogicalSize::new(878.67, 412.))
        );
        // 1920×1080 at 150%.
        assert_eq!(
            fitted(DEFAULT, MIN, LogicalSize::new(1280., 720.)),
            Some(LogicalSize::new(1120., 620.))
        );
    }

    #[test]
    fn never_goes_below_the_minimum() {
        assert_eq!(
            fitted(DEFAULT, MIN, LogicalSize::new(600., 400.)),
            Some(MIN)
        );
        assert_eq!(fitted(MIN, MIN, LogicalSize::new(600., 400.)), None);
    }
}
