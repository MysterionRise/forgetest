//! Anthropic API provider implementation.

use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use forgetest_core::results::TokenUsage;
use forgetest_core::traits::{
    extract_code_from_markdown, GenerateRequest, GenerateResponse, LlmProvider, ModelInfo,
};

use crate::error::ProviderError;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const SYSTEM_PROMPT: &str = "You are a code generation assistant. Respond ONLY with code. Do not include explanations, comments about the code, or markdown formatting unless the code itself requires comments. Output valid, compilable code.";

/// Anthropic API provider.
pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, base_url: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("failed to build HTTP client");

        Self {
            api_key: api_key.to_string(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            client,
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    #[serde(default)]
    usage: AnthropicUsage,
    model: String,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Deserialize, Default)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Deserialize)]
struct AnthropicError {
    error: AnthropicErrorBody,
}

#[derive(Deserialize)]
struct AnthropicErrorBody {
    message: String,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn generate(&self, request: &GenerateRequest) -> anyhow::Result<GenerateResponse> {
        let start = Instant::now();

        let system_prompt = request
            .system_prompt
            .clone()
            .unwrap_or_else(|| SYSTEM_PROMPT.to_string());

        // Build context into the prompt
        let mut full_prompt = String::new();
        for file in &request.context_files {
            full_prompt.push_str(&format!(
                "File `{}`:\n```\n{}\n```\n\n",
                file.path, file.content
            ));
        }
        full_prompt.push_str(&request.prompt);

        let body = AnthropicRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            system: Some(system_prompt),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: full_prompt,
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(DEFAULT_TIMEOUT_SECS)
                } else {
                    ProviderError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status().as_u16();
        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5)
                * 1000;
            return Err(ProviderError::RateLimited {
                retry_after_ms: retry_after,
            }
            .into());
        }
        if status == 401 {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::AuthenticationFailed(body).into());
        }
        if status == 404 {
            return Err(ProviderError::ModelNotFound(request.model.clone()).into());
        }
        if status >= 400 {
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<AnthropicError>(&body)
                .map(|e| e.error.message)
                .unwrap_or(body);
            return Err(ProviderError::ApiError { status, message }.into());
        }

        let api_response: AnthropicResponse =
            response.json().await.map_err(|e| ProviderError::ApiError {
                status: 0,
                message: format!("failed to parse response: {e}"),
            })?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let content = api_response
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();
        let extracted_code = extract_code_from_markdown(&content);

        let total_tokens = api_response.usage.input_tokens + api_response.usage.output_tokens;
        // Pricing: Claude Sonnet $3/$15 per 1M tokens
        let estimated_cost = (api_response.usage.input_tokens as f64 * 3.0
            + api_response.usage.output_tokens as f64 * 15.0)
            / 1_000_000.0;

        Ok(GenerateResponse {
            content,
            extracted_code,
            model: api_response.model,
            token_usage: TokenUsage {
                prompt_tokens: api_response.usage.input_tokens,
                completion_tokens: api_response.usage.output_tokens,
                total_tokens,
                estimated_cost_usd: estimated_cost,
            },
            latency_ms,
        })
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".into(),
                name: "Claude Sonnet 4".into(),
                provider: "anthropic".into(),
                max_context: 200_000,
                cost_per_1k_input: 0.003,
                cost_per_1k_output: 0.015,
            },
            ModelInfo {
                id: "claude-haiku-4-5-20251001".into(),
                name: "Claude Haiku 4.5".into(),
                provider: "anthropic".into(),
                max_context: 200_000,
                cost_per_1k_input: 0.0008,
                cost_per_1k_output: 0.004,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn successful_generation() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "content": [{"type": "text", "text": "fn add(a: i32, b: i32) -> i32 { a + b }"}],
            "model": "claude-sonnet-4-20250514",
            "usage": {"input_tokens": 50, "output_tokens": 20}
        });

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let provider = AnthropicProvider::new("test-key", Some(server.uri()));
        let request = GenerateRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt: "Write an add function".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 1024,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(response.content.contains("fn add"));
        assert_eq!(response.token_usage.prompt_tokens, 50);
        assert_eq!(response.token_usage.completion_tokens, 20);
    }

    #[tokio::test]
    async fn authentication_failure() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;

        let provider = AnthropicProvider::new("bad-key", Some(server.uri()));
        let request = GenerateRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt: "test".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 1024,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let err = provider.generate(&request).await.unwrap_err();
        assert!(err.to_string().contains("authentication"));
    }

    #[tokio::test]
    async fn rate_limiting() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "5"))
            .mount(&server)
            .await;

        let provider = AnthropicProvider::new("test-key", Some(server.uri()));
        let request = GenerateRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt: "test".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 1024,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let err = provider.generate(&request).await.unwrap_err();
        assert!(err.to_string().contains("rate limited"));
    }
}
