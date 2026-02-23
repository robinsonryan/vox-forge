//! Provider registry — factory functions for creating providers from config.
//!
//! This module is the single place where config values are mapped to concrete
//! provider types. Business logic elsewhere only sees trait objects.

use std::path::PathBuf;

use crate::config::Config;
use crate::error::{Error, Result};

use super::llm::LlmProvider;
use super::llm_anthropic::AnthropicProvider;
use super::llm_openai::OpenAiProvider;
use super::stt::SttProvider;
use super::stt_openai_whisper::OpenAiWhisperProvider;
use super::stt_whisper_local::WhisperLocalProvider;

use super::stt::ComputeDevice;

/// Create an STT provider based on the current configuration.
///
/// # Errors
///
/// Returns `Error::Provider` if the configured provider name is unknown,
/// or if the concrete provider's constructor fails (e.g. missing API key).
pub fn create_stt_provider(config: &Config, models_dir: PathBuf) -> Result<Box<dyn SttProvider>> {
    match config.transcription.provider.as_str() {
        "whisper_local" => {
            let wl = &config.transcription.whisper_local;
            let device = match wl.device.as_str() {
                "cuda" => ComputeDevice::Cuda,
                "cpu" => ComputeDevice::Cpu,
                other => {
                    return Err(Error::Provider(format!(
                        "Unknown compute device: '{other}' (expected 'cuda' or 'cpu')"
                    )));
                }
            };
            let provider = WhisperLocalProvider::new(&wl.model, device, &wl.language, models_dir)?;
            Ok(Box::new(provider))
        }
        "openai_whisper" => {
            let ow = &config.transcription.openai_whisper;
            let api_key = config.effective_openai_key();
            let provider = OpenAiWhisperProvider::new(&api_key, &ow.model, &ow.language)?;
            Ok(Box::new(provider))
        }
        other => Err(Error::Provider(format!(
            "Unknown STT provider: '{other}' (expected 'whisper_local' or 'openai_whisper')"
        ))),
    }
}

/// Create an LLM provider based on the current configuration.
///
/// # Errors
///
/// Returns `Error::Provider` if the configured provider name is unknown,
/// or if the concrete provider's constructor fails (e.g. missing API key).
pub fn create_llm_provider(config: &Config) -> Result<Box<dyn LlmProvider>> {
    match config.formatting.provider.as_str() {
        "anthropic" => {
            let api_key = config.effective_anthropic_key();
            let model = &config.formatting.anthropic.model;
            let timeout_ms = config.formatting.timeout_ms;
            let provider = AnthropicProvider::new(&api_key, model, timeout_ms)?;
            Ok(Box::new(provider))
        }
        "openai" => {
            let api_key = config.effective_openai_key();
            let oai = &config.formatting.openai;
            let provider = OpenAiProvider::new(
                &api_key,
                &oai.model,
                config.formatting.timeout_ms,
                &oai.base_url,
            )?;
            Ok(Box::new(provider))
        }
        other => Err(Error::Provider(format!(
            "Unknown LLM provider: '{other}' (expected 'anthropic' or 'openai')"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_stt_whisper_local_default_config() {
        let config = Config::default();
        let models_dir = PathBuf::from("/tmp/voxforge-test-models");
        let provider = create_stt_provider(&config, models_dir);
        assert!(provider.is_ok());
        let provider = provider.expect("provider created");
        assert_eq!(provider.display_name(), "Local Whisper");
        assert!(provider.is_local());
    }

    #[test]
    fn create_stt_openai_whisper_requires_key() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
        let mut config = Config::default();
        config.transcription.provider = "openai_whisper".to_string();
        let models_dir = PathBuf::from("/tmp/voxforge-test-models");
        let result = create_stt_provider(&config, models_dir);
        assert!(result.is_err());
    }

    #[test]
    fn create_stt_openai_whisper_with_key() {
        let mut config = Config::default();
        config.transcription.provider = "openai_whisper".to_string();
        config.formatting.openai.api_key = "sk-test-key".to_string();
        let models_dir = PathBuf::from("/tmp/voxforge-test-models");
        let result = create_stt_provider(&config, models_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn create_stt_unknown_provider_errors() {
        let mut config = Config::default();
        config.transcription.provider = "unknown_provider".to_string();
        let models_dir = PathBuf::from("/tmp/voxforge-test-models");
        let result = create_stt_provider(&config, models_dir);
        assert!(result.is_err());
    }

    #[test]
    fn create_llm_anthropic_requires_key() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let config = Config::default();
        let result = create_llm_provider(&config);
        assert!(result.is_err());
    }

    #[test]
    fn create_llm_anthropic_with_key() {
        let mut config = Config::default();
        config.formatting.anthropic.api_key = "sk-ant-test-key".to_string();
        let result = create_llm_provider(&config);
        assert!(result.is_ok());
        let provider = result.expect("provider created");
        assert_eq!(provider.display_name(), "Anthropic Claude");
    }

    #[test]
    fn create_llm_openai_requires_key() {
        // SAFETY: Test-only env var manipulation.
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }
        let mut config = Config::default();
        config.formatting.provider = "openai".to_string();
        let result = create_llm_provider(&config);
        assert!(result.is_err());
    }

    #[test]
    fn create_llm_openai_with_key() {
        let mut config = Config::default();
        config.formatting.provider = "openai".to_string();
        config.formatting.openai.api_key = "sk-test-key".to_string();
        let result = create_llm_provider(&config);
        assert!(result.is_ok());
        let provider = result.expect("provider created");
        assert_eq!(provider.display_name(), "OpenAI");
    }

    #[test]
    fn create_llm_unknown_provider_errors() {
        let mut config = Config::default();
        config.formatting.provider = "unknown".to_string();
        let result = create_llm_provider(&config);
        assert!(result.is_err());
    }

    #[test]
    fn create_stt_invalid_device_errors() {
        let mut config = Config::default();
        config.transcription.whisper_local.device = "tpu".to_string();
        let models_dir = PathBuf::from("/tmp/voxforge-test-models");
        let result = create_stt_provider(&config, models_dir);
        assert!(result.is_err());
    }
}
