//! `OpenAI` Whisper API STT provider.

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;

use crate::audio::capture::AudioBuffer;
use crate::error::{Error, Result};

use super::stt::{ModelInfo, ProviderHealth, SttProvider, TranscriptionResult};

/// JSON response from the `OpenAI` Whisper transcription endpoint.
#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
}

/// `OpenAI` Whisper API provider for cloud-based transcription.
pub struct OpenAiWhisperProvider {
    api_key: String,
    model: String,
    language: String,
    client: reqwest::Client,
}

impl OpenAiWhisperProvider {
    /// Create a new `OpenAI` Whisper API provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is empty.
    pub fn new(api_key: &str, model: &str, language: &str) -> Result<Self> {
        if api_key.is_empty() {
            return Err(Error::Provider(
                "OpenAI Whisper requires an API key (set OPENAI_API_KEY or configure in config.toml)".to_string(),
            ));
        }

        Ok(Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            language: language.to_string(),
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl SttProvider for OpenAiWhisperProvider {
    async fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<TranscriptionResult> {
        let start = Instant::now();

        // Encode audio samples to WAV bytes in memory.
        let audio_buffer = AudioBuffer {
            samples: audio.to_vec(),
            sample_rate,
            duration_ms: (audio.len() as u64 * 1000) / u64::from(sample_rate),
        };
        let wav_bytes = audio_buffer.to_wav_bytes()?;

        // Build multipart form.
        let file_part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| Error::Transcription(format!("failed to set MIME type: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("language", self.language.clone())
            .text("response_format", "json");

        // Send request to OpenAI.
        let response = self
            .client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::Transcription(format!("request failed: {e}")))?;

        // Check for non-success status codes.
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read body>".to_string());
            return Err(Error::Transcription(format!(
                "OpenAI API returned {status}: {body}"
            )));
        }

        // Parse the JSON response.
        let whisper_response: WhisperResponse = response
            .json()
            .await
            .map_err(|e| Error::Transcription(format!("failed to parse response: {e}")))?;

        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;
        let audio_duration_ms = (audio.len() as u64) * 1000 / u64::from(sample_rate);

        Ok(TranscriptionResult {
            text: whisper_response.text,
            language_detected: None,
            duration_ms,
            audio_duration_ms,
        })
    }

    fn display_name(&self) -> &'static str {
        "OpenAI Whisper"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Validate API key format (OpenAI keys start with "sk-" or "proj-")
        if self.api_key.starts_with("sk-") || self.api_key.starts_with("proj-") {
            Ok(ProviderHealth {
                ready: true,
                message: format!("API key configured, model: {}", self.model),
            })
        } else {
            Ok(ProviderHealth {
                ready: false,
                message:
                    "API key does not appear to be a valid OpenAI key (expected sk-... or proj-...)"
                        .to_string(),
            })
        }
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "whisper-1".into(),
            display_name: "Whisper v1".into(),
            description: "OpenAI cloud-hosted Whisper model".into(),
            is_local: false,
            size_bytes: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_api_key() {
        let result = OpenAiWhisperProvider::new("", "whisper-1", "en");
        assert!(result.is_err());
    }

    #[test]
    fn new_accepts_valid_api_key() {
        let provider =
            OpenAiWhisperProvider::new("sk-test-key", "whisper-1", "en").expect("create provider");
        assert_eq!(provider.display_name(), "OpenAI Whisper");
    }

    #[test]
    fn is_not_local() {
        let provider =
            OpenAiWhisperProvider::new("sk-test", "whisper-1", "en").expect("create provider");
        assert!(!provider.is_local());
    }

    #[test]
    fn requires_api_key_returns_true() {
        let provider =
            OpenAiWhisperProvider::new("sk-test", "whisper-1", "en").expect("create provider");
        assert!(provider.requires_api_key());
    }

    #[test]
    fn available_models_lists_whisper_1() {
        let provider =
            OpenAiWhisperProvider::new("sk-test", "whisper-1", "en").expect("create provider");
        let models = provider.available_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "whisper-1");
        assert!(!models[0].is_local);
    }

    #[tokio::test]
    async fn health_check_valid_key() {
        let provider =
            OpenAiWhisperProvider::new("sk-test-key", "whisper-1", "en").expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(health.ready);
    }

    #[tokio::test]
    async fn health_check_invalid_key_format() {
        let provider = OpenAiWhisperProvider::new("bad-key-format", "whisper-1", "en")
            .expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(!health.ready);
        assert!(health.message.contains("sk-"));
    }

    #[tokio::test]
    async fn transcribe_with_fake_key_returns_api_error() {
        let provider =
            OpenAiWhisperProvider::new("sk-test", "whisper-1", "en").expect("create provider");
        let result = provider.transcribe(&[0.0_f32; 100], 16000).await;
        match result {
            Ok(_) => panic!("expected an error from fake API key"),
            Err(e) => {
                let err_msg = e.to_string();
                // Should be a transcription/network error, not "not yet implemented".
                assert!(
                    !err_msg.contains("not yet implemented"),
                    "expected a real API/network error, got: {err_msg}"
                );
            }
        }
    }
}
