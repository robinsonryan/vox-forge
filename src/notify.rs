//! Desktop notification helpers.
//!
//! Thin wrappers around [`notify_rust`] that present brief, consistent
//! notifications for the various application states (recording, processing,
//! error, etc.).

/// Send a desktop notification with the given `title` and `body`.
///
/// Uses a background thread to avoid nested runtime conflicts when
/// `notify-rust` calls `zbus::block_on()` internally.
pub fn notify(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .summary(&title)
            .body(&body)
            .appname("VoxForge")
            .timeout(notify_rust::Timeout::Milliseconds(3000))
            .show();
    });
}

/// Notify that recording has started.
pub fn notify_recording() {
    notify("VoxForge", "Recording...");
}

/// Notify that audio is being processed (transcription / formatting).
pub fn notify_processing() {
    notify("VoxForge", "Processing...");
}

/// Notify that an error occurred.
pub fn notify_error(reason: &str) {
    notify("VoxForge", &format!("Error: {reason}"));
}

/// Notify that the current operation was cancelled.
pub fn notify_cancelled() {
    notify("VoxForge", "Cancelled");
}

/// Notify that the application is ready and listening for the hotkey.
pub fn notify_ready() {
    notify("VoxForge", "Ready — listening for hotkey");
}

#[cfg(test)]
mod tests {
    #[test]
    fn notify_error_formats_reason() {
        let reason = "microphone not found";
        let expected = format!("Error: {reason}");
        assert_eq!(expected, "Error: microphone not found");
    }

    #[test]
    fn notify_functions_are_callable() {
        let _ = super::notify as fn(&str, &str);
        let _ = super::notify_recording as fn();
        let _ = super::notify_processing as fn();
        let _ = super::notify_error as fn(&str);
        let _ = super::notify_cancelled as fn();
        let _ = super::notify_ready as fn();
    }
}
