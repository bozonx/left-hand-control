use slint::winit_030::winit;

#[derive(Default)]
pub struct Activation;

impl Activation {
    pub fn activate(
        &mut self,
        window: &winit::window::Window,
        _: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        window.focus_window();
        Ok(())
    }
}
