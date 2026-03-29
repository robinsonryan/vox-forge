//! vLLM-served STT provider for Cohere Transcribe and Voxtral Mini.
//!
//! Both models expose an OpenAI-compatible `/v1/audio/transcriptions`
//! endpoint when served via vLLM. This single implementation handles
//! both by parameterising the endpoint URL and model name.

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;

use crate::audio::capture::AudioBuffer;
use crate::error::{Error, Result};

use super::stt::{ModelInfo, ProviderHealth, SttProvider, TranscriptionResult};

/// JSON response from the vLLM transcription endpoint (OpenAI-compatible).
#[derive(Deserialize)]
struct VllmTranscribeResponse {
    text: String,
}

/// Which vLLM-served model this provider represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VllmSttModel {
    CohereTranscribe,
    Voxtral,
}

/// vLLM-based STT provider for locally-served transcription models.
pub struct VllmTranscribeProvider {
    endpoint: String,
    model_name: String,
    variant: VllmSttModel,
    client: reqwest::Client,
}

impl VllmTranscribeProvider {
    /// Create a new vLLM transcription provider.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Base URL of the vLLM server (e.g. `http://localhost:8000`)
    /// * `model_name` - Model identifier sent in the request
    /// * `variant` - Which model this provider represents (for display purposes)
    ///
    /// # Errors
    ///
    /// Returns an error if the endpoint is empty.
    pub fn new(endpoint: &str, model_name: &str, variant: VllmSttModel) -> Result<Self> {
        if endpoint.is_empty() {
            return Err(Error::Provider(format!(
                "{} requires an endpoint URL (e.g. http://localhost:8000)",
                match variant {
                    VllmSttModel::CohereTranscribe => "Cohere Transcribe",
                    VllmSttModel::Voxtral => "Voxtral",
                }
            )));
        }

        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            model_name: model_name.to_string(),
            variant,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl SttProvider for VllmTranscribeProvider {
    async fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<TranscriptionResult> {
        let start = Instant::now();

        // Encode audio samples to WAV bytes in memory.
        let audio_buffer = AudioBuffer {
            samples: audio.to_vec(),
            sample_rate,
            duration_ms: (audio.len() as u64 * 1000) / u64::from(sample_rate),
        };
        let wav_bytes = audio_buffer.to_wav_bytes()?;

        // Build multipart form (OpenAI-compatible).
        let file_part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| Error::Transcription(format!("failed to set MIME type: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model_name.clone())
            .text("language", "en".to_string())
            .text("response_format", "json")
            .text("temperature", "0.0");

        let url = format!("{}/v1/audio/transcriptions", self.endpoint);

        let response = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                Error::Transcription(format!(
                    "{} request to {} failed: {e}",
                    self.display_name(),
                    url
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read body>".to_string());
            return Err(Error::Transcription(format!(
                "{} returned {status}: {body}",
                self.display_name()
            )));
        }

        let vllm_response: VllmTranscribeResponse = response
            .json()
            .await
            .map_err(|e| Error::Transcription(format!("failed to parse response: {e}")))?;

        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;
        let audio_duration_ms = (audio.len() as u64) * 1000 / u64::from(sample_rate);

        Ok(TranscriptionResult {
            text: vllm_response.text,
            language_detected: None,
            duration_ms,
            audio_duration_ms,
        })
    }

    fn display_name(&self) -> &'static str {
        match self.variant {
            VllmSttModel::CohereTranscribe => "Cohere Transcribe",
            VllmSttModel::Voxtral => "Voxtral",
        }
    }

    fn is_local(&self) -> bool {
        true
    }

    fn requires_api_key(&self) -> bool {
        false
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Ping the vLLM /v1/models endpoint to check if the server is up.
        let url = format!("{}/v1/models", self.endpoint);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(ProviderHealth {
                ready: true,
                message: format!("{} is running at {}", self.display_name(), self.endpoint),
            }),
            Ok(resp) => Ok(ProviderHealth {
                ready: false,
                message: format!(
                    "{} returned status {} at {}",
                    self.display_name(),
                    resp.status(),
                    self.endpoint
                ),
            }),
            Err(e) => Ok(ProviderHealth {
                ready: false,
                message: format!(
                    "{} is not reachable at {}: {e}",
                    self.display_name(),
                    self.endpoint
                ),
            }),
        }
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        match self.variant {
            VllmSttModel::CohereTranscribe => vec![ModelInfo {
                id: "CohereLabs/cohere-transcribe-03-2026".into(),
                display_name: "Cohere Transcribe".into(),
                description: "2B param ASR model, #1 on Open ASR Leaderboard (5.42% WER)".into(),
                is_local: true,
                size_bytes: Some(4_100_000_000),
            }],
            VllmSttModel::Voxtral => vec![ModelInfo {
                id: "mistralai/Voxtral-Mini-3B-2507".into(),
                display_name: "Voxtral Mini 3B".into(),
                description: "3B param ASR model from Mistral, supports 8 languages".into(),
                is_local: true,
                size_bytes: Some(6_000_000_000),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_endpoint() {
        let result = VllmTranscribeProvider::new("", "model", VllmSttModel::CohereTranscribe);
        assert!(result.is_err());
    }

    #[test]
    fn new_accepts_valid_endpoint() {
        let provider = VllmTranscribeProvider::new(
            "http://localhost:8000",
            "CohereLabs/cohere-transcribe-03-2026",
            VllmSttModel::CohereTranscribe,
        )
        .expect("create provider");
        assert_eq!(provider.display_name(), "Cohere Transcribe");
        assert!(provider.is_local());
        assert!(!provider.requires_api_key());
    }

    #[test]
    fn voxtral_variant_display_name() {
        let provider = VllmTranscribeProvider::new(
            "http://localhost:8000",
            "mistralai/Voxtral-Mini-3B-2507",
            VllmSttModel::Voxtral,
        )
        .expect("create provider");
        assert_eq!(provider.display_name(), "Voxtral");
    }

    #[test]
    fn trailing_slash_stripped() {
        let provider = VllmTranscribeProvider::new(
            "http://localhost:8000/",
            "model",
            VllmSttModel::CohereTranscribe,
        )
        .expect("create provider");
        assert_eq!(provider.endpoint, "http://localhost:8000");
    }

    #[test]
    fn available_models_cohere() {
        let provider = VllmTranscribeProvider::new(
            "http://localhost:8000",
            "model",
            VllmSttModel::CohereTranscribe,
        )
        .expect("create provider");
        let models = provider.available_models();
        assert_eq!(models.len(), 1);
        assert!(models[0].is_local);
        assert!(models[0].id.contains("cohere"));
    }

    #[test]
    fn available_models_voxtral() {
        let provider =
            VllmTranscribeProvider::new("http://localhost:8000", "model", VllmSttModel::Voxtral)
                .expect("create provider");
        let models = provider.available_models();
        assert_eq!(models.len(), 1);
        assert!(models[0].is_local);
        assert!(models[0].id.contains("Voxtral"));
    }

    #[tokio::test]
    async fn health_check_unreachable_endpoint() {
        let provider = VllmTranscribeProvider::new(
            "http://localhost:19999",
            "model",
            VllmSttModel::CohereTranscribe,
        )
        .expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(!health.ready);
        assert!(health.message.contains("not reachable"));
    }

    #[tokio::test]
    async fn transcribe_unreachable_returns_error() {
        let provider = VllmTranscribeProvider::new(
            "http://localhost:19999",
            "model",
            VllmSttModel::CohereTranscribe,
        )
        .expect("create provider");
        let result = provider.transcribe(&[0.0_f32; 100], 16000).await;
        assert!(result.is_err());
    }
}
