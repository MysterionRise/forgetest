//! Core trait definitions for LLM providers and code runners.
//!
//! These async traits are implemented by the `forgetest-providers` and
//! `forgetest-runner` crates respectively.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::model::{ContextFile, Language};
use crate::results::{ClippyResult, CompilationResult, TestResult, TokenUsage};

// ---------------------------------------------------------------------------
// LLM Provider trait
// ---------------------------------------------------------------------------

/// Trait for LLM backends that generate code from prompts.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Human-readable provider name (e.g. "anthropic").
    fn name(&self) -> &str;

    /// Generate code from a prompt.
    async fn generate(&self, request: &GenerateRequest) -> anyhow::Result<GenerateResponse>;

    /// List available models for this provider.
    fn available_models(&self) -> Vec<ModelInfo>;
}

/// Request to generate code from an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Model identifier (e.g. "claude-sonnet-4-20250514").
    pub model: String,
    /// The main prompt.
    pub prompt: String,
    /// Optional system prompt override.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Additional context files.
    #[serde(default)]
    pub context_files: Vec<ContextFile>,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f64,
    /// Stop sequences.
    #[serde(default)]
    pub stop_sequences: Vec<String>,
}

/// Response from an LLM code generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    /// The raw response content.
    pub content: String,
    /// Code extracted from markdown blocks.
    pub extracted_code: String,
    /// Model that actually generated the response.
    pub model: String,
    /// Token usage.
    pub token_usage: TokenUsage,
    /// Latency in milliseconds.
    pub latency_ms: u64,
}

/// Information about an available model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier.
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Provider name.
    pub provider: String,
    /// Maximum context window size in tokens.
    pub max_context: u32,
    /// Cost per 1K input tokens in USD.
    pub cost_per_1k_input: f64,
    /// Cost per 1K output tokens in USD.
    pub cost_per_1k_output: f64,
}

// ---------------------------------------------------------------------------
// Code Runner trait
// ---------------------------------------------------------------------------

/// Trait for sandboxed code compilation and test execution.
#[async_trait]
pub trait CodeRunner: Send + Sync {
    /// Compile generated code.
    async fn compile(&self, request: &CompileRequest) -> anyhow::Result<CompilationResult>;

    /// Run tests against generated code.
    async fn run_tests(&self, request: &TestRequest) -> anyhow::Result<TestResult>;

    /// Run clippy on generated code.
    async fn run_clippy(&self, request: &ClippyRequest) -> anyhow::Result<ClippyResult>;
}

/// Request to compile code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileRequest {
    /// The source code to compile.
    pub code: String,
    /// Programming language.
    pub language: Language,
    /// Additional dependencies needed.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Compilation timeout in seconds.
    pub timeout_secs: u64,
}

/// A crate dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Crate name.
    pub name: String,
    /// Version requirement.
    pub version: String,
    /// Features to enable.
    #[serde(default)]
    pub features: Vec<String>,
}

/// Request to run tests. Extends CompileRequest with test code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRequest {
    /// The source code to test.
    pub code: String,
    /// Test code to compile against the source.
    pub test_code: String,
    /// Programming language.
    pub language: Language,
    /// Additional dependencies.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Timeout in seconds.
    pub timeout_secs: u64,
}

/// Request to run clippy. Same shape as CompileRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClippyRequest {
    /// The source code to analyze.
    pub code: String,
    /// Programming language.
    pub language: Language,
    /// Additional dependencies.
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Timeout in seconds.
    pub timeout_secs: u64,
}

// ---------------------------------------------------------------------------
// Default system prompt
// ---------------------------------------------------------------------------

/// Default system prompt for code generation providers.
pub const DEFAULT_SYSTEM_PROMPT: &str = "You are a code generation assistant. Respond ONLY with code. Do not include explanations, comments about the code, or markdown formatting unless the code itself requires comments. Output valid, compilable code.";

// ---------------------------------------------------------------------------
// Markdown code extraction
// ---------------------------------------------------------------------------

/// Extract code from markdown-formatted LLM responses.
///
/// Handles:
/// - Single or multiple ```rust``` blocks (concatenated)
/// - Generic ``` blocks (if no rust-specific blocks found)
/// - Raw code with no markdown blocks (returned as-is)
pub fn extract_code_from_markdown(response: &str) -> String {
    let mut rust_blocks = Vec::new();
    let mut generic_blocks = Vec::new();
    let mut in_block = false;
    let mut is_rust_block = false;
    let mut is_generic_block = false;
    let mut current_block = String::new();

    for line in response.lines() {
        let trimmed = line.trim();

        if !in_block && trimmed.starts_with("```") {
            in_block = true;
            let lang = trimmed.trim_start_matches('`').trim().to_lowercase();
            is_rust_block = lang == "rust" || lang == "rs";
            is_generic_block = lang.is_empty();
            current_block.clear();
            continue;
        }

        if in_block && trimmed == "```" {
            in_block = false;
            if is_rust_block {
                rust_blocks.push(current_block.clone());
            } else if is_generic_block {
                generic_blocks.push(current_block.clone());
            }
            current_block.clear();
            continue;
        }

        if in_block {
            if !current_block.is_empty() {
                current_block.push('\n');
            }
            current_block.push_str(line);
        }
    }

    // Handle truncated (unclosed) code blocks — treat accumulated content as a block
    if in_block && !current_block.is_empty() {
        if is_rust_block {
            rust_blocks.push(current_block);
        } else if is_generic_block {
            generic_blocks.push(current_block);
        }
    }

    // Prefer rust-specific blocks
    if !rust_blocks.is_empty() {
        return rust_blocks.join("\n\n");
    }

    // Fall back to generic blocks
    if !generic_blocks.is_empty() {
        return generic_blocks.join("\n\n");
    }

    // No code blocks found — return raw response
    response.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_single_rust_block() {
        let input = r#"Here is the code:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

That's it!"#;
        let code = extract_code_from_markdown(input);
        assert_eq!(code, "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}");
    }

    #[test]
    fn extract_multiple_rust_blocks() {
        let input = r#"First part:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

Second part:

```rust
fn sub(a: i32, b: i32) -> i32 {
    a - b
}
```
"#;
        let code = extract_code_from_markdown(input);
        assert!(code.contains("fn add"));
        assert!(code.contains("fn sub"));
    }

    #[test]
    fn extract_no_code_blocks_returns_raw() {
        let input = "fn hello() -> &'static str {\n    \"hello\"\n}";
        let code = extract_code_from_markdown(input);
        assert_eq!(code, input);
    }

    #[test]
    fn extract_generic_block_fallback() {
        let input = "```\nfn generic() {}\n```";
        let code = extract_code_from_markdown(input);
        assert_eq!(code, "fn generic() {}");
    }

    #[test]
    fn extract_prefers_rust_over_generic() {
        let input = r#"```
fn generic() {}
```

```rust
fn specific() {}
```
"#;
        let code = extract_code_from_markdown(input);
        assert_eq!(code, "fn specific() {}");
    }

    #[test]
    fn extract_truncated_unclosed_block() {
        let input = "Here is code:\n\n```rust\nfn truncated() -> i32 {\n    42\n}";
        let code = extract_code_from_markdown(input);
        assert!(
            code.contains("fn truncated"),
            "truncated block should be captured, got: {code}"
        );
    }

    #[test]
    fn extract_ignores_other_languages() {
        let input = r#"```python
def hello():
    pass
```

```rust
fn hello() {}
```
"#;
        let code = extract_code_from_markdown(input);
        assert_eq!(code, "fn hello() {}");
    }
}
