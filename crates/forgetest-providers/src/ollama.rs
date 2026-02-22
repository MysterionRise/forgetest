//! Ollama (local LLM) provider implementation.

use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use forgetest_core::results::TokenUsage;
use forgetest_core::traits::{
    extract_code_from_markdown, GenerateRequest, GenerateResponse, LlmProvider, ModelInfo,
};

use crate::error::ProviderError;

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_TIMEOUT_SECS: u64 = 300; // Local models are slower
const SYSTEM_PROMPT: &str = "You are a code generation assistant. Respond ONLY with code. Do not include explanations, comments about the code, or markdown formatting unless the code itself requires comments. Output valid, compilable code.";

/// Ollama local LLM provider.
pub struct OllamaProvider {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: &str) -> Self {
        let base = if base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            base_url
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("failed to build HTTP client");

        Self {
            base_url: base.to_string(),
            client,
        }
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f64,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
    model: String,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    size: u64,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
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
            full_prompt.push_str(&format!(
                "File `{}`:\n```\n{}\n```\n\n",
                file.path, file.content
            ));
        }
        full_prompt.push_str(&request.prompt);

        let body = OllamaRequest {
            model: request.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: full_prompt,
                },
            ],
            stream: false,
            options: Some(OllamaOptions {
                temperature: request.temperature,
            }),
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(DEFAULT_TIMEOUT_SECS)
                } else if e.is_connect() {
                    ProviderError::NetworkError(format!(
                        "Ollama not reachable at {}. Is it running? Start with: ollama serve",
                        self.base_url
                    ))
                } else {
                    ProviderError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status().as_u16();
        if status == 404 {
            return Err(ProviderError::ModelNotFound(format!(
                "Model '{}' not found locally. Pull it with: ollama pull {}",
                request.model, request.model
            ))
            .into());
        }
        if status >= 400 {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError {
                status,
                message: body,
            }
            .into());
        }

        let api_response: OllamaResponse =
            response.json().await.map_err(|e| ProviderError::ApiError {
                status: 0,
                message: format!("failed to parse response: {e}"),
            })?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let content = api_response.message.content;
        let extracted_code = extract_code_from_markdown(&content);

        let prompt_tokens = api_response.prompt_eval_count.unwrap_or(0);
        let completion_tokens = api_response.eval_count.unwrap_or(0);

        Ok(GenerateResponse {
            content,
            extracted_code,
            model: api_response.model,
            token_usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
                estimated_cost_usd: 0.0, // Local models are free
            },
            latency_ms,
        })
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        // Try to dynamically fetch models — fall back to empty on error
        // Note: this is a sync context so we can't use async here.
        // For now return a placeholder; the CLI can call list_models_async.
        vec![]
    }
}

impl OllamaProvider {
    /// Dynamically fetch available models from the Ollama instance.
    pub async fn list_models_async(&self) -> anyhow::Result<Vec<ModelInfo>> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|_| {
                ProviderError::NetworkError(format!(
                    "Ollama not reachable at {}. Is it running? Start with: ollama serve",
                    self.base_url
                ))
            })?;

        let tags: OllamaTagsResponse =
            response.json().await.map_err(|e| ProviderError::ApiError {
                status: 0,
                message: format!("failed to parse tags response: {e}"),
            })?;

        Ok(tags
            .models
            .into_iter()
            .map(|m| ModelInfo {
                id: m.name.clone(),
                name: m.name,
                provider: "ollama".into(),
                max_context: 0,
                cost_per_1k_input: 0.0,
                cost_per_1k_output: 0.0,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn successful_generation() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "message": {"role": "assistant", "content": "fn add(a: i32, b: i32) -> i32 { a + b }"},
            "model": "llama3.1:70b",
            "prompt_eval_count": 30,
            "eval_count": 15
        });

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(&server.uri());
        let request = GenerateRequest {
            model: "llama3.1:70b".into(),
            prompt: "Write an add function".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 1024,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let response = provider.generate(&request).await.unwrap();
        assert!(response.content.contains("fn add"));
        assert_eq!(response.token_usage.prompt_tokens, 30);
        assert_eq!(response.token_usage.estimated_cost_usd, 0.0);
    }

    #[tokio::test]
    async fn model_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(404).set_body_string("model not found"))
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(&server.uri());
        let request = GenerateRequest {
            model: "nonexistent".into(),
            prompt: "test".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 100,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let err = provider.generate(&request).await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn dynamic_model_listing() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "models": [
                {"name": "llama3.1:70b", "size": 40000000000_u64},
                {"name": "codellama:13b", "size": 7000000000_u64}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(&server.uri());
        let models = provider.list_models_async().await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3.1:70b");
    }
}
