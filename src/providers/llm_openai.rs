//! `OpenAI` LLM provider.
//!
//! Sends transcripts to the `OpenAI` Chat Completions API (or any
//! compatible endpoint) and returns the formatted text.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;

use crate::error::{Error, Result};

use super::llm::{FormattingResult, LlmProvider};
use super::stt::{ModelInfo, ProviderHealth};

// ── Response deserialization structs ──────────────────────────────────

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatUsage {
    total_tokens: u64,
}

/// Default base URL for the `OpenAI` API.
const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// `OpenAI` provider for cloud-based text formatting.
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    timeout_ms: u64,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// Create a new `OpenAI` LLM provider.
    ///
    /// If `base_url` is empty, defaults to `https://api.openai.com`.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is empty.
    pub fn new(api_key: &str, model: &str, timeout_ms: u64, base_url: &str) -> Result<Self> {
        if api_key.is_empty() {
            return Err(Error::Provider(
                "OpenAI provider requires an API key (set OPENAI_API_KEY or configure in config.toml)".to_string(),
            ));
        }

        let effective_base_url = if base_url.is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            base_url.to_string()
        };

        Ok(Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            timeout_ms,
            base_url: effective_base_url,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn format(&self, system_prompt: &str, transcript: &str) -> Result<FormattingResult> {
        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": transcript }
            ],
            "max_tokens": 1024,
            "temperature": 0.3
        });

        let start = Instant::now();

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .timeout(Duration::from_millis(self.timeout_ms))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Formatting(format!("OpenAI request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unable to read body>".to_string());
            return Err(Error::Formatting(format!(
                "OpenAI API returned {status}: {body_text}"
            )));
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| Error::Formatting(format!("Failed to parse OpenAI response: {e}")))?;

        #[allow(clippy::cast_possible_truncation)] // duration in ms will never exceed u64
        let duration_ms = start.elapsed().as_millis() as u64;

        let text = completion
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| {
                Error::Formatting("OpenAI response contained no choices or empty content".into())
            })?;

        let tokens_used = completion.usage.map(|u| u.total_tokens);

        Ok(FormattingResult {
            text,
            duration_ms,
            tokens_used,
            cost_estimate: None,
        })
    }

    fn display_name(&self) -> &'static str {
        "OpenAI"
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
                message: "API key does not appear to be a valid OpenAI key".to_string(),
            })
        }
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gpt-4o-mini".into(),
                display_name: "GPT-4o Mini".into(),
                description: "Fast, cost-effective formatting".into(),
                is_local: false,
                size_bytes: None,
            },
            ModelInfo {
                id: "gpt-4o".into(),
                display_name: "GPT-4o".into(),
                description: "High quality formatting".into(),
                is_local: false,
                size_bytes: None,
            },
            ModelInfo {
                id: "gpt-4.1-mini".into(),
                display_name: "GPT-4.1 Mini".into(),
                description: "Next-gen fast formatting".into(),
                is_local: false,
                size_bytes: None,
            },
            ModelInfo {
                id: "gpt-4.1".into(),
                display_name: "GPT-4.1".into(),
                description: "Next-gen high quality formatting".into(),
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
        let result = OpenAiProvider::new("", "gpt-4o-mini", 3000, "");
        assert!(result.is_err());
    }

    #[test]
    fn new_accepts_valid_api_key() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "").expect("create provider");
        assert_eq!(provider.display_name(), "OpenAI");
    }

    #[test]
    fn default_base_url_applied() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "").expect("create provider");
        assert_eq!(provider.base_url, "https://api.openai.com");
    }

    #[test]
    fn custom_base_url_preserved() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "https://custom.api.com")
                .expect("create provider");
        assert_eq!(provider.base_url, "https://custom.api.com");
    }

    #[test]
    fn is_not_local() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "").expect("create provider");
        assert!(!provider.is_local());
    }

    #[test]
    fn requires_api_key_returns_true() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "").expect("create provider");
        assert!(provider.requires_api_key());
    }

    #[test]
    fn available_models_lists_four_models() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "").expect("create provider");
        let models = provider.available_models();
        assert_eq!(models.len(), 4);
        assert_eq!(models[0].id, "gpt-4o-mini");
        assert_eq!(models[1].id, "gpt-4o");
        assert_eq!(models[2].id, "gpt-4.1-mini");
        assert_eq!(models[3].id, "gpt-4.1");
    }

    #[tokio::test]
    async fn health_check_valid_key() {
        let provider =
            OpenAiProvider::new("sk-test", "gpt-4o-mini", 3000, "").expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(health.ready);
    }

    #[tokio::test]
    async fn health_check_invalid_key_format() {
        let provider =
            OpenAiProvider::new("bad-key", "gpt-4o-mini", 3000, "").expect("create provider");
        let health = provider.health_check().await.expect("health_check");
        assert!(!health.ready);
    }

    #[tokio::test]
    async fn format_with_fake_key_returns_network_error() {
        // Use a non-routable address so the request fails fast with a network error
        let provider = OpenAiProvider::new("sk-test", "gpt-4o-mini", 500, "http://192.0.2.1:1")
            .expect("create provider");
        let result = provider.format("system", "hello world").await;
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                // Should be a formatting error wrapping a network problem, not "not implemented"
                assert!(
                    !err_msg.contains("not yet implemented"),
                    "expected a network error, got: {err_msg}"
                );
            }
            Ok(_) => panic!("expected an error from a non-routable address"),
        }
    }
}
