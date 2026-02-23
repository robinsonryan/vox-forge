//! Linux active window detection supporting Hyprland, Sway, and X11.

use std::process::Command;

use crate::error::{Error, Result};

use super::{AppContext, WindowDetector};

pub struct LinuxWindowDetector {
    session_type: SessionType,
}

enum SessionType {
    Hyprland,
    Sway,
    X11,
    Unknown,
}

impl LinuxWindowDetector {
    pub fn new() -> Self {
        let session_type = detect_session_type();
        Self { session_type }
    }

    #[allow(clippy::unused_self)]
    fn detect_hyprland(&self) -> Result<AppContext> {
        // hyprctl activewindow -j returns JSON with window info
        let output = Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .output()
            .map_err(|e| Error::Platform(format!("hyprctl failed: {e}")))?;

        if !output.status.success() {
            return Err(Error::Platform("hyprctl returned non-zero".to_string()));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::Platform(format!("Failed to parse hyprctl JSON: {e}")))?;

        let class = json
            .get("class")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let title = json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract executable name from initialClass or class
        let executable = json
            .get("initialClass")
            .and_then(|v| v.as_str())
            .unwrap_or(&class)
            .to_lowercase();

        Ok(AppContext {
            app_name: class,
            window_title: title,
            executable,
        })
    }

    #[allow(clippy::unused_self)]
    fn detect_sway(&self) -> Result<AppContext> {
        // swaymsg -t get_tree returns the full window tree
        let output = Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
            .map_err(|e| Error::Platform(format!("swaymsg failed: {e}")))?;

        if !output.status.success() {
            return Err(Error::Platform("swaymsg returned non-zero".to_string()));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::Platform(format!("Failed to parse sway tree: {e}")))?;

        // Recursively find the focused node
        if let Some(focused) = find_focused_node(&json) {
            let app_id = focused
                .get("app_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let name = focused
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            Ok(AppContext {
                app_name: app_id.clone(),
                window_title: name,
                executable: app_id.to_lowercase(),
            })
        } else {
            Err(Error::Platform(
                "No focused window found in sway tree".to_string(),
            ))
        }
    }

    #[allow(clippy::unused_self)]
    fn detect_x11(&self) -> Result<AppContext> {
        // xdotool getactivewindow getwindowclassname
        let class_output = Command::new("xdotool")
            .args(["getactivewindow", "getwindowclassname"])
            .output()
            .map_err(|e| Error::Platform(format!("xdotool class failed: {e}")))?;

        let class = String::from_utf8_lossy(&class_output.stdout)
            .trim()
            .to_string();

        // xdotool getactivewindow getwindowname
        let name_output = Command::new("xdotool")
            .args(["getactivewindow", "getwindowname"])
            .output()
            .map_err(|e| Error::Platform(format!("xdotool name failed: {e}")))?;

        let title = String::from_utf8_lossy(&name_output.stdout)
            .trim()
            .to_string();

        Ok(AppContext {
            app_name: class.clone(),
            window_title: title,
            executable: class.to_lowercase(),
        })
    }
}

impl WindowDetector for LinuxWindowDetector {
    fn active_window(&self) -> Result<AppContext> {
        let result = match self.session_type {
            SessionType::Hyprland => self.detect_hyprland(),
            SessionType::Sway => self.detect_sway(),
            SessionType::X11 => self.detect_x11(),
            SessionType::Unknown => {
                // Try each method in order
                self.detect_hyprland()
                    .or_else(|_| self.detect_sway())
                    .or_else(|_| self.detect_x11())
            }
        };

        // Never fail — fall back to unknown context
        result.or_else(|e| {
            tracing::debug!("Window detection failed: {e}, using fallback");
            Ok(AppContext::unknown())
        })
    }
}

fn detect_session_type() -> SessionType {
    detect_session_type_with(|key| std::env::var(key).ok())
}

/// Determine session type by probing environment variables via a lookup function.
/// Separated from [`detect_session_type`] so tests can inject values without
/// mutating process-global env (which is unsafe in Rust 2024 edition).
fn detect_session_type_with<F>(env: F) -> SessionType
where
    F: Fn(&str) -> Option<String>,
{
    if env("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        SessionType::Hyprland
    } else if env("SWAYSOCK").is_some() {
        SessionType::Sway
    } else if env("DISPLAY").is_some() {
        SessionType::X11
    } else {
        SessionType::Unknown
    }
}

fn find_focused_node(node: &serde_json::Value) -> Option<&serde_json::Value> {
    if node.get("focused").and_then(serde_json::Value::as_bool) == Some(true) {
        // Check if this is a leaf node (has app_id or window properties)
        if node.get("app_id").is_some() || node.get("window_properties").is_some() {
            return Some(node);
        }
    }

    // Recurse into child nodes
    if let Some(nodes) = node.get("nodes").and_then(|v| v.as_array()) {
        for child in nodes {
            if let Some(found) = find_focused_node(child) {
                return Some(found);
            }
        }
    }
    if let Some(floating) = node.get("floating_nodes").and_then(|v| v.as_array()) {
        for child in floating {
            if let Some(found) = find_focused_node(child) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_session_hyprland() {
        let session = detect_session_type_with(|key| match key {
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("test".to_string()),
            _ => None,
        });
        assert!(matches!(session, SessionType::Hyprland));
    }

    #[test]
    fn detect_session_sway() {
        let session = detect_session_type_with(|key| match key {
            "SWAYSOCK" => Some("/run/user/1000/sway-ipc.sock".to_string()),
            _ => None,
        });
        assert!(matches!(session, SessionType::Sway));
    }

    #[test]
    fn detect_session_x11() {
        let session = detect_session_type_with(|key| match key {
            "DISPLAY" => Some(":0".to_string()),
            _ => None,
        });
        assert!(matches!(session, SessionType::X11));
    }

    #[test]
    fn detect_session_unknown_when_no_env_vars() {
        let session = detect_session_type_with(|_| None);
        assert!(matches!(session, SessionType::Unknown));
    }

    #[test]
    fn detect_session_hyprland_takes_priority_over_sway() {
        let session = detect_session_type_with(|key| match key {
            "HYPRLAND_INSTANCE_SIGNATURE" => Some("test".to_string()),
            "SWAYSOCK" => Some("/run/user/1000/sway-ipc.sock".to_string()),
            _ => None,
        });
        assert!(matches!(session, SessionType::Hyprland));
    }

    #[test]
    fn find_focused_in_sway_tree() {
        let tree = serde_json::json!({
            "type": "root",
            "focused": false,
            "nodes": [
                {
                    "type": "output",
                    "focused": false,
                    "nodes": [
                        {
                            "type": "workspace",
                            "focused": false,
                            "nodes": [
                                {
                                    "type": "con",
                                    "focused": true,
                                    "app_id": "firefox",
                                    "name": "Mozilla Firefox",
                                    "nodes": [],
                                    "floating_nodes": []
                                }
                            ],
                            "floating_nodes": []
                        }
                    ],
                    "floating_nodes": []
                }
            ],
            "floating_nodes": []
        });

        let focused = find_focused_node(&tree);
        assert!(focused.is_some());
        let node = focused.expect("should find focused node");
        assert_eq!(node.get("app_id").and_then(|v| v.as_str()), Some("firefox"));
        assert_eq!(
            node.get("name").and_then(|v| v.as_str()),
            Some("Mozilla Firefox")
        );
    }

    #[test]
    fn find_focused_in_floating_nodes() {
        let tree = serde_json::json!({
            "type": "root",
            "focused": false,
            "nodes": [],
            "floating_nodes": [
                {
                    "type": "floating_con",
                    "focused": true,
                    "app_id": "pavucontrol",
                    "name": "Volume Control",
                    "nodes": [],
                    "floating_nodes": []
                }
            ]
        });

        let focused = find_focused_node(&tree);
        assert!(focused.is_some());
        let node = focused.expect("should find floating focused node");
        assert_eq!(
            node.get("app_id").and_then(|v| v.as_str()),
            Some("pavucontrol")
        );
    }

    #[test]
    fn find_focused_returns_none_when_nothing_focused() {
        let tree = serde_json::json!({
            "type": "root",
            "focused": false,
            "nodes": [
                {
                    "type": "con",
                    "focused": false,
                    "app_id": "firefox",
                    "name": "Firefox",
                    "nodes": [],
                    "floating_nodes": []
                }
            ],
            "floating_nodes": []
        });

        let focused = find_focused_node(&tree);
        assert!(focused.is_none());
    }

    #[test]
    fn active_window_never_panics() {
        // LinuxWindowDetector::active_window should always return Ok
        // even when no desktop session tools are available (CI environment).
        let detector = LinuxWindowDetector {
            session_type: SessionType::Unknown,
        };
        let result = detector.active_window();
        assert!(result.is_ok());
    }
}
