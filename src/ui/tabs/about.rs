//! About / status tab — version info, diagnostics, and quick links.

use egui::Ui;

use crate::config::Config;
use crate::ui::widgets::status_badge::{self, StatusLevel};

/// Draw the about / status tab.
pub fn draw(ui: &mut Ui, config: &Config) {
    ui.heading("About VoxForge");
    ui.add_space(8.0);

    // Version info
    ui.group(|ui| {
        ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
        ui.label(format!("OS: {}", std::env::consts::OS));
        ui.label(format!("Arch: {}", std::env::consts::ARCH));

        if crate::platform::is_wayland() {
            ui.label("Display: Wayland");
        } else {
            ui.label("Display: X11 / Other");
        }
    });

    // Provider status
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Provider Status");
        ui.add_space(4.0);

        // Transcription provider
        let stt_label = format!("STT: {}", config.transcription.provider);
        status_badge::status_badge(ui, StatusLevel::Ready, &stt_label);

        // Formatting provider
        let llm_label = format!("LLM: {}", config.formatting.provider);
        let has_key = match config.formatting.provider.as_str() {
            "anthropic" => config.has_anthropic_key(),
            "openai" => config.has_openai_key(),
            _ => false,
        };
        let llm_status = if has_key {
            StatusLevel::Ready
        } else {
            StatusLevel::Warning
        };
        status_badge::status_badge(ui, llm_status, &llm_label);

        if !has_key {
            ui.colored_label(
                crate::ui::theme::WARNING,
                "API key not configured for the selected formatting provider.",
            );
        }
    });

    // Configuration
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Configuration");
        ui.add_space(4.0);

        let config_path = Config::default_path();
        ui.label(format!("Config: {}", config_path.display()));

        ui.horizontal(|ui| {
            if ui.button("Open Config Folder").clicked()
                && let Some(parent) = config_path.parent()
                && let Err(e) = open::that(parent)
            {
                tracing::error!("Failed to open config folder: {e}");
            }

            if ui.button("Open Log File").clicked() {
                let log_path = config_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(&config.general.log_file);
                if let Err(e) = open::that(&log_path) {
                    tracing::error!("Failed to open log file: {e}");
                }
            }
        });
    });

    // Diagnostics
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Diagnostics");
        ui.add_space(4.0);

        ui.colored_label(
            crate::ui::theme::MUTED,
            "Use the CLI for full diagnostic tests: voxforge test mic / hotkey / type",
        );
    });
}
