//! Windows active window detection stub.
//!
//! TODO: Implement using Win32 API (`GetForegroundWindow`,
//! `GetWindowTextW`, `GetWindowThreadProcessId`).

use crate::error::Result;

use super::{AppContext, WindowDetector};

pub struct WindowsWindowDetector;

impl WindowsWindowDetector {
    pub fn new() -> Self {
        Self
    }
}

impl WindowDetector for WindowsWindowDetector {
    fn active_window(&self) -> Result<AppContext> {
        // TODO: Implement Win32 API window detection
        // GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId
        Ok(AppContext::unknown())
    }
}
