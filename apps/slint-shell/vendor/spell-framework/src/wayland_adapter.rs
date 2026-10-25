//! It provides various widget types for implementing properties
//! across various functionalities for your shell. The most common widget (or
//! window as called by many) is [SpellWin].

mod common;
mod fractional_scaling;
mod viewporter;
mod window;

pub use window::SpellXDGPopup;
pub use window::WinHandle;
pub use window::{SpellWin, WindowEvent};

/// Furture virtual keyboard implementation will be on this type. Currently, it is redundent.
pub struct SpellBoard;
