//! The `forgetest run` command.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;

use forgetest_core::engine::{EvalEngine, EvalEngineConfig, ModelSpec, ProgressReporter};
use forgetest_core::model::EvalSet;
use forgetest_core::parser;
use forgetest_core::report::{
    stable_hash_hex, EvalReport, ModelManifest, RunManifest, RunnerManifest,
};
use forgetest_core::results::EvalResult;
use forgetest_core::traits::{CodeRunner, LlmProvider};
use forgetest_providers::config::{
    load_config_from, ForgetestConfig, ProviderConfig, RunnerConfig, RunnerType,
    UNCONFIGURED_MODEL_ID,
};
use forgetest_providers::create_provider;
use forgetest_report::html::write_html_report;
use forgetest_report::sarif::write_sarif_report;
use forgetest_runner::{
    ensure_docker_dependency_allowed, DockerRunner, DockerRunnerConfig, LocalRunner,
};

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
    parallelism: Option<usize>,
    temperature: Option<f64>,
    output: Option<PathBuf>,
    format: String,
    filter: Option<String>,
    config_path: Option<PathBuf>,
    runner_override: Option<String>,
) -> Result<()> {
    parse_formats(&format)?;
    // Resolve config defaults only when the corresponding CLI flag is absent.
    let config = load_config_from(config_path.as_deref())?;
    let parallelism = parallelism.unwrap_or(config.parallelism);
    let temperature = temperature.unwrap_or(config.default_temperature);
    let output = output.unwrap_or_else(|| config.output_dir.clone());

    // Validate effective inputs
    anyhow::ensure!(parallelism >= 1, "parallelism must be at least 1");
    anyhow::ensure!(
        (0.0..=2.0).contains(&temperature),
        "temperature must be between 0.0 and 2.0"
    );

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
    ensure_no_validation_errors(&eval_sets)?;

    let runner_type = resolve_runner_type(runner_override, &config)?;
    if runner_type == RunnerType::Docker {
        ensure_docker_eval_sets_supported(&eval_sets)?;
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
    ensure_models_are_configured(&models)?;

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

    // Create the selected code runner.
    let runner = build_code_runner(&output, &config, runner_type);

    let reporter = ConsoleReporter;

    for eval_set in &eval_sets {
        let case_count = eval_set.cases.len();
        let model_count = models.len();
        let max_k = pass_k.iter().copied().max().unwrap_or(1);
        eprintln!(
            "forgetest v{} - Running {} eval cases x {} models x {} attempts",
            env!("CARGO_PKG_VERSION"),
            case_count,
            model_count,
            max_k
        );
        eprintln!();

        let manifest = build_run_manifest(
            eval_set,
            &models,
            &pass_k,
            temperature,
            &config,
            runner_type,
        )?;
        let engine_config = EvalEngineConfig {
            parallelism,
            pass_k: pass_k.clone(),
            temperature,
            max_tokens: 4096,
            max_retries_per_case: config.max_retries,
            retry_delay: Duration::from_millis(config.retry_delay_ms),
            system_prompt_override: None,
            manifest: Some(manifest),
        };
        let engine = EvalEngine::new(providers.clone(), Arc::clone(&runner), engine_config);
        let report = engine.run(eval_set, &models, &reporter).await?;

        // Print summary table
        print_summary(&report);

        // Save outputs
        save_report_outputs(&report, &output, &format)?;
    }

    Ok(())
}

pub(crate) fn save_report_outputs(
    report: &EvalReport,
    output: &std::path::Path,
    format: &str,
) -> Result<()> {
    std::fs::create_dir_all(output)?;
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H%M%S");
    let formats = parse_formats(format)?;

    for fmt in &formats {
        match *fmt {
            "json" => {
                let path = output.join(format!("report-{timestamp}.json"));
                report.save_json(&path)?;
                eprintln!("Results saved to: {}", path.display());
            }
            "html" => {
                let path = output.join(format!("report-{timestamp}.html"));
                write_html_report(report, &path)?;
                eprintln!("HTML report: {}", path.display());
            }
            "sarif" => {
                let path = output.join(format!("report-{timestamp}.sarif"));
                write_sarif_report(report, &path)?;
                eprintln!("SARIF report: {}", path.display());
            }
            _ => unreachable!("formats are validated by parse_formats"),
        }
    }

    Ok(())
}

pub(crate) fn parse_formats(format: &str) -> Result<Vec<&str>> {
    let formats: Vec<&str> = if format == "all" {
        vec!["json", "html", "sarif"]
    } else {
        format.split(',').map(str::trim).collect()
    };
    anyhow::ensure!(
        !formats.is_empty(),
        "at least one output format is required"
    );
    for fmt in &formats {
        anyhow::ensure!(
            matches!(*fmt, "json" | "html" | "sarif"),
            "unknown output format: {fmt}"
        );
    }
    Ok(formats)
}

pub(crate) fn resolve_runner_type(
    runner_override: Option<String>,
    config: &ForgetestConfig,
) -> Result<RunnerType> {
    match runner_override {
        Some(runner) => runner
            .parse::<RunnerType>()
            .map_err(|e| anyhow::anyhow!("{e}")),
        None => Ok(config.runner.runner_type),
    }
}

pub(crate) fn build_code_runner(
    output: &Path,
    config: &ForgetestConfig,
    runner_type: RunnerType,
) -> Arc<dyn CodeRunner> {
    build_code_runner_with_target(output.join(".forgetest-target"), config, runner_type)
}

pub(crate) fn build_code_runner_with_target(
    shared_target: PathBuf,
    config: &ForgetestConfig,
    runner_type: RunnerType,
) -> Arc<dyn CodeRunner> {
    match runner_type {
        RunnerType::Local => Arc::new(LocalRunner::new(shared_target)),
        RunnerType::Docker => Arc::new(DockerRunner::new(
            shared_target,
            DockerRunnerConfig {
                image: config.runner.docker_image.clone(),
                memory: config.runner.memory.clone(),
                cpus: config.runner.cpus,
                pids_limit: config.runner.pids_limit,
                network: config.runner.network.clone(),
                ..DockerRunnerConfig::default()
            },
        )),
    }
}

pub(crate) fn ensure_docker_eval_sets_supported(eval_sets: &[EvalSet]) -> Result<()> {
    for eval_set in eval_sets {
        for case in &eval_set.cases {
            for dep in &case.dependencies {
                ensure_docker_dependency_allowed(dep).with_context(|| {
                    format!(
                        "Docker runner dependency preflight failed for eval set '{}' case '{}'",
                        eval_set.id, case.id
                    )
                })?;
            }
        }
    }
    Ok(())
}

pub(crate) fn print_summary(report: &EvalReport) {
    use comfy_table::{Cell, Table};

    let mut table = Table::new();
    table.set_header(vec![
        "Model",
        "Pass@1",
        "Compile %",
        "Test Pass %",
        "Cost",
        "Avg Trial",
    ]);

    for (model, stats) in &report.aggregate.per_model {
        let pass_1 = stats.pass_at_k.get(&1).copied().unwrap_or(0.0);
        table.add_row(vec![
            Cell::new(model),
            Cell::new(format!("{:.1}%", pass_1 * 100.0)),
            Cell::new(format!("{:.1}%", stats.avg_compilation_rate * 100.0)),
            Cell::new(format!("{:.1}%", stats.avg_test_pass_rate * 100.0)),
            Cell::new(format!("${:.4}", stats.total_cost_usd)),
            Cell::new(format!("{}ms", stats.avg_trial_duration_ms)),
        ]);
    }

    eprintln!("\n{table}");
}

pub(crate) fn build_run_manifest(
    eval_set: &forgetest_core::model::EvalSet,
    models: &[ModelSpec],
    pass_k: &[u32],
    temperature: f64,
    config: &ForgetestConfig,
    runner_type: RunnerType,
) -> Result<RunManifest> {
    let eval_set_json =
        serde_json::to_string(eval_set).context("failed to serialize eval set for manifest")?;
    let mut case_hashes = BTreeMap::new();
    for case in &eval_set.cases {
        let case_json =
            serde_json::to_string(case).context("failed to serialize eval case for manifest")?;
        case_hashes.insert(case.id.clone(), stable_hash_hex(&case_json));
    }

    let docker_image =
        (runner_type == RunnerType::Docker).then(|| config.runner.docker_image.clone());
    let docker_image_digest = docker_image.as_deref().and_then(|image| {
        command_stdout(
            "docker",
            &["image", "inspect", "--format", "{{.Id}}", image],
        )
    });

    Ok(RunManifest {
        schema_version: 2,
        hash_algorithm: "sha256".into(),
        forgetest_version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha: command_stdout("git", &["rev-parse", "HEAD"]),
        git_dirty: command_stdout(
            "git",
            &["status", "--porcelain", "--untracked-files=normal"],
        )
        .map(|status| !status.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["status", "--porcelain", "--untracked-files=normal"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| !output.stdout.is_empty())
        }),
        rustc_version: command_stdout("rustc", &["--version"]),
        cargo_version: command_stdout("cargo", &["--version"]),
        runner: RunnerManifest {
            runner_type: runner_type.to_string(),
            docker_image,
            docker_image_digest,
        },
        eval_set_hash: stable_hash_hex(&eval_set_json),
        case_hashes,
        models: models
            .iter()
            .map(|m| ModelManifest {
                provider: m.provider.clone(),
                model: m.model.clone(),
            })
            .collect(),
        pass_k: pass_k.to_vec(),
        temperature,
        created_at: chrono::Utc::now(),
        config_hash: canonical_config_hash(config)?,
    })
}

#[derive(Serialize)]
struct CanonicalConfig<'a> {
    providers: BTreeMap<&'a str, CanonicalProvider<'a>>,
    default_provider: &'a str,
    default_model: &'a str,
    default_temperature: f64,
    max_retries: u32,
    retry_delay_ms: u64,
    parallelism: usize,
    output_dir: &'a Path,
    runner: &'a RunnerConfig,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum CanonicalProvider<'a> {
    OpenAI {
        credential_configured: bool,
        base_url: &'a Option<String>,
        org_id: &'a Option<String>,
    },
    Anthropic {
        credential_configured: bool,
        base_url: &'a Option<String>,
    },
    Ollama {
        base_url: &'a str,
    },
}

fn canonical_config_hash(config: &ForgetestConfig) -> Result<String> {
    let providers = config
        .providers
        .iter()
        .map(|(name, provider)| {
            let provider = match provider {
                ProviderConfig::OpenAI {
                    api_key,
                    base_url,
                    org_id,
                } => CanonicalProvider::OpenAI {
                    credential_configured: !api_key.is_empty(),
                    base_url,
                    org_id,
                },
                ProviderConfig::Anthropic { api_key, base_url } => CanonicalProvider::Anthropic {
                    credential_configured: !api_key.is_empty(),
                    base_url,
                },
                ProviderConfig::Ollama { base_url } => CanonicalProvider::Ollama { base_url },
            };
            (name.as_str(), provider)
        })
        .collect();
    let canonical = CanonicalConfig {
        providers,
        default_provider: &config.default_provider,
        default_model: &config.default_model,
        default_temperature: config.default_temperature,
        max_retries: config.max_retries,
        retry_delay_ms: config.retry_delay_ms,
        parallelism: config.parallelism,
        output_dir: &config.output_dir,
        runner: &config.runner,
    };
    Ok(stable_hash_hex(&serde_json::to_string(&canonical)?))
}

fn ensure_no_validation_errors(eval_sets: &[forgetest_core::model::EvalSet]) -> Result<()> {
    let errors: Vec<_> = eval_sets
        .iter()
        .flat_map(parser::validate_eval_set_errors)
        .collect();
    if errors.is_empty() {
        return Ok(());
    }

    let summary = errors
        .iter()
        .map(|e| match &e.case_id {
            Some(case_id) => format!("{case_id}: {}", e.message),
            None => e.message.clone(),
        })
        .collect::<Vec<_>>()
        .join("; ");
    anyhow::bail!("validation failed: {summary}");
}

fn ensure_models_are_configured(models: &[ModelSpec]) -> Result<()> {
    anyhow::ensure!(!models.is_empty(), "at least one model must be selected");
    for model in models {
        anyhow::ensure!(
            !model.provider.trim().is_empty(),
            "model provider cannot be empty"
        );
        anyhow::ensure!(!model.model.trim().is_empty(), "model ID cannot be empty");
        anyhow::ensure!(
            model.model != UNCONFIGURED_MODEL_ID,
            "no model configured; pass --models provider/model-id or set default_model in the config"
        );
    }
    Ok(())
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unconfigured_default_model() {
        let error = ensure_models_are_configured(&[ModelSpec {
            provider: "anthropic".into(),
            model: forgetest_providers::config::UNCONFIGURED_MODEL_ID.into(),
        }])
        .unwrap_err();

        assert!(error.to_string().contains("no model configured"));
        assert!(error.to_string().contains("--models"));
    }

    #[test]
    fn config_hash_is_canonical_and_does_not_hash_secrets() {
        let openai = ProviderConfig::OpenAI {
            api_key: "first-secret".into(),
            base_url: Some("https://api.example.invalid".into()),
            org_id: Some("org".into()),
        };
        let anthropic = ProviderConfig::Anthropic {
            api_key: "second-secret".into(),
            base_url: None,
        };
        let mut first = ForgetestConfig::default();
        first.providers.insert("openai".into(), openai.clone());
        first
            .providers
            .insert("anthropic".into(), anthropic.clone());
        let mut reordered = ForgetestConfig::default();
        reordered.providers.insert(
            "anthropic".into(),
            ProviderConfig::Anthropic {
                api_key: "changed-secret".into(),
                base_url: None,
            },
        );
        reordered.providers.insert(
            "openai".into(),
            ProviderConfig::OpenAI {
                api_key: "another-secret".into(),
                base_url: Some("https://api.example.invalid".into()),
                org_id: Some("org".into()),
            },
        );

        assert_eq!(
            canonical_config_hash(&first).unwrap(),
            canonical_config_hash(&reordered).unwrap()
        );

        reordered.providers.insert(
            "openai".into(),
            ProviderConfig::OpenAI {
                api_key: "another-secret".into(),
                base_url: Some("https://different.example.invalid".into()),
                org_id: Some("org".into()),
            },
        );
        assert_ne!(
            canonical_config_hash(&first).unwrap(),
            canonical_config_hash(&reordered).unwrap()
        );
    }
}
