//! Platform abstraction layer.
//!
//! Defines the [`Platform`] trait for OS-specific behavior and provides
//! compile-time dispatch to the correct implementation via [`current_platform`].
//! All `#[cfg(target_os)]` directives are contained within this module.

// Many trait methods and types are scaffolded for future consumers.
#![allow(dead_code)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

use std::path::PathBuf;

use crate::error::Result;

/// A permission issue discovered during startup checks.
#[derive(Debug)]
pub struct PermissionIssue {
    /// Which system component has the issue (e.g. "uinput", "audio").
    pub component: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// Optional shell command the user can run to fix the issue.
    pub fix_command: Option<String>,
}

/// A lock that prevents multiple daemon instances from running.
///
/// On drop the backing PID file is removed automatically.
pub struct DaemonLock {
    path: Option<PathBuf>,
}

impl DaemonLock {
    pub fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        if let Some(ref path) = self.path {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Platform-specific behaviour that the rest of the application consumes
/// through a trait object (`&dyn Platform` / `Box<dyn Platform>`).
pub trait Platform: Send + Sync {
    /// Directory for configuration files (TOML, etc.).
    fn config_dir(&self) -> PathBuf;

    /// Directory for persistent application data.
    fn data_dir(&self) -> PathBuf;

    /// Directory for expendable cache files.
    fn cache_dir(&self) -> PathBuf;

    /// Runtime directory (typically tmpfs on Linux).  Returns `None` when
    /// the OS does not provide a suitable runtime directory.
    fn runtime_dir(&self) -> Option<PathBuf>;

    /// Directory where downloaded models are stored.
    fn models_dir(&self) -> PathBuf;

    /// Path to the JSONL corrections file.
    fn corrections_path(&self) -> PathBuf;

    /// Path to the application log file.
    fn log_path(&self) -> PathBuf;

    /// Check for common permission issues at startup.
    fn check_permissions(&self) -> Vec<PermissionIssue>;

    /// Acquire a daemon lock (PID file) to prevent duplicate instances.
    fn daemon_lock(&self) -> Result<DaemonLock>;

    /// Path to the IPC socket (Unix) or named pipe (Windows).
    fn ipc_socket_path(&self) -> PathBuf;

    /// Whether the current session is running under Wayland.
    fn is_wayland(&self) -> bool;

    /// Human-readable platform description (e.g. "Linux (Wayland)").
    fn display_name(&self) -> &str;
}

/// Return the [`Platform`] implementation for the OS this binary was compiled for.
pub fn current_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxPlatform::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatform::new())
    }
}
