//! Status indicator widget — colored dot with label text.

use egui::Ui;

use crate::ui::theme;

/// Severity level for a status badge.
#[derive(Debug, Clone, Copy)]
pub enum StatusLevel {
    Ready,
    Warning,
    Error,
    Inactive,
}

/// Draw a small colored circle followed by a label.
pub fn status_badge(ui: &mut Ui, level: StatusLevel, text: &str) {
    let color = match level {
        StatusLevel::Ready => theme::SUCCESS,
        StatusLevel::Warning => theme::WARNING,
        StatusLevel::Error => theme::ERROR,
        StatusLevel::Inactive => theme::MUTED,
    };

    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, color);
        ui.label(text);
    });
}
