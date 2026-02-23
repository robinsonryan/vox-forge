//! Anthropic Claude LLM provider.
//!
//! Calls the Anthropic Messages API to format transcripts.

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{Error, Result};

use super::llm::{FormattingResult, LlmProvider};
use super::stt::{ModelInfo, ProviderHealth};

// ── Anthropic API response types ──────────────────────────────────────────

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
}

/// Anthropic Claude provider for cloud-based text formatting.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    timeout_ms: u64,
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic Claude provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is empty.
    pub fn new(api_key: &str, model: &str, timeout_ms: u64) -> Result<Self> {
        if api_key.is_empty() {
            return Err(Error::Provider(
                "Anthropic provider requires an API key (set ANTHROPIC_API_KEY or configure in config.toml)".to_string(),
            ));
        }

        Ok(Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            timeout_ms,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn format(&self, system_prompt: &str, transcript: &str) -> Result<FormattingResult> {
        let body = json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": system_prompt,
            "messages": [
                { "role": "user", "content": transcript }
            ]
        });

        let start = Instant::now();

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Formatting(format!("Anthropic API request failed: {e}")))?;

        #[allow(clippy::cast_possible_truncation)]
        let duration_ms = start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(Error::Formatting(format!(
                "Anthropic API returned {status}: {error_body}"
            )));
        }

        let api_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| Error::Formatting(format!("Failed to parse Anthropic response: {e}")))?;

        let text = api_response
            .content
            .iter()
            .find(|b| b.block_type == "text")
            .and_then(|b| b.text.clone())
            .ok_or_else(|| {
                Error::Formatting("Anthropic response contained no text content block".to_string())
            })?;

        let tokens_used = api_response.usage.map(|u| u.input_tokens + u.output_tokens);

        Ok(FormattingResult {
            text,
            duration_ms,
            tokens_used,
            cost_estimate: None,
        })
    }

    fn display_name(&self) -> &'static str {
        "Anthropic Claude"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // Validate API key format (Anthropic keys start with "sk-ant-")
        if self.api_key.starts_with("sk-ant-") {
            Ok(ProviderHealth {
                ready: true,
                message: format!("API key configured, model: {}", self.model),
            })
        } else {
            Ok(ProviderHealth {
                ready: false,
                message: "API key does not appear to be a valid Anthropic key (expected 'sk-ant-' prefix)".to_string(),
            })
        }
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-haiku-4-5-20251001".into(),
                display_name: "Claude Haiku 4.5".into(),
                description: "Fast, cost-effective formatting".into(),
                is_local: false,
                size_bytes: None,
            },
            ModelInfo {
                id: "claude-sonnet-4-5-20250929".into(),
                display_name: "Claude Sonnet 4.5".into(),
                description: "Higher quality formatting, slower".into(),
                is_local: false,
                size_bytes: None,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_api_key() {
        let result = AnthropicProvider::new("", "claude-haiku-4-5-20251001", 3000);
        assert!(result.is_err());
    }

    #[test]
    fn new_accepts_valid_api_key() {
        let provider = AnthropicProvider::new("sk-ant-test", "claude-haiku-4-5-20251001", 3000)
            .expect("create provider");
        assert_eq!(provider.display_name(), "Anthropic Claude");
    }

    #[test]
    fn is_not_local() {
        let provider = AnthropicProvider::new("sk-ant-test", "claude-haiku-4-5-20251001", 3000)
            .expect("create provider");
        assert!(!provider.is_local());
    }

    #[test]
    fn requires_api_key_returns_true() {
        let provider = AnthropicProvider::new("sk-ant-test", "claude-haiku-4-5-20251001", 3000)
            .expect("create provider");
        assert!(provider.requires_api_key());
    }

    #[test]
    fn available_models_lists_two_models() {
        let provider = AnthropicProvider::new("sk-ant-test", "claude-haiku-4-5-20251001", 3000)
            .expect("create provider");
        let models = provider.available_models();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "claude-haiku-4-5-20251001");
        assert_eq!(models[1].id, "claude-sonnet-4-5-20250929");
    }

    #[tokio::test]
    async fn health_check_valid_key() {
        let provider = AnthropicProvider::new("sk-ant-test", "claude-haiku-4-5-20251001", 3000)
            .expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(health.ready);
    }

    #[tokio::test]
    async fn health_check_invalid_key_format() {
        let provider = AnthropicProvider::new("bad-key", "claude-haiku-4-5-20251001", 3000)
            .expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(!health.ready);
        assert!(health.message.contains("sk-ant-"));
    }

    #[tokio::test]
    async fn format_with_fake_key_returns_api_error() {
        let provider = AnthropicProvider::new("sk-ant-fake", "claude-haiku-4-5-20251001", 5000)
            .expect("create provider");
        let result = provider.format("You are a formatter.", "hello world").await;
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                // Should be a formatting error from the API (auth failure or network),
                // not "not implemented"
                assert!(
                    err_msg.contains("Anthropic API"),
                    "Expected Anthropic API error, got: {err_msg}"
                );
            }
            Ok(_) => panic!("Expected an error from format() with a fake API key"),
        }
    }
}
