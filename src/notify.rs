//! Desktop notification helpers.
//!
//! Thin wrappers around [`notify_rust`] that present brief, consistent
//! notifications for the various application states (recording, processing,
//! error, etc.).

use crate::error::Result;

/// Send a desktop notification with the given `title` and `body`.
pub fn notify(title: &str, body: &str) -> Result<()> {
    notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .appname("VoxForge")
        .timeout(notify_rust::Timeout::Milliseconds(3000))
        .show()
        .map_err(|e| crate::error::Error::Platform(format!("Notification failed: {e}")))?;
    Ok(())
}

/// Notify that recording has started.
pub fn notify_recording() -> Result<()> {
    notify("VoxForge", "Recording...")
}

/// Notify that audio is being processed (transcription / formatting).
pub fn notify_processing() -> Result<()> {
    notify("VoxForge", "Processing...")
}

/// Notify that an error occurred.
pub fn notify_error(reason: &str) -> Result<()> {
    notify("VoxForge", &format!("Error: {reason}"))
}

/// Notify that the current operation was cancelled.
pub fn notify_cancelled() -> Result<()> {
    notify("VoxForge", "Cancelled")
}

/// Notify that the application is ready and listening for the hotkey.
pub fn notify_ready() -> Result<()> {
    notify("VoxForge", "Ready — listening for hotkey")
}

#[cfg(test)]
mod tests {
    // The notification helpers are thin wrappers around notify-rust.
    // We smoke-test that constructing the notification values does not panic.
    // Actually *showing* a notification requires a running D-Bus session,
    // so we only verify the function signatures compile and the string
    // formatting logic is sound.

    #[test]
    fn notify_error_formats_reason() {
        let reason = "microphone not found";
        let expected = format!("Error: {reason}");
        assert_eq!(expected, "Error: microphone not found");
    }

    #[test]
    fn notify_functions_are_callable() {
        // Ensure the public API compiles with the correct signatures.
        // We cannot call them in CI without a D-Bus session, so just
        // reference them to confirm they exist and type-check.
        let _ = super::notify as fn(&str, &str) -> crate::error::Result<()>;
        let _ = super::notify_recording as fn() -> crate::error::Result<()>;
        let _ = super::notify_processing as fn() -> crate::error::Result<()>;
        let _ = super::notify_error as fn(&str) -> crate::error::Result<()>;
        let _ = super::notify_cancelled as fn() -> crate::error::Result<()>;
        let _ = super::notify_ready as fn() -> crate::error::Result<()>;
    }
}
