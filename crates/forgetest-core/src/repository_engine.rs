//! Trusted lifecycle orchestration for repository-level agent trials.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::{stream, StreamExt};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::agent::{
    AgentEvent, AgentExecutor, AgentLimits, AgentRequest, AgentTerminationReason, AgentUsage,
    EventSink, GradeCheckRequest, GradeRequest, Grader, WorkspaceEnvironment,
};
use crate::repository_report::{
    ExecutionPolicyManifest, ExecutionPolicyParameters, FileChange, FileChangeKind,
    GraderCheckKind, RepositoryReport, RepositorySuiteSummary, TrialResult, TrialStatus,
};
use crate::suite::{ResolvedRepositoryTask, ResolvedSuite};

/// Limits and output locations for a repository evaluation run.
#[derive(Debug, Clone)]
pub struct RepositoryEngineConfig {
    pub trials: u32,
    pub parallelism: usize,
    pub output_dir: PathBuf,
    pub agent_limits: AgentLimits,
    pub max_workspace_files: usize,
    pub max_workspace_bytes: u64,
    pub max_patch_bytes: usize,
    pub policy: ExecutionPolicyManifest,
}

impl Default for RepositoryEngineConfig {
    fn default() -> Self {
        Self {
            trials: 1,
            parallelism: 1,
            output_dir: PathBuf::from("./forgetest-results/raw"),
            agent_limits: AgentLimits::default(),
            max_workspace_files: 10_000,
            max_workspace_bytes: 64 * 1024 * 1024,
            max_patch_bytes: 4 * 1024 * 1024,
            policy: ExecutionPolicyManifest {
                schema_version: 1,
                profile: "development".into(),
                agent_environment: "host".into(),
                verifier_environment: "local".into(),
                verifier_image: None,
                network: "unspecified".into(),
                parameters: ExecutionPolicyParameters {
                    trials: 1,
                    parallelism: 1,
                    agent_timeout_secs: 900,
                    max_agent_output_bytes: 4 * 1024 * 1024,
                    max_workspace_files: 10_000,
                    max_workspace_bytes: 64 * 1024 * 1024,
                    max_patch_bytes: 4 * 1024 * 1024,
                    verifier_max_output_bytes: 4 * 1024 * 1024,
                    ..ExecutionPolicyParameters::default()
                },
                digest: String::new(),
            }
            .sealed(),
        }
    }
}

/// Coordinates external agents and an independent grader.
pub struct RepositoryEngine {
    environment: Arc<dyn WorkspaceEnvironment>,
    grader: Arc<dyn Grader>,
    config: RepositoryEngineConfig,
}

impl RepositoryEngine {
    pub fn new(
        environment: Arc<dyn WorkspaceEnvironment>,
        grader: Arc<dyn Grader>,
        config: RepositoryEngineConfig,
    ) -> Self {
        Self {
            environment,
            grader,
            config,
        }
    }

    /// Execute every task/agent/trial combination and persist partial evidence.
    pub async fn run(
        &self,
        suite: &ResolvedSuite,
        agents: Vec<Arc<dyn AgentExecutor>>,
    ) -> Result<RepositoryReport> {
        anyhow::ensure!(self.config.trials > 0, "trials must be at least 1");
        anyhow::ensure!(
            self.config.parallelism > 0,
            "parallelism must be at least 1"
        );
        anyhow::ensure!(
            self.config.policy.verify_digest(),
            "execution policy digest does not match its recorded parameters"
        );
        let policy = &self.config.policy.parameters;
        anyhow::ensure!(
            policy.trials == self.config.trials
                && policy.parallelism == self.config.parallelism
                && policy.agent_timeout_secs == self.config.agent_limits.timeout_secs
                && policy.max_agent_output_bytes == self.config.agent_limits.max_output_bytes
                && policy.max_agent_tokens == self.config.agent_limits.max_tokens
                && policy.max_agent_cost_usd == self.config.agent_limits.max_cost_usd
                && policy.agent_retries == self.config.agent_limits.max_retries
                && policy.max_workspace_files == self.config.max_workspace_files
                && policy.max_workspace_bytes == self.config.max_workspace_bytes
                && policy.max_patch_bytes == self.config.max_patch_bytes,
            "execution policy parameters do not match repository engine configuration"
        );
        anyhow::ensure!(!agents.is_empty(), "at least one agent is required");
        ensure_private_directory(&self.config.output_dir)?;
        ensure_private_directory(&self.config.output_dir.join("trials"))?;

        let start = Instant::now();
        let capacity = suite.tasks.len() * agents.len() * self.config.trials as usize;
        let mut jobs = Vec::with_capacity(capacity);
        for agent in agents {
            for task in &suite.tasks {
                for trial_index in 1..=self.config.trials {
                    let order = jobs.len();
                    jobs.push((order, task, Arc::clone(&agent), trial_index));
                }
            }
        }

        let mut completed = stream::iter(jobs)
            .map(|(order, task, agent, trial_index)| async move {
                (order, self.run_trial(task, agent, trial_index).await)
            })
            .buffer_unordered(self.config.parallelism);
        let mut ordered_results = Vec::with_capacity(capacity);
        while let Some((order, result)) = completed.next().await {
            self.persist_trial(&result)?;
            ordered_results.push((order, result));
            ordered_results.sort_by_key(|(order, _)| *order);
            let partial = ordered_results
                .iter()
                .map(|(_, result)| result.clone())
                .collect::<Vec<_>>();
            self.persist_partial_report(suite, &partial, start.elapsed())?;
        }
        ordered_results.sort_by_key(|(order, _)| *order);
        let results = ordered_results
            .into_iter()
            .map(|(_, result)| result)
            .collect();

        Ok(RepositoryReport::new(
            suite_summary(suite),
            self.config.policy.clone(),
            results,
            start.elapsed().as_millis() as u64,
        ))
    }

    async fn run_trial(
        &self,
        task: &ResolvedRepositoryTask,
        agent: Arc<dyn AgentExecutor>,
        trial_index: u32,
    ) -> TrialResult {
        let trial_start = Instant::now();
        let trial_id = Uuid::new_v4();
        let trial_dir = self
            .config
            .output_dir
            .join("trials")
            .join(trial_id.to_string());
        let trace_path = trial_dir.join("trace.jsonl");
        let mut artifacts = BTreeMap::from([(
            "trace".into(),
            relative_artifact(&self.config.output_dir, &trace_path),
        )]);
        let environment_digest = sha256_bytes(
            format!(
                "{}\0{}",
                self.environment.identity(),
                self.config.policy.digest
            )
            .as_bytes(),
        );

        let setup = (|| -> Result<(PathBuf, TreeSnapshot, JsonlEventSink)> {
            ensure_private_directory(&trial_dir)?;
            let agent_workspace = trial_dir.join("agent-workspace");
            copy_tree(&task.workspace, &agent_workspace, false)?;
            let baseline = snapshot_tree(
                &task.workspace,
                self.config.max_workspace_files,
                self.config.max_workspace_bytes,
            )?;
            let sink = JsonlEventSink::new(&trace_path)?;
            Ok((agent_workspace, baseline, sink))
        })();

        let (agent_workspace, baseline, sink) = match setup {
            Ok(setup) => setup,
            Err(error) => {
                return failure_trial(
                    task,
                    error,
                    FailureTrial {
                        agent: agent.identity().clone(),
                        environment_digest,
                        trial_index,
                        agent_attempts: 0,
                        id: trial_id,
                        status: TrialStatus::EnvironmentError,
                        elapsed: trial_start.elapsed(),
                        artifacts,
                        events: Vec::new(),
                        evidence: FailureEvidence::default(),
                    },
                )
            }
        };

        let trial_agent_budget =
            Duration::from_secs(task.timeout_secs.min(self.config.agent_limits.timeout_secs));
        let mut agent_attempts = 0;
        let mut prior_usage = AgentUsage::default();
        let execution = loop {
            let Some(remaining) = trial_agent_budget.checked_sub(trial_start.elapsed()) else {
                break AgentExecution::Finished(Box::new(Ok(timeout_outcome(
                    agent.identity().clone(),
                    prior_usage.clone(),
                    "agent trial time budget was exhausted before another attempt",
                ))));
            };
            let remaining_secs = remaining.as_secs();
            if remaining_secs == 0 {
                break AgentExecution::Finished(Box::new(Ok(timeout_outcome(
                    agent.identity().clone(),
                    prior_usage.clone(),
                    "agent trial time budget was exhausted before another attempt",
                ))));
            }
            let mut attempt_limits = self.config.agent_limits.clone();
            attempt_limits.timeout_secs = remaining_secs;
            if let Some(limit) = attempt_limits.max_tokens {
                let consumed = prior_usage
                    .input_tokens
                    .saturating_add(prior_usage.output_tokens);
                let remaining_tokens = limit.saturating_sub(consumed);
                if remaining_tokens == 0 {
                    break AgentExecution::Finished(Box::new(Ok(budget_outcome(
                        agent.identity().clone(),
                        prior_usage.clone(),
                        "agent token budget was exhausted before another attempt",
                    ))));
                }
                attempt_limits.max_tokens = Some(remaining_tokens);
            }
            if let Some(limit) = attempt_limits.max_cost_usd {
                let remaining_cost = limit - prior_usage.estimated_cost_usd;
                if remaining_cost <= 0.0 {
                    break AgentExecution::Finished(Box::new(Ok(budget_outcome(
                        agent.identity().clone(),
                        prior_usage.clone(),
                        "agent cost budget was exhausted before another attempt",
                    ))));
                }
                attempt_limits.max_cost_usd = Some(remaining_cost);
            }
            agent_attempts += 1;
            let request = AgentRequest {
                trial_id,
                task_id: task.id.clone(),
                prompt: task.prompt.clone(),
                workspace: agent_workspace.clone(),
                limits: attempt_limits,
            };
            let attempt = self
                .environment
                .execute(agent.as_ref(), &request, &sink)
                .await;
            let retryable = match &attempt {
                Err(_) => true,
                Ok(outcome) => outcome.termination == AgentTerminationReason::ExitNonZero,
            };
            if !retryable || agent_attempts > self.config.agent_limits.max_retries {
                break AgentExecution::Finished(Box::new(attempt.map(|mut outcome| {
                    merge_usage(&mut outcome.usage, &prior_usage);
                    outcome
                })));
            }
            if let Ok(outcome) = &attempt {
                merge_usage(&mut prior_usage, &outcome.usage);
            }
            let reason = match &attempt {
                Ok(outcome) => outcome
                    .error
                    .as_deref()
                    .unwrap_or("agent exited non-zero")
                    .to_string(),
                Err(error) => error.root_cause().to_string(),
            };
            let retry_event = AgentEvent {
                sequence: 0,
                timestamp: chrono::Utc::now(),
                kind: crate::agent::AgentEventKind::Warning,
                message: format!("retrying agent after attempt {agent_attempts}: {reason}"),
                raw: None,
            };
            if let Err(error) = sink.emit(&retry_event) {
                break AgentExecution::EnvironmentError(
                    error.context("failed to persist agent retry event"),
                );
            }
            if let Err(error) = reset_workspace(&task.workspace, &agent_workspace) {
                break AgentExecution::EnvironmentError(
                    error.context("failed to restore clean workspace for agent retry"),
                );
            }
        };
        let events = sink.events();

        if let AgentExecution::EnvironmentError(error) = execution {
            return failure_trial(
                task,
                error,
                FailureTrial {
                    agent: agent.identity().clone(),
                    environment_digest,
                    trial_index,
                    agent_attempts,
                    id: trial_id,
                    status: TrialStatus::EnvironmentError,
                    elapsed: trial_start.elapsed(),
                    artifacts,
                    events,
                    evidence: FailureEvidence::default(),
                },
            );
        }
        let AgentExecution::Finished(execution) = execution else {
            unreachable!("environment errors returned above")
        };

        let after = match snapshot_tree(
            &agent_workspace,
            self.config.max_workspace_files,
            self.config.max_workspace_bytes,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return failure_trial(
                    task,
                    error,
                    FailureTrial {
                        agent: agent.identity().clone(),
                        environment_digest,
                        trial_index,
                        agent_attempts,
                        id: trial_id,
                        status: TrialStatus::EnvironmentError,
                        elapsed: trial_start.elapsed(),
                        artifacts,
                        events,
                        evidence: FailureEvidence::default(),
                    },
                )
            }
        };
        let (changes, patch) = match diff_snapshots(&baseline, &after, self.config.max_patch_bytes)
        {
            Ok(diff) => diff,
            Err(error) => {
                let (identity, usage) = match execution.as_ref() {
                    Ok(outcome) => (outcome.identity.clone(), outcome.usage.clone()),
                    Err(_) => (agent.identity().clone(), prior_usage.clone()),
                };
                return failure_trial(
                    task,
                    error,
                    FailureTrial {
                        agent: identity,
                        environment_digest,
                        trial_index,
                        agent_attempts,
                        id: trial_id,
                        status: TrialStatus::AgentError,
                        elapsed: trial_start.elapsed(),
                        artifacts,
                        events,
                        evidence: FailureEvidence {
                            usage,
                            termination_reason: Some(AgentTerminationReason::OutputLimit),
                            ..FailureEvidence::default()
                        },
                    },
                );
            }
        };
        let patch_path = trial_dir.join("changes.patch");
        if let Err(error) = fs::write(&patch_path, &patch) {
            return failure_trial(
                task,
                error.into(),
                FailureTrial {
                    agent: agent.identity().clone(),
                    environment_digest,
                    trial_index,
                    agent_attempts,
                    id: trial_id,
                    status: TrialStatus::EnvironmentError,
                    elapsed: trial_start.elapsed(),
                    artifacts,
                    events,
                    evidence: FailureEvidence::default(),
                },
            );
        }
        artifacts.insert(
            "patch".into(),
            relative_artifact(&self.config.output_dir, &patch_path),
        );

        let outcome = match *execution {
            Ok(outcome) => outcome,
            Err(error) => {
                return TrialResult {
                    id: trial_id,
                    task_id: task.id.clone(),
                    task_digest: task.digest.clone(),
                    agent: agent.identity().clone(),
                    environment_digest,
                    trial_index,
                    agent_attempts,
                    status: TrialStatus::AgentError,
                    changed_files: changes,
                    patch,
                    grader: None,
                    events,
                    usage: prior_usage,
                    duration_ms: trial_start.elapsed().as_millis() as u64,
                    termination_reason: None,
                    artifacts,
                    error: Some(format!("{error:#}")),
                }
            }
        };

        if outcome.termination != AgentTerminationReason::Completed {
            let status = match outcome.termination {
                AgentTerminationReason::Timeout => TrialStatus::Timeout,
                AgentTerminationReason::Cancelled => TrialStatus::Cancelled,
                AgentTerminationReason::Completed
                | AgentTerminationReason::ExitNonZero
                | AgentTerminationReason::OutputLimit
                | AgentTerminationReason::BudgetExceeded => TrialStatus::AgentError,
            };
            return TrialResult {
                id: trial_id,
                task_id: task.id.clone(),
                task_digest: task.digest.clone(),
                agent: outcome.identity,
                environment_digest,
                trial_index,
                agent_attempts,
                status,
                changed_files: changes,
                patch,
                grader: None,
                events,
                usage: outcome.usage,
                duration_ms: trial_start.elapsed().as_millis() as u64,
                termination_reason: Some(outcome.termination),
                artifacts,
                error: outcome.error,
            };
        }

        let verifier_workspace = trial_dir.join("verifier-workspace");
        let verification_setup = (|| -> Result<()> {
            copy_tree(&task.workspace, &verifier_workspace, false)?;
            apply_snapshot_changes(&agent_workspace, &verifier_workspace, &changes)?;
            copy_tree(&task.grader, &verifier_workspace, true)?;
            Ok(())
        })();
        if let Err(error) = verification_setup {
            return failure_trial(
                task,
                error,
                FailureTrial {
                    agent: outcome.identity,
                    environment_digest,
                    trial_index,
                    agent_attempts,
                    id: trial_id,
                    status: TrialStatus::EnvironmentError,
                    elapsed: trial_start.elapsed(),
                    artifacts,
                    events,
                    evidence: FailureEvidence {
                        changed_files: changes,
                        patch,
                        usage: outcome.usage,
                        termination_reason: Some(outcome.termination),
                    },
                },
            );
        }

        let grade_request = GradeRequest {
            trial_id,
            workspace: verifier_workspace,
            checks: if task.verifier.checks.is_empty() {
                vec![GradeCheckRequest {
                    name: "verifier command".into(),
                    kind: GraderCheckKind::Other,
                    command: task.verifier.command.clone(),
                }]
            } else {
                task.verifier
                    .checks
                    .iter()
                    .map(|check| GradeCheckRequest {
                        name: check.name.clone(),
                        kind: check.kind,
                        command: check.command.clone(),
                    })
                    .collect()
            },
            timeout: Duration::from_secs(task.verifier.timeout_secs),
        };
        match self.grader.grade(&grade_request).await {
            Ok(grader) => {
                let stdout_path = trial_dir.join("grader.stdout.log");
                let stderr_path = trial_dir.join("grader.stderr.log");
                if let Err(error) = fs::write(&stdout_path, &grader.stdout)
                    .and_then(|_| fs::write(&stderr_path, &grader.stderr))
                {
                    return failure_trial(
                        task,
                        error.into(),
                        FailureTrial {
                            agent: outcome.identity,
                            environment_digest,
                            trial_index,
                            agent_attempts,
                            id: trial_id,
                            status: TrialStatus::EnvironmentError,
                            elapsed: trial_start.elapsed(),
                            artifacts,
                            events,
                            evidence: FailureEvidence {
                                changed_files: changes,
                                patch,
                                usage: outcome.usage,
                                termination_reason: Some(outcome.termination),
                            },
                        },
                    );
                }
                artifacts.insert(
                    "grader_stdout".into(),
                    relative_artifact(&self.config.output_dir, &stdout_path),
                );
                artifacts.insert(
                    "grader_stderr".into(),
                    relative_artifact(&self.config.output_dir, &stderr_path),
                );
                TrialResult {
                    id: trial_id,
                    task_id: task.id.clone(),
                    task_digest: task.digest.clone(),
                    agent: outcome.identity,
                    environment_digest,
                    trial_index,
                    agent_attempts,
                    status: if grader.success {
                        TrialStatus::Passed
                    } else {
                        TrialStatus::Failed
                    },
                    changed_files: changes,
                    patch,
                    grader: Some(grader),
                    events,
                    usage: outcome.usage,
                    duration_ms: trial_start.elapsed().as_millis() as u64,
                    termination_reason: Some(outcome.termination),
                    artifacts,
                    error: None,
                }
            }
            Err(error) => failure_trial(
                task,
                error,
                FailureTrial {
                    agent: outcome.identity,
                    environment_digest,
                    trial_index,
                    agent_attempts,
                    id: trial_id,
                    status: TrialStatus::GraderError,
                    elapsed: trial_start.elapsed(),
                    artifacts,
                    events,
                    evidence: FailureEvidence {
                        changed_files: changes,
                        patch,
                        usage: outcome.usage,
                        termination_reason: Some(outcome.termination),
                    },
                },
            ),
        }
    }

    fn persist_trial(&self, result: &TrialResult) -> Result<()> {
        let path = self
            .config
            .output_dir
            .join("trials")
            .join(result.id.to_string())
            .join("trial.json");
        atomic_write_json(&path, result)
    }

    fn persist_partial_report(
        &self,
        suite: &ResolvedSuite,
        trials: &[TrialResult],
        elapsed: Duration,
    ) -> Result<()> {
        let report = RepositoryReport::new(
            suite_summary(suite),
            self.config.policy.clone(),
            trials.to_vec(),
            elapsed.as_millis() as u64,
        );
        atomic_write_json(&self.config.output_dir.join("report.partial.json"), &report)
    }
}

enum AgentExecution {
    Finished(Box<Result<crate::agent::AgentOutcome>>),
    EnvironmentError(anyhow::Error),
}

fn timeout_outcome(
    identity: crate::agent::AgentIdentity,
    usage: AgentUsage,
    message: &str,
) -> crate::agent::AgentOutcome {
    crate::agent::AgentOutcome {
        identity,
        termination: AgentTerminationReason::Timeout,
        exit_code: None,
        duration_ms: 0,
        usage,
        events: Vec::new(),
        error: Some(message.into()),
    }
}

fn budget_outcome(
    identity: crate::agent::AgentIdentity,
    usage: AgentUsage,
    message: &str,
) -> crate::agent::AgentOutcome {
    crate::agent::AgentOutcome {
        identity,
        termination: AgentTerminationReason::BudgetExceeded,
        exit_code: None,
        duration_ms: 0,
        usage,
        events: Vec::new(),
        error: Some(message.into()),
    }
}

fn merge_usage(total: &mut AgentUsage, additional: &AgentUsage) {
    total.input_tokens = total.input_tokens.saturating_add(additional.input_tokens);
    total.output_tokens = total.output_tokens.saturating_add(additional.output_tokens);
    total.cached_tokens = total.cached_tokens.saturating_add(additional.cached_tokens);
    total.estimated_cost_usd += additional.estimated_cost_usd;
}

fn reset_workspace(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    copy_tree(source, destination, false)
}

#[derive(Debug, Clone)]
struct SnapshotFile {
    sha256: String,
    content: Vec<u8>,
}

type TreeSnapshot = BTreeMap<String, SnapshotFile>;

struct JsonlEventSink {
    file: Mutex<File>,
    events: Mutex<Vec<AgentEvent>>,
}

impl JsonlEventSink {
    fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            ensure_private_directory(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("failed to create trace: {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
            events: Mutex::new(Vec::new()),
        })
    }

    fn events(&self) -> Vec<AgentEvent> {
        self.events
            .lock()
            .expect("event collection lock poisoned")
            .clone()
    }
}

impl EventSink for JsonlEventSink {
    fn emit(&self, event: &AgentEvent) -> Result<()> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| anyhow::anyhow!("event collection lock poisoned"))?;
        let mut normalized = event.clone();
        normalized.sequence = events.len() as u64 + 1;
        let line = serde_json::to_string(&normalized)?;
        let mut file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("trace file lock poisoned"))?;
        writeln!(file, "{line}")?;
        file.flush()?;
        events.push(normalized);
        Ok(())
    }
}

fn suite_summary(suite: &ResolvedSuite) -> RepositorySuiteSummary {
    RepositorySuiteSummary {
        id: suite.id.clone(),
        name: suite.name.clone(),
        digest: suite.digest.clone(),
        task_digests: suite
            .tasks
            .iter()
            .map(|task| (task.id.clone(), task.digest.clone()))
            .collect(),
    }
}

fn snapshot_tree(root: &Path, max_files: usize, max_bytes: u64) -> Result<TreeSnapshot> {
    let root = root.canonicalize()?;
    let mut snapshot = BTreeMap::new();
    let mut bytes = 0_u64;
    snapshot_directory(
        &root,
        &root,
        &mut snapshot,
        &mut bytes,
        max_files,
        max_bytes,
    )?;
    Ok(snapshot)
}

fn snapshot_directory(
    root: &Path,
    directory: &Path,
    snapshot: &mut TreeSnapshot,
    bytes: &mut u64,
    max_files: usize,
    max_bytes: u64,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.parent() == Some(root) && matches!(name, ".git" | "target" | ".forgetest") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "symlink is not allowed in trial workspace: {}",
            path.display()
        );
        if metadata.is_dir() {
            snapshot_directory(root, &path, snapshot, bytes, max_files, max_bytes)?;
            continue;
        }
        anyhow::ensure!(
            metadata.is_file(),
            "unsupported filesystem entry: {}",
            path.display()
        );
        anyhow::ensure!(
            snapshot.len() < max_files,
            "workspace exceeds file limit of {max_files}"
        );
        *bytes = bytes.saturating_add(metadata.len());
        anyhow::ensure!(
            *bytes <= max_bytes,
            "workspace exceeds byte limit of {max_bytes}"
        );
        let relative = normalized_relative(root, &path)?;
        let content = fs::read(&path)?;
        snapshot.insert(
            relative,
            SnapshotFile {
                sha256: sha256_bytes(&content),
                content,
            },
        );
    }
    Ok(())
}

fn diff_snapshots(
    before: &TreeSnapshot,
    after: &TreeSnapshot,
    max_patch_bytes: usize,
) -> Result<(Vec<FileChange>, String)> {
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    let mut changes = Vec::new();
    let mut patch = String::new();
    for path in paths {
        let old = before.get(&path);
        let new = after.get(&path);
        let kind = match (old, new) {
            (None, Some(_)) => FileChangeKind::Added,
            (Some(_), None) => FileChangeKind::Deleted,
            (Some(old), Some(new)) if old.sha256 != new.sha256 => FileChangeKind::Modified,
            _ => continue,
        };
        changes.push(FileChange {
            path: path.clone(),
            kind,
            before_sha256: old.map(|file| file.sha256.clone()),
            after_sha256: new.map(|file| file.sha256.clone()),
        });
        append_patch(&mut patch, &path, old, new);
        anyhow::ensure!(
            patch.len() <= max_patch_bytes,
            "agent patch exceeds byte limit of {max_patch_bytes}"
        );
    }
    Ok((changes, patch))
}

fn append_patch(
    patch: &mut String,
    path: &str,
    before: Option<&SnapshotFile>,
    after: Option<&SnapshotFile>,
) {
    patch.push_str(&format!("diff --git a/{path} b/{path}\n"));
    if before.is_some() {
        patch.push_str(&format!("--- a/{path}\n"));
    } else {
        patch.push_str("--- /dev/null\n");
    }
    if after.is_some() {
        patch.push_str(&format!("+++ b/{path}\n"));
    } else {
        patch.push_str("+++ /dev/null\n");
    }
    match (
        before.and_then(|file| std::str::from_utf8(&file.content).ok()),
        after.and_then(|file| std::str::from_utf8(&file.content).ok()),
    ) {
        (old, new) if old.is_some() || new.is_some() => {
            let old_lines = old.map_or(0, |value| value.lines().count());
            let new_lines = new.map_or(0, |value| value.lines().count());
            patch.push_str(&format!("@@ -1,{old_lines} +1,{new_lines} @@\n"));
            if let Some(old) = old {
                for line in old.lines() {
                    patch.push('-');
                    patch.push_str(line);
                    patch.push('\n');
                }
            }
            if let Some(new) = new {
                for line in new.lines() {
                    patch.push('+');
                    patch.push_str(line);
                    patch.push('\n');
                }
            }
        }
        _ => patch.push_str("Binary files differ\n"),
    }
}

fn apply_snapshot_changes(source: &Path, destination: &Path, changes: &[FileChange]) -> Result<()> {
    for change in changes {
        let relative = validated_relative(&change.path)?;
        let target = destination.join(&relative);
        match change.kind {
            FileChangeKind::Deleted => {
                if target.exists() {
                    fs::remove_file(&target)?;
                }
            }
            FileChangeKind::Added | FileChangeKind::Modified => {
                let source_path = source.join(&relative);
                let metadata = fs::symlink_metadata(&source_path)?;
                anyhow::ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "changed path is not a regular file: {}",
                    change.path
                );
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source_path, target)?;
            }
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path, overlay: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "copy source must be a real directory: {}",
        source.display()
    );
    if destination.exists() {
        anyhow::ensure!(
            overlay,
            "copy destination already exists: {}",
            destination.display()
        );
    } else {
        fs::create_dir_all(destination)?;
    }
    let mut entries = fs::read_dir(source)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "symlink is not allowed: {}",
            path.display()
        );
        let target = destination.join(path.file_name().context("path has no file name")?);
        if metadata.is_dir() {
            copy_tree(&path, &target, overlay)?;
        } else if metadata.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
            fs::set_permissions(&target, metadata.permissions())?;
        } else {
            anyhow::bail!("unsupported filesystem entry: {}", path.display());
        }
    }
    Ok(())
}

#[derive(Default)]
struct FailureEvidence {
    changed_files: Vec<FileChange>,
    patch: String,
    usage: AgentUsage,
    termination_reason: Option<AgentTerminationReason>,
}

struct FailureTrial {
    agent: crate::agent::AgentIdentity,
    environment_digest: String,
    trial_index: u32,
    agent_attempts: u32,
    id: Uuid,
    status: TrialStatus,
    elapsed: Duration,
    artifacts: BTreeMap<String, String>,
    events: Vec<AgentEvent>,
    evidence: FailureEvidence,
}

fn failure_trial(
    task: &ResolvedRepositoryTask,
    error: anyhow::Error,
    failure: FailureTrial,
) -> TrialResult {
    let FailureTrial {
        agent,
        environment_digest,
        trial_index,
        agent_attempts,
        id,
        status,
        elapsed,
        artifacts,
        events,
        evidence,
    } = failure;
    TrialResult {
        id,
        task_id: task.id.clone(),
        task_digest: task.digest.clone(),
        agent,
        environment_digest,
        trial_index,
        agent_attempts,
        status,
        changed_files: evidence.changed_files,
        patch: evidence.patch,
        grader: None,
        events,
        usage: evidence.usage,
        duration_ms: elapsed.as_millis() as u64,
        termination_reason: evidence.termination_reason,
        artifacts,
        error: Some(format!("{error:#}")),
    }
}

fn atomic_write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
        .with_context(|| format!("failed to atomically replace {}", path.display()))
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn normalized_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn validated_relative(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    anyhow::ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "artifact path must be relative without '..': {value}"
    );
    Ok(path.to_path_buf())
}

fn relative_artifact(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
