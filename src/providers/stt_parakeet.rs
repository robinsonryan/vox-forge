//! Local Parakeet STT provider (transcribe-rs, ONNX Runtime on CPU).
//!
//! Uses `transcribe-rs` to run NVIDIA Parakeet TDT 0.6B v3 (int8 ONNX export)
//! entirely on the CPU. Intended for machines without an NVIDIA GPU, where it
//! is much faster than Whisper at comparable accuracy.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use tracing;
use transcribe_rs::onnx::Quantization;
use transcribe_rs::onnx::parakeet::{ParakeetModel, ParakeetParams};

use crate::error::{Error, Result};

use super::stt::{ModelInfo, ProviderHealth, SttProvider, TranscriptionResult};

/// Directory name of the model under the models dir. Matches the directory
/// the published `parakeet-v3-int8.tar.gz` tarball extracts to.
pub const PARAKEET_MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3-int8";

/// Expected sample rate for Parakeet (16 kHz mono).
const PARAKEET_SAMPLE_RATE: u32 = 16_000;

/// Local Parakeet provider using `transcribe-rs` for on-device transcription.
pub struct ParakeetProvider {
    model_path: PathBuf,
    /// `ParakeetModel::transcribe_with` takes `&mut self`, so the model sits
    /// behind a mutex and inference runs on the blocking thread pool.
    model: Option<Arc<Mutex<ParakeetModel>>>,
}

impl ParakeetProvider {
    /// Create a new Parakeet provider.
    ///
    /// If the model directory exists at `models_dir/parakeet-tdt-0.6b-v3-int8`,
    /// the model is loaded eagerly. If it is absent the provider is still
    /// constructed (with `model = None`) so that `health_check` can report
    /// "not ready" rather than failing outright.
    ///
    /// # Errors
    ///
    /// Returns an error if the model directory exists but transcribe-rs fails
    /// to load it (e.g. missing or corrupted ONNX files).
    pub fn new(models_dir: &Path) -> Result<Self> {
        let model_path = models_dir.join(PARAKEET_MODEL_DIR_NAME);

        let model = if model_path.is_dir() {
            let loaded = ParakeetModel::load(&model_path, &Quantization::Int8)
                .map_err(|e| Error::Transcription(format!("Failed to load Parakeet model: {e}")))?;
            tracing::info!("Parakeet model loaded from {}", model_path.display());
            Some(Arc::new(Mutex::new(loaded)))
        } else {
            tracing::warn!(
                "Parakeet model directory not found at {}; provider created without model",
                model_path.display()
            );
            None
        };

        Ok(Self { model_path, model })
    }

    /// Full filesystem path to the expected model directory.
    #[allow(dead_code)]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Whether the model has been downloaded and loaded.
    #[allow(dead_code)]
    pub fn model_loaded(&self) -> bool {
        self.model.is_some()
    }
}

#[async_trait]
impl SttProvider for ParakeetProvider {
    async fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<TranscriptionResult> {
        // Validate the input before the model check so bad input is reported
        // the same way whether or not the model is present.
        if sample_rate != PARAKEET_SAMPLE_RATE {
            return Err(Error::Transcription(format!(
                "Expected {PARAKEET_SAMPLE_RATE} Hz audio, got {sample_rate} Hz"
            )));
        }

        if audio.is_empty() {
            return Err(Error::Transcription("Audio buffer is empty".to_string()));
        }

        let model = Arc::clone(
            self.model
                .as_ref()
                .ok_or_else(|| Error::Transcription("Model not loaded".to_string()))?,
        );

        // Integer arithmetic avoids floating-point lint issues.
        let audio_duration_ms = (audio.len() as u64) * 1000 / u64::from(PARAKEET_SAMPLE_RATE);

        // The closure must own its input to run on the blocking pool.
        let samples = audio.to_vec();
        let start = Instant::now();

        let result = tokio::task::spawn_blocking(move || {
            let mut model = model
                .lock()
                .map_err(|_| Error::Transcription("Parakeet model lock poisoned".to_string()))?;
            model
                .transcribe_with(&samples, &ParakeetParams::default())
                .map_err(|e| Error::Transcription(format!("Parakeet inference failed: {e}")))
        })
        .await
        .map_err(|e| Error::Transcription(format!("Parakeet inference task failed: {e}")))??;

        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(TranscriptionResult {
            text: result.text.trim().to_string(),
            language_detected: None,
            duration_ms,
            audio_duration_ms,
        })
    }

    fn display_name(&self) -> &'static str {
        "Parakeet"
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
                message: format!("Model '{PARAKEET_MODEL_DIR_NAME}' loaded (CPU)"),
            })
        } else {
            Ok(ProviderHealth {
                ready: false,
                message: format!(
                    "Model '{PARAKEET_MODEL_DIR_NAME}' not downloaded (expected at {})",
                    self.model_path.display()
                ),
            })
        }
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: PARAKEET_MODEL_DIR_NAME.into(),
            display_name: "Parakeet TDT 0.6B v3 int8 (~670MB)".into(),
            description: "English + 24 European languages, fast on CPU".into(),
            is_local: true,
            size_bytes: Some(670_000_000),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider() -> ParakeetProvider {
        ParakeetProvider::new(Path::new("/tmp/voxforge-test-models")).expect("create provider")
    }

    #[test]
    fn display_name_is_parakeet() {
        assert_eq!(test_provider().display_name(), "Parakeet");
    }

    #[test]
    fn is_local_and_needs_no_api_key() {
        let provider = test_provider();
        assert!(provider.is_local());
        assert!(!provider.requires_api_key());
    }

    #[test]
    fn model_path_is_model_dir_under_models_dir() {
        let provider = test_provider();
        assert_eq!(
            provider.model_path(),
            Path::new("/tmp/voxforge-test-models/parakeet-tdt-0.6b-v3-int8")
        );
    }

    #[test]
    fn available_models_has_single_local_entry() {
        let models = test_provider().available_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "parakeet-tdt-0.6b-v3-int8");
        assert!(models[0].is_local);
    }

    #[test]
    fn model_loaded_false_for_nonexistent_path() {
        assert!(!test_provider().model_loaded());
    }

    #[tokio::test]
    async fn health_check_not_ready_names_expected_path() {
        let health = test_provider().health_check().await.expect("health_check");
        assert!(!health.ready);
        assert!(health.message.contains("not downloaded"));
        assert!(
            health
                .message
                .contains("/tmp/voxforge-test-models/parakeet-tdt-0.6b-v3-int8"),
            "health message should name the expected path, got: {}",
            health.message
        );
    }

    #[tokio::test]
    async fn transcribe_without_model_returns_not_loaded_error() {
        let err = test_provider()
            .transcribe(&[0.0_f32; 100], 16_000)
            .await
            .expect_err("should be Err");
        assert!(
            err.to_string().contains("Model not loaded"),
            "expected 'Model not loaded' error, got: {err}"
        );
    }

    #[tokio::test]
    async fn transcribe_rejects_wrong_sample_rate() {
        let err = test_provider()
            .transcribe(&[0.0_f32; 100], 44_100)
            .await
            .expect_err("should be Err");
        assert!(
            err.to_string()
                .contains("Expected 16000 Hz audio, got 44100 Hz"),
            "expected sample-rate error, got: {err}"
        );
    }

    #[tokio::test]
    async fn transcribe_rejects_empty_audio() {
        let err = test_provider()
            .transcribe(&[], 16_000)
            .await
            .expect_err("should be Err");
        assert!(
            err.to_string().contains("Audio buffer is empty"),
            "expected empty-audio error, got: {err}"
        );
    }

    /// Real-audio smoke test. Needs the downloaded model and a 16 kHz mono
    /// 16-bit WAV of English speech:
    ///
    /// ```text
    /// VOXFORGE_PARAKEET_MODEL_DIR=/path/to/parakeet-tdt-0.6b-v3-int8 \
    /// VOXFORGE_PARAKEET_WAV=/path/to/speech.wav \
    /// cargo test --release -- --ignored parakeet --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "needs the Parakeet model and a speech WAV on disk"]
    async fn parakeet_smoke_transcribes_real_speech() {
        let model_dir = PathBuf::from(
            std::env::var("VOXFORGE_PARAKEET_MODEL_DIR")
                .expect("set VOXFORGE_PARAKEET_MODEL_DIR to the model directory"),
        );
        let wav = PathBuf::from(
            std::env::var("VOXFORGE_PARAKEET_WAV").expect("set VOXFORGE_PARAKEET_WAV"),
        );
        assert_eq!(
            model_dir.file_name().and_then(|n| n.to_str()),
            Some(PARAKEET_MODEL_DIR_NAME),
            "model dir must be named {PARAKEET_MODEL_DIR_NAME}"
        );
        let parent = model_dir.parent().expect("model dir has a parent");

        let samples = transcribe_rs::audio::read_wav_samples(&wav).expect("read 16 kHz mono WAV");

        let load_start = Instant::now();
        let provider = ParakeetProvider::new(parent).expect("load provider");
        let load_time = load_start.elapsed();
        assert!(
            provider.model_loaded(),
            "model should load from {model_dir:?}"
        );

        let result = provider
            .transcribe(&samples, PARAKEET_SAMPLE_RATE)
            .await
            .expect("transcribe");

        #[allow(clippy::cast_precision_loss)]
        let rtf = result.duration_ms as f64 / result.audio_duration_ms as f64;
        println!("text: {}", result.text);
        println!(
            "audio: {} ms | load: {load_time:.2?} | wall: {} ms | RTF: {rtf:.3}",
            result.audio_duration_ms, result.duration_ms
        );
        assert!(!result.text.is_empty(), "transcript should not be empty");
    }
}
