//! The `forgetest run` command.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use forgetest_core::engine::{EvalEngine, EvalEngineConfig, ModelSpec, ProgressReporter};
use forgetest_core::parser;
use forgetest_core::results::EvalResult;
use forgetest_core::traits::LlmProvider;
use forgetest_providers::config::load_config_from;
use forgetest_providers::create_provider;
use forgetest_report::html::write_html_report;
use forgetest_report::sarif::write_sarif_report;
use forgetest_runner::LocalRunner;

/// Console progress reporter.
struct ConsoleReporter;

impl ProgressReporter for ConsoleReporter {
    fn on_eval_start(&self, case_id: &str, model: &str, attempt: u32) {
        eprintln!("  Starting: {model} :: {case_id} (attempt {attempt})");
    }

    fn on_eval_complete(&self, result: &EvalResult) {
        let compile_icon = if result.compilation.success {
            "OK"
        } else {
            "FAIL"
        };
        let test_info = match &result.test_execution {
            Some(t) => format!(" tests {}/{}", t.passed, t.passed + t.failed),
            None => String::new(),
        };
        eprintln!(
            "  Done: {} :: {} [{}] compile {}{} ({}ms)",
            result.model,
            result.case_id,
            result.attempt,
            compile_icon,
            test_info,
            result.timing.total_ms,
        );
    }

    fn on_eval_error(&self, case_id: &str, model: &str, error: &str) {
        eprintln!("  ERROR: {model} :: {case_id}: {error}");
    }

    fn on_set_complete(&self, total: usize, completed: usize, failed: usize, elapsed: Duration) {
        eprintln!(
            "\nComplete: {completed}/{total} succeeded, {failed} failed ({:.1}s)",
            elapsed.as_secs_f64()
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    eval_set_path: PathBuf,
    models_str: Option<String>,
    pass_k_str: String,
    parallelism: usize,
    temperature: f64,
    output: PathBuf,
    format: String,
    filter: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<()> {
    // Validate inputs
    anyhow::ensure!(parallelism >= 1, "parallelism must be at least 1");
    anyhow::ensure!(
        (0.0..=2.0).contains(&temperature),
        "temperature must be between 0.0 and 2.0"
    );

    // Load config
    let config = load_config_from(config_path.as_deref())?;

    // Load eval set
    let mut eval_sets = if eval_set_path.is_dir() {
        parser::load_eval_directory(&eval_set_path)?
    } else {
        vec![parser::parse_eval_set(&eval_set_path)?]
    };

    // Apply tag filter
    if let Some(filter_tags) = &filter {
        let tags: Vec<&str> = filter_tags.split(',').map(|s| s.trim()).collect();
        for set in &mut eval_sets {
            set.cases
                .retain(|c| c.tags.iter().any(|t| tags.contains(&t.as_str())));
        }
    }

    // Parse models
    let models: Vec<ModelSpec> = if let Some(m) = &models_str {
        m.split(',')
            .map(|s| {
                let parts: Vec<&str> = s.trim().splitn(2, '/').collect();
                if parts.len() == 2 {
                    ModelSpec {
                        provider: parts[0].to_string(),
                        model: parts[1].to_string(),
                    }
                } else {
                    ModelSpec {
                        provider: config.default_provider.clone(),
                        model: parts[0].to_string(),
                    }
                }
            })
            .collect()
    } else {
        vec![ModelSpec {
            provider: config.default_provider.clone(),
            model: config.default_model.clone(),
        }]
    };

    // Parse Pass@k values
    let pass_k: Vec<u32> = pass_k_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("invalid pass@k value: '{}'", s.trim()))
        })
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(!pass_k.is_empty(), "pass@k must have at least one value");
    anyhow::ensure!(
        pass_k.iter().all(|&k| k >= 1),
        "pass@k values must be at least 1"
    );

    // Warn about deterministic sampling with Pass@k > 1
    let max_k = pass_k.iter().copied().max().unwrap_or(1);
    if max_k > 1 && temperature == 0.0 {
        eprintln!(
            "Warning: Using Pass@k={max_k} with temperature=0.0. \
             Consider setting --temperature 0.8 for diverse samples."
        );
    }

    // Create providers
    let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
    for model_spec in &models {
        if providers.contains_key(&model_spec.provider) {
            continue;
        }
        if let Some(pconfig) = config.providers.get(&model_spec.provider) {
            let provider = create_provider(&model_spec.provider, pconfig)?;
            providers.insert(model_spec.provider.clone(), Arc::from(provider));
        } else {
            anyhow::bail!(
                "provider '{}' not found in config. Available: {:?}",
                model_spec.provider,
                config.providers.keys().collect::<Vec<_>>()
            );
        }
    }

    let engine_config = EvalEngineConfig {
        parallelism,
        pass_k: pass_k.clone(),
        temperature,
        max_tokens: 4096,
        max_retries_per_case: config.max_retries,
        retry_delay: Duration::from_millis(config.retry_delay_ms),
        system_prompt_override: None,
    };

    // Create the sandboxed code runner
    let shared_target = output.join(".forgetest-target");
    let runner = Arc::new(LocalRunner::new(shared_target));

    let engine = EvalEngine::new(providers, runner, engine_config);
    let reporter = ConsoleReporter;

    for eval_set in &eval_sets {
        let case_count = eval_set.cases.len();
        let model_count = models.len();
        let max_k = pass_k.iter().copied().max().unwrap_or(1);
        eprintln!(
            "forgetest v0.1.0 — Running {} eval cases x {} models x {} attempts",
            case_count, model_count, max_k
        );
        eprintln!();

        let report = engine.run(eval_set, &models, &reporter).await?;

        // Print summary table
        print_summary(&report);

        // Save outputs
        std::fs::create_dir_all(&output)?;
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H%M%S");

        let formats: Vec<&str> = if format == "all" {
            vec!["json", "html", "sarif"]
        } else {
            format.split(',').collect()
        };

        for fmt in &formats {
            match *fmt {
                "json" => {
                    let path = output.join(format!("report-{timestamp}.json"));
                    report.save_json(&path)?;
                    eprintln!("Results saved to: {}", path.display());
                }
                "html" => {
                    let path = output.join(format!("report-{timestamp}.html"));
                    write_html_report(&report, &path)?;
                    eprintln!("HTML report: {}", path.display());
                }
                "sarif" => {
                    let path = output.join(format!("report-{timestamp}.sarif"));
                    write_sarif_report(&report, &path)?;
                    eprintln!("SARIF report: {}", path.display());
                }
                _ => {
                    eprintln!("Unknown format: {fmt}");
                }
            }
        }
    }

    Ok(())
}

fn print_summary(report: &forgetest_core::report::EvalReport) {
    use comfy_table::{Cell, Table};

    let mut table = Table::new();
    table.set_header(vec![
        "Model",
        "Pass@1",
        "Compile %",
        "Test Pass %",
        "Cost",
        "Latency",
    ]);

    for (model, stats) in &report.aggregate.per_model {
        let pass_1 = stats.pass_at_k.get(&1).copied().unwrap_or(0.0);
        table.add_row(vec![
            Cell::new(model),
            Cell::new(format!("{:.1}%", pass_1 * 100.0)),
            Cell::new(format!("{:.1}%", stats.avg_compilation_rate * 100.0)),
            Cell::new(format!("{:.1}%", stats.avg_test_pass_rate * 100.0)),
            Cell::new(format!("${:.4}", stats.total_cost_usd)),
            Cell::new(format!("{}ms", stats.avg_latency_ms)),
        ]);
    }

    eprintln!("\n{table}");
}
