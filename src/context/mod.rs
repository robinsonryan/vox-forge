//! Context detection — identifies the currently focused application.
//!
//! Platform-specific detection lives behind the [`WindowDetector`] trait.
//! Use [`create_window_detector`] to get the appropriate implementation.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

/// Information about the currently focused application.
#[derive(Debug, Clone, Default)]
pub struct AppContext {
    pub app_name: String,
    pub window_title: String,
    pub executable: String,
}

impl AppContext {
    /// Create an unknown/fallback context.
    pub fn unknown() -> Self {
        Self {
            app_name: "unknown".to_string(),
            window_title: String::new(),
            executable: String::new(),
        }
    }
}

/// Detect the currently focused application.
pub trait WindowDetector: Send + Sync {
    fn active_window(&self) -> crate::error::Result<AppContext>;
}

/// Create the platform-appropriate window detector.
pub fn create_window_detector() -> Box<dyn WindowDetector> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxWindowDetector::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsWindowDetector::new())
    }
}

/// A no-op detector that always returns unknown context.
/// Used as fallback when detection isn't available.
#[allow(dead_code)]
pub struct FallbackDetector;

impl WindowDetector for FallbackDetector {
    fn active_window(&self) -> crate::error::Result<AppContext> {
        Ok(AppContext::unknown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_context_has_expected_defaults() {
        let ctx = AppContext::unknown();
        assert_eq!(ctx.app_name, "unknown");
        assert!(ctx.window_title.is_empty());
        assert!(ctx.executable.is_empty());
    }

    #[test]
    fn fallback_detector_returns_ok_with_unknown() {
        let detector = FallbackDetector;
        let result = detector.active_window();
        assert!(result.is_ok());
        let ctx = result.expect("fallback should never fail");
        assert_eq!(ctx.app_name, "unknown");
        assert!(ctx.window_title.is_empty());
        assert!(ctx.executable.is_empty());
    }

    #[test]
    fn default_app_context_is_empty() {
        let ctx = AppContext::default();
        assert!(ctx.app_name.is_empty());
        assert!(ctx.window_title.is_empty());
        assert!(ctx.executable.is_empty());
    }
}
