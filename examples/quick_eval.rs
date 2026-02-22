//! Quick eval example — minimal programmatic usage of forgetest.
//!
//! This example demonstrates how to use forgetest as a library to run
//! eval cases programmatically.
//!
//! ```bash
//! # Set your API key first:
//! export ANTHROPIC_API_KEY="your-key-here"
//!
//! # Run the example:
//! cargo run --example quick_eval
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use forgetest_core::engine::{EvalEngine, EvalEngineConfig, ModelSpec, NoopReporter};
use forgetest_core::parser;
use forgetest_providers::config::load_config;
use forgetest_providers::create_provider;
use forgetest_runner::LocalRunner;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load provider config from forgetest.toml
    let config = load_config()?;

    // Parse an eval set from a TOML file
    let eval_set = parser::parse_eval_set("eval-sets/rust-basics.toml".as_ref())?;
    println!("Loaded eval set: {} ({} cases)", eval_set.name, eval_set.cases.len());

    // Create a provider
    let provider_config = config
        .providers
        .get("anthropic")
        .expect("anthropic provider not configured");
    let provider = create_provider("anthropic", provider_config)?;

    let mut providers = HashMap::new();
    providers.insert("anthropic".to_string(), Arc::from(provider));

    // Configure the eval engine
    let engine_config = EvalEngineConfig {
        parallelism: 2,
        pass_k: vec![1],
        temperature: 0.0,
        ..Default::default()
    };

    // Create the sandboxed code runner
    let runner = Arc::new(LocalRunner::new(std::path::PathBuf::from(".forgetest-target")));

    let engine = EvalEngine::new(providers, runner, engine_config);

    // Define which models to evaluate
    let models = vec![ModelSpec {
        provider: "anthropic".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
    }];

    // Run the evaluation
    println!("\nRunning evaluation...\n");
    let report = engine.run(&eval_set, &models, &NoopReporter).await?;

    // Print results
    println!("Evaluation complete!");
    println!("  Results: {}", report.results.len());
    println!("  Duration: {}ms", report.duration_ms);

    for (model, stats) in &report.aggregate.per_model {
        let pass_1 = stats.pass_at_k.get(&1).copied().unwrap_or(0.0);
        println!(
            "  {model}: Pass@1={:.1}%, Compile={:.1}%, Tests={:.1}%",
            pass_1 * 100.0,
            stats.avg_compilation_rate * 100.0,
            stats.avg_test_pass_rate * 100.0,
        );
    }

    // Save the report
    report.save_json("quick_eval_results.json".as_ref())?;
    println!("\nResults saved to quick_eval_results.json");

    Ok(())
}
