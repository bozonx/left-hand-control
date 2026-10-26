//! Text input for `text:` actions and popup insertion: job accounting, the
//! XDG RemoteDesktop portal backends and temporary clipboard ownership.
//! Used by the mapper and by the shell's popups.

pub(crate) mod job;

#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub(crate) mod portal_clipboard;
