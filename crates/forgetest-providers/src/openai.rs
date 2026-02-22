//! OpenAI API provider implementation.

use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use forgetest_core::results::TokenUsage;
use forgetest_core::traits::{
    extract_code_from_markdown, GenerateRequest, GenerateResponse, LlmProvider, ModelInfo,
};

use crate::error::ProviderError;

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const SYSTEM_PROMPT: &str = "You are a code generation assistant. Respond ONLY with code. Do not include explanations, comments about the code, or markdown formatting unless the code itself requires comments. Output valid, compilable code.";

/// OpenAI-compatible API provider.
pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    org_id: Option<String>,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(api_key: &str, base_url: Option<String>, org_id: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("failed to build HTTP client");

        Self {
            api_key: api_key.to_string(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            org_id,
            client,
        }
    }
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    max_tokens: u32,
    temperature: f64,
    messages: Vec<OpenAiMessage>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: OpenAiUsage,
    model: String,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

#[derive(Deserialize, Default)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn generate(&self, request: &GenerateRequest) -> anyhow::Result<GenerateResponse> {
        let start = Instant::now();

        let system_prompt = request
            .system_prompt
            .clone()
            .unwrap_or_else(|| SYSTEM_PROMPT.to_string());

        let mut full_prompt = String::new();
        for file in &request.context_files {
            full_prompt.push_str(&format!("File `{}`:\n```\n{}\n```\n\n", file.path, file.content));
        }
        full_prompt.push_str(&request.prompt);

        let body = OpenAiRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: full_prompt,
                },
            ],
        };

        let mut req = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json");

        if let Some(org) = &self.org_id {
            req = req.header("OpenAI-Organization", org);
        }

        let response = req.json(&body).send().await.map_err(|e| {
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
        if status >= 400 {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: body,
            }
            .into());
        }

        let api_response: OpenAiResponse = response.json().await.map_err(|e| {
            ProviderError::ApiError {
                status: 0,
                message: format!("failed to parse response: {e}"),
            }
        })?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let content = api_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();
        let extracted_code = extract_code_from_markdown(&content);

        // GPT-4.1 pricing: $2/$8 per 1M tokens
        let estimated_cost = (api_response.usage.prompt_tokens as f64 * 2.0
            + api_response.usage.completion_tokens as f64 * 8.0)
            / 1_000_000.0;

        Ok(GenerateResponse {
            content,
            extracted_code,
            model: api_response.model,
            token_usage: TokenUsage {
                prompt_tokens: api_response.usage.prompt_tokens,
                completion_tokens: api_response.usage.completion_tokens,
                total_tokens: api_response.usage.total_tokens,
                estimated_cost_usd: estimated_cost,
            },
            latency_ms,
        })
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gpt-4.1".into(),
                name: "GPT-4.1".into(),
                provider: "openai".into(),
                max_context: 1_000_000,
                cost_per_1k_input: 0.002,
                cost_per_1k_output: 0.008,
            },
            ModelInfo {
                id: "gpt-4.1-mini".into(),
                name: "GPT-4.1 Mini".into(),
                provider: "openai".into(),
                max_context: 1_000_000,
                cost_per_1k_input: 0.0004,
                cost_per_1k_output: 0.0016,
            },
            ModelInfo {
                id: "gpt-4.1-nano".into(),
                name: "GPT-4.1 Nano".into(),
                provider: "openai".into(),
                max_context: 1_000_000,
                cost_per_1k_input: 0.0001,
                cost_per_1k_output: 0.0004,
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
            "choices": [{"message": {"content": "fn add(a: i32, b: i32) -> i32 { a + b }", "role": "assistant"}, "index": 0}],
            "model": "gpt-4.1",
            "usage": {"prompt_tokens": 40, "completion_tokens": 15, "total_tokens": 55}
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let provider = OpenAiProvider::new("test-key", Some(server.uri()), None);
        let request = GenerateRequest {
            model: "gpt-4.1".into(),
            prompt: "Write an add function".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 1024,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(response.content.contains("fn add"));
        assert_eq!(response.token_usage.total_tokens, 55);
    }

    #[tokio::test]
    async fn custom_base_url() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "choices": [{"message": {"content": "code", "role": "assistant"}, "index": 0}],
            "model": "custom-model",
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let provider = OpenAiProvider::new("key", Some(server.uri()), None);
        let request = GenerateRequest {
            model: "custom-model".into(),
            prompt: "test".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 100,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let response = provider.generate(&request).await.unwrap();
        assert_eq!(response.model, "custom-model");
    }

    #[tokio::test]
    async fn error_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&server)
            .await;

        let provider = OpenAiProvider::new("key", Some(server.uri()), None);
        let request = GenerateRequest {
            model: "gpt-4.1".into(),
            prompt: "test".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 100,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let err = provider.generate(&request).await.unwrap_err();
        assert!(err.to_string().contains("500") || err.to_string().contains("error"));
    }
}
