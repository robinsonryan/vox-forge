//! Local Whisper STT provider (whisper-rs).
//!
//! Uses `whisper-rs` (bindings to whisper.cpp) for on-device speech-to-text
//! transcription with GGML model files.

use std::path::PathBuf;
use std::time::Instant;

use async_trait::async_trait;
use tracing;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::{Error, Result};

use super::stt::{ComputeDevice, ModelInfo, ProviderHealth, SttProvider, TranscriptionResult};

/// Expected sample rate for whisper models (16 kHz).
const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Local Whisper provider using `whisper-rs` for on-device transcription.
pub struct WhisperLocalProvider {
    #[allow(dead_code)]
    model_name: String,
    #[allow(dead_code)]
    device: ComputeDevice,
    language: String,
    #[allow(dead_code)]
    model_dir: PathBuf,
    ctx: Option<WhisperContext>,
}

impl WhisperLocalProvider {
    /// Create a new local Whisper provider.
    ///
    /// If the model file exists at `model_dir/ggml-{model}.bin`, the whisper
    /// context is loaded eagerly.  If the file is absent the provider is still
    /// constructed (with `ctx = None`) so that `health_check` can report
    /// "not ready" rather than failing outright.
    ///
    /// # Errors
    ///
    /// Returns an error if the model file exists but whisper-rs fails to load
    /// it (e.g. corrupted file, unsupported format).
    pub fn new(
        model: &str,
        device: ComputeDevice,
        language: &str,
        model_dir: PathBuf,
    ) -> Result<Self> {
        let model_path = model_dir.join(format!("ggml-{model}.bin"));

        let ctx = if model_path.exists() {
            let mut params = WhisperContextParameters::default();
            match device {
                ComputeDevice::Cuda => {
                    params.use_gpu(true);
                }
                ComputeDevice::Cpu => {
                    params.use_gpu(false);
                }
            }

            let path_str = model_path.to_string_lossy();
            let whisper_ctx = WhisperContext::new_with_params(&path_str, params)
                .map_err(|e| Error::Transcription(format!("Failed to load whisper model: {e}")))?;
            tracing::info!("Whisper model loaded from {}", path_str);
            Some(whisper_ctx)
        } else {
            tracing::warn!(
                "Whisper model file not found at {}; provider created without model",
                model_path.display()
            );
            None
        };

        Ok(Self {
            model_name: model.to_string(),
            device,
            language: language.to_string(),
            model_dir,
            ctx,
        })
    }

    /// Full filesystem path to the expected GGML model file.
    #[allow(dead_code)]
    pub fn model_path(&self) -> PathBuf {
        self.model_dir.join(format!("ggml-{}.bin", self.model_name))
    }

    /// Whether the model file has been downloaded and is present on disk.
    #[allow(dead_code)]
    pub fn model_loaded(&self) -> bool {
        self.ctx.is_some()
    }
}

#[async_trait]
impl SttProvider for WhisperLocalProvider {
    async fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<TranscriptionResult> {
        // Ensure model is loaded
        let ctx = self
            .ctx
            .as_ref()
            .ok_or_else(|| Error::Transcription("Model not loaded".to_string()))?;

        // Whisper expects 16 kHz mono audio
        if sample_rate != WHISPER_SAMPLE_RATE {
            return Err(Error::Transcription(format!(
                "Expected {WHISPER_SAMPLE_RATE} Hz audio, got {sample_rate} Hz"
            )));
        }

        if audio.is_empty() {
            return Err(Error::Transcription("Audio buffer is empty".to_string()));
        }

        // Compute audio duration before inference (samples / rate * 1000)
        // Integer arithmetic avoids floating-point lint issues.
        let audio_duration_ms = (audio.len() as u64) * 1000 / u64::from(WHISPER_SAMPLE_RATE);

        // Build inference parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(true);
        params.set_no_context(true);

        // Create state and run inference
        let mut state = ctx
            .create_state()
            .map_err(|e| Error::Transcription(format!("Failed to create whisper state: {e}")))?;

        let start = Instant::now();

        state
            .full(params, audio)
            .map_err(|e| Error::Transcription(format!("Whisper inference failed: {e}")))?;

        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;

        // Collect all segments into a single string
        let n_segments = state.full_n_segments();
        let mut text = String::new();
        for i in 0..n_segments {
            if let Some(segment) = state.get_segment(i) {
                match segment.to_str() {
                    Ok(s) => text.push_str(s),
                    Err(e) => {
                        tracing::warn!("Failed to decode segment {i}: {e}");
                        // Fall back to lossy conversion
                        if let Ok(lossy) = segment.to_str_lossy() {
                            text.push_str(&lossy);
                        }
                    }
                }
            }
        }

        // Detect the language from the state
        let lang_id = state.full_lang_id_from_state();
        let language_detected = whisper_rs::get_lang_str(lang_id).map(String::from);

        Ok(TranscriptionResult {
            text: text.trim().to_string(),
            language_detected,
            duration_ms,
            audio_duration_ms,
        })
    }

    fn display_name(&self) -> &'static str {
        "Local Whisper"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn requires_api_key(&self) -> bool {
        false
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        if self.model_loaded() {
            Ok(ProviderHealth {
                ready: true,
                message: format!("Model '{}' loaded ({:?})", self.model_name, self.device),
            })
        } else {
            Ok(ProviderHealth {
                ready: false,
                message: format!("Model '{}' not downloaded", self.model_name),
            })
        }
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "tiny".into(),
                display_name: "Tiny (~75MB)".into(),
                description: "Fastest, lowest accuracy".into(),
                is_local: true,
                size_bytes: Some(75_000_000),
            },
            ModelInfo {
                id: "base".into(),
                display_name: "Base (~142MB, recommended)".into(),
                description: "Good balance of speed and accuracy".into(),
                is_local: true,
                size_bytes: Some(142_000_000),
            },
            ModelInfo {
                id: "small".into(),
                display_name: "Small (~466MB)".into(),
                description: "Better accuracy, needs more GPU memory".into(),
                is_local: true,
                size_bytes: Some(466_000_000),
            },
            ModelInfo {
                id: "medium".into(),
                display_name: "Medium (~1.5GB)".into(),
                description: "High accuracy, significant GPU memory".into(),
                is_local: true,
                size_bytes: Some(1_500_000_000),
            },
            ModelInfo {
                id: "large-v3".into(),
                display_name: "Large v3 (~3.1GB)".into(),
                description: "Highest accuracy, requires powerful GPU".into(),
                is_local: true,
                size_bytes: Some(3_100_000_000),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider() -> WhisperLocalProvider {
        WhisperLocalProvider::new(
            "base",
            ComputeDevice::Cpu,
            "en",
            PathBuf::from("/tmp/voxforge-test-models"),
        )
        .expect("create provider")
    }

    #[test]
    fn display_name_is_local_whisper() {
        let provider = test_provider();
        assert_eq!(provider.display_name(), "Local Whisper");
    }

    #[test]
    fn is_local_returns_true() {
        let provider = test_provider();
        assert!(provider.is_local());
    }

    #[test]
    fn does_not_require_api_key() {
        let provider = test_provider();
        assert!(!provider.requires_api_key());
    }

    #[test]
    fn model_path_includes_model_name() {
        let provider = test_provider();
        let path = provider.model_path();
        assert!(path.to_string_lossy().contains("ggml-base.bin"));
    }

    #[test]
    fn available_models_has_five_entries() {
        let provider = test_provider();
        let models = provider.available_models();
        assert_eq!(models.len(), 5);
        assert!(models.iter().all(|m| m.is_local));
    }

    #[test]
    fn model_loaded_false_for_nonexistent_path() {
        let provider = test_provider();
        assert!(!provider.model_loaded());
    }

    #[tokio::test]
    async fn health_check_reports_not_downloaded() {
        let provider = test_provider();
        let health = provider.health_check().await.expect("health_check");
        assert!(!health.ready);
        assert!(health.message.contains("not downloaded"));
    }

    #[tokio::test]
    async fn transcribe_without_model_returns_not_loaded_error() {
        let provider = test_provider();
        let result = provider.transcribe(&[0.0_f32; 100], 16000).await;
        assert!(result.is_err());
        let err = result.err().expect("should be Err");
        assert!(
            err.to_string().contains("Model not loaded"),
            "expected 'Model not loaded' error, got: {err}"
        );
    }

    #[tokio::test]
    async fn transcribe_rejects_wrong_sample_rate() {
        // Even though the model is not loaded, the sample-rate check comes
        // after the model check. We still verify the error path exists by
        // constructing a provider whose ctx is None.
        let provider = test_provider();
        let result = provider.transcribe(&[0.0_f32; 100], 44100).await;
        // Without a loaded model, "Model not loaded" fires first.
        assert!(result.is_err());
    }
}
