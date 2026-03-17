//! Windows stub implementation of the [`Platform`] trait.
//!
//! This is a placeholder that provides correct directory paths via the `dirs`
//! crate but does not contain production-ready logic.  It compiles behind
//! `#[cfg(target_os = "windows")]` and will not be included on Linux builds.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

use super::{DaemonLock, PermissionIssue, Platform};

const APP_DIR_NAME: &str = "VoxForge";

/// Windows platform stub.
pub struct WindowsPlatform {
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
}

impl WindowsPlatform {
    pub fn new() -> Self {
        let config = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData\\VoxForge"))
            .join(APP_DIR_NAME);

        let data = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData\\VoxForge"))
            .join(APP_DIR_NAME);

        let cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\Temp\\VoxForge"))
            .join(APP_DIR_NAME);

        Self {
            config,
            data,
            cache,
        }
    }
}

/// Ensure a directory exists, creating it (and parents) if necessary.
fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

impl Platform for WindowsPlatform {
    fn config_dir(&self) -> PathBuf {
        if let Err(e) = ensure_dir(&self.config) {
            tracing::warn!("Failed to create {}: {e}", self.config.display());
        }
        self.config.clone()
    }

    fn data_dir(&self) -> PathBuf {
        if let Err(e) = ensure_dir(&self.data) {
            tracing::warn!("Failed to create {}: {e}", self.data.display());
        }
        self.data.clone()
    }

    fn cache_dir(&self) -> PathBuf {
        if let Err(e) = ensure_dir(&self.cache) {
            tracing::warn!("Failed to create {}: {e}", self.cache.display());
        }
        self.cache.clone()
    }

    fn runtime_dir(&self) -> Option<PathBuf> {
        // Windows has no equivalent of XDG_RUNTIME_DIR; use the data dir.
        Some(self.data.clone())
    }

    fn models_dir(&self) -> PathBuf {
        let dir = self.data.join("models");
        if let Err(e) = ensure_dir(&dir) {
            tracing::warn!("Failed to create {}: {e}", dir.display());
        }
        dir
    }

    fn corrections_path(&self) -> PathBuf {
        if let Err(e) = ensure_dir(&self.data) {
            tracing::warn!("Failed to create {}: {e}", self.data.display());
        }
        self.data.join("corrections.jsonl")
    }

    fn log_path(&self) -> PathBuf {
        if let Err(e) = ensure_dir(&self.data) {
            tracing::warn!("Failed to create {}: {e}", self.data.display());
        }
        self.data.join("voxforge.log")
    }

    fn check_permissions(&self) -> Vec<PermissionIssue> {
        // Windows typically does not need the same permission checks.
        Vec::new()
    }

    fn daemon_lock(&self) -> Result<DaemonLock> {
        // TODO: implement via named mutex on Windows.
        Err(Error::Platform(
            "daemon lock is not yet implemented on Windows".into(),
        ))
    }

    fn ipc_socket_path(&self) -> PathBuf {
        // Named pipe path — the actual pipe creation is handled elsewhere.
        PathBuf::from(r"\\.\pipe\voxforge")
    }

    fn is_wayland(&self) -> bool {
        false
    }

    fn display_name(&self) -> &str {
        "Windows"
    }
}
