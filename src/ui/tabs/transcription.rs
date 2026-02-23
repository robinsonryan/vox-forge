//! Transcription settings tab — provider selection and audio configuration.

use egui::Ui;

use crate::config::Config;
use crate::ui::widgets::{api_key_input, status_badge};

/// Per-tab state for the transcription settings panel.
#[derive(Default)]
pub struct TranscriptionTabState {
    pub openai_key_state: api_key_input::ApiKeyState,
}

/// Draw the transcription settings tab.
pub fn draw(ui: &mut Ui, config: &mut Config, state: &mut TranscriptionTabState) {
    ui.heading("Transcription Provider");
    ui.add_space(8.0);

    // Provider selector
    let is_local = config.transcription.provider == "whisper_local";
    ui.horizontal(|ui| {
        if ui.radio(is_local, "Local Whisper").clicked() {
            config.transcription.provider = "whisper_local".to_string();
        }
        if ui.radio(!is_local, "OpenAI Whisper").clicked() {
            config.transcription.provider = "openai_whisper".to_string();
        }
    });

    ui.separator();

    if is_local {
        draw_local_whisper(ui, config);
    } else {
        draw_openai_whisper(ui, config, state);
    }

    // Advanced audio settings (collapsible)
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    egui::CollapsingHeader::new("Advanced Audio Settings")
        .default_open(false)
        .show(ui, |ui| {
            #[allow(clippy::cast_possible_truncation)]
            let mut threshold = config.audio.silence_threshold_db as f32;
            ui.horizontal(|ui| {
                ui.label("Silence threshold (dB):");
                ui.add(egui::Slider::new(&mut threshold, -60.0..=-20.0));
            });
            config.audio.silence_threshold_db = f64::from(threshold);

            let mut max_rec = i32::try_from(config.audio.max_recording_s).unwrap_or(i32::MAX);
            ui.horizontal(|ui| {
                ui.label("Max recording (s):");
                ui.add(egui::Slider::new(&mut max_rec, 10..=300));
            });
            config.audio.max_recording_s = u64::from(max_rec.max(0).cast_unsigned());

            #[allow(clippy::cast_possible_truncation)]
            let mut silence_timeout = config.audio.silence_timeout_s as f32;
            ui.horizontal(|ui| {
                ui.label("Silence timeout (s):");
                ui.add(egui::Slider::new(&mut silence_timeout, 1.0..=10.0));
            });
            config.audio.silence_timeout_s = f64::from(silence_timeout);
        });
}

fn draw_local_whisper(ui: &mut Ui, config: &mut Config) {
    ui.group(|ui| {
        ui.label("Local Whisper Settings");
        ui.add_space(4.0);

        egui::ComboBox::from_label("Model")
            .selected_text(&config.transcription.whisper_local.model)
            .show_ui(ui, |ui| {
                for model in &["tiny", "base", "small", "medium", "large-v3"] {
                    ui.selectable_value(
                        &mut config.transcription.whisper_local.model,
                        (*model).to_string(),
                        *model,
                    );
                }
            });

        egui::ComboBox::from_label("Device")
            .selected_text(&config.transcription.whisper_local.device)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut config.transcription.whisper_local.device,
                    "cuda".to_string(),
                    "CUDA (GPU)",
                );
                ui.selectable_value(
                    &mut config.transcription.whisper_local.device,
                    "cpu".to_string(),
                    "CPU",
                );
            });

        egui::ComboBox::from_label("Language")
            .selected_text(if config.transcription.whisper_local.language.is_empty() {
                "Auto-detect"
            } else {
                &config.transcription.whisper_local.language
            })
            .show_ui(ui, |ui| {
                for lang in &[
                    "en", "es", "fr", "de", "it", "pt", "nl", "ja", "ko", "zh", "",
                ] {
                    let label = if lang.is_empty() { "Auto-detect" } else { lang };
                    ui.selectable_value(
                        &mut config.transcription.whisper_local.language,
                        (*lang).to_string(),
                        label,
                    );
                }
            });

        status_badge::status_badge(
            ui,
            status_badge::StatusLevel::Warning,
            "Model status: check not implemented",
        );
    });
}

fn draw_openai_whisper(ui: &mut Ui, config: &mut Config, state: &mut TranscriptionTabState) {
    ui.group(|ui| {
        ui.label("OpenAI Whisper Settings");
        ui.add_space(4.0);

        api_key_input::api_key_input(
            ui,
            "API Key:",
            &mut config.transcription.openai_whisper.api_key,
            "OPENAI_API_KEY",
            &mut state.openai_key_state,
        );

        ui.label("Model: whisper-1");
        ui.colored_label(
            crate::ui::theme::WARNING,
            "Note: Audio is sent to OpenAI's servers for transcription",
        );
    });
}
