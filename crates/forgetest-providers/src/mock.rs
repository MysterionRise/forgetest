//! Mock provider for testing.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;

use forgetest_core::results::TokenUsage;
use forgetest_core::traits::{
    extract_code_from_markdown, GenerateRequest, GenerateResponse, LlmProvider, ModelInfo,
};

/// A mock LLM provider for testing the eval engine without real API calls.
///
/// Returns configurable responses based on prompt content matching.
pub struct MockProvider {
    /// Map of prompt substring → response code.
    responses: HashMap<String, String>,
    /// Default response if no prompt matches.
    default_response: String,
    /// Number of calls made.
    call_count: AtomicU32,
    /// Last request received.
    last_request: Mutex<Option<GenerateRequest>>,
}

impl MockProvider {
    /// Create a new mock provider with the given prompt→response mappings.
    pub fn new(responses: HashMap<String, String>) -> Self {
        Self {
            responses,
            default_response: "fn placeholder() {}".to_string(),
            call_count: AtomicU32::new(0),
            last_request: Mutex::new(None),
        }
    }

    /// Create a mock that always returns the same response.
    pub fn with_fixed_response(response: &str) -> Self {
        Self {
            responses: HashMap::new(),
            default_response: response.to_string(),
            call_count: AtomicU32::new(0),
            last_request: Mutex::new(None),
        }
    }

    /// Get the number of calls made to this provider.
    pub fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::Relaxed)
    }

    /// Get the last request made to this provider.
    pub fn last_request(&self) -> Option<GenerateRequest> {
        self.last_request.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn generate(&self, request: &GenerateRequest) -> anyhow::Result<GenerateResponse> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        *self.last_request.lock().unwrap() = Some(request.clone());

        // Find a matching response based on prompt content
        let content = self
            .responses
            .iter()
            .find(|(key, _)| request.prompt.contains(key.as_str()))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| self.default_response.clone());

        let extracted_code = extract_code_from_markdown(&content);
        let token_count = (content.len() / 4) as u32; // Rough estimate

        Ok(GenerateResponse {
            content: content.clone(),
            extracted_code,
            model: request.model.clone(),
            token_usage: TokenUsage {
                prompt_tokens: (request.prompt.len() / 4) as u32,
                completion_tokens: token_count,
                total_tokens: (request.prompt.len() / 4) as u32 + token_count,
                estimated_cost_usd: 0.0,
            },
            latency_ms: 1,
        })
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "mock-model".into(),
            name: "Mock Model".into(),
            provider: "mock".into(),
            max_context: 100_000,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixed_response() {
        let provider = MockProvider::with_fixed_response("fn hello() {}");
        let request = GenerateRequest {
            model: "mock".into(),
            prompt: "anything".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 100,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let response = provider.generate(&request).await.unwrap();
        assert_eq!(response.content, "fn hello() {}");
        assert_eq!(provider.call_count(), 1);
    }

    #[tokio::test]
    async fn prompt_matching() {
        let mut responses = HashMap::new();
        responses.insert(
            "fibonacci".to_string(),
            "fn fibonacci(n: u64) -> u64 { 0 }".to_string(),
        );
        responses.insert(
            "add".to_string(),
            "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        );

        let provider = MockProvider::new(responses);

        let req_fib = GenerateRequest {
            model: "mock".into(),
            prompt: "Write a fibonacci function".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 100,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let resp = provider.generate(&req_fib).await.unwrap();
        assert!(resp.content.contains("fibonacci"));

        let req_add = GenerateRequest {
            model: "mock".into(),
            prompt: "Write an add function".into(),
            system_prompt: None,
            context_files: vec![],
            max_tokens: 100,
            temperature: 0.0,
            stop_sequences: vec![],
        };

        let resp = provider.generate(&req_add).await.unwrap();
        assert!(resp.content.contains("add"));
        assert_eq!(provider.call_count(), 2);
    }
}
