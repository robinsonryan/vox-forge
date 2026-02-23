//! Output delivery layer.
//!
//! Provides the [`TextOutput`] trait and two implementations:
//! - [`typing::TypingOutput`] — simulates keystrokes via enigo.
//! - [`clipboard`] — clipboard-paste fallback for terminal emulators.

pub mod clipboard;
pub mod typing;

use crate::error::Result;

/// Output method for delivering transcribed text to the active application.
pub trait TextOutput: Send + Sync {
    /// Deliver `text` into the currently focused window.
    ///
    /// `app_executable` is the name (or path) of the focused application,
    /// used to decide whether clipboard paste should be preferred over
    /// keystroke simulation.
    fn output_text(&self, text: &str, app_executable: &str) -> Result<()>;
}
