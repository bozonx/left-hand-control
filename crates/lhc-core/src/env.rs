//! Environment variables read by the core.

/// Directory that replaces the real config and data directories
/// (see [`crate::storage::StoragePaths::resolve`]).
pub const DEV_DIR: &str = "LHC_DEV_DIR";
/// `off` disables the built-in popup hotkeys.
pub const HOTKEYS: &str = "LHC_HOTKEYS";

/// Whether the built-in popup hotkeys are enabled (see [`HOTKEYS`]).
pub fn popup_hotkeys_enabled() -> bool {
    std::env::var(HOTKEYS).as_deref() != Ok("off")
}
