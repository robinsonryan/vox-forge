//! Clipboard-paste fallback for output delivery.
//!
//! Copies text to the system clipboard, simulates Ctrl+V (or Ctrl+Shift+V on
//! some terminals), then restores the previous clipboard contents.

use std::time::Duration;

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::error::{Error, Result};

/// Paste `text` via the system clipboard and a simulated Ctrl+V keystroke.
///
/// The previous clipboard contents are saved beforehand and restored after the
/// paste completes, so the user's clipboard is not permanently clobbered.
pub fn paste_text(text: &str) -> Result<()> {
    let mut clipboard =
        Clipboard::new().map_err(|e| Error::Output(format!("Failed to access clipboard: {e}")))?;

    // Save current clipboard content (best-effort — may fail if clipboard is
    // empty or holds non-text data).
    let saved = clipboard.get_text().ok();

    // Set our text.
    clipboard
        .set_text(text)
        .map_err(|e| Error::Output(format!("Failed to set clipboard text: {e}")))?;

    // Brief delay to let clipboard settle.
    std::thread::sleep(Duration::from_millis(50));

    // Simulate Ctrl+V paste.
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| Error::Output(format!("Failed to create enigo: {e}")))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| Error::Output(format!("Ctrl press failed: {e}")))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| Error::Output(format!("V click failed: {e}")))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| Error::Output(format!("Ctrl release failed: {e}")))?;

    // Delay for the target application to process the paste.
    std::thread::sleep(Duration::from_millis(100));

    // Restore original clipboard content (best-effort).
    if let Some(original) = saved {
        let _ = clipboard.set_text(&original);
    }

    Ok(())
}
