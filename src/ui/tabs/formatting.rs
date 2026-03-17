//! Formatting / LLM settings tab — provider selection, API keys, model choice.

use egui::Ui;

use crate::config::Config;
use crate::ui::widgets::api_key_input;

/// Per-tab state for the formatting settings panel.
#[derive(Default)]
pub struct FormattingTabState {
    pub anthropic_key_state: api_key_input::ApiKeyState,
    pub openai_key_state: api_key_input::ApiKeyState,
}

/// Draw the formatting settings tab.
pub fn draw(ui: &mut Ui, config: &mut Config, state: &mut FormattingTabState) {
    ui.heading("Formatting Provider");
    ui.add_space(8.0);

    // Provider selector
    let is_anthropic = config.formatting.provider == "anthropic";
    ui.horizontal(|ui| {
        if ui.radio(is_anthropic, "Anthropic").clicked() {
            config.formatting.provider = "anthropic".to_string();
        }
        if ui.radio(!is_anthropic, "OpenAI").clicked() {
            config.formatting.provider = "openai".to_string();
        }
    });

    ui.separator();

    if is_anthropic {
        draw_anthropic(ui, config, state);
    } else {
        draw_openai(ui, config, state);
    }

    // Formatting mode
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.label("Formatting Mode");
        ui.add_space(4.0);

        egui::ComboBox::from_label("Default mode")
            .selected_text(&config.formatting.default_mode)
            .show_ui(ui, |ui| {
                for mode in &["auto", "code", "email", "chat", "raw"] {
                    ui.selectable_value(
                        &mut config.formatting.default_mode,
                        (*mode).to_string(),
                        *mode,
                    );
                }
            });
    });

    // Timeout
    ui.add_space(crate::ui::theme::FIELD_SPACING);
    ui.group(|ui| {
        let mut timeout_ms = i32::try_from(config.formatting.timeout_ms).unwrap_or(i32::MAX);
        ui.horizontal(|ui| {
            ui.label("Timeout:");
            ui.add(egui::Slider::new(&mut timeout_ms, 1000..=15000).suffix(" ms"));
        });
        config.formatting.timeout_ms = u64::from(timeout_ms.max(0).cast_unsigned());
    });

    // Local LLM — coming soon placeholder
    ui.add_space(crate::ui::theme::SECTION_SPACING);
    ui.group(|ui| {
        ui.disable();
        ui.label("Local LLM (Coming Soon)");
        ui.add_space(4.0);
        ui.colored_label(
            crate::ui::theme::MUTED,
            "Support for local LLM providers (llama.cpp, Ollama) is planned.",
        );
    });
}

fn draw_anthropic(ui: &mut Ui, config: &mut Config, state: &mut FormattingTabState) {
    ui.group(|ui| {
        ui.label("Anthropic Settings");
        ui.add_space(4.0);

        api_key_input::api_key_input(
            ui,
            "API Key:",
            &mut config.formatting.anthropic.api_key,
            "ANTHROPIC_API_KEY",
            &mut state.anthropic_key_state,
        );

        ui.add_space(crate::ui::theme::FIELD_SPACING);

        egui::ComboBox::from_label("Model")
            .selected_text(&config.formatting.anthropic.model)
            .show_ui(ui, |ui| {
                for (value, label) in &[
                    ("claude-haiku-4-5-20251001", "Claude 4.5 Haiku"),
                    ("claude-sonnet-4-20250514", "Claude Sonnet 4"),
                ] {
                    ui.selectable_value(
                        &mut config.formatting.anthropic.model,
                        (*value).to_string(),
                        *label,
                    );
                }
            });
    });
}

fn draw_openai(ui: &mut Ui, config: &mut Config, state: &mut FormattingTabState) {
    ui.group(|ui| {
        ui.label("OpenAI Settings");
        ui.add_space(4.0);

        api_key_input::api_key_input(
            ui,
            "API Key:",
            &mut config.formatting.openai.api_key,
            "OPENAI_API_KEY",
            &mut state.openai_key_state,
        );

        ui.add_space(crate::ui::theme::FIELD_SPACING);

        egui::ComboBox::from_label("Model")
            .selected_text(&config.formatting.openai.model)
            .show_ui(ui, |ui| {
                for (value, label) in &[
                    ("gpt-4o-mini", "GPT-4o Mini"),
                    ("gpt-4o", "GPT-4o"),
                    ("gpt-4.1-mini", "GPT-4.1 Mini"),
                    ("gpt-4.1", "GPT-4.1"),
                ] {
                    ui.selectable_value(
                        &mut config.formatting.openai.model,
                        (*value).to_string(),
                        *label,
                    );
                }
            });

        ui.add_space(crate::ui::theme::FIELD_SPACING);

        ui.horizontal(|ui| {
            ui.label("Base URL (optional):");
            ui.add(
                egui::TextEdit::singleline(&mut config.formatting.openai.base_url)
                    .desired_width(300.0)
                    .hint_text("https://api.openai.com/v1"),
            );
        });
    });
}
