//! Text input for `text:` actions and popup insertion: job accounting, the
//! XDG RemoteDesktop portal backends and temporary clipboard ownership.
//! Used by the mapper and by the shell's popups.

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(crate) mod job;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use job::MacroText;
mod timing;
pub use timing::TextTiming;

#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "linux")]
mod clipboard_transaction;
#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub(crate) mod portal_clipboard;
