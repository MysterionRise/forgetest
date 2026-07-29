//! The `forgetest demo` command.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};

use forgetest_agents::{
    doctor_verifier_container, DirectWorkspaceEnvironment, ScriptedAgent, ScriptedEdit,
};
use forgetest_core::agent::{AgentIdentity, AgentLimits};
use forgetest_core::engine::{EvalEngine, EvalEngineConfig, ModelSpec, ProgressReporter};
use forgetest_core::model::{EvalCase, EvalSet, Expectations, Language};
use forgetest_core::repository_engine::{RepositoryEngine, RepositoryEngineConfig};
use forgetest_core::repository_report::{
    ContainerLimitsManifest, ExecutionPolicyManifest, ExecutionPolicyParameters,
};
use forgetest_core::results::EvalResult;
use forgetest_core::suite::load_suite;
use forgetest_core::traits::LlmProvider;
use forgetest_providers::config::ForgetestConfig;
use forgetest_providers::mock::MockProvider;
use forgetest_report::html::write_repository_html_report;
use forgetest_report::redaction::{redact_repository_report, RedactionOptions};
use forgetest_report::sarif::write_repository_sarif_report;
use forgetest_runner::{DockerRepositoryGrader, DockerVerifierConfig, LocalRepositoryGrader};
use sha2::{Digest, Sha256};

use crate::commands::run::{
    build_code_runner_with_target, build_run_manifest, ensure_docker_eval_sets_supported,
    print_summary, resolve_runner_type, save_report_outputs,
};

/// Console progress reporter for the offline demo.
struct DemoReporter;

impl ProgressReporter for DemoReporter {
    fn on_eval_start(&self, case_id: &str, model: &str, attempt: u32) {
        eprintln!("  Starting: {model} :: {case_id} (attempt {attempt})");
    }

    fn on_eval_complete(&self, result: &EvalResult) {
        let test_info = result
            .test_execution
            .as_ref()
            .map(|t| format!(" tests {}/{}", t.passed, t.passed + t.failed))
            .unwrap_or_default();
        eprintln!(
            "  Done: {} :: {} [{}] compile {}{} ({}ms)",
            result.model,
            result.case_id,
            result.attempt,
            if result.compilation.success {
                "OK"
            } else {
                "FAIL"
            },
            test_info,
            result.timing.total_ms
        );
    }

    fn on_eval_error(&self, case_id: &str, model: &str, error: &str) {
        eprintln!("  ERROR: {model} :: {case_id}: {error}");
    }

    fn on_set_complete(&self, total: usize, completed: usize, failed: usize, elapsed: Duration) {
        eprintln!(
            "\nDemo complete: {completed}/{total} succeeded, {failed} failed ({:.1}s)",
            elapsed.as_secs_f64()
        );
    }
}

pub async fn execute(output: PathBuf, format: String, runner: String, mode: String) -> Result<()> {
    crate::commands::run::parse_formats(&format)?;
    match mode.as_str() {
        "snippet" => execute_snippet(output, format, runner).await,
        "repository" => execute_repository(output, format, runner).await,
        other => anyhow::bail!("unknown demo mode: {other}; expected snippet or repository"),
    }
}

async fn execute_snippet(output: PathBuf, format: String, runner: String) -> Result<()> {
    let eval_set = demo_eval_set();
    let models = vec![ModelSpec {
        provider: "mock".into(),
        model: "mock-model".into(),
    }];
    let pass_k = vec![1];
    let temperature = 0.0;
    let config = ForgetestConfig::default();
    let runner_type = resolve_runner_type(Some(runner), &config)?;
    if runner_type == forgetest_providers::config::RunnerType::Docker {
        ensure_docker_eval_sets_supported(std::slice::from_ref(&eval_set))?;
    }

    let mut responses = HashMap::new();
    responses.insert(
        "add function".to_string(),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
    );
    responses.insert(
        "reverse string".to_string(),
        "pub fn reverse_string(s: &str) -> String { s.chars().rev().collect() }".to_string(),
    );

    let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();
    providers.insert("mock".into(), Arc::new(MockProvider::new(responses)));

    let target = demo_target_root(runner_type).join(format!(
        "forgetest-demo-target-{runner_type}-{}",
        std::process::id()
    ));
    let runner = build_code_runner_with_target(target, &config, runner_type);
    let manifest = build_run_manifest(
        &eval_set,
        &models,
        &pass_k,
        temperature,
        &config,
        runner_type,
    )?;
    let engine = EvalEngine::new(
        providers,
        runner,
        EvalEngineConfig {
            parallelism: if runner_type == forgetest_providers::config::RunnerType::Docker {
                1
            } else {
                2
            },
            pass_k,
            temperature,
            max_tokens: 1024,
            max_retries_per_case: 0,
            retry_delay: Duration::from_millis(0),
            system_prompt_override: None,
            manifest: Some(manifest),
        },
    );

    eprintln!(
        "forgetest v{} - Running deterministic offline demo ({runner_type} runner)",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!();
    let report = engine.run(&eval_set, &models, &DemoReporter).await?;
    print_summary(&report);
    save_report_outputs(&report, &output, &format)?;
    eprintln!(
        "\nDemo reports are deterministic mock-provider outputs, not paid/API model benchmark results."
    );
    Ok(())
}

async fn execute_repository(output: PathBuf, format: String, runner: String) -> Result<()> {
    let config = ForgetestConfig::default();
    let runner_type = resolve_runner_type(Some(runner), &config)?;
    let raw_dir = output.join("raw");
    let public_dir = output.join("public");
    ensure_fresh_evidence_directory(&raw_dir)?;
    ensure_fresh_evidence_directory(&public_dir)?;
    let fixture = tempfile::tempdir()?;
    write_repository_demo_suite(fixture.path())?;
    let suite = load_suite(&fixture.path().join("suite.toml"))?;
    if runner_type == forgetest_providers::config::RunnerType::Docker {
        doctor_verifier_container(&config.runner.docker_image)
            .await
            .with_context(|| {
                format!(
                    "Docker verifier preflight failed for image {}",
                    config.runner.docker_image
                )
            })?;
    }

    let agent = Arc::new(ScriptedAgent::new(
        AgentIdentity {
            adapter: "scripted".into(),
            adapter_version: env!("CARGO_PKG_VERSION").into(),
            executable_sha256: None,
            model: "deterministic-repository-demo".into(),
            configuration_digest: sha256_hex(b"forgetest-scripted-demo-v1"),
        },
        vec![ScriptedEdit {
            path: "src/lib.rs".into(),
            content: "pub fn add(a: i32, b: i32) -> i32 { a + b }\n".into(),
        }],
    ));
    let grader: Arc<dyn forgetest_core::agent::Grader> = match runner_type {
        forgetest_providers::config::RunnerType::Local => {
            Arc::new(LocalRepositoryGrader::new(1024 * 1024))
        }
        forgetest_providers::config::RunnerType::Docker => {
            Arc::new(DockerRepositoryGrader::new(DockerVerifierConfig {
                image: config.runner.docker_image.clone(),
                memory: config.runner.memory.clone(),
                cpus: config.runner.cpus,
                pids_limit: config.runner.pids_limit,
                ..DockerVerifierConfig::default()
            }))
        }
    };
    let engine_defaults = RepositoryEngineConfig::default();
    let verifier_defaults = DockerVerifierConfig::default();
    let verifier_max_output_bytes = if runner_type == forgetest_providers::config::RunnerType::Local
    {
        1024 * 1024
    } else {
        verifier_defaults.max_output_bytes
    };
    let policy = ExecutionPolicyManifest {
        schema_version: 1,
        profile: "offline-demo".into(),
        agent_environment: "host-trusted".into(),
        verifier_environment: runner_type.to_string(),
        verifier_image: (runner_type == forgetest_providers::config::RunnerType::Docker)
            .then(|| config.runner.docker_image.clone()),
        network: if runner_type == forgetest_providers::config::RunnerType::Docker {
            "agent=none;verifier=none".into()
        } else {
            "agent=none;verifier=host-trusted".into()
        },
        parameters: ExecutionPolicyParameters {
            trials: 1,
            parallelism: 1,
            agent_timeout_secs: 60,
            max_agent_output_bytes: 1024 * 1024,
            max_workspace_files: engine_defaults.max_workspace_files,
            max_workspace_bytes: engine_defaults.max_workspace_bytes,
            max_patch_bytes: engine_defaults.max_patch_bytes,
            verifier_max_output_bytes,
            verifier_container: (runner_type == forgetest_providers::config::RunnerType::Docker)
                .then(|| ContainerLimitsManifest {
                    memory: config.runner.memory.clone(),
                    cpus: config.runner.cpus,
                    pids_limit: config.runner.pids_limit,
                    tmpfs_size: verifier_defaults.tmpfs_size.clone(),
                }),
            ..ExecutionPolicyParameters::default()
        },
        digest: String::new(),
    }
    .sealed();
    let engine = RepositoryEngine::new(
        Arc::new(DirectWorkspaceEnvironment),
        grader,
        RepositoryEngineConfig {
            trials: 1,
            parallelism: 1,
            output_dir: raw_dir.clone(),
            agent_limits: AgentLimits {
                timeout_secs: 60,
                max_output_bytes: 1024 * 1024,
                ..AgentLimits::default()
            },
            policy,
            ..RepositoryEngineConfig::default()
        },
    );

    eprintln!(
        "forgetest v{} - Running deterministic repository-agent demo ({runner_type} verifier)",
        env!("CARGO_PKG_VERSION")
    );
    let report = engine.run(&suite, vec![agent]).await?;
    write_repository_outputs(&report, &raw_dir, &format)?;

    let public =
        redact_repository_report(&report, &public_redaction_options(&suite.root, &output))?;
    write_repository_outputs(&public, &public_dir, &format)?;
    eprintln!(
        "Repository demo reports are scripted offline evidence, not real-agent benchmark results."
    );
    let passed = report
        .trials
        .iter()
        .filter(|trial| trial.status == forgetest_core::repository_report::TrialStatus::Passed)
        .count();
    anyhow::ensure!(
        passed == report.trials.len(),
        "repository demo verification failed: {passed}/{} trials passed; inspect evidence in {}",
        report.trials.len(),
        raw_dir.display()
    );
    Ok(())
}

pub(crate) fn public_redaction_options(suite_root: &Path, output: &Path) -> RedactionOptions {
    let mut path_replacements = Vec::new();
    add_path_replacement(&mut path_replacements, suite_root, "$SUITE");
    add_path_replacement(&mut path_replacements, output, "$OUTPUT");
    if let Ok(current_dir) = std::env::current_dir() {
        add_path_replacement(&mut path_replacements, &current_dir, "$CWD");
    }
    let temp_dir = std::env::temp_dir();
    add_path_replacement(&mut path_replacements, &temp_dir, "$TMP");
    for (name, replacement) in [
        ("HOME", "$HOME"),
        ("USERPROFILE", "$HOME"),
        ("CARGO_HOME", "$CARGO_HOME"),
        ("RUSTUP_HOME", "$RUSTUP_HOME"),
    ] {
        if let Some(path) = std::env::var_os(name) {
            add_path_replacement(&mut path_replacements, &PathBuf::from(path), replacement);
        }
    }
    let secret_values = [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "FORGETEST_OPENAI_KEY",
        "FORGETEST_ANTHROPIC_KEY",
    ]
    .into_iter()
    .filter_map(|name| std::env::var(name).ok())
    .collect();
    RedactionOptions {
        path_replacements,
        secret_values,
    }
}

fn add_path_replacement(replacements: &mut Vec<(PathBuf, String)>, path: &Path, replacement: &str) {
    if path.as_os_str().is_empty() {
        return;
    }
    let normalized = path.components().collect::<PathBuf>();
    replacements.push((normalized.clone(), replacement.into()));
    if let Ok(canonical) = normalized.canonicalize() {
        if canonical != normalized {
            replacements.push((canonical, replacement.into()));
        }
    }
}

pub(crate) fn write_repository_outputs(
    report: &forgetest_core::repository_report::RepositoryReport,
    output: &Path,
    format: &str,
) -> Result<()> {
    std::fs::create_dir_all(output)?;
    for selected in crate::commands::run::parse_formats(format)? {
        match selected {
            "json" => report.save_json(&output.join("report.json"))?,
            "html" => write_repository_html_report(report, &output.join("report.html"))?,
            "sarif" => write_repository_sarif_report(report, &output.join("report.sarif"))?,
            _ => unreachable!("formats are validated"),
        }
    }
    forgetest_report::evidence::write_artifact_manifest(output)?;
    Ok(())
}

pub(crate) fn ensure_fresh_evidence_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    anyhow::ensure!(
        path.is_dir(),
        "evidence path is not a directory: {}",
        path.display()
    );
    anyhow::ensure!(
        std::fs::read_dir(path)?.next().transpose()?.is_none(),
        "evidence directory is not empty: {}; select a fresh output path",
        path.display()
    );
    Ok(())
}

fn write_repository_demo_suite(root: &Path) -> Result<()> {
    let task = root.join("tasks/fix-add");
    std::fs::create_dir_all(task.join("workspace/src"))?;
    std::fs::create_dir_all(task.join("grader/tests"))?;
    std::fs::write(
        root.join("suite.toml"),
        r#"schema_version = 2
id = "forgetest-repository-demo"
name = "forgetest Repository Demo"

[[tasks]]
id = "fix-add"
path = "tasks/fix-add"
"#,
    )?;
    std::fs::write(
        task.join("task.toml"),
        r#"schema_version = 1
id = "fix-add"
name = "Fix integer addition"
description = "A deterministic repository-level demo task."
category = "bug_fix"
language = "rust"
prompt = "prompt.md"
workspace = "workspace"
grader = "grader"
timeout_secs = 60
tags = ["demo"]

[verifier]
command = ["cargo", "test", "--all-targets", "--locked"]
timeout_secs = 60

[provenance]
kind = "authored"
license = "MIT OR Apache-2.0"
"#,
    )?;
    std::fs::write(
        task.join("prompt.md"),
        "Fix `add` so it returns the sum of two signed integers. Preserve the public API.",
    )?;
    std::fs::write(
        task.join("workspace/Cargo.toml"),
        r#"[package]
name = "forgetest-demo-repository"
version = "0.1.0"
edition = "2021"
"#,
    )?;
    std::fs::write(
        task.join("workspace/Cargo.lock"),
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "forgetest-demo-repository"
version = "0.1.0"
"#,
    )?;
    std::fs::write(
        task.join("workspace/src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a - b }\n",
    )?;
    std::fs::write(
        task.join("grader/tests/hidden.rs"),
        r#"use forgetest_demo_repository::add;

#[test]
fn handles_positive_and_negative_values() {
    assert_eq!(add(2, 3), 5);
    assert_eq!(add(-4, 1), -3);
}
"#,
    )?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn demo_target_root(runner_type: forgetest_providers::config::RunnerType) -> PathBuf {
    if runner_type == forgetest_providers::config::RunnerType::Docker
        && Path::new("/private/tmp").is_dir()
    {
        PathBuf::from("/private/tmp")
    } else {
        std::env::temp_dir()
    }
}

fn demo_eval_set() -> EvalSet {
    EvalSet {
        id: "forgetest-demo".into(),
        name: "forgetest Offline Demo".into(),
        description: "Small deterministic eval set for no-key portfolio review.".into(),
        default_language: Language::Rust,
        default_timeout_secs: 60,
        cases: vec![add_case(), reverse_case()],
    }
}

fn add_case() -> EvalCase {
    EvalCase {
        id: "add_function".into(),
        name: "Add function".into(),
        description: "Write a simple add function".into(),
        prompt: "Write a Rust add function `fn add(a: i32, b: i32) -> i32`.".into(),
        language: Some(Language::Rust),
        context: vec![],
        expectations: Expectations {
            test_file: Some(
                r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
        assert_eq!(add(-1, 1), 0);
    }
}
"#
                .to_string(),
            ),
            expected_functions: vec!["add".into()],
            ..Expectations::default()
        },
        tags: vec!["demo".into(), "basics".into()],
        dependencies: vec![],
        timeout_secs: Some(60),
        max_tokens: Some(256),
    }
}

fn reverse_case() -> EvalCase {
    EvalCase {
        id: "reverse_string".into(),
        name: "Reverse string".into(),
        description: "Write a function that reverses a string".into(),
        prompt: "Write a Rust reverse string function `fn reverse_string(s: &str) -> String`."
            .into(),
        language: Some(Language::Rust),
        context: vec![],
        expectations: Expectations {
            test_file: Some(
                r#"
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_reverse() {
        assert_eq!(reverse_string("hello"), "olleh");
        assert_eq!(reverse_string(""), "");
    }
}
"#
                .to_string(),
            ),
            expected_functions: vec!["reverse_string".into()],
            ..Expectations::default()
        },
        tags: vec!["demo".into(), "strings".into()],
        dependencies: vec![],
        timeout_secs: Some(60),
        max_tokens: Some(256),
    }
}
