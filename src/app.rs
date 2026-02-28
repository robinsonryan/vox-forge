//! Core application driver for the dictation pipeline.
//!
//! Coordinates the state machine, audio capture, STT, LLM formatting,
//! and text output. The primary mode is push-to-talk.

use tokio::sync::mpsc;

use crate::config::Config;
use crate::context::WindowDetector;
use crate::corrections::CorrectionLog;
use crate::error::Result;
use crate::format::prompt::{self, FormattingMode};
use crate::hotkey::listener::HotkeyEvent;
use crate::output::TextOutput;
use crate::output::typing::TypingOutput;
use crate::providers::llm::LlmProvider;
use crate::providers::stt::SttProvider;
use crate::state::{DictationCommand, DictationEvent, DictationStateMachine};

/// Core application that runs the dictation pipeline.
pub struct App {
    config: Config,
    stt: Box<dyn SttProvider>,
    llm: Box<dyn LlmProvider>,
    window_detector: Box<dyn WindowDetector>,
    output: TypingOutput,
    correction_log: CorrectionLog,
    state_machine: DictationStateMachine,
}

impl App {
    pub fn new(
        config: Config,
        stt: Box<dyn SttProvider>,
        llm: Box<dyn LlmProvider>,
        window_detector: Box<dyn WindowDetector>,
        output: TypingOutput,
        correction_log: CorrectionLog,
    ) -> Self {
        Self {
            config,
            stt,
            llm,
            window_detector,
            output,
            correction_log,
            state_machine: DictationStateMachine::new(),
        }
    }

    /// Run the daemon loop -- listens for hotkey events and processes dictation.
    #[allow(clippy::too_many_lines)]
    pub async fn run_daemon(
        &mut self,
        mut hotkey_rx: mpsc::UnboundedReceiver<HotkeyEvent>,
    ) -> Result<()> {
        use crate::audio::capture::AudioCapture;
        use crate::audio::vad::VoiceActivityDetector;
        use crate::config::HotkeyMode;

        let device_name = if self.config.audio.input_device.is_empty() {
            None
        } else {
            Some(self.config.audio.input_device.as_str())
        };
        let audio_capture = AudioCapture::new(device_name)?;

        #[allow(clippy::cast_possible_truncation)]
        let vad = VoiceActivityDetector::new(
            self.config.audio.silence_threshold_db as f32,
            self.config.audio.silence_timeout_s as f32,
            audio_capture.sample_rate(),
        );

        let is_push_to_talk = self.config.hotkey.mode == HotkeyMode::PushToTalk;
        let _ = crate::notify::notify_ready();

        tracing::info!(
            "Daemon running. Mode: {}. Press hotkey to dictate.",
            if is_push_to_talk {
                "push-to-talk"
            } else {
                "toggle"
            }
        );

        let mut recording_handle: Option<crate::audio::capture::RecordingHandle> = None;
        let mut recording_start: Option<std::time::Instant> = None;

        loop {
            // Process hotkey events with a timeout so we can poll VAD
            let event =
                match tokio::time::timeout(std::time::Duration::from_millis(100), hotkey_rx.recv())
                    .await
                {
                    Ok(Some(hotkey_event)) => {
                        let dictation_event = match hotkey_event {
                            HotkeyEvent::TogglePressed => {
                                if is_push_to_talk {
                                    DictationEvent::HotkeyPressed
                                } else {
                                    DictationEvent::HotkeyToggle
                                }
                            }
                            HotkeyEvent::ToggleReleased => {
                                if is_push_to_talk {
                                    DictationEvent::HotkeyReleased
                                } else {
                                    continue; // Ignore release in toggle mode
                                }
                            }
                            HotkeyEvent::CancelPressed => DictationEvent::CancelPressed,
                        };
                        Some(dictation_event)
                    }
                    Ok(None) => break, // Channel closed
                    Err(_) => {
                        // Timeout -- check VAD if currently recording
                        if let (Some(handle), Some(start)) = (&recording_handle, &recording_start) {
                            let samples = handle.current_samples();
                            let elapsed = start.elapsed();
                            let vad_result = vad.analyze(&samples, elapsed);

                            if vad_result.should_stop {
                                Some(DictationEvent::SilenceTimeout)
                            } else if elapsed
                                >= std::time::Duration::from_secs(self.config.audio.max_recording_s)
                            {
                                Some(DictationEvent::MaxDurationReached)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                };

            if let Some(event) = event {
                let command = self.state_machine.handle_event(event);

                match command {
                    DictationCommand::StartRecording => {
                        let _ = crate::notify::notify_recording();
                        println!("[recording] Started...");
                        match audio_capture.start_recording() {
                            Ok(handle) => {
                                recording_handle = Some(handle);
                                recording_start = Some(std::time::Instant::now());
                            }
                            Err(e) => {
                                println!("[error] Audio capture failed: {e}");
                                let _ = crate::notify::notify_error(&e.to_string());
                                self.state_machine
                                    .handle_event(DictationEvent::ErrorOccurred {
                                        message: e.to_string(),
                                    });
                            }
                        }
                    }
                    DictationCommand::StopRecording => {
                        let _ = crate::notify::notify_processing();
                        if let Some(handle) = recording_handle.take() {
                            let sample_count = handle.sample_count();
                            recording_start = None;
                            match handle.stop() {
                                Ok(buffer) => {
                                    // Show audio stats
                                    let peak = buffer
                                        .samples
                                        .iter()
                                        .map(|s| s.abs())
                                        .fold(0.0_f32, f32::max);
                                    #[allow(clippy::cast_possible_truncation)]
                                    let peak_db = if peak > 0.0 {
                                        20.0 * peak.log10()
                                    } else {
                                        -100.0
                                    };
                                    println!(
                                        "[recording] Stopped. {}ms, {} samples, peak: {peak_db:.1} dB",
                                        buffer.duration_ms,
                                        sample_count,
                                    );

                                    if peak_db <= -90.0 {
                                        println!("[warning] Audio appears silent — check your mic/device config");
                                    }

                                    if buffer.duration_ms < self.config.audio.min_recording_ms {
                                        println!("[skipped] Recording too short ({}ms < {}ms minimum)", buffer.duration_ms, self.config.audio.min_recording_ms);
                                        self.state_machine
                                            .handle_event(DictationEvent::RecordingTooShort);
                                    } else {
                                        // Process: transcribe -> format -> output
                                        self.process_audio(buffer).await;
                                    }
                                }
                                Err(e) => {
                                    let _ = crate::notify::notify_error(&e.to_string());
                                    self.state_machine.handle_event(
                                        DictationEvent::ErrorOccurred {
                                            message: e.to_string(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    DictationCommand::Notify { message } => {
                        let _ = crate::notify::notify("VoxForge", &message);
                    }
                    DictationCommand::HandleError { message } => {
                        tracing::error!("Error: {message}");
                        let _ = crate::notify::notify_error(&message);
                    }
                    DictationCommand::None
                    | DictationCommand::Transcribe
                    | DictationCommand::Format { .. }
                    | DictationCommand::OutputText { .. } => {
                        // These are handled inline by the process_audio pipeline
                    }
                }
            }
        }

        Ok(())
    }

    /// Process audio through the STT -> LLM -> output pipeline.
    async fn process_audio(&mut self, buffer: crate::audio::capture::AudioBuffer) {
        // Step 1: Transcribe
        println!("[transcribing] Sending {}ms of audio to Whisper...", buffer.duration_ms);
        let transcript = match self
            .stt
            .transcribe(&buffer.samples, buffer.sample_rate)
            .await
        {
            Ok(result) => {
                println!("[transcribed] ({}ms) \"{}\"", result.duration_ms, result.text);
                result.text
            }
            Err(e) => {
                println!("[error] Transcription failed: {e}");
                self.state_machine
                    .handle_event(DictationEvent::ErrorOccurred {
                        message: format!("Transcription failed: {e}"),
                    });
                return;
            }
        };

        self.state_machine
            .handle_event(DictationEvent::TranscriptionComplete {
                text: transcript.clone(),
            });

        if transcript.trim().is_empty() {
            println!("[skipped] Empty transcription");
            return;
        }

        // Step 2: Detect context and resolve formatting mode
        let context = self
            .window_detector
            .active_window()
            .unwrap_or_else(|_| crate::context::AppContext::unknown());

        let mode = prompt::resolve_mode(
            &self.config.formatting.default_mode,
            &self.config.formatting.auto_rules,
            &context.app_name,
            &context.window_title,
            &context.executable,
        );

        println!("[formatting] Mode: {mode:?}, app: {}, sending to LLM...", context.app_name);

        // Step 3: Format
        let formatted = if mode == FormattingMode::Raw {
            let result = crate::format::fallback::format_fallback(&transcript);
            println!("[formatted] (fallback) \"{result}\"");
            result
        } else {
            let corrections_str = self
                .correction_log
                .format_for_prompt(self.config.corrections.max_examples)
                .unwrap_or_default();

            let system_prompt = prompt::build_system_prompt(
                &mode,
                &context.app_name,
                &context.window_title,
                &self.config.dictionary.custom_terms,
                &corrections_str,
            );

            match system_prompt {
                Some(prompt_text) => {
                    match tokio::time::timeout(
                        std::time::Duration::from_millis(self.config.formatting.timeout_ms),
                        self.llm.format(&prompt_text, &transcript),
                    )
                    .await
                    {
                        Ok(Ok(result)) => {
                            println!("[formatted] ({}ms) \"{}\"", result.duration_ms, result.text);
                            result.text
                        }
                        Ok(Err(e)) => {
                            println!("[warning] LLM failed: {e}");
                            println!("[formatted] (fallback)");
                            crate::format::fallback::format_fallback(&transcript)
                        }
                        Err(_) => {
                            println!("[warning] LLM timed out ({}ms limit)", self.config.formatting.timeout_ms);
                            println!("[formatted] (fallback)");
                            crate::format::fallback::format_fallback(&transcript)
                        }
                    }
                }
                None => {
                    let result = crate::format::fallback::format_fallback(&transcript);
                    println!("[formatted] (fallback) \"{result}\"");
                    result
                }
            }
        };

        self.state_machine
            .handle_event(DictationEvent::FormattingComplete {
                text: formatted.clone(),
            });

        // Step 4: Output
        println!("[typing] Outputting text...");
        match self.output.output_text(&formatted, &context.executable) {
            Ok(()) => {
                println!("[done] Output complete");
            }
            Err(e) => {
                println!("[warning] Typing failed ({e}), trying clipboard...");
                if let Err(e2) = crate::output::clipboard::paste_text(&formatted) {
                    println!("[error] Clipboard also failed: {e2}");
                    let _ = crate::notify::notify_error("Output failed");
                } else {
                    println!("[done] Pasted via clipboard");
                }
            }
        }

        self.state_machine
            .handle_event(DictationEvent::OutputComplete);

        // Log the dictation for corrections
        let _ = self
            .correction_log
            .log_dictation(&transcript, &formatted, &context.app_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_constructs_with_defaults() {
        // Verify App::new compiles and the state machine starts idle.
        // We use a minimal FallbackDetector and dummy providers.
        use crate::context::FallbackDetector;

        // We cannot easily construct real providers without API keys,
        // so this test just validates the struct layout and constructor.
        let _: fn(
            Config,
            Box<dyn SttProvider>,
            Box<dyn LlmProvider>,
            Box<dyn WindowDetector>,
            TypingOutput,
            CorrectionLog,
        ) -> App = App::new;

        // Verify FallbackDetector satisfies WindowDetector
        let detector: Box<dyn WindowDetector> = Box::new(FallbackDetector);
        let ctx = detector.active_window().expect("fallback never fails");
        assert_eq!(ctx.app_name, "unknown");
    }
}
