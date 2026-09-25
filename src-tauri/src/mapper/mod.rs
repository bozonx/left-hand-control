pub mod config;

pub use lhc_core::mapper::runtime::*;
pub use lhc_core::mapper_types::{InputDevice, KeyboardDevice};

#[cfg(target_os = "linux")]
pub fn set_app_handle(app: tauri::AppHandle) {
    lhc_core::mapper::set_host(std::sync::Arc::new(TauriMapperHost(app)));
}

#[cfg(not(target_os = "linux"))]
pub fn set_app_handle(_app: tauri::AppHandle) {}

#[cfg(target_os = "linux")]
struct TauriMapperHost(tauri::AppHandle);

#[cfg(target_os = "linux")]
impl lhc_core::mapper::MapperHost for TauriMapperHost {
    fn mapper_stopped(&self, error: &str) {
        use tauri::Emitter;
        let _ = self.0.emit("mapper-stopped", error.to_string());
    }

    fn app_event(&self, name: &str) {
        use tauri::Emitter;
        let _ = self.0.emit(name, ());
    }

    fn refresh_layout(&self) {
        let _ = crate::layout::refresh_cache();
    }
}
