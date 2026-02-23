//! Keystroke-simulation output via [`enigo`].
//!
//! Types text character-by-character (with an optional inter-key delay) or
//! falls back to clipboard paste for applications listed in `clipboard_apps`.

use enigo::{Enigo, Keyboard, Settings};

use crate::error::{Error, Result};

/// Delivers text by simulating keyboard input.
pub struct TypingOutput {
    keystroke_delay_ms: u64,
    clipboard_apps: Vec<String>,
}

impl TypingOutput {
    pub fn new(keystroke_delay_ms: u64, clipboard_apps: Vec<String>) -> Self {
        Self {
            keystroke_delay_ms,
            clipboard_apps,
        }
    }

    /// Returns `true` when `app_executable` matches one of the configured
    /// clipboard-preferred applications (case-insensitive substring match).
    fn should_use_clipboard(&self, app_executable: &str) -> bool {
        let lower = app_executable.to_lowercase();
        self.clipboard_apps
            .iter()
            .any(|app| lower.contains(&app.to_lowercase()))
    }
}

impl super::TextOutput for TypingOutput {
    fn output_text(&self, text: &str, app_executable: &str) -> Result<()> {
        if self.should_use_clipboard(app_executable) {
            // Delegate to clipboard paste for terminal apps
            return super::clipboard::paste_text(text);
        }

        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| Error::Output(format!("Failed to create enigo instance: {e}")))?;

        if self.keystroke_delay_ms == 0 {
            // Batch typing — send the entire string at once.
            enigo
                .text(text)
                .map_err(|e| Error::Output(format!("Typing failed: {e}")))?;
        } else {
            // Character-by-character with inter-key delay.
            for ch in text.chars() {
                enigo
                    .text(&ch.to_string())
                    .map_err(|e| Error::Output(format!("Typing char '{ch}' failed: {e}")))?;
                std::thread::sleep(std::time::Duration::from_millis(self.keystroke_delay_ms));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_use_clipboard_matches_case_insensitive() {
        let output = TypingOutput::new(5, vec!["kitty".to_string(), "Alacritty".to_string()]);

        assert!(output.should_use_clipboard("kitty"));
        assert!(output.should_use_clipboard("Kitty"));
        assert!(output.should_use_clipboard("/usr/bin/kitty"));
        assert!(output.should_use_clipboard("alacritty"));
        assert!(output.should_use_clipboard("ALACRITTY"));
    }

    #[test]
    fn should_use_clipboard_rejects_non_matching() {
        let output = TypingOutput::new(5, vec!["kitty".to_string(), "alacritty".to_string()]);

        assert!(!output.should_use_clipboard("firefox"));
        assert!(!output.should_use_clipboard("code"));
        assert!(!output.should_use_clipboard(""));
    }

    #[test]
    fn should_use_clipboard_empty_list() {
        let output = TypingOutput::new(0, vec![]);
        assert!(!output.should_use_clipboard("kitty"));
    }

    #[test]
    fn should_use_clipboard_empty_app() {
        let output = TypingOutput::new(5, vec!["kitty".to_string()]);
        assert!(!output.should_use_clipboard(""));
    }
}
