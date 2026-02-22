# Advanced Usage

## Programmatic API

Use forgetest as a library in your own Rust projects:

```rust
use forgetest_core::parser;
use forgetest_core::engine::{EvalEngine, EvalEngineConfig, ModelSpec, NoopReporter};
use forgetest_providers::config::load_config;
use forgetest_providers::create_provider;
use forgetest_runner::LocalRunner;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = load_config()?;
    let eval_set = parser::parse_eval_set("eval-sets/rust-basics.toml".as_ref())?;

    let provider = create_provider("anthropic", config.providers.get("anthropic").unwrap())?;
    let mut providers = HashMap::new();
    providers.insert("anthropic".to_string(), Arc::from(provider));

    let runner = Arc::new(LocalRunner::new(".forgetest-target".into()));
    let engine = EvalEngine::new(providers, runner, EvalEngineConfig::default());
    let models = vec![ModelSpec {
        provider: "anthropic".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
    }];

    let report = engine.run(&eval_set, &models, &NoopReporter).await?;

    println!("Results: {} cases evaluated", report.results.len());
    report.save_json("results/report.json".as_ref())?;
    Ok(())
}
```

## Custom Scoring

The default scoring weights (40% compile, 50% tests, 10% clippy) can be computed manually:

```rust
use forgetest_core::results::Score;

let score = Score::compute(&eval_result, &expectations);
println!("Overall: {:.1}%", score.overall * 100.0);
println!("Compilation: {:.1}%", score.compilation * 100.0);
println!("Tests: {:.1}%", score.test_pass_rate * 100.0);
println!("Clippy: {:.1}%", score.clippy * 100.0);
```

To implement custom scoring, compute your own weighted combination from the `EvalResult` fields.

## Sandbox Details

Each eval case runs in an isolated Cargo project:

1. A temporary directory is created with a fresh `Cargo.toml`
2. Generated code is written to `src/lib.rs`
3. Test code is appended to `src/lib.rs`
4. `cargo build` compiles the code
5. `cargo test` runs the test suite
6. `cargo clippy` checks for warnings
7. The temp directory is cleaned up

The sandbox:

- Uses a **shared target directory** for caching compiled dependencies
- Clears sensitive environment variables (`SSH_AUTH_SOCK`, `AWS_*`)
- Enforces configurable **timeouts** on all operations
- Supports adding **dependencies** (e.g., `tokio` for async eval cases)

## Adding Dependencies to Eval Cases

For cases that need external crates (like `tokio`), the sandbox automatically handles dependency management. Dependencies defined in the eval set configuration are added to the sandbox's `Cargo.toml`.

## Extending with New Providers

Implement the `LlmProvider` trait to add a new provider:

```rust
use async_trait::async_trait;
use forgetest_core::traits::{LlmProvider, GenerateRequest, GenerateResponse, ModelInfo};

pub struct MyProvider {
    api_key: String,
}

#[async_trait]
impl LlmProvider for MyProvider {
    fn name(&self) -> &str {
        "my-provider"
    }

    async fn generate(&self, request: &GenerateRequest) -> anyhow::Result<GenerateResponse> {
        // Call your API and return the response
        todo!()
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "my-model".to_string(),
            name: "My Model".to_string(),
            max_context: 128_000,
            cost_per_1k_input: 0.001,
            cost_per_1k_output: 0.002,
        }]
    }
}
```

## Report Post-Processing

Load and manipulate reports programmatically:

```rust
use forgetest_core::report::EvalReport;

let baseline = EvalReport::load_json("baseline.json".as_ref())?;
let current = EvalReport::load_json("current.json".as_ref())?;

let regression_report = current.compare(&baseline, 0.05);

if regression_report.has_regressions() {
    println!("{}", regression_report.to_markdown());
    std::process::exit(1);
}
```
