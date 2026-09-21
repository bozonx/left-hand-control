use enigo::{Enigo, Keyboard, Settings};

#[derive(Default)]
pub struct ReturnInput {
    target: Option<Target>,
}

impl ReturnInput {
    pub fn capture(&mut self) {
        self.target = capture_target();
    }

    pub fn selected(&mut self, text: String) {
        let Some(target) = self.target.take() else {
            log::error!("native return input has no captured target");
            return;
        };
        slint::Timer::single_shot(std::time::Duration::from_millis(100), move || {
            if let Err(error) = restore_target(target) {
                log::error!("native focus restore failed: {error}");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            match Enigo::new(&Settings::default()) {
                Ok(mut enigo) => {
                    if let Err(error) = enigo.text(&text) {
                        log::error!("native return input unavailable: {error}");
                    }
                }
                Err(error) => log::error!("native input initialization failed: {error}"),
            }
        });
    }
}

#[cfg(target_os = "windows")]
type Target = isize;

#[cfg(target_os = "windows")]
fn capture_target() -> Option<Target> {
    let window = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    (!window.0.is_null()).then_some(window.0 as isize)
}

#[cfg(target_os = "windows")]
fn restore_target(target: Target) -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::SetForegroundWindow};
    let restored = unsafe { SetForegroundWindow(HWND(target as *mut _)) }.as_bool();
    restored
        .then_some(())
        .ok_or_else(|| "SetForegroundWindow rejected the target".into())
}

#[cfg(target_os = "macos")]
type Target = i32;

#[cfg(target_os = "macos")]
fn capture_target() -> Option<Target> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get unix id of first application process whose frontmost is true",
        ])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .flatten()
}

#[cfg(target_os = "macos")]
fn restore_target(target: Target) -> Result<(), Box<dyn std::error::Error>> {
    let script = format!(
        "tell application \"System Events\" to set frontmost of first application process whose unix id is {target} to true"
    );
    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("osascript exited with {status}").into())
}
