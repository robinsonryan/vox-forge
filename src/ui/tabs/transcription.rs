//! Transcription settings tab — provider selection and audio configuration.

use std::sync::{Arc, Mutex};

use egui::Ui;

use crate::config::Config;
use crate::ipc::IpcResponse;
use crate::ui::widgets::{api_key_input, status_badge};

/// Status of a background IPC operation (recalibrate, etc.).
#[derive(Default)]
enum IpcStatus {
    #[default]
    Idle,
    Pending,
    Done(IpcResponse),
    Failed(String),
}

/// Per-tab state for the transcription settings panel.
#[derive(Default)]
pub struct TranscriptionTabState {
    pub openai_key_state: api_key_input::ApiKeyState,
    recalibrate_status: Arc<Mutex<IpcStatus>>,
}

/// Draw the transcription settings tab.
#[allow(clippy::too_many_lines)]
pub fn draw(ui: &mut Ui, config: &mut Config, state: &mut TranscriptionTabState) {
    ui.heading("Transcription Provider");
    ui.add_space(8.0);

    // Provider selector — snapshot the current value to avoid borrow conflicts in closures.
    let provider = config.transcription.provider.clone();
    ui.horizontal(|ui| {
        if ui
            .radio(provider == "whisper_local", "Local Whisper")
            .clicked()
        {
            config.transcription.provider = "whisper_local".to_string();
        }
        if ui
            .radio(provider == "openai_whisper", "OpenAI Whisper")
            .clicked()
        {
            config.transcription.provider = "openai_whisper".to_string();
        }
    });
    let provider = config.transcription.provider.clone();
    ui.horizontal(|ui| {
        if ui
            .radio(provider == "cohere_transcribe", "Cohere Transcribe")
            .clicked()
        {
            config.transcription.provider = "cohere_transcribe".to_string();
        }
        if ui.radio(provider == "voxtral", "Voxtral").clicked() {
            config.transcription.provider = "voxtral".to_string();
        }
    });

    ui.separator();

    match config.transcription.provider.as_str() {
        "openai_whisper" => draw_openai_whisper(ui, config, state),
        "cohere_transcribe" => draw_cohere_transcribe(ui, config),
        "voxtral" => draw_voxtral(ui, config),
        _ => draw_local_whisper(ui, config),
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

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Recalibrate button
            let status = state
                .recalibrate_status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let is_pending = matches!(*status, IpcStatus::Pending);
            drop(status);

            ui.horizontal(|ui| {
                let button = ui.add_enabled(
                    !is_pending,
                    egui::Button::new(if is_pending {
                        "Recalibrating..."
                    } else {
                        "Recalibrate Microphone"
                    }),
                );

                if button.clicked() {
                    let status_ref = Arc::clone(&state.recalibrate_status);
                    if let Ok(mut s) = status_ref.lock() {
                        *s = IpcStatus::Pending;
                    }

                    std::thread::spawn(move || {
                        let socket = crate::platform::current_platform().ipc_socket_path();
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build();
                        let result = match rt {
                            Ok(rt) => rt.block_on(crate::ipc::send_command(
                                &socket,
                                crate::ipc::IpcCommand::Recalibrate,
                            )),
                            Err(e) => Err(crate::error::Error::Ipc(e.to_string())),
                        };
                        if let Ok(mut s) = status_ref.lock() {
                            match result {
                                Ok(resp) => *s = IpcStatus::Done(resp),
                                Err(e) => *s = IpcStatus::Failed(e.to_string()),
                            }
                        }
                    });
                }

                // Show status feedback
                let status = state
                    .recalibrate_status
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match &*status {
                    IpcStatus::Idle => {}
                    IpcStatus::Pending => {
                        ui.spinner();
                    }
                    IpcStatus::Done(resp) => {
                        ui.colored_label(crate::ui::theme::SUCCESS, &resp.message);
                    }
                    IpcStatus::Failed(msg) => {
                        ui.colored_label(crate::ui::theme::WARNING, msg);
                    }
                }
            });
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

fn draw_cohere_transcribe(ui: &mut Ui, config: &mut Config) {
    ui.group(|ui| {
        ui.label("Cohere Transcribe Settings");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("vLLM Endpoint:");
            ui.text_edit_singleline(&mut config.transcription.cohere_transcribe.endpoint);
        });

        ui.add_space(4.0);
        ui.label("Model: CohereLabs/cohere-transcribe-03-2026");
        ui.label("2B params | ~4-6 GB VRAM | 5.42% WER");
        ui.add_space(4.0);
        ui.colored_label(
            crate::ui::theme::MUTED,
            "Requires vLLM sidecar: vllm serve CohereLabs/cohere-transcribe-03-2026 --trust-remote-code",
        );
    });
}

fn draw_voxtral(ui: &mut Ui, config: &mut Config) {
    ui.group(|ui| {
        ui.label("Voxtral Settings");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("vLLM Endpoint:");
            ui.text_edit_singleline(&mut config.transcription.voxtral.endpoint);
        });

        ui.add_space(4.0);
        ui.label("Model: mistralai/Voxtral-Mini-3B-2507");
        ui.label("3B params | ~6 GB VRAM | 8 languages");
        ui.add_space(4.0);
        ui.colored_label(
            crate::ui::theme::MUTED,
            "Requires vLLM sidecar: vllm serve mistralai/Voxtral-Mini-3B-2507 --tokenizer_mode mistral --config_format mistral --load_format mistral",
        );
    });
}
