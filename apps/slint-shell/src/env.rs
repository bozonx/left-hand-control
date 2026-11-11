//! Environment variables read by the shell; the core ones are in `tecloid_core::env`.

/// IPC socket name of this process (Linux/macOS).
pub const SOCKET: &str = "TECLOID_SOCKET";
/// Loopback IPC port (Windows).
pub const PORT: &str = "TECLOID_PORT";
/// IPC socket of the settings process, given to the Spell worker.
pub const PARENT_SOCKET: &str = "TECLOID_PARENT_SOCKET";
/// CSV file for timing metrics.
pub const METRICS: &str = "TECLOID_METRICS";
/// Monitor of the Spell popups.
pub const OUTPUT: &str = "TECLOID_OUTPUT";
/// Popup backend: `winit`, `auto` or `spell`.
pub const POPUPS: &str = "TECLOID_POPUPS";
/// Input device read by the built-in hotkeys instead of scanning.
pub const INPUT: &str = "TECLOID_INPUT";
/// Spell worker diagnostics: popup lifecycle and initial popup.
pub const SPELL_LIFECYCLE: &str = "TECLOID_SPELL_LIFECYCLE";
pub const SPELL_INITIAL: &str = "TECLOID_SPELL_INITIAL";
/// Diagnostic stands: focus return with text insertion, and its test mode.
pub const INSERT: &str = "TECLOID_INSERT";
pub const TEST_INJECT: &str = "TECLOID_TEST_INJECT";
/// `1` gives test keyboards their own input seat.
pub const ISOLATED_INPUT: &str = "TECLOID_ISOLATED_INPUT";
