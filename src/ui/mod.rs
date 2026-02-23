//! GUI module — egui-based settings UI and system tray.
//!
//! All GUI code lives here. Communication with async backends
//! happens exclusively through channels (never calls async directly).

pub mod app;
pub mod tabs;
pub mod theme;
pub mod tray;
pub mod widgets;
