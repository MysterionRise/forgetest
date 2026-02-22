# forgetest — Claude Code Development Sessions

> LLM Code-Quality Eval Harness for Rust
> "pytest for LLM outputs" — CI-ready, Rust-native, blazing fast

---

## Architecture Overview

```
forgetest/
├── crates/
│   ├── forgetest-core/        # Eval engine, traits, scoring
│   ├── forgetest-runner/      # Sandboxed compilation & test execution
│   ├── forgetest-providers/   # LLM provider integrations
│   ├── forgetest-report/      # HTML/JSON/SARIF report generation
│   └── forgetest-cli/         # CLI binary
├── eval-sets/                 # Built-in eval case collections
│   ├── rust-basics/
│   ├── rust-algorithms/
│   └── rust-async/
├── examples/
├── benches/
└── tests/
```

```
User writes TOML eval cases
        │
        ▼
┌──────────────────┐
│  forgetest-cli   │  ← parses config, orchestrates
└────────┬─────────┘
         │
    ┌────▼────┐        ┌───────────────────┐
    │  core   │───────►│    providers      │  ← calls LLMs
    │(engine) │        │ (OpenAI/Anthropic │
    └────┬────┘        │  /Ollama)         │
         │             └───────────────────┘
    ┌────▼────┐
    │ runner  │  ← sandbox compile + test
    │(sandbox)│
    └────┬────┘
         │
    ┌────▼────┐
    │ report  │  ← HTML / JSON / SARIF
    └─────────┘
```

---

## Session 1: Project Scaffolding & Core Traits

**Goal:** Set up the Cargo workspace, define the core trait system, and establish the data model that everything else builds on.

---

### 1.1 — Workspace & Crate Scaffolding

```
Create a new Rust Cargo workspace called `forgetest` with the following structure:

- Root `Cargo.toml` as a workspace with members: `crates/forgetest-core`, `crates/forgetest-runner`, `crates/forgetest-providers`, `crates/forgetest-report`, `crates/forgetest-cli`
- Each crate should have a basic `lib.rs` (or `main.rs` for cli) with a doc comment describing its purpose
- Root should also have:
  - `.gitignore` (standard Rust + target/ + .env)
  - `README.md` with project description, badges placeholder, and architecture diagram in mermaid
  - `LICENSE-MIT` and `LICENSE-APACHE` dual license files
  - `rust-toolchain.toml` pinning to stable
  - `.github/workflows/ci.yml` with: cargo fmt check, cargo clippy, cargo test, cargo doc

Common workspace dependencies in root Cargo.toml:
- serde (with derive), serde_json, toml
- tokio (full features)
- anyhow, thiserror
- tracing, tracing-subscriber
- uuid (v4)
- chrono

The CLI crate should depend on all other crates. Core should have no internal deps. Runner depends on core. Providers depends on core. Report depends on core.
```

---

### 1.2 — Core Data Model

```
In `forgetest-core/src/`, create the core data model types. These are the fundamental types the entire system uses.

Create `model.rs` with these types:

1. `EvalCase` — a single evaluation task:
   - `id: String` (unique identifier)
   - `name: String` (human-readable)  
   - `description: String`
   - `prompt: String` (the prompt sent to the LLM)
   - `language: Language` (enum: Rust, Python, TypeScript, Go)
   - `context: Vec<ContextFile>` (additional files provided as context)
   - `expectations: Expectations`
   - `tags: Vec<String>` (for filtering)
   - `timeout_secs: Option<u64>`
   - `max_tokens: Option<u32>`

2. `ContextFile` — a file provided as context to the LLM:
   - `path: String` (relative path like "src/lib.rs")
   - `content: String`

3. `Expectations` — what we check about the output:
   - `should_compile: bool` (default true)
   - `should_pass_tests: bool` (default true)
   - `test_file: Option<String>` (test code to compile against the output)
   - `expected_functions: Vec<String>` (function names that must exist)
   - `expected_types: Vec<String>` (type names that must exist)
   - `max_clippy_warnings: Option<u32>`
   - `custom_check: Option<String>` (shell command that receives the code on stdin, exits 0 for pass)

4. `Language` — enum with `Rust`, `Python`, `TypeScript`, `Go`
   - Implement `Display` and `FromStr`

5. `EvalSet` — a collection of eval cases:
   - `id: String`
   - `name: String`
   - `description: String`
   - `cases: Vec<EvalCase>`
   - `default_language: Language`
   - `default_timeout_secs: u64`

All types should derive: `Debug, Clone, Serialize, Deserialize`. Use `#[serde(default)]` where there are sensible defaults. Add doc comments on every type and field.
```

---

### 1.3 — Result & Scoring Types

```
In `forgetest-core/src/`, create `results.rs` with result and scoring types:

1. `EvalResult` — the result of running one eval case:
   - `case_id: String`
   - `model: String` (e.g., "claude-sonnet-4-20250514")
   - `provider: String` (e.g., "anthropic")
   - `generated_code: String` (the LLM's output)
   - `compilation: CompilationResult`
   - `test_execution: Option<TestResult>`
   - `clippy: Option<ClippyResult>`
   - `timing: TimingInfo`
   - `token_usage: TokenUsage`
   - `attempt: u32` (which attempt, for Pass@k)
   - `run_id: Uuid`

2. `CompilationResult`:
   - `success: bool`
   - `errors: Vec<CompilerDiagnostic>`
   - `warnings: Vec<CompilerDiagnostic>`
   - `duration_ms: u64`

3. `CompilerDiagnostic`:
   - `level: DiagnosticLevel` (Error, Warning, Note, Help)
   - `message: String`
   - `code: Option<String>` (e.g., "E0308")
   - `spans: Vec<DiagnosticSpan>`

4. `DiagnosticSpan`:
   - `file: String`
   - `line_start: u32`
   - `line_end: u32`
   - `column_start: u32`
   - `column_end: u32`
   - `text: Option<String>`

5. `TestResult`:
   - `passed: u32`
   - `failed: u32`
   - `ignored: u32`
   - `duration_ms: u64`
   - `failures: Vec<TestFailure>`

6. `TestFailure`:
   - `name: String`
   - `message: String`
   - `stdout: String`

7. `ClippyResult`:
   - `warnings: Vec<CompilerDiagnostic>`
   - `warning_count: u32`

8. `TimingInfo`:
   - `llm_request_ms: u64`
   - `compilation_ms: u64`
   - `test_execution_ms: u64`
   - `total_ms: u64`

9. `TokenUsage`:
   - `prompt_tokens: u32`
   - `completion_tokens: u32`
   - `total_tokens: u32`
   - `estimated_cost_usd: f64`

10. `Score` — the final computed score for a result:
    - `compilation: f64` (0.0 or 1.0)
    - `tests: f64` (0.0–1.0, ratio of passed)
    - `clippy: f64` (0.0–1.0, penalty per warning)
    - `overall: f64` (weighted composite)

Add a method `Score::compute(result: &EvalResult, expectations: &Expectations) -> Score` that calculates the score.

All types derive Debug, Clone, Serialize, Deserialize. Add doc comments on every type.
```

---

### 1.4 — TOML Eval Case Parser

```
In `forgetest-core/src/`, create `parser.rs` that handles loading eval cases from TOML files.

The TOML format should look like this:

```toml
[eval_set]
id = "rust-basics"
name = "Rust Basics"
description = "Fundamental Rust coding tasks"
default_language = "rust"
default_timeout_secs = 60

[[cases]]
id = "fibonacci"
name = "Fibonacci function"
description = "Write a function that returns the nth Fibonacci number"
prompt = """
Write a Rust function `fn fibonacci(n: u64) -> u64` that returns 
the nth Fibonacci number. Use an iterative approach for efficiency.
"""
tags = ["algorithms", "basics"]

[cases.expectations]
should_compile = true
should_pass_tests = true
test_file = """
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_base_cases() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
    }
    #[test]
    fn test_sequence() {
        assert_eq!(fibonacci(10), 55);
        assert_eq!(fibonacci(20), 6765);
    }
}
"""
expected_functions = ["fibonacci"]
```

Implement:
1. `parse_eval_set(path: &Path) -> Result<EvalSet>` — load a single TOML file
2. `load_eval_directory(dir: &Path) -> Result<Vec<EvalSet>>` — recursively load all .toml files in a directory
3. `validate_eval_set(set: &EvalSet) -> Result<Vec<ValidationWarning>>` — check for issues (missing test files when should_pass_tests=true, duplicate IDs, etc.)

Add comprehensive unit tests that include:
- Parsing a valid TOML eval set
- Handling missing optional fields
- Validation catching duplicate IDs
- Validation warning for should_pass_tests=true without test_file
- Error on malformed TOML

Also create `eval-sets/rust-basics.toml` with 5 starter eval cases:
1. fibonacci (iterative)
2. is_palindrome (string)
3. binary_search (generic)
4. flatten_nested (Vec<Vec<T>> -> Vec<T>)
5. word_count (HashMap<String, usize>)
```

---

### 1.5 — Core Traits (Provider & Runner)

```
In `forgetest-core/src/`, create `traits.rs` with the async trait definitions that the provider and runner crates implement.

1. `LlmProvider` trait (async):
   ```rust
   #[async_trait]
   pub trait LlmProvider: Send + Sync {
       /// Human-readable provider name (e.g., "anthropic")
       fn name(&self) -> &str;
       
       /// Generate code from a prompt
       async fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse>;
       
       /// List available models
       fn available_models(&self) -> Vec<ModelInfo>;
   }
   ```

2. `GenerateRequest`:
   - `model: String`
   - `prompt: String`
   - `system_prompt: Option<String>`
   - `context_files: Vec<ContextFile>`
   - `max_tokens: u32`
   - `temperature: f64`
   - `stop_sequences: Vec<String>`

3. `GenerateResponse`:
   - `content: String` (the raw response)
   - `extracted_code: String` (code extracted from markdown blocks)
   - `model: String`
   - `token_usage: TokenUsage`
   - `latency_ms: u64`

4. `ModelInfo`:
   - `id: String`
   - `name: String`
   - `provider: String`
   - `max_context: u32`
   - `cost_per_1k_input: f64`
   - `cost_per_1k_output: f64`

5. `CodeRunner` trait (async):
   ```rust
   #[async_trait]
   pub trait CodeRunner: Send + Sync {
       async fn compile(&self, request: &CompileRequest) -> Result<CompilationResult>;
       async fn run_tests(&self, request: &TestRequest) -> Result<TestResult>;
       async fn run_clippy(&self, request: &ClippyRequest) -> Result<ClippyResult>;
   }
   ```

6. `CompileRequest`:
   - `code: String`
   - `language: Language`
   - `dependencies: Vec<Dependency>` (crate name + version)
   - `timeout_secs: u64`

7. `Dependency`:
   - `name: String`
   - `version: String`
   - `features: Vec<String>`

8. `TestRequest` (extends CompileRequest with test_code)
9. `ClippyRequest` (same as CompileRequest)

Also create a helper function `extract_code_from_markdown(response: &str) -> String` that pulls code out of ```rust``` or ``` blocks. Handle edge cases: multiple blocks (concatenate), no blocks (return raw response), language tag variations.

Write tests for the markdown extraction covering:
- Single rust code block
- Multiple code blocks
- No code blocks (raw code)
- Mixed language blocks (only extract rust)
- Nested backticks
```

---

## Session 2: LLM Provider Integrations

**Goal:** Implement the LLM provider trait for OpenAI, Anthropic, and Ollama so forgetest can actually call models.

---

### 2.1 — Provider Configuration & Factory

```
In `forgetest-providers/src/`, create the provider configuration and factory system.

Create `config.rs`:
1. `ProviderConfig` enum:
   - `OpenAI { api_key: String, base_url: Option<String>, org_id: Option<String> }`
   - `Anthropic { api_key: String, base_url: Option<String> }`
   - `Ollama { base_url: String }` (defaults to http://localhost:11434)

2. `ForgetestConfig` — the top-level config for the whole tool:
   - `providers: HashMap<String, ProviderConfig>`
   - `default_provider: String`
   - `default_model: String`
   - `default_temperature: f64` (default 0.0 for deterministic evals)
   - `max_retries: u32` (default 3)
   - `retry_delay_ms: u64` (default 1000)
   - `parallelism: usize` (default 4)
   - `output_dir: PathBuf` (default ./forgetest-results)

3. `load_config() -> Result<ForgetestConfig>`:
   - Check for `forgetest.toml` in current directory
   - Fall back to `~/.config/forgetest/config.toml`
   - Environment variable overrides: `FORGETEST_OPENAI_KEY`, `FORGETEST_ANTHROPIC_KEY`, etc.
   - If no config found, return helpful error message listing where config was searched

4. `create_provider(name: &str, config: &ProviderConfig) -> Result<Box<dyn LlmProvider>>`:
   - Factory function that creates the appropriate provider

Write tests for config loading with temp directories and env vars.

Also create a sample `forgetest.toml`:
```toml
[providers.anthropic]
type = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"  # env var interpolation

[providers.openai]
type = "openai"  
api_key = "${OPENAI_API_KEY}"

[providers.ollama]
type = "ollama"
base_url = "http://localhost:11434"

default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"
default_temperature = 0.0
parallelism = 4
```
```

---

### 2.2 — Anthropic Provider

```
In `forgetest-providers/src/`, create `anthropic.rs` implementing `LlmProvider` for the Anthropic API.

Use `reqwest` for HTTP calls. Do NOT use any Anthropic SDK crate — implement the API calls directly for minimal dependencies.

Implementation details:
- POST to `https://api.anthropic.com/v1/messages`
- Headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- Request body: `{ model, max_tokens, temperature, system, messages: [{ role: "user", content }] }`
- Parse response to extract `content[0].text` and `usage` fields
- Handle rate limiting: check for 429 status, respect `retry-after` header, exponential backoff
- Handle streaming: NOT needed for v1 (batch evals don't need streaming)
- Timeout: 120 seconds default for code generation

The system prompt should be:
"You are a code generation assistant. Respond ONLY with code. Do not include explanations, comments about the code, or markdown formatting unless the code itself requires comments. Output valid, compilable code."

`available_models()` should return a hardcoded list:
- claude-sonnet-4-20250514 (200K context, $3/$15 per 1M tokens)
- claude-haiku-4-5-20251001 (200K context, $0.80/$4 per 1M tokens)

Use `extract_code_from_markdown` from core on the response content to handle cases where the model wraps code in markdown despite the system prompt.

Add the `tracing` instrumentation: trace span on each API call with model name, token count, latency.

Error handling:
- `ProviderError::RateLimited { retry_after_ms }`
- `ProviderError::AuthenticationFailed`
- `ProviderError::ModelNotFound(String)`
- `ProviderError::ApiError { status: u16, message: String }`
- `ProviderError::Timeout`
- `ProviderError::NetworkError(String)`

Write unit tests using `wiremock` to mock the Anthropic API:
- Successful generation
- Rate limiting with retry
- Authentication failure
- Timeout handling
```

---

### 2.3 — OpenAI Provider

```
In `forgetest-providers/src/`, create `openai.rs` implementing `LlmProvider` for the OpenAI API.

Same approach as Anthropic: direct `reqwest` calls, no SDK crate.

Implementation details:
- POST to `{base_url}/v1/chat/completions` (base_url defaults to https://api.openai.com)
- Headers: `Authorization: Bearer {api_key}`, `content-type: application/json`
- Request body: `{ model, max_tokens, temperature, messages: [{ role: "system", content }, { role: "user", content }] }`
- Parse response: `choices[0].message.content` and `usage`
- Same rate limiting / retry / timeout logic as Anthropic
- Support custom base_url so it works with OpenAI-compatible APIs (OpenRouter, Together, etc.)

Same system prompt as Anthropic provider.

`available_models()`:
- gpt-4.1 (1M context, $2/$8 per 1M)
- gpt-4.1-mini (1M context, $0.40/$1.60 per 1M)
- gpt-4.1-nano (1M context, $0.10/$0.40 per 1M)

Reuse the same ProviderError types from the anthropic module (move them to a shared `error.rs` in the providers crate).

Write unit tests with wiremock:
- Successful generation
- Custom base_url routing
- Error response handling
```

---

### 2.4 — Ollama Provider

```
In `forgetest-providers/src/`, create `ollama.rs` implementing `LlmProvider` for local Ollama.

Implementation details:
- POST to `{base_url}/api/chat` (base_url defaults to http://localhost:11434)
- Request body: `{ model, messages: [{ role: "system", content }, { role: "user", content }], stream: false, options: { temperature } }`
- Parse response: `message.content`
- Token usage: Ollama returns `eval_count` and `prompt_eval_count`
- No rate limiting needed for local
- Timeout should be longer: 300 seconds default (local models are slow)

`available_models()`:
- Dynamically fetch from `GET {base_url}/api/tags`
- Return list of locally available models with their sizes
- Cost per token is 0.0 for all local models

Special handling:
- If Ollama is not running, return a clear error: "Ollama not reachable at {base_url}. Is it running? Start with: ollama serve"
- If requested model is not pulled, return: "Model '{model}' not found locally. Pull it with: ollama pull {model}"

Write tests:
- Successful generation (wiremock)
- Ollama not running (connection refused)
- Model not found
- Dynamic model listing
```

---

### 2.5 — Provider Integration Tests

```
In `forgetest-providers/tests/`, create integration test infrastructure.

Create `integration_tests.rs`:
1. A test that uses ALL providers against a simple prompt: "Write a Rust function `fn add(a: i32, b: i32) -> i32` that returns the sum of a and b."
2. Verify each response contains valid-looking Rust code (contains "fn add", contains "->")
3. Mark tests with `#[ignore]` so they don't run in CI without API keys
4. Use env vars for API keys: `FORGETEST_TEST_OPENAI_KEY`, `FORGETEST_TEST_ANTHROPIC_KEY`
5. Add a test helper that skips if the env var is missing (don't fail, just skip)

Create `mock_provider.rs` in the providers crate:
- `MockProvider` struct implementing `LlmProvider`
- Returns configurable responses from a `HashMap<String, String>` (prompt → response)
- Tracks call count, last request, total tokens
- Useful for testing the eval engine without real API calls

Also add a `providers/src/lib.rs` that re-exports everything cleanly:
```rust
pub mod anthropic;
pub mod openai;
pub mod ollama;
pub mod config;
pub mod error;
pub mod mock;

pub use config::{ProviderConfig, ForgetestConfig, create_provider};
pub use error::ProviderError;
```
```

---

## Session 3: Sandboxed Code Runner

**Goal:** Build the engine that takes generated code, compiles it in isolation, runs tests, and runs clippy — all safely sandboxed.

---

### 3.1 — Workspace Sandbox Manager

```
In `forgetest-runner/src/`, create `sandbox.rs` — the core sandboxing system.

The approach: for each eval, create a temporary Cargo project in a temp directory, write the generated code, compile it, and run tests. Use process isolation (not kernel sandboxing) for portability.

Create `Sandbox` struct:
- `work_dir: TempDir` (auto-cleaned on drop)
- `timeout: Duration`
- `language: Language`

Implement methods:
1. `Sandbox::new(language: Language, timeout: Duration) -> Result<Self>`:
   - Create temp dir using `tempfile::TempDir`
   - For Rust: run `cargo init --name eval_target` inside it
   - Pre-write Cargo.toml with common dependencies (serde, itertools, etc.)

2. `Sandbox::write_source(&self, code: &str) -> Result<()>`:
   - Write the generated code to `src/lib.rs`
   - If code contains `fn main`, write to `src/main.rs` instead

3. `Sandbox::write_test(&self, test_code: &str) -> Result<()>`:
   - Append test code to `src/lib.rs` (after the main code)
   - Or write to `tests/eval_test.rs` if it's a standalone test file

4. `Sandbox::add_dependency(&self, name: &str, version: &str) -> Result<()>`:
   - Parse Cargo.toml, add dependency, write back
   - Use `toml_edit` crate for preserving formatting

5. `Sandbox::cleanup(self)`:
   - Explicit cleanup (TempDir already does this on drop, but provide manual option)

Important implementation details:
- Set `CARGO_TARGET_DIR` environment variable to a shared target directory to speed up dependency compilation (deps are cached across evals)
- All child processes must inherit a restricted environment (no access to user's git credentials, SSH keys, etc.)
- Set env vars: `HOME=/tmp/forgetest-sandbox`, clear `SSH_AUTH_SOCK`, clear `AWS_*` env vars

Write tests:
- Sandbox creates valid Cargo project
- Write source and verify file exists
- Add dependency and verify Cargo.toml
- Cleanup removes temp directory
```

---

### 3.2 — Compilation Runner

```
In `forgetest-runner/src/`, create `compiler.rs` implementing compilation.

Implement `compile()` on the Sandbox:

```rust
pub async fn compile(&self) -> Result<CompilationResult>
```

Implementation:
1. Run `cargo build --message-format=json` in the sandbox work_dir
2. Parse stdout line-by-line using `cargo_metadata::Message::parse_stream`
3. Collect `CompilerMessage` entries into `CompilerDiagnostic` structs
4. Check exit code for success/failure
5. Measure duration with `Instant::now()`
6. Handle timeout: use `tokio::time::timeout` wrapping the child process, kill on timeout

The `cargo build` command should be spawned with:
- `stdout: Stdio::piped()` (for JSON messages)
- `stderr: Stdio::piped()` (for non-JSON output)
- Working directory set to sandbox work_dir
- The shared `CARGO_TARGET_DIR` env var
- `RUSTFLAGS="-D warnings"` optionally (configurable)

Parse `cargo_metadata::Message` variants:
- `CompilerMessage` → extract diagnostic level, message, code, spans
- `CompilerArtifact` → note successful artifact compilation
- `BuildFinished` → final success/failure status

Edge cases to handle:
- Build script panics
- OOM during compilation (large generated code)
- Infinite compile times (recursive macros)
- Code that generates build.rs (strip it out for safety)

Write tests:
- Compile valid code → success with no errors
- Compile code with type error → failure with E0308 diagnostic
- Compile code with warning → success with warnings collected
- Compile timeout → clean timeout error
- Parse real cargo JSON output fixture files
```

---

### 3.3 — Test Runner

```
In `forgetest-runner/src/`, create `test_runner.rs` implementing test execution.

Implement `run_tests()` on the Sandbox:

```rust
pub async fn run_tests(&self) -> Result<TestResult>
```

Implementation:
1. First compile (if not already compiled) — reuse compile() logic
2. Run `cargo test --message-format=json -- --format=json -Z unstable-options` (nightly) 
   OR parse the human-readable test output for stable (provide both strategies)
3. For stable Rust parsing strategy:
   - Parse lines matching: `test <name> ... ok`, `test <name> ... FAILED`
   - Parse summary line: `test result: ok. X passed; Y failed; Z ignored`
   - Capture everything between "failures:" and "test result:" as failure details
4. Measure duration
5. Handle timeout the same as compilation

Create `TestOutputParser`:
- `parse_stable(output: &str) -> TestResult` — parse human-readable test output
- `parse_json(output: &str) -> TestResult` — parse JSON test output (unstable)
- Auto-detect which format based on first line

Handle:
- Tests that panic (captured as failures)
- Tests that hang (timeout kills the process)
- Tests that write to stdout (captured in TestFailure.stdout)
- No tests found (return TestResult with all zeros)
- `#[should_panic]` tests

Write tests with fixture outputs:
- All tests pass
- Some tests fail with assertion messages
- Test panics  
- No tests found
- Mixed pass/fail/ignore results
- Parse real `cargo test` output fixtures (create 3-4 fixture files with real output)
```

---

### 3.4 — Clippy Integration

```
In `forgetest-runner/src/`, create `clippy.rs` implementing Clippy analysis.

Implement `run_clippy()` on the Sandbox:

```rust
pub async fn run_clippy(&self) -> Result<ClippyResult>
```

Implementation:
1. Run `cargo clippy --message-format=json -- -W clippy::all` in sandbox
2. Parse JSON output same as compiler (reuse diagnostic parsing from compiler.rs)
3. Filter to only clippy-specific warnings (code starts with "clippy::")
4. Count warnings
5. Handle case where clippy is not installed: check for the binary first, return a clear error

Scoring logic (add to `Score::compute`):
- 0 clippy warnings → clippy score 1.0
- Each warning reduces score by 0.1 (configurable)
- Cap at 0.0 minimum

Also create a helper `check_toolchain_available()`:
- Check `rustc --version` works
- Check `cargo --version` works
- Check `cargo clippy --version` works
- Return a `ToolchainStatus` enum with what's available

Write tests:
- Code with no clippy warnings
- Code with clippy warnings (unused variables, etc.)
- Clippy not available (graceful fallback)
```

---

### 3.5 — Runner Trait Implementation & Pipeline

```
In `forgetest-runner/src/`, create `lib.rs` that ties everything together and implements the `CodeRunner` trait from core.

Create `LocalRunner` struct:
- `shared_target_dir: PathBuf` (shared across all sandboxes for dep caching)
- `default_timeout: Duration`
- `default_dependencies: Vec<Dependency>` (common deps added to every sandbox)

Implement `CodeRunner` for `LocalRunner`:
- `compile()`: create sandbox → write source → compile → return result
- `run_tests()`: create sandbox → write source → write tests → run tests → return result
- `run_clippy()`: create sandbox → write source → run clippy → return result

Create a higher-level `run_eval` function:
```rust
pub async fn run_eval(
    runner: &LocalRunner,
    case: &EvalCase,
    generated_code: &str,
) -> Result<EvalResult>
```

This function:
1. Creates a sandbox
2. Writes the generated code
3. Compiles (collect result)
4. If compilation succeeded AND case expects tests: write test file, run tests
5. If compilation succeeded: run clippy
6. Compute timing info
7. Return EvalResult with all fields populated

Also handle the code extraction step: if the LLM response contains markdown code blocks, extract only the code. If it contains explanatory text before/after code, strip it.

Write an end-to-end integration test:
- Generate a known piece of Rust code (hardcoded, not from an LLM)
- Run through the full pipeline
- Verify compilation succeeds
- Verify tests pass
- Verify clippy results are collected
- Also test with intentionally broken code (type error) and verify the failure is captured correctly
```

---

## Session 4: Eval Engine & Orchestration

**Goal:** Build the central engine that coordinates multiple eval cases across multiple models with parallelism, retries, and Pass@k support.

---

### 4.1 — Eval Engine Core

```
In `forgetest-core/src/`, create `engine.rs` — the central eval orchestrator.

Create `EvalEngine`:
```rust
pub struct EvalEngine {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    runner: Box<dyn CodeRunner>,
    config: EvalEngineConfig,
}

pub struct EvalEngineConfig {
    pub parallelism: usize,           // max concurrent evals
    pub pass_k: Vec<u32>,             // e.g., [1, 5, 10] for Pass@1, Pass@5, Pass@10
    pub temperature: f64,              // temperature for generation
    pub max_retries_per_case: u32,     // retries on provider errors (not code failures)
    pub retry_delay: Duration,
    pub system_prompt_override: Option<String>,
}
```

Implement `EvalEngine::run`:
```rust
pub async fn run(
    &self,
    eval_set: &EvalSet,
    models: &[ModelSpec],     // which models to evaluate
    progress: &dyn ProgressReporter,
) -> Result<EvalReport>
```

Where `ModelSpec` is:
- `provider: String`
- `model: String`

The `run` method:
1. For each model in models:
   2. For each case in eval_set.cases:
      3. For k in 1..=max(pass_k):
         4. Call provider.generate() with the case prompt
         5. Call runner.run_eval() on the generated code
         6. Compute Score
         7. Store EvalResult
8. Compute aggregate statistics (Pass@k for each k value)
9. Return EvalReport

Use `tokio::sync::Semaphore` to limit parallelism.
Use `futures::stream::FuturesUnordered` for concurrent execution.

Create `ProgressReporter` trait:
```rust
pub trait ProgressReporter: Send + Sync {
    fn on_eval_start(&self, case_id: &str, model: &str, attempt: u32);
    fn on_eval_complete(&self, result: &EvalResult);
    fn on_eval_error(&self, case_id: &str, model: &str, error: &str);
    fn on_set_complete(&self, stats: &SetStats);
}
```

Create `SetStats`:
- `total_cases: usize`
- `completed: usize`
- `failed: usize`
- `elapsed: Duration`
```

---

### 4.2 — Pass@k Statistical Scoring

```
In `forgetest-core/src/`, create `statistics.rs` with Pass@k computation.

Implement the standard Pass@k estimator from the Codex paper (Chen et al., 2021):

```
Pass@k = 1 - (C(n-c, k) / C(n, k))
```

Where:
- n = total number of samples generated
- c = number of correct samples (compiled + tests passed)
- k = the k in Pass@k

Use the `num` crate for BigInt combinations to avoid overflow.

Create:
1. `pass_at_k(n: u32, c: u32, k: u32) -> f64`
2. `compute_pass_at_k_batch(results: &[EvalResult], expectations: &Expectations, k_values: &[u32]) -> HashMap<u32, f64>`
   - Groups results by (case_id, model)
   - For each group, computes Pass@k for each k value
3. `compute_aggregate_stats(results: &[EvalResult], eval_set: &EvalSet, k_values: &[u32]) -> AggregateStats`

`AggregateStats`:
- `per_model: HashMap<String, ModelStats>`
- `per_case: HashMap<String, CaseStats>`

`ModelStats`:
- `model: String`
- `pass_at_k: HashMap<u32, f64>` (k → score)
- `avg_compilation_rate: f64`
- `avg_test_pass_rate: f64`
- `avg_clippy_score: f64`
- `total_tokens: u64`
- `total_cost_usd: f64`
- `avg_latency_ms: u64`

`CaseStats`:
- `case_id: String`
- `per_model_pass_rate: HashMap<String, f64>`
- `hardest_for: Vec<String>` (models that scored lowest)

Write extensive tests:
- Pass@1 with all successes = 1.0
- Pass@1 with all failures = 0.0
- Pass@1 with 50% success rate ≈ 0.5
- Pass@10 with 1/10 success = 1.0 (at least one passes)
- Verify against known values from the Codex paper
- Edge cases: k > n, c = 0, c = n
```

---

### 4.3 — EvalReport & Regression Detection

```
In `forgetest-core/src/`, create `report.rs` for the structured eval report.

`EvalReport`:
- `id: Uuid`
- `created_at: DateTime<Utc>`
- `eval_set: EvalSetSummary` (id, name, case count)
- `models_evaluated: Vec<String>`
- `config: EvalEngineConfig`
- `results: Vec<EvalResult>`
- `aggregate: AggregateStats`
- `duration: Duration`

Implement:
1. `EvalReport::save_json(&self, path: &Path) -> Result<()>`
2. `EvalReport::load_json(path: &Path) -> Result<Self>`
3. `EvalReport::compare(&self, baseline: &EvalReport) -> RegressionReport`

`RegressionReport`:
- `regressions: Vec<Regression>` — cases where score went DOWN
- `improvements: Vec<Improvement>` — cases where score went UP  
- `unchanged: usize`
- `new_cases: usize` (in current but not baseline)
- `removed_cases: usize`

`Regression`:
- `case_id: String`
- `model: String`
- `baseline_score: f64`
- `current_score: f64`
- `delta: f64`
- `category: RegressionCategory` (Compilation, Tests, Clippy)

Regression detection logic:
- A regression is when `current_score < baseline_score - threshold` (threshold default 0.05)
- An improvement is the reverse
- Match cases by (case_id, model) pair

`RegressionReport::to_markdown(&self) -> String`:
- Format as a markdown table showing regressions and improvements
- Include summary line like "3 regressions, 5 improvements, 12 unchanged"

Write tests:
- Compare identical reports = no changes
- Compare with one regression
- Compare with removed/added cases
- Markdown output formatting
```

---

### 4.4 — Eval Engine Integration Test

```
In `forgetest-core/tests/`, create `engine_integration.rs` — an end-to-end test of the eval engine using the mock provider.

The test should:
1. Create a MockProvider with known responses:
   - "fibonacci" prompt → valid fibonacci implementation
   - "is_palindrome" prompt → valid palindrome check
   - "binary_search" prompt → code with a compilation error (intentional)
2. Create a LocalRunner (real compilation/testing)
3. Create an EvalEngine with the mock provider and runner
4. Load the `rust-basics.toml` eval set created in Session 1
5. Run the engine against the first 3 cases
6. Verify:
   - fibonacci: compilation passes, tests pass, score near 1.0
   - is_palindrome: compilation passes, tests pass
   - binary_search: compilation fails, score 0.0 for compilation
7. Verify aggregate stats are computed
8. Save report as JSON, reload, verify roundtrip
9. Run again with a "regression" response (fibonacci now fails) and verify regression detection

This test validates the ENTIRE pipeline end-to-end: config → provider → generation → sandbox → compilation → test execution → scoring → reporting → regression detection.

Mark with `#[ignore]` only if it needs real cargo/rustc (it does). Add a note that CI needs Rust toolchain installed.
```

---

## Session 5: CLI & Report Generation

**Goal:** Build the user-facing CLI and the HTML report generator.

---

### 5.1 — CLI Structure with Clap

```
In `forgetest-cli/src/`, build the CLI using `clap` v4 with derive macros.

Commands:

1. `forgetest run` — run evaluations
   - `--eval-set <PATH>` (required) — path to .toml eval set or directory
   - `--models <MODELS>` — comma-separated list like "anthropic/claude-sonnet-4-20250514,openai/gpt-4.1"
   - `--pass-k <K>` — Pass@k values, comma-separated (default: "1")
   - `--parallelism <N>` — concurrent evals (default: 4)
   - `--temperature <T>` — generation temperature (default: 0.0)
   - `--output <PATH>` — output directory (default: ./forgetest-results)
   - `--format <FMT>` — output format: json, html, sarif, all (default: json)
   - `--filter <TAGS>` — only run cases matching these tags
   - `--config <PATH>` — path to forgetest.toml

2. `forgetest compare` — compare two eval reports
   - `--baseline <PATH>` — path to baseline report JSON
   - `--current <PATH>` — path to current report JSON
   - `--threshold <F>` — regression threshold (default: 0.05)
   - `--fail-on-regression` — exit code 1 if regressions found (for CI)
   - `--format <FMT>` — output format: text, json, markdown (default: text)

3. `forgetest validate` — validate eval set TOML files
   - `--eval-set <PATH>` — path to validate
   - Lists any warnings or errors

4. `forgetest list-models` — list available models from configured providers
   - `--provider <NAME>` — filter to specific provider
   - `--config <PATH>` — config file path

5. `forgetest init` — create a starter forgetest.toml and example eval set
   - Creates `forgetest.toml` with placeholders
   - Creates `eval-sets/example.toml` with 2 simple cases

For the `run` command, implement a console progress reporter:
- Use `indicatif` for progress bars
- Show: `[3/15] claude-sonnet-4-20250514 :: fibonacci (attempt 1/5) ✓ compile ✓ tests`
- At the end, show a summary table of Pass@k scores per model

Handle Ctrl+C gracefully: catch SIGINT, stop spawning new evals, wait for in-progress evals to finish, save partial results.
```

---

### 5.2 — Console Progress Reporter & Summary

```
In `forgetest-cli/src/`, create `progress.rs` implementing the ProgressReporter trait with a rich terminal UI.

Use `indicatif` for progress bars and `comfy-table` for result tables.

The progress display should look like:
```
forgetest v0.1.0 — Running 15 eval cases × 2 models × 5 attempts

[████████░░░░░░░░░░] 42/150 (28%) │ 2 running │ ETA: 3m 12s

  ✓ claude-sonnet :: fibonacci      [1/5] compile ✓ tests 2/2 ✓ clippy 0 warns
  ✓ claude-sonnet :: is_palindrome  [1/5] compile ✓ tests 3/3 ✓ clippy 1 warn
  ✗ gpt-4.1       :: binary_search  [1/5] compile ✗ (E0308: mismatched types)
  ⠋ claude-sonnet :: word_count     [2/5] compiling...
```

After completion, print a summary table:
```
┌───────────────────────┬──────────┬──────────┬──────────┬────────┬─────────┐
│ Model                 │ Pass@1   │ Pass@5   │ Compile% │ Cost   │ Latency │
├───────────────────────┼──────────┼──────────┼──────────┼────────┼─────────┤
│ claude-sonnet-4-20250514    │ 73.3%    │ 93.3%    │ 100.0%   │ $0.42  │ 2.1s    │
│ gpt-4.1               │ 60.0%    │ 86.7%    │ 93.3%    │ $0.38  │ 1.8s    │
│ llama3.1:70b          │ 46.7%    │ 73.3%    │ 86.7%    │ $0.00  │ 12.4s   │
└───────────────────────┴──────────┴──────────┴──────────┴────────┴─────────┘

Results saved to: ./forgetest-results/report-2026-02-18T14:30:00.json
```

Also implement a `QuietReporter` that only prints the final summary (for CI pipelines).

Create the reporter factory:
- `--verbose` → full progress display
- `--quiet` → only summary
- Default: progress display if terminal is interactive (isatty), quiet otherwise
```

---

### 5.3 — HTML Report Generator

```
In `forgetest-report/src/`, create `html.rs` that generates a self-contained HTML report.

The HTML report should be a SINGLE .html file (all CSS/JS inlined) that contains:

1. **Header**: forgetest report title, date, eval set name, models tested
2. **Summary Dashboard**: 
   - Pass@k scores per model (bar chart — use inline SVG, no JS dependencies)
   - Compilation rate, test pass rate per model
   - Total cost and token usage
3. **Per-Case Results Table**:
   - Sortable by case name, model, score (use vanilla JS for sorting)
   - Color-coded: green (pass), red (fail), yellow (partial)
   - Expandable rows showing: generated code, compiler errors, test failures
4. **Comparison Section** (if baseline is provided):
   - Regressions highlighted in red
   - Improvements highlighted in green
5. **Raw Data**: collapsible JSON dump of the full report

Use `askama` (or `maud`) for HTML templating — keep it compile-time.

The SVG bar chart should be generated with a simple helper function — no charting library needed. Just compute bar widths from scores and emit SVG rects with text labels.

CSS should use a clean, professional theme:
- Monospace font for code
- Dark theme option via CSS media query (prefers-color-scheme)
- Responsive layout

Write the report to `{output_dir}/report-{timestamp}.html`.

Write a test that generates an HTML report from a fixture EvalReport and verifies:
- File is valid HTML (contains <html>, </html>)
- Contains all model names
- Contains all case IDs
- Contains the summary statistics
```

---

### 5.4 — SARIF Output for CI Integration

```
In `forgetest-report/src/`, create `sarif.rs` that generates SARIF (Static Analysis Results Interchange Format) output.

SARIF is the format used by GitHub Code Scanning, so this enables forgetest results to appear as annotations in PRs.

Generate a SARIF 2.1.0 document with:
- One `run` per model evaluated
- One `result` per eval case that had issues (compilation failures, test failures)
- `level`: "error" for compilation failures, "warning" for test failures, "note" for clippy warnings
- `message`: include the compiler error or test failure message
- `ruleId`: "compilation-failure", "test-failure", "clippy-warning"
- `locations`: point to a virtual file path like `eval-cases/{case_id}.rs`

The SARIF structure (simplified):
```json
{
  "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [{
    "tool": {
      "driver": {
        "name": "forgetest",
        "version": "0.1.0",
        "rules": [...]
      }
    },
    "results": [...]
  }]
}
```

Write to `{output_dir}/report-{timestamp}.sarif`.

Write a test verifying SARIF structure is valid JSON Schema compliant (basic checks — correct keys, correct types).
```

---

## Session 6: Built-in Eval Sets & Testing

**Goal:** Create a rich collection of eval cases that ships with forgetest, and comprehensive testing.

---

### 6.1 — Rust Basics Eval Set (15 cases)

```
Create `eval-sets/rust-basics.toml` with 15 eval cases testing fundamental Rust skills.

Each case should have: clear prompt, test file with at least 3 test functions, expected_functions list, appropriate tags.

Cases:
1. fibonacci — iterative nth fibonacci
2. is_palindrome — check if string is palindrome (Unicode-aware)
3. binary_search — generic binary search returning Option<usize>
4. flatten — Vec<Vec<T>> → Vec<T>  
5. word_count — count word frequencies in a string → HashMap
6. roman_numerals — integer to Roman numeral string
7. matrix_transpose — transpose a 2D vector
8. merge_sorted — merge two sorted Vecs into one sorted Vec
9. brackets_balanced — check if brackets/parens/braces are balanced
10. run_length_encoding — encode "aaabbc" → "3a2b1c"
11. caesar_cipher — encrypt/decrypt with shift
12. unique_elements — remove duplicates preserving order
13. parse_csv — parse a CSV string into Vec<Vec<String>>
14. gcd_lcm — compute GCD and LCM of two numbers
15. reverse_words — reverse word order in a sentence

For each case, write thorough tests covering:
- Normal cases
- Edge cases (empty input, single element, etc.)
- Large input (performance-ish)

Tag each case: ["basics"], ["strings"], ["algorithms"], ["data-structures"] as appropriate.
```

---

### 6.2 — Rust Algorithms Eval Set (10 cases)

```
Create `eval-sets/rust-algorithms.toml` with 10 harder algorithmic eval cases.

Cases:
1. topological_sort — Kahn's algorithm on a DAG
2. dijkstra — shortest path in weighted graph
3. lru_cache — LRU cache with O(1) get/put using HashMap + LinkedList
4. trie — prefix tree with insert, search, starts_with
5. min_spanning_tree — Kruskal's algorithm
6. longest_common_subsequence — dynamic programming LCS
7. serialize_binary_tree — serialize/deserialize binary tree to string
8. rabin_karp — string search with rolling hash
9. union_find — disjoint set with path compression and union by rank
10. a_star — A* pathfinding on a 2D grid

Each case should have comprehensive tests. These are harder so the prompts should be more detailed (describe the algorithm approach expected, define the input types clearly).

Use custom types in the prompt where needed:
```toml
prompt = """
Implement a Trie (prefix tree) with the following interface:

```rust
pub struct Trie { /* your fields */ }

impl Trie {
    pub fn new() -> Self { ... }
    pub fn insert(&mut self, word: &str) { ... }
    pub fn search(&self, word: &str) -> bool { ... }
    pub fn starts_with(&self, prefix: &str) -> bool { ... }
}
```
"""
```

Tag all cases: ["algorithms", "advanced"].
```

---

### 6.3 — Rust Async Eval Set (5 cases)

```
Create `eval-sets/rust-async.toml` with 5 eval cases testing async Rust.

These cases need `tokio` as a dependency. Add this to the eval set config:
```toml
[eval_set]
dependencies = [
  { name = "tokio", version = "1", features = ["full"] },
]
```

Cases:
1. async_timeout — wrap a future with a timeout, return Result
2. concurrent_fetch — fetch multiple URLs concurrently, return results
3. rate_limiter — token bucket rate limiter using tokio::time
4. async_channel — producer-consumer with bounded async channel
5. retry_with_backoff — retry an async operation with exponential backoff

These are significantly harder for LLMs. The tests should use `#[tokio::test]`.

Tag all: ["async", "tokio", "advanced"].
```

---

### 6.4 — End-to-End Test Suite

```
Create a comprehensive test suite in `tests/` at the workspace root.

`tests/e2e_mock.rs` — full pipeline with MockProvider:
1. Load each eval set (basics, algorithms, async)
2. Run all cases against MockProvider with known-good implementations
3. Verify all pass with score 1.0
4. This validates the test cases themselves are correct

`tests/e2e_broken.rs` — full pipeline with intentionally broken code:
1. MockProvider returns code with various types of errors:
   - Missing semicolons
   - Wrong return types
   - Infinite loops (tests timeout)
   - Correct code but wrong logic (tests fail)
   - Code with clippy warnings
2. Verify each error type is correctly categorized and scored

`tests/cli_tests.rs` — CLI integration tests using `assert_cmd`:
1. `forgetest validate --eval-set eval-sets/rust-basics.toml` → exits 0
2. `forgetest validate --eval-set nonexistent.toml` → exits 1 with error
3. `forgetest init` → creates files, exits 0
4. `forgetest list-models --config test-config.toml` → lists mock models
5. `forgetest compare --baseline a.json --current b.json` → shows diff

`tests/regression.rs` — regression detection:
1. Create two reports with known scores
2. Run comparison
3. Verify regressions and improvements are detected correctly
4. Verify `--fail-on-regression` exit code behavior
```

---

## Session 7: Polish, Docs & Launch Prep

**Goal:** Documentation, benchmarks, CI hardening, and everything needed for a crates.io publish and GitHub launch.

---

### 7.1 — Documentation

```
Create comprehensive documentation for forgetest:

1. `README.md` — complete rewrite with:
   - One-line description
   - Feature list
   - Quick start (install, create config, write first eval, run)
   - Output screenshot/example (terminal output)
   - Architecture diagram (mermaid)
   - Comparison table vs alternatives (EleutherAI harness, custom scripts)
   - Link to full docs
   - Contributing section
   - License

2. `docs/` directory with mdBook structure:
   - `book.toml` config
   - `src/SUMMARY.md`
   - `src/getting-started.md` — install, config, first eval
   - `src/writing-eval-cases.md` — TOML format reference, examples
   - `src/providers.md` — configuring OpenAI, Anthropic, Ollama
   - `src/ci-integration.md` — GitHub Actions, SARIF, regression detection
   - `src/scoring.md` — how Pass@k works, score calculation, interpretation
   - `src/advanced.md` — custom runners, extending with new languages

3. Add doc comments to ALL public types and functions across all crates.
   Use `#![warn(missing_docs)]` in each crate's lib.rs.

4. Create `examples/` directory with runnable examples:
   - `examples/quick_eval.rs` — minimal programmatic usage
   - `examples/custom_scorer.rs` — implement a custom scoring function
   - `examples/ci_workflow.yml` — copy-paste GitHub Actions workflow
```

---

### 7.2 — GitHub Actions CI Workflow

```
Create `.github/workflows/ci.yml` with a comprehensive CI pipeline:

Jobs:
1. `check` — runs on every push:
   - cargo fmt --check
   - cargo clippy -- -D warnings
   - cargo test (unit tests only, no integration)
   - cargo doc --no-deps

2. `test` — runs on every push:
   - cargo test --all (including integration tests)
   - Needs Rust toolchain with clippy component
   - Run on ubuntu-latest, macos-latest, windows-latest

3. `self-eval` (weekly scheduled + manual trigger):
   - Install forgetest from the workspace
   - Run the rust-basics eval set against a real LLM (use a secret ANTHROPIC_API_KEY)
   - Save results as a GitHub artifact
   - Compare against the last saved baseline
   - Post results as a comment on a tracking issue

4. `release` — on tag push (v*):
   - Build release binaries for linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64, windows-x86_64
   - Create GitHub release with binaries
   - Publish to crates.io

Use `dtolnay/rust-toolchain@stable` for Rust setup.
Use `actions/cache@v3` for Cargo registry + target dir caching.

Also create `.github/dependabot.yml` for cargo dependency updates.
```

---

### 7.3 — Benchmarks

```
Create `benches/` with criterion benchmarks.

`benches/parsing.rs`:
- Benchmark TOML eval set parsing for various sizes (10, 100, 1000 cases)
- Benchmark markdown code extraction on various inputs
- Benchmark test output parsing

`benches/scoring.rs`:
- Benchmark Pass@k computation for various n, c, k values
- Benchmark Score::compute for various result types
- Benchmark report comparison (regression detection)

`benches/compilation.rs`:
- Benchmark sandbox creation time
- Benchmark time to compile a simple "hello world" Rust program in sandbox
- Benchmark time to run a simple test suite in sandbox

These benchmarks serve dual purpose:
1. Track performance regressions in forgetest itself
2. Provide data points for the README ("compiles eval code in <500ms")

Add a `justfile` (or `Makefile`) with common tasks:
```
bench:        cargo bench
test:         cargo test --all
test-full:    cargo test --all -- --include-ignored  
lint:         cargo clippy -- -D warnings && cargo fmt --check
doc:          cargo doc --no-deps --open
install:      cargo install --path crates/forgetest-cli
```
```

---

### 7.4 — Crate Publishing Prep

```
Prepare all crates for publishing to crates.io:

1. Update each crate's Cargo.toml with:
   - `version = "0.1.0"`
   - `edition = "2021"`
   - `authors = ["Your Name <email>"]`
   - `description = "..."` (crate-specific)
   - `documentation = "https://docs.rs/forgetest-{name}"`
   - `repository = "https://github.com/YOU/forgetest"`
   - `license = "MIT OR Apache-2.0"`
   - `keywords = ["llm", "eval", "benchmark", "testing", "ai"]`
   - `categories = ["development-tools::testing", "command-line-utilities"]`
   - `readme = "../../README.md"` (point to root)

2. Ensure inter-crate dependencies use `version = "0.1.0"` (not path only):
   ```toml
   [dependencies]
   forgetest-core = { version = "0.1.0", path = "../forgetest-core" }
   ```

3. Run `cargo publish --dry-run` for each crate to verify.

4. Publishing order (respecting dependency graph):
   1. forgetest-core (no internal deps)
   2. forgetest-providers (depends on core)
   3. forgetest-runner (depends on core)
   4. forgetest-report (depends on core)
   5. forgetest-cli (depends on all)

5. Create a `release.sh` script that publishes in the correct order with delays between each (crates.io needs time to index).

6. Verify `cargo install forgetest-cli` works from the published crate (test post-publish).
```

---

## Quick Reference: Session Map

| Session | Focus | Key Output | Est. Time |
|---------|-------|------------|-----------|
| 1 | Scaffolding & Core | Workspace, data model, traits, TOML parser | 1–2 weeks |
| 2 | LLM Providers | Anthropic, OpenAI, Ollama integrations | 1 week |
| 3 | Sandboxed Runner | Compile, test, clippy in isolation | 1–2 weeks |
| 4 | Eval Engine | Orchestration, Pass@k, regression detect | 1 week |
| 5 | CLI & Reports | Clap CLI, HTML reports, SARIF output | 1–2 weeks |
| 6 | Eval Sets & Testing | 30 eval cases, E2E tests | 1 week |
| 7 | Polish & Launch | Docs, CI, benchmarks, publish | 1 week |

**Total estimated: 7–10 weeks of focused development**