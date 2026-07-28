//! Repository-suite command implementation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use forgetest_agents::{
    builtin_profile, doctor, doctor_container, doctor_verifier_container, BenchmarkLock,
    CommandProfile, DirectWorkspaceEnvironment, DockerAgentConfig, DockerProcessAgent,
    ProcessAgent,
};
use forgetest_core::agent::{AgentExecutor, AgentIdentity, AgentLimits, Grader};
use forgetest_core::repository_engine::{RepositoryEngine, RepositoryEngineConfig};
use forgetest_core::repository_report::{
    ContainerLimitsManifest, ExecutionPolicyManifest, ExecutionPolicyParameters,
};
use forgetest_core::suite::load_suite;
use forgetest_providers::config::{load_config_from, ForgetestConfig, RunnerType};
use forgetest_report::redaction::{redact_repository_report, RedactionOptions};
use forgetest_runner::{DockerRepositoryGrader, DockerVerifierConfig, LocalRepositoryGrader};
use sha2::{Digest, Sha256};

pub(crate) struct RepositoryPolicyOptions {
    pub profile: String,
    pub runner_type: RunnerType,
    pub verifier_image: String,
    pub agent_images: std::collections::BTreeMap<String, String>,
    pub trials: u32,
    pub parallelism: usize,
    pub agent_limits: AgentLimits,
}

pub(crate) struct RepositoryRunOptions {
    pub suite_path: PathBuf,
    pub agents: Option<String>,
    pub trials: u32,
    pub profile: String,
    pub benchmark_lock_path: Option<PathBuf>,
    pub max_agent_output_bytes: usize,
    pub agent_timeout_secs: Option<u64>,
    pub max_agent_tokens: Option<u64>,
    pub max_agent_cost_usd: Option<f64>,
    pub agent_retries: u32,
    pub parallelism: Option<usize>,
    pub output: Option<PathBuf>,
    pub format: String,
    pub config_path: Option<PathBuf>,
    pub runner_override: Option<String>,
}

pub(crate) fn build_execution_policy(
    config: &ForgetestConfig,
    options: RepositoryPolicyOptions,
) -> ExecutionPolicyManifest {
    let engine_defaults = RepositoryEngineConfig::default();
    let agent_container = (options.profile == "benchmark").then(|| {
        let limits = DockerAgentConfig::default();
        ContainerLimitsManifest {
            memory: limits.memory,
            cpus: limits.cpus,
            pids_limit: limits.pids_limit,
            tmpfs_size: limits.tmpfs_size,
        }
    });
    let verifier_defaults = DockerVerifierConfig::default();
    let verifier_container =
        (options.runner_type == RunnerType::Docker).then(|| ContainerLimitsManifest {
            memory: config.runner.memory.clone(),
            cpus: config.runner.cpus,
            pids_limit: config.runner.pids_limit,
            tmpfs_size: verifier_defaults.tmpfs_size.clone(),
        });
    let network = match (options.profile.as_str(), options.runner_type) {
        ("benchmark", _) => "agent=bridge;verifier=none",
        (_, RunnerType::Docker) => "agent=host;verifier=none",
        (_, RunnerType::Local) => "agent=host;verifier=host-trusted",
    };
    ExecutionPolicyManifest {
        schema_version: 1,
        profile: options.profile.clone(),
        agent_environment: if options.profile == "benchmark" {
            "docker".into()
        } else {
            "host-trusted".into()
        },
        verifier_environment: options.runner_type.to_string(),
        verifier_image: (options.runner_type == RunnerType::Docker)
            .then_some(options.verifier_image),
        network: network.into(),
        parameters: ExecutionPolicyParameters {
            trials: options.trials,
            parallelism: options.parallelism,
            agent_images: options.agent_images,
            agent_timeout_secs: options.agent_limits.timeout_secs,
            max_agent_output_bytes: options.agent_limits.max_output_bytes,
            max_agent_tokens: options.agent_limits.max_tokens,
            max_agent_cost_usd: options.agent_limits.max_cost_usd,
            agent_retries: options.agent_limits.max_retries,
            max_workspace_files: engine_defaults.max_workspace_files,
            max_workspace_bytes: engine_defaults.max_workspace_bytes,
            max_patch_bytes: engine_defaults.max_patch_bytes,
            verifier_max_output_bytes: verifier_defaults.max_output_bytes,
            agent_container,
            verifier_container,
        },
        digest: String::new(),
    }
    .sealed()
}

pub async fn execute(options: RepositoryRunOptions) -> Result<()> {
    let RepositoryRunOptions {
        suite_path,
        agents,
        trials,
        profile,
        benchmark_lock_path,
        max_agent_output_bytes,
        agent_timeout_secs,
        max_agent_tokens,
        max_agent_cost_usd,
        agent_retries,
        parallelism,
        output,
        format,
        config_path,
        runner_override,
    } = options;
    crate::commands::run::parse_formats(&format)?;
    anyhow::ensure!(trials > 0, "trials must be at least 1");
    anyhow::ensure!(
        max_agent_output_bytes > 0,
        "max-agent-output-bytes must be positive"
    );
    if let Some(timeout) = agent_timeout_secs {
        anyhow::ensure!(timeout > 0, "agent-timeout-secs must be positive");
    }
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
        matches!(profile.as_str(), "development" | "benchmark"),
        "unknown profile: {profile}; expected development or benchmark"
    );
    let requested_agents = parse_requested_agents(
        agents
            .as_deref()
            .context("--agents is required when --suite is used")?,
    )?;
    let suite = load_suite(&suite_path)?;
    let config = load_config_from(config_path.as_deref())?;
    let parallelism = parallelism.unwrap_or(config.parallelism);
    anyhow::ensure!(parallelism > 0, "parallelism must be at least 1");
    let output = output.unwrap_or_else(|| config.output_dir.clone());
    let raw_dir = output.join("raw");
    let public_dir = output.join("public");
    crate::commands::demo::ensure_fresh_evidence_directory(&raw_dir)?;
    crate::commands::demo::ensure_fresh_evidence_directory(&public_dir)?;
    let agent_timeout_secs = agent_timeout_secs.unwrap_or_else(|| {
        suite
            .tasks
            .iter()
            .map(|task| task.timeout_secs)
            .max()
            .unwrap_or(900)
    });
    let lock = benchmark_lock_path
        .as_deref()
        .map(BenchmarkLock::load)
        .transpose()?;

    let runner_type = match runner_override {
        Some(value) => value.parse::<RunnerType>().map_err(anyhow::Error::msg)?,
        None if profile == "benchmark" => RunnerType::Docker,
        None => config.runner.runner_type,
    };
    if profile == "benchmark" {
        anyhow::ensure!(
            runner_type == RunnerType::Docker,
            "benchmark profile requires the Docker verifier"
        );
        anyhow::ensure!(
            lock.is_some(),
            "benchmark profile requires --benchmark-lock"
        );
    }

    let verifier_image = lock
        .as_ref()
        .map(|lock| lock.verifier_image.clone())
        .unwrap_or_else(|| config.runner.docker_image.clone());
    let agent_images = lock
        .as_ref()
        .map(|lock| {
            requested_agents
                .iter()
                .filter_map(|requested| {
                    lock.agent(&requested.name)
                        .ok()
                        .map(|agent| (requested.name.clone(), agent.container_image.clone()))
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        })
        .unwrap_or_default();
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
            profile: profile.clone(),
            runner_type,
            verifier_image: verifier_image.clone(),
            agent_images,
            trials,
            parallelism,
            agent_limits: agent_limits.clone(),
        },
    );
    if let Some(lock) = &lock {
        lock.validate(
            &suite.digest,
            &policy.digest,
            &requested_agents
                .iter()
                .map(|agent| agent.name.as_str())
                .collect::<Vec<_>>(),
        )?;
        preflight_locked_environment(&requested_agents, lock).await?;
    }

    let agents = build_agents(&requested_agents, lock.as_ref(), profile.as_str())?;
    let grader: Arc<dyn Grader> = match runner_type {
        RunnerType::Local => Arc::new(LocalRepositoryGrader::new(4 * 1024 * 1024)),
        RunnerType::Docker => Arc::new(DockerRepositoryGrader::new(DockerVerifierConfig {
            image: verifier_image.clone(),
            memory: config.runner.memory.clone(),
            cpus: config.runner.cpus,
            pids_limit: config.runner.pids_limit,
            ..DockerVerifierConfig::default()
        })),
    };
    let engine = RepositoryEngine::new(
        Arc::new(DirectWorkspaceEnvironment),
        grader,
        RepositoryEngineConfig {
            trials,
            parallelism,
            output_dir: raw_dir.clone(),
            agent_limits,
            policy,
            ..RepositoryEngineConfig::default()
        },
    );

    eprintln!(
        "forgetest - Running {} repository tasks x {} agents x {} trials ({profile})",
        suite.tasks.len(),
        agents.len(),
        trials
    );
    let report = engine.run(&suite, agents).await?;
    crate::commands::demo::write_repository_outputs(&report, &raw_dir, &format)?;
    let public =
        redact_repository_report(&report, &public_redaction_options(&suite.root, &output))?;
    crate::commands::demo::write_repository_outputs(&public, &public_dir, &format)?;

    let passed = report
        .trials
        .iter()
        .filter(|trial| trial.status == forgetest_core::repository_report::TrialStatus::Passed)
        .count();
    eprintln!(
        "{passed}/{} trials passed; raw evidence: {}; public evidence: {}",
        report.trials.len(),
        raw_dir.display(),
        public_dir.display()
    );
    Ok(())
}

async fn preflight_locked_environment(
    requested: &[RequestedAgent],
    lock: &BenchmarkLock,
) -> Result<()> {
    doctor_verifier_container(&lock.verifier_image)
        .await
        .with_context(|| {
            format!(
                "benchmark preflight failed for verifier image {}",
                lock.verifier_image
            )
        })?;
    for requested in requested {
        let locked = lock.agent(&requested.name)?;
        let profile = profile_for(&requested.name, &locked.model, locked.effort.as_deref())?;
        locked.verify_profile(&profile)?;
        let observed = doctor_container(&profile, &locked.container_image)
            .await
            .with_context(|| {
                format!(
                    "benchmark preflight failed for '{}' image {}",
                    requested.name, locked.container_image
                )
            })?;
        locked.verify_container(&observed)?;
    }
    Ok(())
}

#[derive(Debug)]
struct RequestedAgent {
    name: String,
    model: Option<String>,
}

fn parse_requested_agents(value: &str) -> Result<Vec<RequestedAgent>> {
    let mut seen = std::collections::HashSet::new();
    let agents: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let mut parts = value.splitn(2, '/');
            let name = parts.next().unwrap_or_default().to_ascii_lowercase();
            anyhow::ensure!(
                matches!(name.as_str(), "codex" | "claude"),
                "unsupported repository agent: {name}; expected codex or claude"
            );
            anyhow::ensure!(seen.insert(name.clone()), "duplicate agent: {name}");
            Ok(RequestedAgent {
                name,
                model: parts.next().map(str::to_string),
            })
        })
        .collect::<Result<_>>()?;
    anyhow::ensure!(!agents.is_empty(), "--agents contains no agents");
    Ok(agents)
}

fn build_agents(
    requested: &[RequestedAgent],
    lock: Option<&BenchmarkLock>,
    profile: &str,
) -> Result<Vec<Arc<dyn AgentExecutor>>> {
    requested
        .iter()
        .map(|requested| {
            if profile == "benchmark" {
                let locked = lock
                    .context("benchmark lock is required")?
                    .agent(&requested.name)?;
                anyhow::ensure!(
                    requested
                        .model
                        .as_deref()
                        .is_none_or(|model| model == locked.model),
                    "requested model for '{}' differs from benchmark lock",
                    requested.name
                );
                let profile =
                    profile_for(&requested.name, &locked.model, locked.effort.as_deref())?;
                let identity = AgentIdentity {
                    adapter: requested.name.clone(),
                    adapter_version: locked.cli_version.clone(),
                    executable_sha256: Some(locked.executable_sha256.clone()),
                    model: locked.model.clone(),
                    configuration_digest: locked.configuration_digest.clone(),
                };
                Ok(Arc::new(DockerProcessAgent::new(
                    profile,
                    identity,
                    DockerAgentConfig {
                        image: locked.container_image.clone(),
                        ..DockerAgentConfig::default()
                    },
                )) as Arc<dyn AgentExecutor>)
            } else {
                let model = requested.model.as_deref().with_context(|| {
                    format!(
                        "development agent '{}' requires an explicit model as {}/MODEL",
                        requested.name, requested.name
                    )
                })?;
                let profile = profile_for(&requested.name, model, None)?;
                let doctor = doctor(&profile)?;
                anyhow::ensure!(
                    doctor.executable_found,
                    "{} executable was not found",
                    requested.name
                );
                let identity = AgentIdentity {
                    adapter: requested.name.clone(),
                    adapter_version: doctor
                        .version
                        .unwrap_or_else(|| "version unavailable".into()),
                    executable_sha256: doctor.executable_sha256,
                    model: model.into(),
                    configuration_digest: sha256_hex(
                        format!("{}\0{}\0development", requested.name, model).as_bytes(),
                    ),
                };
                Ok(Arc::new(ProcessAgent::new(profile, identity)) as Arc<dyn AgentExecutor>)
            }
        })
        .collect()
}

fn profile_for(name: &str, model: &str, effort: Option<&str>) -> Result<CommandProfile> {
    builtin_profile(name, model, effort)
}

fn public_redaction_options(suite_root: &Path, output: &Path) -> RedactionOptions {
    let mut path_replacements = vec![
        (suite_root.to_path_buf(), "$SUITE".into()),
        (output.to_path_buf(), "$OUTPUT".into()),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        path_replacements.push((PathBuf::from(home), "$HOME".into()));
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

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
