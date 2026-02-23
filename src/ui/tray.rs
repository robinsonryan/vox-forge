//! System tray management.
//!
//! Placeholder for full tray-icon integration. The actual implementation
//! will create a system tray icon with a context menu for quick access
//! to recording controls and the settings window.

/// System tray manager (placeholder for full implementation).
#[allow(dead_code)]
pub struct TrayManager;

#[allow(dead_code)]
impl TrayManager {
    /// Create a new tray manager instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}
