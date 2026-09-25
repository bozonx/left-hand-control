pub struct Worker;

impl Worker {
    pub fn is_alive(&mut self) -> bool {
        true
    }
}

pub fn start() -> Result<Option<Worker>, Box<dyn std::error::Error>> {
    match std::env::var("SLINT_SHELL_POPUPS").as_deref() {
        Ok("spell") => Err("Spell popups are available only on Linux Wayland".into()),
        Ok("auto" | "winit") | Err(_) => Ok(None),
        Ok(_) => Err("SLINT_SHELL_POPUPS must be winit, auto or spell".into()),
    }
}

impl Worker {
    pub fn send(
        &self,
        _: String,
        _: &str,
        _: std::time::Instant,
        _: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("Spell worker is unavailable on this platform".into())
    }
}
