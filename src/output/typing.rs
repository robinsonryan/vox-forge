//! Keystroke-simulation output via [`enigo`] (X11) or `wtype` (Wayland).
//!
//! Types text character-by-character or falls back to clipboard paste for
//! applications listed in `clipboard_apps`.

use crate::error::{Error, Result};

/// Delivers text by simulating keyboard input.
pub struct TypingOutput {
    keystroke_delay_ms: u64,
    auto_enter: bool,
    auto_enter_delay_ms: u64,
    clipboard_apps: Vec<String>,
}

impl TypingOutput {
    pub fn new(
        keystroke_delay_ms: u64,
        auto_enter: bool,
        auto_enter_delay_ms: u64,
        clipboard_apps: Vec<String>,
    ) -> Self {
        Self {
            keystroke_delay_ms,
            auto_enter,
            auto_enter_delay_ms,
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
            type_wayland(text, self.keystroke_delay_ms)?;
        } else {
            type_x11(text, self.keystroke_delay_ms)?;
        }

        if self.auto_enter {
            if self.auto_enter_delay_ms > 0 {
                if !wait_for_escape(self.auto_enter_delay_ms) {
                    send_enter()?;
                } else {
                    tracing::info!("Auto-enter cancelled by Escape");
                }
            } else {
                send_enter()?;
            }
        }

        Ok(())
    }
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Send a Return keypress.
fn send_enter() -> Result<()> {
    if is_wayland() {
        let status = std::process::Command::new("wtype")
            .args(["-k", "Return"])
            .status()
            .map_err(|e| Error::Output(format!("Failed to run wtype: {e}")))?;
        if !status.success() {
            return Err(Error::Output("wtype -k Return failed".to_string()));
        }
    } else {
        use enigo::{Enigo, Key, Keyboard, Settings};
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| Error::Output(format!("Failed to create enigo instance: {e}")))?;
        enigo
            .key(Key::Return, enigo::Direction::Click)
            .map_err(|e| Error::Output(format!("Enter keypress failed: {e}")))?;
    }
    Ok(())
}

/// Wait for `duration_ms` while listening for Escape via evdev.
/// Returns `true` if Escape was pressed, `false` if the timeout expired.
#[cfg(target_os = "linux")]
fn wait_for_escape(duration_ms: u64) -> bool {
    use evdev::{Device, InputEventKind, Key};
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    let deadline = Instant::now() + Duration::from_millis(duration_ms);

    // Find keyboard devices
    let devices: Vec<_> = evdev::enumerate().collect();
    if devices.is_empty() {
        tracing::warn!("No evdev devices found — skipping Escape detection");
        std::thread::sleep(Duration::from_millis(duration_ms));
        return false;
    }

    let keyboards: Vec<Device> = devices
        .into_iter()
        .filter_map(|(_, dev)| {
            if dev
                .supported_keys()
                .is_some_and(|keys| keys.contains(Key::KEY_ESC))
            {
                Some(dev)
            } else {
                None
            }
        })
        .collect();

    if keyboards.is_empty() {
        tracing::warn!("No keyboard devices found — skipping Escape detection");
        std::thread::sleep(Duration::from_millis(duration_ms));
        return false;
    }

    // Set all keyboards to non-blocking via fcntl
    let mut keyboards: Vec<Device> = keyboards
        .into_iter()
        .map(|dev| {
            let fd = dev.as_raw_fd();
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL);
                libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
            dev
        })
        .collect();

    // Poll for Escape key press
    while Instant::now() < deadline {
        for dev in &mut keyboards {
            if let Ok(events) = dev.fetch_events() {
                for event in events {
                    if let InputEventKind::Key(Key::KEY_ESC) = event.kind() {
                        if event.value() == 1 {
                            return true;
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    false
}

#[cfg(not(target_os = "linux"))]
fn wait_for_escape(duration_ms: u64) -> bool {
    std::thread::sleep(std::time::Duration::from_millis(duration_ms));
    false
}

/// Type text using `wtype` on Wayland.
fn type_wayland(text: &str, delay_ms: u64) -> Result<()> {
    let mut args = Vec::new();
    if delay_ms > 0 {
        args.push("-d".to_string());
        args.push(delay_ms.to_string());
    }
    args.push(text.to_string());

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

/// Type text using enigo on X11.
fn type_x11(text: &str, delay_ms: u64) -> Result<()> {
    use enigo::{Enigo, Keyboard, Settings};

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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_use_clipboard_matches_case_insensitive() {
        let output = TypingOutput::new(5, true, 2000, vec!["kitty".to_string(), "Alacritty".to_string()]);

        assert!(output.should_use_clipboard("kitty"));
        assert!(output.should_use_clipboard("Kitty"));
        assert!(output.should_use_clipboard("/usr/bin/kitty"));
        assert!(output.should_use_clipboard("alacritty"));
        assert!(output.should_use_clipboard("ALACRITTY"));
    }

    #[test]
    fn should_use_clipboard_rejects_non_matching() {
        let output = TypingOutput::new(5, true, 2000, vec!["kitty".to_string(), "alacritty".to_string()]);

        assert!(!output.should_use_clipboard("firefox"));
        assert!(!output.should_use_clipboard("code"));
        assert!(!output.should_use_clipboard(""));
    }

    #[test]
    fn should_use_clipboard_empty_list() {
        let output = TypingOutput::new(0, true, 2000, vec![]);
        assert!(!output.should_use_clipboard("kitty"));
    }

    #[test]
    fn should_use_clipboard_empty_app() {
        let output = TypingOutput::new(5, true, 2000, vec!["kitty".to_string()]);
        assert!(!output.should_use_clipboard(""));
    }
}
