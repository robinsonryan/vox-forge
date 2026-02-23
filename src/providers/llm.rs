//! Large-language-model provider trait and associated types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::stt::{ModelInfo, ProviderHealth};

/// Identifies which LLM backend to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderType {
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
}

/// Result of a formatting / LLM operation.
#[derive(Debug)]
pub struct FormattingResult {
    /// The formatted text.
    pub text: String,
    /// Wall-clock time spent on the LLM call, in milliseconds.
    pub duration_ms: u64,
    /// Tokens consumed, if reported by the API.
    pub tokens_used: Option<u64>,
    /// Estimated cost in USD, if calculable.
    pub cost_estimate: Option<f64>,
}

/// Trait that every LLM formatting backend must implement.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a transcript through the LLM with the given system prompt
    /// and return the cleaned / formatted text.
    async fn format(&self, system_prompt: &str, transcript: &str) -> Result<FormattingResult>;

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
    fn llm_provider_type_serde_roundtrip() {
        let anthropic = LlmProviderType::Anthropic;
        let json = serde_json::to_string(&anthropic).expect("serialize");
        assert_eq!(json, "\"anthropic\"");
        let back: LlmProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, LlmProviderType::Anthropic);
    }

    #[test]
    fn llm_provider_type_openai_serde() {
        let openai = LlmProviderType::OpenAi;
        let json = serde_json::to_string(&openai).expect("serialize");
        assert_eq!(json, "\"openai\"");
        let back: LlmProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, LlmProviderType::OpenAi);
    }
}
