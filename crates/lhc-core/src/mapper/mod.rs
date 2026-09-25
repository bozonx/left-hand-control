#[cfg(target_os = "linux")]
pub mod action;
#[cfg(target_os = "linux")]
pub mod engine;
#[cfg(target_os = "linux")]
pub mod keys;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod portal;
pub mod runtime;
#[cfg(target_os = "linux")]
pub mod system;
#[cfg(target_os = "linux")]
pub mod system_macros;
#[cfg(target_os = "linux")]
pub mod validation;

use std::sync::{Arc, Mutex};

pub use crate::mapper_types::{InputDevice, KeyboardDevice};

pub trait MapperHost: Send + Sync {
    fn mapper_stopped(&self, error: &str);
    fn app_event(&self, name: &str);
    fn refresh_layout(&self);
}

static HOST: Mutex<Option<Arc<dyn MapperHost>>> = Mutex::new(None);

pub fn set_host(host: Arc<dyn MapperHost>) {
    if let Ok(mut current) = HOST.lock() {
        *current = Some(host);
    }
}

fn with_host(f: impl FnOnce(&dyn MapperHost)) {
    let host = HOST.lock().ok().and_then(|value| value.clone());
    if let Some(host) = host {
        f(host.as_ref());
    }
}

pub(crate) fn notify_mapper_stopped(error: &str) {
    with_host(|host| host.mapper_stopped(error));
}

pub(crate) fn emit_app_event(name: &str) {
    with_host(|host| host.app_event(name));
}

pub(crate) fn refresh_layout() {
    with_host(|host| host.refresh_layout());
}

pub mod config {
    pub use crate::mapper_config::*;
}
