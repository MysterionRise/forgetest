//! Version 2 reports for repository-level coding-agent trials.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::agent::{AgentEvent, AgentIdentity, AgentTerminationReason, AgentUsage};

/// Repository report schema version.
pub const REPOSITORY_REPORT_SCHEMA_VERSION: u32 = 2;

/// Complete repository-level evaluation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryReport {
    pub schema_version: u32,
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub suite: RepositorySuiteSummary,
    pub policy: ExecutionPolicyManifest,
    pub agents: Vec<AgentIdentity>,
    pub trials: Vec<TrialResult>,
    pub aggregate: RepositoryAggregate,
    #[serde(default)]
    pub redaction: RedactionMetadata,
    pub duration_ms: u64,
}

impl RepositoryReport {
    pub fn new(
        suite: RepositorySuiteSummary,
        policy: ExecutionPolicyManifest,
        trials: Vec<TrialResult>,
        duration_ms: u64,
    ) -> Self {
        let mut seen = HashSet::new();
        let agents = trials
            .iter()
            .filter(|trial| seen.insert(trial.agent.clone()))
            .map(|trial| trial.agent.clone())
            .collect();
        let aggregate = RepositoryAggregate::compute(&trials);
        Self {
            schema_version: REPOSITORY_REPORT_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            created_at: Utc::now(),
            suite,
            policy,
            agents,
            trials,
            aggregate,
            redaction: RedactionMetadata::default(),
            duration_ms,
        }
    }

    pub fn save_json(&self, path: &Path) -> Result<()> {
        self.validate_integrity()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).context("failed to serialize v2 report")?;
        std::fs::write(path, json)
            .with_context(|| format!("failed to write report: {}", path.display()))
    }

    pub fn load_json(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read report: {}", path.display()))?;
        let report: Self = serde_json::from_str(&json).context("failed to parse v2 report")?;
        anyhow::ensure!(
            report.schema_version == REPOSITORY_REPORT_SCHEMA_VERSION,
            "unsupported repository report schema version: {}",
            report.schema_version
        );
        report.validate_integrity()?;
        Ok(report)
    }

    /// Validate trusted identities and all fields derived from trial evidence.
    pub fn validate_integrity(&self) -> Result<()> {
        anyhow::ensure!(
            self.policy.schema_version == 1,
            "unsupported execution policy schema version: {}",
            self.policy.schema_version
        );
        anyhow::ensure!(
            self.policy.verify_digest(),
            "execution policy digest does not match its recorded parameters"
        );

        let mut trial_ids = HashSet::new();
        let mut scheduled = HashSet::new();
        for trial in &self.trials {
            let expected_digest =
                self.suite
                    .task_digests
                    .get(&trial.task_id)
                    .with_context(|| {
                        format!(
                            "trial {} references task '{}' outside the suite",
                            trial.id, trial.task_id
                        )
                    })?;
            anyhow::ensure!(
                expected_digest == &trial.task_digest,
                "trial {} task digest does not match suite task '{}'",
                trial.id,
                trial.task_id
            );
            anyhow::ensure!(
                trial_ids.insert(trial.id),
                "duplicate trial ID: {}",
                trial.id
            );
            anyhow::ensure!(
                trial.trial_index > 0,
                "trial {} index must be at least 1",
                trial.id
            );
            anyhow::ensure!(
                scheduled.insert((trial.task_id.as_str(), &trial.agent, trial.trial_index)),
                "duplicate scheduled trial for task '{}', agent '{}', index {}",
                trial.task_id,
                trial.agent.key(),
                trial.trial_index
            );
        }

        let mut seen_agents = HashSet::new();
        let expected_agents: Vec<_> = self
            .trials
            .iter()
            .filter(|trial| seen_agents.insert(trial.agent.clone()))
            .map(|trial| trial.agent.clone())
            .collect();
        anyhow::ensure!(
            self.agents == expected_agents,
            "report agent inventory does not match trial evidence"
        );

        let expected_aggregate = RepositoryAggregate::compute(&self.trials);
        anyhow::ensure!(
            serde_json::to_value(&self.aggregate)? == serde_json::to_value(expected_aggregate)?,
            "report aggregate does not match trial evidence"
        );
        Ok(())
    }

    pub fn compatibility_with(&self, baseline: &Self) -> Compatibility {
        let mut reasons = Vec::new();
        if self.suite.digest != baseline.suite.digest {
            reasons.push("suite digest differs".to_string());
        }
        if self.suite.task_digests != baseline.suite.task_digests {
            reasons.push("task digests differ".to_string());
        }
        if self.policy.digest != baseline.policy.digest {
            reasons.push("execution policy digest differs".to_string());
        }
        if reasons.is_empty() {
            Compatibility::Comparable
        } else {
            Compatibility::Incomparable { reasons }
        }
    }

    /// Compare observed resolution rates for matching agent configurations.
    pub fn compare(&self, baseline: &Self, threshold: f64) -> RepositoryComparison {
        let compatibility = self.compatibility_with(baseline);
        let gating_eligible = compatibility == Compatibility::Comparable;
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut unchanged = Vec::new();
        let mut new_agents = Vec::new();
        let mut removed_agents = Vec::new();

        for (agent, current) in &self.aggregate.per_agent {
            match baseline.aggregate.per_agent.get(agent) {
                Some(previous) => {
                    let delta =
                        current.observed_resolution_rate - previous.observed_resolution_rate;
                    let change = AgentRateChange {
                        agent: agent.clone(),
                        baseline_rate: previous.observed_resolution_rate,
                        current_rate: current.observed_resolution_rate,
                        delta,
                    };
                    if delta < -threshold {
                        regressions.push(change);
                    } else if delta > threshold {
                        improvements.push(change);
                    } else {
                        unchanged.push(agent.clone());
                    }
                }
                None => new_agents.push(agent.clone()),
            }
        }
        for agent in baseline.aggregate.per_agent.keys() {
            if !self.aggregate.per_agent.contains_key(agent) {
                removed_agents.push(agent.clone());
            }
        }

        RepositoryComparison {
            compatibility,
            gating_eligible,
            regressions,
            improvements,
            unchanged,
            new_agents,
            removed_agents,
        }
    }
}

/// Content identity for a loaded suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySuiteSummary {
    pub id: String,
    pub name: String,
    pub digest: String,
    pub task_digests: BTreeMap<String, String>,
}

/// Security- and grading-relevant execution policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicyManifest {
    #[serde(default = "default_execution_policy_schema")]
    pub schema_version: u32,
    pub profile: String,
    pub agent_environment: String,
    pub verifier_environment: String,
    #[serde(default)]
    pub verifier_image: Option<String>,
    pub network: String,
    #[serde(default)]
    pub parameters: ExecutionPolicyParameters,
    pub digest: String,
}

impl ExecutionPolicyManifest {
    /// Compute and attach a digest over every security- and budget-relevant field.
    pub fn sealed(mut self) -> Self {
        self.digest = self
            .computed_digest()
            .expect("execution policy values must serialize as finite JSON");
        self
    }

    /// Check that the recorded digest still matches the policy contents.
    pub fn verify_digest(&self) -> bool {
        self.computed_digest()
            .is_some_and(|digest| digest == self.digest)
    }

    fn computed_digest(&self) -> Option<String> {
        let material = (
            self.schema_version,
            &self.profile,
            &self.agent_environment,
            &self.verifier_environment,
            &self.verifier_image,
            &self.network,
            &self.parameters,
        );
        let encoded = serde_json::to_vec(&material).ok()?;
        let digest = Sha256::digest(encoded);
        Some(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

fn default_execution_policy_schema() -> u32 {
    1
}

/// Explicit limits and immutable images covered by the execution-policy digest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionPolicyParameters {
    pub trials: u32,
    pub parallelism: usize,
    #[serde(default)]
    pub agent_images: BTreeMap<String, String>,
    pub agent_timeout_secs: u64,
    pub max_agent_output_bytes: usize,
    #[serde(default)]
    pub max_agent_tokens: Option<u64>,
    #[serde(default)]
    pub max_agent_cost_usd: Option<f64>,
    pub agent_retries: u32,
    pub max_workspace_files: usize,
    pub max_workspace_bytes: u64,
    pub max_patch_bytes: usize,
    pub verifier_max_output_bytes: usize,
    #[serde(default)]
    pub agent_container: Option<ContainerLimitsManifest>,
    #[serde(default)]
    pub verifier_container: Option<ContainerLimitsManifest>,
}

/// Container resource limits recorded in a policy manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerLimitsManifest {
    pub memory: String,
    pub cpus: f64,
    pub pids_limit: u32,
    pub tmpfs_size: String,
}

/// Result of one scheduled task/agent/trial combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    pub id: Uuid,
    pub task_id: String,
    pub task_digest: String,
    pub agent: AgentIdentity,
    pub environment_digest: String,
    pub trial_index: u32,
    #[serde(default = "default_agent_attempts")]
    pub agent_attempts: u32,
    pub status: TrialStatus,
    #[serde(default)]
    pub changed_files: Vec<FileChange>,
    #[serde(default)]
    pub patch: String,
    #[serde(default)]
    pub grader: Option<GraderOutcome>,
    #[serde(default)]
    pub events: Vec<AgentEvent>,
    #[serde(default)]
    pub usage: AgentUsage,
    pub duration_ms: u64,
    #[serde(default)]
    pub termination_reason: Option<AgentTerminationReason>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, String>,
    #[serde(default)]
    pub error: Option<String>,
}

fn default_agent_attempts() -> u32 {
    1
}

/// Complete status space for scheduled repository trials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialStatus {
    Passed,
    Failed,
    AgentError,
    EnvironmentError,
    GraderError,
    Timeout,
    Cancelled,
}

/// Trusted file-tree change observed after an agent exits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub kind: FileChangeKind,
    #[serde(default)]
    pub before_sha256: Option<String>,
    #[serde(default)]
    pub after_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Added,
    Modified,
    Deleted,
}

/// Independent verifier evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraderOutcome {
    pub success: bool,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub checks: Vec<GraderCheck>,
}

/// One deterministic check reported by a grader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraderCheck {
    pub name: String,
    pub kind: GraderCheckKind,
    pub passed: bool,
    #[serde(default)]
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraderCheckKind {
    FailToPass,
    PassToPass,
    Compile,
    Clippy,
    Other,
}

/// Aggregate metrics grouped by agent identity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepositoryAggregate {
    pub per_agent: BTreeMap<String, AgentAggregate>,
    #[serde(default)]
    pub pairwise: Vec<PairwiseAgentComparison>,
}

impl RepositoryAggregate {
    pub fn compute(trials: &[TrialResult]) -> Self {
        let mut grouped: BTreeMap<String, Vec<&TrialResult>> = BTreeMap::new();
        for trial in trials {
            grouped.entry(trial.agent.key()).or_default().push(trial);
        }

        let per_agent = grouped
            .iter()
            .map(|(agent, trials)| (agent.clone(), AgentAggregate::compute(trials)))
            .collect();
        let agent_keys: Vec<_> = grouped.keys().cloned().collect();
        let mut pairwise = Vec::new();
        for left_index in 0..agent_keys.len() {
            for right_index in (left_index + 1)..agent_keys.len() {
                let agent_a = &agent_keys[left_index];
                let agent_b = &agent_keys[right_index];
                pairwise.push(paired_agent_comparison(
                    agent_a,
                    grouped
                        .get(agent_a)
                        .expect("agent key came from grouped trials"),
                    agent_b,
                    grouped
                        .get(agent_b)
                        .expect("agent key came from grouped trials"),
                ));
            }
        }
        Self {
            per_agent,
            pairwise,
        }
    }
}

/// Portfolio-facing reliability metrics for one agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAggregate {
    pub scheduled: u64,
    pub passed: u64,
    /// All scheduled outcomes other than `passed`, retained for v2 compatibility.
    pub failed: u64,
    #[serde(default)]
    pub task_failures: u64,
    #[serde(default)]
    pub agent_errors: u64,
    #[serde(default)]
    pub environment_errors: u64,
    #[serde(default)]
    pub grader_errors: u64,
    #[serde(default)]
    pub timeouts: u64,
    #[serde(default)]
    pub cancelled: u64,
    /// Failures caused by the execution or grading infrastructure.
    pub infrastructure_errors: u64,
    pub observed_resolution_rate: f64,
    pub valid_trial_resolution_rate: f64,
    pub wilson_95_low: f64,
    pub wilson_95_high: f64,
    pub pass_at_1: f64,
    pub pass_power_3: f64,
    pub total_cost_usd: f64,
    pub total_duration_ms: u64,
}

impl AgentAggregate {
    fn compute(trials: &[&TrialResult]) -> Self {
        let scheduled = trials.len() as u64;
        let passed = trials
            .iter()
            .filter(|trial| trial.status == TrialStatus::Passed)
            .count() as u64;
        let count = |status| trials.iter().filter(|trial| trial.status == status).count() as u64;
        let task_failures = count(TrialStatus::Failed);
        let agent_errors = count(TrialStatus::AgentError);
        let environment_errors = count(TrialStatus::EnvironmentError);
        let grader_errors = count(TrialStatus::GraderError);
        let timeouts = count(TrialStatus::Timeout);
        let cancelled = count(TrialStatus::Cancelled);
        let infrastructure_errors = environment_errors + grader_errors;
        let failed = scheduled - passed;
        let valid = scheduled
            .saturating_sub(infrastructure_errors)
            .saturating_sub(cancelled);
        let observed_resolution_rate = ratio(passed, scheduled);
        let valid_trial_resolution_rate = ratio(passed, valid);
        let (wilson_95_low, wilson_95_high) = wilson_interval_95(passed, scheduled);

        let mut by_task: HashMap<&str, Vec<&TrialResult>> = HashMap::new();
        for trial in trials {
            by_task.entry(&trial.task_id).or_default().push(*trial);
        }
        for task_trials in by_task.values_mut() {
            task_trials.sort_by_key(|trial| trial.trial_index);
        }
        let pass_at_1 = if by_task.is_empty() {
            0.0
        } else {
            by_task
                .values()
                .filter(|task_trials| {
                    task_trials
                        .first()
                        .is_some_and(|trial| trial.status == TrialStatus::Passed)
                })
                .count() as f64
                / by_task.len() as f64
        };
        let trial_sets_of_three: Vec<_> = by_task
            .values()
            .filter(|task_trials| task_trials.len() >= 3)
            .collect();
        let pass_power_3 = if trial_sets_of_three.is_empty() {
            0.0
        } else {
            trial_sets_of_three
                .iter()
                .filter(|task_trials| {
                    task_trials
                        .iter()
                        .take(3)
                        .all(|trial| trial.status == TrialStatus::Passed)
                })
                .count() as f64
                / trial_sets_of_three.len() as f64
        };

        Self {
            scheduled,
            passed,
            failed,
            task_failures,
            agent_errors,
            environment_errors,
            grader_errors,
            timeouts,
            cancelled,
            infrastructure_errors,
            observed_resolution_rate,
            valid_trial_resolution_rate,
            wilson_95_low,
            wilson_95_high,
            pass_at_1,
            pass_power_3,
            total_cost_usd: trials
                .iter()
                .map(|trial| trial.usage.estimated_cost_usd)
                .sum(),
            total_duration_ms: trials.iter().map(|trial| trial.duration_ms).sum(),
        }
    }
}

/// Deterministic paired bootstrap over task-level resolution rates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairwiseAgentComparison {
    pub agent_a: String,
    pub agent_b: String,
    pub common_tasks: u64,
    pub delta_b_minus_a: f64,
    pub ci_95_low: f64,
    pub ci_95_high: f64,
    pub bootstrap_iterations: u32,
}

const PAIRED_BOOTSTRAP_ITERATIONS: usize = 10_000;

fn paired_agent_comparison(
    agent_a: &str,
    trials_a: &[&TrialResult],
    agent_b: &str,
    trials_b: &[&TrialResult],
) -> PairwiseAgentComparison {
    let rates_a = task_resolution_rates(trials_a);
    let rates_b = task_resolution_rates(trials_b);
    let differences: Vec<_> = rates_a
        .iter()
        .filter_map(|(task, rate_a)| rates_b.get(task).map(|rate_b| rate_b - rate_a))
        .collect();
    let delta_b_minus_a = mean(&differences);
    let (ci_95_low, ci_95_high) = paired_bootstrap_interval(
        &differences,
        deterministic_seed(agent_a, agent_b, &differences),
        PAIRED_BOOTSTRAP_ITERATIONS,
    );
    PairwiseAgentComparison {
        agent_a: agent_a.into(),
        agent_b: agent_b.into(),
        common_tasks: differences.len() as u64,
        delta_b_minus_a,
        ci_95_low,
        ci_95_high,
        bootstrap_iterations: PAIRED_BOOTSTRAP_ITERATIONS as u32,
    }
}

fn task_resolution_rates(trials: &[&TrialResult]) -> BTreeMap<String, f64> {
    let mut counts: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for trial in trials {
        let entry = counts.entry(trial.task_id.clone()).or_default();
        entry.0 += u64::from(trial.status == TrialStatus::Passed);
        entry.1 += 1;
    }
    counts
        .into_iter()
        .map(|(task, (passed, scheduled))| (task, ratio(passed, scheduled)))
        .collect()
}

fn paired_bootstrap_interval(differences: &[f64], mut state: u64, iterations: usize) -> (f64, f64) {
    if differences.is_empty() || iterations == 0 {
        return (0.0, 0.0);
    }
    if state == 0 {
        state = 0x9e37_79b9_7f4a_7c15;
    }
    let mut estimates = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let mut total = 0.0;
        for _ in 0..differences.len() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            total += differences[state as usize % differences.len()];
        }
        estimates.push(total / differences.len() as f64);
    }
    estimates.sort_by(f64::total_cmp);
    let last = estimates.len() - 1;
    (estimates[last * 25 / 1000], estimates[last * 975 / 1000])
}

fn deterministic_seed(agent_a: &str, agent_b: &str, differences: &[f64]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in agent_a
        .bytes()
        .chain(std::iter::once(0))
        .chain(agent_b.bytes())
        .chain(std::iter::once(0))
        .chain(
            differences
                .iter()
                .flat_map(|difference| difference.to_bits().to_le_bytes()),
        )
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Whether two reports are valid inputs to a regression gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Compatibility {
    Comparable,
    Incomparable { reasons: Vec<String> },
}

/// Resolution-rate comparison between two v2 reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryComparison {
    pub compatibility: Compatibility,
    pub gating_eligible: bool,
    pub regressions: Vec<AgentRateChange>,
    pub improvements: Vec<AgentRateChange>,
    pub unchanged: Vec<String>,
    pub new_agents: Vec<String>,
    pub removed_agents: Vec<String>,
}

impl RepositoryComparison {
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRateChange {
    pub agent: String,
    pub baseline_rate: f64,
    pub current_rate: f64,
    pub delta: f64,
}

/// Metadata describing whether a report is suitable for publication.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RedactionMetadata {
    pub redacted: bool,
    #[serde(default)]
    pub redacted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub rules_version: Option<String>,
    #[serde(default)]
    pub replacements: u64,
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Wilson score interval for a Bernoulli proportion at 95% confidence.
pub fn wilson_interval_95(successes: u64, total: u64) -> (f64, f64) {
    if total == 0 {
        return (0.0, 0.0);
    }
    let z = 1.959_963_984_540_054_f64;
    let n = total as f64;
    let p = successes as f64 / n;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let margin = z * ((p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt()) / denominator;
    ((center - margin).max(0.0), (center + margin).min(1.0))
}
