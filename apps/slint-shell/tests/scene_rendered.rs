//! Frame observation must also work without a native graphics API.

use slint::{ComponentHandle, platform::software_renderer::MinimalSoftwareWindow};
use std::{cell::Cell, rc::Rc};

struct Platform(Rc<MinimalSoftwareWindow>);

impl slint::platform::Platform for Platform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn software_frames_notify_only_after_drawing() {
    let window = MinimalSoftwareWindow::new(Default::default());
    slint::platform::set_platform(Box::new(Platform(window.clone()))).unwrap();
    let popup = slint_shell::ui::EmojiPopup::new().unwrap();
    popup.window().set_size(slint::PhysicalSize::new(480, 350));
    let frames = Rc::new(Cell::new(0));
    i_slint_core::window::WindowInner::from_pub(popup.window()).on_scene_rendered({
        let frames = frames.clone();
        move || frames.set(frames.get() + 1)
    });
    popup.show().unwrap();
    assert_eq!(frames.get(), 0);
    let size = window.size();
    let mut pixels = vec![slint::Rgb8Pixel::default(); (size.width * size.height) as usize];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, size.width as usize);
    });
    assert_eq!(frames.get(), 1);
    popup.window().request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, size.width as usize);
    });
    assert_eq!(frames.get(), 2);
}
