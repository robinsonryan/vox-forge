//! Speech-to-text provider trait and associated types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Identifies which STT backend to use.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderType {
    WhisperLocal,
    OpenaiWhisper,
    CohereTranscribe,
    Voxtral,
    Parakeet,
}

/// Compute device for local inference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComputeDevice {
    Cuda,
    Cpu,
}

/// Result of a transcription operation.
#[derive(Debug)]
pub struct TranscriptionResult {
    /// The transcribed text.
    pub text: String,
    /// Language detected by the model, if available.
    #[allow(dead_code)]
    pub language_detected: Option<String>,
    /// Wall-clock time spent on transcription, in milliseconds.
    pub duration_ms: u64,
    /// Duration of the source audio, in milliseconds.
    #[allow(dead_code)]
    pub audio_duration_ms: u64,
}

/// Metadata about an available model.
#[allow(dead_code)]
pub struct ModelInfo {
    /// Machine-readable identifier (e.g. `"base"`, `"whisper-1"`).
    pub id: String,
    /// Human-friendly name shown in the UI.
    pub display_name: String,
    /// Short description of capabilities / trade-offs.
    pub description: String,
    /// Whether the model runs locally (no network required).
    pub is_local: bool,
    /// Download size in bytes, if known.
    pub size_bytes: Option<u64>,
}

/// Health status of a provider.
#[allow(dead_code)]
pub struct ProviderHealth {
    /// `true` when the provider is ready to accept requests.
    pub ready: bool,
    /// Human-readable status message.
    pub message: String,
}

/// Duration in milliseconds of `sample_count` mono samples at `sample_rate` Hz.
///
/// Integer arithmetic (rounds down) avoids float-cast lints. A zero sample
/// rate yields 0 rather than dividing by zero.
pub fn samples_to_ms(sample_count: usize, sample_rate: u32) -> u64 {
    (sample_count as u64 * 1000)
        .checked_div(u64::from(sample_rate))
        .unwrap_or(0)
}

/// Trait that every speech-to-text backend must implement.
#[allow(dead_code)]
#[async_trait]
pub trait SttProvider: Send + Sync {
    /// Transcribe raw audio samples at the given sample rate.
    async fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<TranscriptionResult>;

    /// Human-friendly name of this provider.
    fn display_name(&self) -> &'static str;

    /// Whether this provider runs entirely on the local machine.
    fn is_local(&self) -> bool;

    /// Whether this provider requires an API key to function.
    fn requires_api_key(&self) -> bool;

    /// Check whether the provider is ready to serve requests.
    async fn health_check(&self) -> Result<ProviderHealth>;

    /// List the models this provider can use.
    fn available_models(&self) -> Vec<ModelInfo>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stt_provider_type_serde_roundtrip() {
        let local = SttProviderType::WhisperLocal;
        let json = serde_json::to_string(&local).expect("serialize");
        assert_eq!(json, "\"whisper_local\"");
        let back: SttProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, SttProviderType::WhisperLocal);

        let api = SttProviderType::OpenaiWhisper;
        let json = serde_json::to_string(&api).expect("serialize");
        assert_eq!(json, "\"openai_whisper\"");

        let cohere = SttProviderType::CohereTranscribe;
        let json = serde_json::to_string(&cohere).expect("serialize");
        assert_eq!(json, "\"cohere_transcribe\"");
        let back: SttProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, SttProviderType::CohereTranscribe);

        let voxtral = SttProviderType::Voxtral;
        let json = serde_json::to_string(&voxtral).expect("serialize");
        assert_eq!(json, "\"voxtral\"");
        let back: SttProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, SttProviderType::Voxtral);

        let parakeet = SttProviderType::Parakeet;
        let json = serde_json::to_string(&parakeet).expect("serialize");
        assert_eq!(json, "\"parakeet\"");
        let back: SttProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, SttProviderType::Parakeet);
    }

    #[test]
    fn samples_to_ms_converts_and_rounds_down() {
        assert_eq!(samples_to_ms(16_000, 16_000), 1000);
        assert_eq!(samples_to_ms(8_000, 16_000), 500);
        assert_eq!(samples_to_ms(15, 16_000), 0);
        assert_eq!(samples_to_ms(44_100 * 3, 44_100), 3000);
        assert_eq!(samples_to_ms(0, 16_000), 0);
        assert_eq!(samples_to_ms(16_000, 0), 0);
    }

    #[test]
    fn compute_device_serde_roundtrip() {
        let cuda = ComputeDevice::Cuda;
        let json = serde_json::to_string(&cuda).expect("serialize");
        assert_eq!(json, "\"cuda\"");

        let cpu = ComputeDevice::Cpu;
        let json = serde_json::to_string(&cpu).expect("serialize");
        assert_eq!(json, "\"cpu\"");
    }
}
