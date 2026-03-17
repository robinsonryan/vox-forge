//! Keystroke-simulation output via [`enigo`] (X11) or `wtype` (Wayland).
//!
//! Types text character-by-character or falls back to clipboard paste for
//! applications listed in `clipboard_apps`.

use crate::error::{Error, Result};

/// Delivers text by simulating keyboard input.
pub struct TypingOutput {
    keystroke_delay_ms: u64,
    auto_enter: bool,
    clipboard_apps: Vec<String>,
}

impl TypingOutput {
    pub fn new(keystroke_delay_ms: u64, auto_enter: bool, clipboard_apps: Vec<String>) -> Self {
        Self {
            keystroke_delay_ms,
            auto_enter,
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
            return super::clipboard::paste_text(text);
        }

        if is_wayland() {
            type_wayland(text, self.keystroke_delay_ms, self.auto_enter)
        } else {
            type_x11(text, self.keystroke_delay_ms, self.auto_enter)
        }
    }
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Type text using `wtype` on Wayland, optionally followed by a Return keypress.
fn type_wayland(text: &str, delay_ms: u64, auto_enter: bool) -> Result<()> {
    let mut args = Vec::new();
    if delay_ms > 0 {
        args.push("-d".to_string());
        args.push(delay_ms.to_string());
    }
    args.push(text.to_string());
    if auto_enter {
        args.push("-k".to_string());
        args.push("Return".to_string());
    }

    let status = std::process::Command::new("wtype")
        .args(&args)
        .status()
        .map_err(|e| Error::Output(format!("Failed to run wtype: {e}")))?;

    if !status.success() {
        return Err(Error::Output(
            "wtype failed — is it installed? (sudo apt install wtype)".to_string(),
        ));
    }

    Ok(())
}

/// Type text using enigo on X11, optionally followed by a Return keypress.
fn type_x11(text: &str, delay_ms: u64, auto_enter: bool) -> Result<()> {
    use enigo::{Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| Error::Output(format!("Failed to create enigo instance: {e}")))?;

    if delay_ms == 0 {
        enigo
            .text(text)
            .map_err(|e| Error::Output(format!("Typing failed: {e}")))?;
    } else {
        for ch in text.chars() {
            enigo
                .text(&ch.to_string())
                .map_err(|e| Error::Output(format!("Typing char '{ch}' failed: {e}")))?;
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
    }

    if auto_enter {
        enigo
            .key(Key::Return, enigo::Direction::Click)
            .map_err(|e| Error::Output(format!("Enter keypress failed: {e}")))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_use_clipboard_matches_case_insensitive() {
        let output = TypingOutput::new(5, true, vec!["kitty".to_string(), "Alacritty".to_string()]);

        assert!(output.should_use_clipboard("kitty"));
        assert!(output.should_use_clipboard("Kitty"));
        assert!(output.should_use_clipboard("/usr/bin/kitty"));
        assert!(output.should_use_clipboard("alacritty"));
        assert!(output.should_use_clipboard("ALACRITTY"));
    }

    #[test]
    fn should_use_clipboard_rejects_non_matching() {
        let output = TypingOutput::new(5, true, vec!["kitty".to_string(), "alacritty".to_string()]);

        assert!(!output.should_use_clipboard("firefox"));
        assert!(!output.should_use_clipboard("code"));
        assert!(!output.should_use_clipboard(""));
    }

    #[test]
    fn should_use_clipboard_empty_list() {
        let output = TypingOutput::new(0, true, vec![]);
        assert!(!output.should_use_clipboard("kitty"));
    }

    #[test]
    fn should_use_clipboard_empty_app() {
        let output = TypingOutput::new(5, true, vec!["kitty".to_string()]);
        assert!(!output.should_use_clipboard(""));
    }
}
