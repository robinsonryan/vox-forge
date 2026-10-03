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

use super::stt::{ModelInfo, ProviderHealth, SttProvider, TranscriptionResult, samples_to_ms};

/// Directory name of the model under the models dir. Matches the directory
/// the published `parakeet-v3-int8.tar.gz` tarball extracts to.
pub const PARAKEET_MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3-int8";

/// Files transcribe-rs reads from the model directory when loading the int8
/// model. A directory missing any of them is an incomplete download.
pub const PARAKEET_REQUIRED_FILES: [&str; 4] = [
    "encoder-model.int8.onnx",
    "decoder_joint-model.int8.onnx",
    "nemo128.onnx",
    "vocab.txt",
];

/// Expected sample rate for Parakeet (16 kHz mono).
const PARAKEET_SAMPLE_RATE: u32 = 16_000;

/// Local Parakeet provider using `transcribe-rs` for on-device transcription.
pub struct ParakeetProvider {
    model_path: PathBuf,
    /// `ParakeetModel::transcribe_with` takes `&mut self`, so the model sits
    /// behind a mutex and inference runs on the blocking thread pool.
    model: Option<Arc<Mutex<ParakeetModel>>>,
    /// Why the model directory was present but could not be loaded (e.g. a
    /// truncated download). `None` when the model loaded or was never there.
    load_error: Option<String>,
}

impl ParakeetProvider {
    /// Create a new Parakeet provider.
    ///
    /// If the model directory exists at `models_dir/parakeet-tdt-0.6b-v3-int8`,
    /// the model is loaded eagerly. If it is absent, or present but fails to
    /// load (e.g. a truncated download), the provider is still constructed
    /// without a model so the daemon starts and `health_check` reports why it
    /// is not ready.
    pub fn new(models_dir: &Path) -> Self {
        let model_path = models_dir.join(PARAKEET_MODEL_DIR_NAME);

        if !model_path.is_dir() {
            tracing::warn!(
                "Parakeet model directory not found at {}; provider created without model",
                model_path.display()
            );
            return Self {
                model_path,
                model: None,
                load_error: None,
            };
        }

        match ParakeetModel::load(&model_path, &Quantization::Int8) {
            Ok(loaded) => {
                tracing::info!("Parakeet model loaded from {}", model_path.display());
                Self {
                    model_path,
                    model: Some(Arc::new(Mutex::new(loaded))),
                    load_error: None,
                }
            }
            Err(e) => {
                tracing::error!(
                    "Parakeet model at {} failed to load: {e}; provider created without model",
                    model_path.display()
                );
                Self {
                    model_path,
                    model: None,
                    load_error: Some(e.to_string()),
                }
            }
        }
    }

    /// Whether the model has been downloaded and loaded.
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

        let model = Arc::clone(self.model.as_ref().ok_or_else(|| match &self.load_error {
            Some(cause) => Error::Transcription(format!("Model not loaded: {cause}")),
            None => Error::Transcription("Model not loaded".to_string()),
        })?);

        // Integer arithmetic avoids floating-point lint issues.
        let audio_duration_ms = samples_to_ms(audio.len(), PARAKEET_SAMPLE_RATE);

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
        } else if let Some(cause) = &self.load_error {
            Ok(ProviderHealth {
                ready: false,
                message: format!(
                    "Model '{PARAKEET_MODEL_DIR_NAME}' failed to load from {} ({cause}); \
                     delete that folder and re-download the model",
                    self.model_path.display()
                ),
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

    /// Provider built over an empty temp models dir (no model present). The
    /// `TempDir` is returned so the directory outlives the provider.
    fn test_provider() -> (tempfile::TempDir, ParakeetProvider) {
        let models_dir = tempfile::tempdir().expect("tempdir");
        let provider = ParakeetProvider::new(models_dir.path());
        (models_dir, provider)
    }

    /// Provider built over a model dir whose required files exist but hold
    /// garbage, as left behind by an interrupted download.
    fn unloadable_provider() -> (tempfile::TempDir, ParakeetProvider) {
        let models_dir = tempfile::tempdir().expect("tempdir");
        let parakeet_dir = models_dir.path().join(PARAKEET_MODEL_DIR_NAME);
        std::fs::create_dir(&parakeet_dir).expect("create model dir");
        for name in PARAKEET_REQUIRED_FILES {
            std::fs::write(parakeet_dir.join(name), b"").expect("write empty file");
        }
        let provider = ParakeetProvider::new(models_dir.path());
        (models_dir, provider)
    }

    #[test]
    fn display_name_is_parakeet() {
        assert_eq!(test_provider().1.display_name(), "Parakeet");
    }

    #[test]
    fn is_local_and_needs_no_api_key() {
        let (_dir, provider) = test_provider();
        assert!(provider.is_local());
        assert!(!provider.requires_api_key());
    }

    #[test]
    fn model_path_is_model_dir_under_models_dir() {
        let (dir, provider) = test_provider();
        assert_eq!(
            provider.model_path,
            dir.path().join("parakeet-tdt-0.6b-v3-int8")
        );
    }

    #[test]
    fn available_models_has_single_local_entry() {
        let models = test_provider().1.available_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "parakeet-tdt-0.6b-v3-int8");
        assert!(models[0].is_local);
    }

    #[test]
    fn model_loaded_false_for_nonexistent_path() {
        assert!(!test_provider().1.model_loaded());
    }

    #[tokio::test]
    async fn health_check_not_ready_names_expected_path() {
        let (dir, provider) = test_provider();
        let health = provider.health_check().await.expect("health_check");
        let expected = dir.path().join("parakeet-tdt-0.6b-v3-int8");
        assert!(!health.ready);
        assert!(health.message.contains("not downloaded"));
        assert!(
            health.message.contains(&expected.display().to_string()),
            "health message should name the expected path, got: {}",
            health.message
        );
    }

    #[test]
    fn unloadable_model_dir_constructs_without_model() {
        let (_dir, provider) = unloadable_provider();
        assert!(!provider.model_loaded());
        assert!(provider.load_error.is_some());
    }

    #[tokio::test]
    async fn health_check_reports_load_failure_with_path() {
        let (dir, provider) = unloadable_provider();
        let health = provider.health_check().await.expect("health_check");
        let expected = dir.path().join("parakeet-tdt-0.6b-v3-int8");
        assert!(!health.ready);
        assert!(
            health.message.contains("failed to load"),
            "health should report the load failure, got: {}",
            health.message
        );
        assert!(
            health.message.contains("re-download"),
            "health should tell the user to re-download, got: {}",
            health.message
        );
        assert!(
            health.message.contains(&expected.display().to_string()),
            "health should name the model path, got: {}",
            health.message
        );
    }

    #[tokio::test]
    async fn transcribe_with_unloadable_model_returns_not_loaded_with_cause() {
        let (_dir, provider) = unloadable_provider();
        let err = provider
            .transcribe(&[0.0_f32; 100], 16_000)
            .await
            .expect_err("should be Err");
        let msg = err.to_string();
        assert!(
            msg.contains("Model not loaded: "),
            "expected 'Model not loaded' with a cause, got: {msg}"
        );
    }

    #[tokio::test]
    async fn transcribe_without_model_returns_not_loaded_error() {
        let err = test_provider()
            .1
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
            .1
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
            .1
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
        let provider = ParakeetProvider::new(parent);
        let load_time = load_start.elapsed();
        assert!(
            provider.model_loaded(),
            "model should load from {model_dir:?}: {:?}",
            provider.load_error
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
