//! Visual theme constants and style configuration.
//!
//! Provides consistent colors, spacing, and rounding across the settings UI.

use egui::Color32;

pub const ACCENT: Color32 = Color32::from_rgb(99, 102, 241); // Indigo
pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94); // Green
pub const WARNING: Color32 = Color32::from_rgb(234, 179, 8); // Yellow
pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68); // Red
pub const MUTED: Color32 = Color32::from_rgb(148, 163, 184); // Gray

pub const TAB_WIDTH: f32 = 140.0;
pub const SECTION_SPACING: f32 = 16.0;
pub const FIELD_SPACING: f32 = 8.0;

/// Apply the `VoxForge` visual theme to an egui context.
pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(16);
    let cr = egui::CornerRadius::same(4);
    style.visuals.widgets.noninteractive.corner_radius = cr;
    style.visuals.widgets.inactive.corner_radius = cr;
    style.visuals.widgets.active.corner_radius = cr;
    style.visuals.widgets.hovered.corner_radius = cr;
    ctx.set_style(style);
}
