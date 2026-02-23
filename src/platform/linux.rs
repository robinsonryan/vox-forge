//! Linux implementation of the [`Platform`] trait.

use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};

use super::{DaemonLock, PermissionIssue, Platform};

const APP_DIR_NAME: &str = "voxforge";

/// Linux-specific platform implementation backed by XDG directories.
pub struct LinuxPlatform {
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    runtime: Option<PathBuf>,
    is_wayland: bool,
}

impl LinuxPlatform {
    pub fn new() -> Self {
        let config = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/voxforge-config"))
            .join(APP_DIR_NAME);

        let data = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/voxforge-data"))
            .join(APP_DIR_NAME);

        let cache = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp/voxforge-cache"))
            .join(APP_DIR_NAME);

        let runtime = dirs::runtime_dir()
            .map(|d| d.join(APP_DIR_NAME))
            .or_else(|| {
                let uid = get_uid();
                Some(PathBuf::from(format!("/tmp/voxforge-{uid}")))
            });

        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

        Self {
            config,
            data,
            cache,
            runtime,
            is_wayland,
        }
    }
}

/// Get the current user's UID.
///
/// # Safety
///
/// `libc::getuid()` is a trivially safe POSIX call with no preconditions
/// and no mutable state.
fn get_uid() -> u32 {
    // SAFETY: see doc comment above.
    unsafe { libc::getuid() }
}

/// Ensure a directory exists, creating it (and parents) if necessary.
fn ensure_dir(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Check whether a process with the given PID is still alive.
fn is_pid_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

impl Platform for LinuxPlatform {
    fn config_dir(&self) -> PathBuf {
        let _ = ensure_dir(&self.config);
        self.config.clone()
    }

    fn data_dir(&self) -> PathBuf {
        let _ = ensure_dir(&self.data);
        self.data.clone()
    }

    fn cache_dir(&self) -> PathBuf {
        let _ = ensure_dir(&self.cache);
        self.cache.clone()
    }

    fn runtime_dir(&self) -> Option<PathBuf> {
        if let Some(ref dir) = self.runtime {
            let _ = ensure_dir(dir);
        }
        self.runtime.clone()
    }

    fn models_dir(&self) -> PathBuf {
        let dir = self.data.join("models");
        let _ = ensure_dir(&dir);
        dir
    }

    fn corrections_path(&self) -> PathBuf {
        let _ = ensure_dir(&self.data);
        self.data.join("corrections.jsonl")
    }

    fn log_path(&self) -> PathBuf {
        let _ = ensure_dir(&self.data);
        self.data.join("voxforge.log")
    }

    fn check_permissions(&self) -> Vec<PermissionIssue> {
        let mut issues = Vec::new();

        // Check /dev/uinput access (needed for synthetic keyboard input).
        let uinput = PathBuf::from("/dev/uinput");
        if uinput.exists() {
            let writable = fs::metadata(&uinput)
                .map(|m| {
                    use std::os::unix::fs::MetadataExt;
                    // Writable by owner or group -- a rough heuristic.
                    // Actual permission depends on udev rules.
                    m.mode() & 0o222 != 0
                })
                .unwrap_or(false);

            if !writable {
                issues.push(PermissionIssue {
                    component: "uinput".into(),
                    message: "/dev/uinput is not writable -- synthetic keyboard input will fail"
                        .into(),
                    fix_command: Some(
                        "sudo usermod -aG input $USER && sudo udevadm control --reload-rules"
                            .into(),
                    ),
                });
            }
        }

        // Check audio device access.
        let audio_paths = ["/dev/snd", "/proc/asound"];
        let audio_accessible = audio_paths.iter().any(|p| PathBuf::from(p).exists());
        if !audio_accessible {
            issues.push(PermissionIssue {
                component: "audio".into(),
                message: "No audio devices found -- microphone capture will not work".into(),
                fix_command: Some("sudo usermod -aG audio $USER".into()),
            });
        }

        issues
    }

    fn daemon_lock(&self) -> Result<DaemonLock> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| Error::Platform("no runtime directory available".into()))?;

        ensure_dir(runtime)?;
        let pid_path = runtime.join("voxforge.pid");

        // If a PID file already exists, check whether that process is alive.
        if pid_path.exists() {
            if let Ok(contents) = fs::read_to_string(&pid_path)
                && let Ok(pid) = contents.trim().parse::<u32>()
                && is_pid_alive(pid)
            {
                return Err(Error::Platform(format!(
                    "another vox-forge instance is already running (PID {pid})"
                )));
            }
            // Stale PID file -- remove it.
            let _ = fs::remove_file(&pid_path);
        }

        // Write our PID.
        fs::write(&pid_path, std::process::id().to_string())?;

        Ok(DaemonLock::new(pid_path))
    }

    fn ipc_socket_path(&self) -> PathBuf {
        let runtime = self
            .runtime
            .clone()
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let _ = ensure_dir(&runtime);
        runtime.join("voxforge.sock")
    }

    fn is_wayland(&self) -> bool {
        self.is_wayland
    }

    fn display_name(&self) -> &str {
        if self.is_wayland {
            "Linux (Wayland)"
        } else {
            "Linux (X11)"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_ends_with_voxforge() {
        let platform = LinuxPlatform::new();
        let dir = platform.config_dir();
        assert!(
            dir.ends_with(APP_DIR_NAME),
            "config_dir should end with '{APP_DIR_NAME}', got {dir:?}"
        );
    }

    #[test]
    fn data_dir_ends_with_voxforge() {
        let platform = LinuxPlatform::new();
        let dir = platform.data_dir();
        assert!(
            dir.ends_with(APP_DIR_NAME),
            "data_dir should end with '{APP_DIR_NAME}', got {dir:?}"
        );
    }

    #[test]
    fn cache_dir_ends_with_voxforge() {
        let platform = LinuxPlatform::new();
        let dir = platform.cache_dir();
        assert!(
            dir.ends_with(APP_DIR_NAME),
            "cache_dir should end with '{APP_DIR_NAME}', got {dir:?}"
        );
    }

    #[test]
    fn models_dir_is_under_data() {
        let platform = LinuxPlatform::new();
        let models = platform.models_dir();
        let data = platform.data_dir();
        assert!(
            models.starts_with(&data),
            "models_dir should be under data_dir: {models:?} vs {data:?}"
        );
        assert!(models.ends_with("models"));
    }

    #[test]
    fn corrections_path_is_under_data() {
        let platform = LinuxPlatform::new();
        let path = platform.corrections_path();
        let data = platform.data_dir();
        assert!(path.starts_with(&data));
        assert!(path.ends_with("corrections.jsonl"));
    }

    #[test]
    fn log_path_is_under_data() {
        let platform = LinuxPlatform::new();
        let path = platform.log_path();
        let data = platform.data_dir();
        assert!(path.starts_with(&data));
        assert!(path.ends_with("voxforge.log"));
    }

    #[test]
    fn ipc_socket_path_ends_with_sock() {
        let platform = LinuxPlatform::new();
        let path = platform.ipc_socket_path();
        assert!(
            path.extension().is_some_and(|ext| ext == "sock"),
            "ipc socket path should have .sock extension: {path:?}"
        );
    }

    #[test]
    fn display_name_is_not_empty() {
        let platform = LinuxPlatform::new();
        let name = platform.display_name();
        assert!(!name.is_empty());
        assert!(name.starts_with("Linux"));
    }

    #[test]
    fn check_permissions_returns_vec() {
        let platform = LinuxPlatform::new();
        // We just check it doesn't panic -- actual results depend on host.
        let _issues = platform.check_permissions();
    }

    #[test]
    fn daemon_lock_creates_and_cleans_pid_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = tmp.path().join(APP_DIR_NAME);

        let platform = LinuxPlatform {
            config: PathBuf::from("/tmp/voxforge-test-config"),
            data: PathBuf::from("/tmp/voxforge-test-data"),
            cache: PathBuf::from("/tmp/voxforge-test-cache"),
            runtime: Some(runtime.clone()),
            is_wayland: false,
        };

        let pid_path = runtime.join("voxforge.pid");

        {
            let lock = platform.daemon_lock().expect("should acquire daemon lock");
            assert!(
                pid_path.exists(),
                "PID file should exist while lock is held"
            );

            let contents = fs::read_to_string(&pid_path).expect("read pid file");
            let pid: u32 = contents.trim().parse().expect("pid should be a number");
            assert_eq!(pid, std::process::id());

            // A second lock attempt should fail because the process is alive.
            let result = platform.daemon_lock();
            assert!(result.is_err(), "second lock attempt should fail");

            drop(lock);
        }

        // After drop, the PID file should be cleaned up.
        assert!(
            !pid_path.exists(),
            "PID file should be removed after DaemonLock is dropped"
        );
    }

    #[test]
    fn daemon_lock_removes_stale_pid() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = tmp.path().join(APP_DIR_NAME);
        fs::create_dir_all(&runtime).expect("create runtime dir");

        let pid_path = runtime.join("voxforge.pid");
        // Write a PID that almost certainly does not exist.
        fs::write(&pid_path, "4294967295").expect("write stale pid");

        let platform = LinuxPlatform {
            config: PathBuf::from("/tmp/voxforge-test-config"),
            data: PathBuf::from("/tmp/voxforge-test-data"),
            cache: PathBuf::from("/tmp/voxforge-test-cache"),
            runtime: Some(runtime),
            is_wayland: false,
        };

        let lock = platform
            .daemon_lock()
            .expect("should acquire lock after stale PID");
        drop(lock);
    }
}
