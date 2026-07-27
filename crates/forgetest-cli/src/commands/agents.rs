//! External-agent inspection and benchmark locking.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use forgetest_agents::{
    builtin_profile, doctor as inspect, doctor_container, doctor_verifier_container,
    is_immutable_image, profile_configuration_digest, BenchmarkLock, CommandProfile, LockedAgent,
};
use forgetest_core::agent::AgentLimits;
use forgetest_core::suite::load_suite;
use forgetest_providers::config::{load_config_from, RunnerType};

use crate::commands::repository_run::{build_execution_policy, RepositoryPolicyOptions};

pub async fn doctor(agents: Option<String>, benchmark_lock: Option<PathBuf>) -> Result<()> {
    let default_agents;
    let requested = if agents.is_none() && benchmark_lock.is_none() {
        default_agents = "codex/MODEL,claude/MODEL".to_string();
        Some(default_agents.as_str())
    } else {
        agents.as_deref()
    };
    let requested: Vec<_> = requested
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();

    for requested in requested {
        let (name, model) = requested
            .split_once('/')
            .with_context(|| format!("agent must use NAME/MODEL syntax: {requested}"))?;
        let profile = builtin_profile(name, model, None)?;
        print_doctor_report(name, model, &profile)?;
    }

    if let Some(path) = benchmark_lock {
        verify_locked_images(&path).await?;
    }
    Ok(())
}

async fn verify_locked_images(path: &std::path::Path) -> Result<()> {
    let lock = BenchmarkLock::load(path)?;
    for agent in &lock.agents {
        let profile = builtin_profile(&agent.name, &agent.model, agent.effort.as_deref())?;
        agent.verify_profile(&profile)?;
        let observed = doctor_container(&profile, &agent.container_image)
            .await
            .with_context(|| {
                format!(
                    "failed to inspect '{}' image {}",
                    agent.name, agent.container_image
                )
            })?;
        agent.verify_container(&observed)?;
        println!(
            "{} locked image: verified ({})",
            agent.name, observed.version
        );
    }

    let verifier = doctor_verifier_container(&lock.verifier_image)
        .await
        .with_context(|| format!("failed to inspect verifier image {}", lock.verifier_image))?;
    println!("verifier image: verified ({})", verifier.version);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn lock(
    suite_path: PathBuf,
    agent_values: Vec<String>,
    effort_values: Vec<String>,
    verifier_image: String,
    output: PathBuf,
    force: bool,
    trials: u32,
    parallelism: Option<usize>,
    agent_timeout_secs: Option<u64>,
    max_agent_output_bytes: usize,
    max_agent_tokens: Option<u64>,
    max_agent_cost_usd: Option<f64>,
    agent_retries: u32,
    config_path: Option<PathBuf>,
) -> Result<()> {
    anyhow::ensure!(
        force || !output.exists(),
        "benchmark lock already exists: {}; pass --force to replace it",
        output.display()
    );
    anyhow::ensure!(trials > 0, "trials must be at least 1");
    anyhow::ensure!(
        max_agent_output_bytes > 0,
        "max-agent-output-bytes must be positive"
    );
    if let Some(tokens) = max_agent_tokens {
        anyhow::ensure!(tokens > 0, "max-agent-tokens must be positive");
    }
    if let Some(cost) = max_agent_cost_usd {
        anyhow::ensure!(
            cost.is_finite() && cost > 0.0,
            "max-agent-cost-usd must be a positive finite number"
        );
    }
    anyhow::ensure!(
        is_immutable_image(&verifier_image),
        "verifier image must use NAME@sha256:DIGEST with a complete SHA-256"
    );

    let suite = load_suite(&suite_path)?;
    let config = load_config_from(config_path.as_deref())?;
    let parallelism = parallelism.unwrap_or(config.parallelism);
    anyhow::ensure!(parallelism > 0, "parallelism must be at least 1");
    let agent_timeout_secs = agent_timeout_secs.unwrap_or_else(|| {
        suite
            .tasks
            .iter()
            .map(|task| task.timeout_secs)
            .max()
            .unwrap_or(900)
    });
    anyhow::ensure!(
        agent_timeout_secs > 0,
        "agent-timeout-secs must be positive"
    );
    let efforts = parse_efforts(&effort_values)?;
    let specs = parse_lock_agents(&agent_values)?;
    for name in efforts.keys() {
        anyhow::ensure!(
            specs.iter().any(|spec| &spec.name == name),
            "effort was provided for unrequested agent '{name}'"
        );
    }
    doctor_verifier_container(&verifier_image)
        .await
        .with_context(|| format!("failed to inspect verifier image {verifier_image}"))?;

    let mut locked_agents = Vec::with_capacity(specs.len());
    let mut agent_images = BTreeMap::new();
    for spec in specs {
        let effort = efforts.get(&spec.name).map(String::as_str);
        let profile = builtin_profile(&spec.name, &spec.model, effort)?;
        let observed = doctor_container(&profile, &spec.image)
            .await
            .with_context(|| {
                format!(
                    "failed to inspect '{}' agent image {}",
                    spec.name, spec.image
                )
            })?;
        let configuration_digest = profile_configuration_digest(&profile, &spec.image);
        agent_images.insert(spec.name.clone(), spec.image.clone());
        locked_agents.push(LockedAgent {
            name: spec.name,
            model: spec.model,
            cli_version: observed.version,
            executable_sha256: observed.executable_sha256,
            configuration_digest,
            container_image: spec.image,
            effort: effort.map(str::to_string),
        });
    }

    let agent_limits = AgentLimits {
        timeout_secs: agent_timeout_secs,
        max_output_bytes: max_agent_output_bytes,
        max_retries: agent_retries,
        max_tokens: max_agent_tokens,
        max_cost_usd: max_agent_cost_usd,
    };
    let policy = build_execution_policy(
        &config,
        RepositoryPolicyOptions {
            profile: "benchmark".into(),
            runner_type: RunnerType::Docker,
            verifier_image: verifier_image.clone(),
            agent_images,
            trials,
            parallelism,
            agent_limits,
        },
    );
    let lock = BenchmarkLock {
        schema_version: 1,
        created_at: chrono::Utc::now(),
        suite_digest: suite.digest,
        policy_digest: policy.digest,
        verifier_image,
        agents: locked_agents,
    };
    lock.save(&output)?;
    println!("Benchmark lock: {}", output.display());
    println!("  Suite SHA-256: {}", lock.suite_digest);
    println!("  Policy SHA-256: {}", lock.policy_digest);
    for agent in &lock.agents {
        println!(
            "  {} / {}: {} ({})",
            agent.name,
            agent.model,
            agent.cli_version,
            &agent.executable_sha256[..12]
        );
    }
    Ok(())
}

fn print_doctor_report(name: &str, model: &str, profile: &CommandProfile) -> Result<()> {
    let result = inspect(profile)?;
    println!("{name} / {model}");
    println!(
        "  Executable: {}",
        result
            .executable_path
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "missing".into())
    );
    println!(
        "  Version: {}",
        result.version.as_deref().unwrap_or("unavailable")
    );
    println!(
        "  SHA-256: {}",
        result.executable_sha256.as_deref().unwrap_or("unavailable")
    );
    for variable in &result.available_credentials {
        println!("  {variable}: available");
    }
    for variable in &result.missing_credentials {
        println!("  {variable}: missing");
    }
    Ok(())
}

#[derive(Debug)]
struct LockAgentSpec {
    name: String,
    model: String,
    image: String,
}

fn parse_lock_agents(values: &[String]) -> Result<Vec<LockAgentSpec>> {
    anyhow::ensure!(!values.is_empty(), "at least one --agent is required");
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|value| {
            let (agent, image) = value.split_once('=').with_context(|| {
                format!("agent lock entry must use NAME/MODEL=IMAGE@sha256:DIGEST: {value}")
            })?;
            let (name, model) = agent.split_once('/').with_context(|| {
                format!("agent lock entry must use NAME/MODEL=IMAGE@sha256:DIGEST: {value}")
            })?;
            let name = name.to_ascii_lowercase();
            anyhow::ensure!(
                matches!(name.as_str(), "codex" | "claude"),
                "unsupported repository agent: {name}; expected codex or claude"
            );
            anyhow::ensure!(!model.trim().is_empty(), "agent model is empty");
            anyhow::ensure!(seen.insert(name.clone()), "duplicate agent: {name}");
            anyhow::ensure!(
                is_immutable_image(image),
                "agent image for '{name}' must use NAME@sha256:DIGEST with a complete SHA-256"
            );
            Ok(LockAgentSpec {
                name,
                model: model.into(),
                image: image.into(),
            })
        })
        .collect()
}

fn parse_efforts(values: &[String]) -> Result<BTreeMap<String, String>> {
    let mut efforts = BTreeMap::new();
    for value in values {
        let (name, effort) = value
            .split_once('=')
            .with_context(|| format!("effort must use NAME=VALUE syntax: {value}"))?;
        let name = name.to_ascii_lowercase();
        anyhow::ensure!(
            matches!(name.as_str(), "codex" | "claude"),
            "unsupported effort agent: {name}"
        );
        builtin_profile(&name, "validation-model", Some(effort))?;
        anyhow::ensure!(
            efforts.insert(name.clone(), effort.into()).is_none(),
            "duplicate effort for agent: {name}"
        );
    }
    Ok(efforts)
}
